use std::cell::Cell;
use std::collections::HashMap;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::config::GcConfig;

// ── LLVM shadow-stack types ────────────────────────────────────────────────────
#[repr(C)]
pub struct FrameMap {
    pub num_roots: i32,
    pub num_meta:  i32,
}

#[repr(C)]
pub struct StackEntry {
    pub next: *mut StackEntry,
    pub map:  *const FrameMap,
}

// ── Flags ─────────────────────────────────────────────────────────────────────
pub const GC_MARKED:  u8 = 1 << 0;
pub const GC_OLD:     u8 = 1 << 1;
pub const GC_PINNED:  u8 = 1 << 2;
pub const GC_LARGE:   u8 = 1 << 3;
/// Slot in OldArena has been freed and is available for reuse.
pub const GC_FREE:    u8 = 1 << 4;

pub const CARD_BYTES: usize = 512;

// ── Object header (8 bytes) ───────────────────────────────────────────────────
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjHeader {
    pub size:     u32,
    pub type_id:  u16,
    pub gc_flags: u8,
    pub age:      u8,
}

impl ObjHeader {
    pub(crate) fn new(size: u32, type_id: u16, flags: u8) -> Self {
        Self { size, type_id, gc_flags: flags, age: 0 }
    }
}

// ── Type descriptor ───────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct TypeDescriptor {
    pub size:            u32,
    pub pointer_offsets: Box<[u32]>,
}

// ── Logical heap space ────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapSpace { Young, Old, Large }

pub const HEADER: usize = std::mem::size_of::<ObjHeader>(); // 8 bytes

// ── Per-thread young arena ────────────────────────────────────────────────────
pub const MAX_THREADS: usize = 16;

/// One per-thread bump arena, padded to a single cache line (64 bytes)
/// so concurrent allocations on different threads don't share a cache line.
#[repr(C, align(64))]
pub struct PerThreadArena {
    pub bump:       AtomicUsize,
    pub live_count: AtomicUsize,
    _pad: [u64; 6], // 48 bytes; bump(8) + live_count(8) + pad(48) = 64
}

impl PerThreadArena {
    fn new() -> Self {
        Self { bump: AtomicUsize::new(0), live_count: AtomicUsize::new(0), _pad: [0u64; 6] }
    }
    pub fn reset(&self) {
        self.bump.store(0, Ordering::Release);
        self.live_count.store(0, Ordering::Release);
    }
}

thread_local! {
    /// Index into `YoungPool::slots` assigned to the current OS thread.
    /// `usize::MAX` means "not yet assigned".
    static THREAD_SLOT: Cell<usize> = Cell::new(usize::MAX);
}

/// Pool of per-thread bump arenas backed by one contiguous allocation.
///
/// Layout: `buffer[i * per_thread_size .. (i+1) * per_thread_size]` is owned
/// by thread slot `i`.  The entire pool range is a single O(1) range check
/// for `contains_ptr`, making write-barrier fast paths cheap.
pub struct YoungPool {
    /// Backing store for all per-thread segments.
    pub buffer: Vec<u8>,
    /// Bytes per thread slot — always a power of two.
    pub per_thread_size: usize,
    /// log2(per_thread_size) for O(1) slot index via bit-shift.
    pub slot_shift: u32,
    /// One bumper per thread, cache-line isolated.
    pub slots: Vec<PerThreadArena>,
    /// Monotonic counter for assigning slot indices to new threads.
    next_slot: AtomicUsize,
}

// SAFETY: YoungPool is Sync because:
// - AtomicUsize fields are Sync.
// - buffer bytes are written to non-overlapping per-thread regions (enforced by
//   the CAS in try_alloc) or exclusively during a stop-the-world GC pause.
unsafe impl Sync for YoungPool {}

impl YoungPool {
    pub fn new(total_size: usize) -> Self {
        // Each slot must be a power-of-two size (enables O(1) slot lookup via
        // bit-shift instead of division) and at least 64 KiB.
        let raw = (total_size / MAX_THREADS).max(64 * 1024);
        let per = raw.next_power_of_two();
        let actual = per * MAX_THREADS;
        let mut slots = Vec::with_capacity(MAX_THREADS);
        for _ in 0..MAX_THREADS { slots.push(PerThreadArena::new()); }
        Self {
            buffer:          vec![0u8; actual],
            per_thread_size: per,
            slot_shift:      per.trailing_zeros(),
            slots,
            next_slot:       AtomicUsize::new(0),
        }
    }

    /// Returns the slot index for the calling thread, assigning one if needed.
    pub fn current_slot_idx(&self) -> usize {
        THREAD_SLOT.with(|cell| {
            let s = cell.get();
            if s != usize::MAX { return s; }
            let new_s = self.next_slot.fetch_add(1, Ordering::Relaxed) % MAX_THREADS;
            cell.set(new_s);
            new_s
        })
    }

    /// Lock-free bump allocation in the calling thread's private arena.
    pub fn try_alloc(&self, size: usize, type_id: u16) -> Option<*mut u8> {
        let idx     = self.current_slot_idx();
        let arena   = &self.slots[idx];
        let aligned = (HEADER + size + 7) & !7;
        let old     = arena.bump.fetch_add(aligned, Ordering::AcqRel);
        if old + aligned > self.per_thread_size {
            arena.bump.fetch_sub(aligned, Ordering::AcqRel);
            return None;
        }
        let base = self.buffer.as_ptr().wrapping_add(idx * self.per_thread_size);
        unsafe { ptr::write(base.wrapping_add(old) as *mut ObjHeader,
                            ObjHeader::new(size as u32, type_id, 0)); }
        arena.live_count.fetch_add(1, Ordering::Relaxed);
        Some(base.wrapping_add(old + HEADER) as *mut u8)
    }

    /// True when `ptr` is the payload of a live (not-yet-reset) object in
    /// any per-thread slot.
    ///
    /// Uses a bit-shift to locate the owning slot in O(1), then compares the
    /// payload offset against the slot's atomic bump pointer.  Returns `false`
    /// for any address in a slot that has been reset to bump=0.
    #[inline]
    pub fn contains_ptr(&self, ptr: *const u8) -> bool {
        let base = self.buffer.as_ptr() as usize;
        let addr = ptr as usize;
        if addr < base { return false; }
        let pool_off = addr - base;
        let slot_idx = pool_off >> self.slot_shift;
        if slot_idx >= MAX_THREADS { return false; }
        let slot_off = pool_off & (self.per_thread_size - 1);
        // `slot_off` is the byte offset of the *payload* within the slot segment.
        // The header sits at `slot_off - HEADER`; for that header to exist the
        // header offset must be non-negative and strictly less than slot's bump.
        if slot_off < HEADER { return false; }
        let header_off  = slot_off - HEADER;
        let slot_bump   = self.slots[slot_idx].bump.load(Ordering::Relaxed);
        header_off < slot_bump
    }

    pub unsafe fn read_header(&self, payload: *const u8) -> ObjHeader {
        ptr::read(payload.sub(HEADER).cast::<ObjHeader>())
    }
    pub unsafe fn write_header(&self, payload: *mut u8, hdr: ObjHeader) {
        ptr::write(payload.sub(HEADER).cast::<ObjHeader>(), hdr)
    }

    /// True when the calling thread's slot is ≥ 90 % full.
    #[inline]
    pub fn current_slot_nearly_full(&self) -> bool {
        let bump = self.slots[self.current_slot_idx()]
            .bump.load(Ordering::Relaxed);
        bump >= self.per_thread_size.saturating_sub(self.per_thread_size / 10)
    }

    /// Reset all per-thread arenas. Call only during a stop-the-world pause.
    pub fn reset_all(&self) {
        for slot in &self.slots { slot.reset(); }
    }
}

// ── Card table ────────────────────────────────────────────────────────────────
pub struct CardTable { cards: Vec<u8> }

impl CardTable {
    pub fn new(max_heap_bytes: usize) -> Self {
        let n = (max_heap_bytes / CARD_BYTES) + 2;
        Self { cards: vec![0u8; n] }
    }
    #[inline] fn index(addr: usize) -> usize { addr / CARD_BYTES }
    #[inline] pub fn mark_dirty(&mut self, addr: usize) {
        let i = Self::index(addr);
        if i < self.cards.len() { self.cards[i] = 1; }
    }
    #[inline] pub fn is_dirty(&self, addr: usize) -> bool {
        let i = Self::index(addr);
        i < self.cards.len() && self.cards[i] != 0
    }
    pub fn clear(&mut self, addr: usize) {
        let i = Self::index(addr);
        if i < self.cards.len() { self.cards[i] = 0; }
    }
    pub fn iter_dirty(&self) -> impl Iterator<Item = usize> + '_ {
        self.cards.iter().enumerate()
            .filter(|(_, &v)| v != 0)
            .map(|(i, _)| i * CARD_BYTES)
    }
}

// ── Old-gen contiguous arena ──────────────────────────────────────────────────
/// A freed slot in OldArena, eligible for reuse.
pub struct FreeSlot {
    pub offset: usize,
    pub total:  usize, // header + payload, aligned to 8 bytes
}

/// Contiguous bump arena for old-generation objects.
///
/// Objects are allocated by bumping `bump` forward.  When an object is swept
/// (dead after major GC), its header is marked `GC_FREE` and a `FreeSlot` is
/// appended to the free list for future reuse.  Major GC sweep is a single
/// linear scan — O(old-gen size), cache-friendly.
pub struct OldArena {
    /// Backing store.  Never reallocated; internal pointers are stable.
    pub buffer:    Vec<u8>,
    /// Next free byte offset (increases monotonically until free list reuse).
    pub bump:      usize,
    /// Sorted list of freed slots available for reuse.
    pub free_list: Vec<FreeSlot>,
}

impl OldArena {
    pub fn new(capacity: usize) -> Self {
        Self { buffer: vec![0u8; capacity], bump: 0, free_list: Vec::new() }
    }

    /// Allocate `size` payload bytes with the given header flags.
    /// Returns null on OOM (arena full and no suitable free slot).
    pub fn alloc(&mut self, size: usize, flags: u8, type_id: u16) -> *mut u8 {
        let aligned = (HEADER + size + 7) & !7;

        // First-fit from free list.
        for i in 0..self.free_list.len() {
            let slot_total = self.free_list[i].total;
            if slot_total >= aligned {
                let off = self.free_list[i].offset;
                // If the remainder is large enough to hold at least a header +
                // 1 byte, split the slot so we don't waste space.
                if slot_total > aligned + HEADER {
                    let rem_off   = off + aligned;
                    let rem_total = slot_total - aligned;
                    // Write a synthetic FREE header in the remainder.
                    let rem_hdr = ObjHeader::new((rem_total - HEADER) as u32, 0, GC_FREE | GC_OLD);
                    unsafe {
                        ptr::write(self.buffer.as_mut_ptr().add(rem_off).cast::<ObjHeader>(),
                                   rem_hdr);
                    }
                    self.free_list[i] = FreeSlot { offset: rem_off, total: rem_total };
                } else {
                    self.free_list.swap_remove(i);
                }
                let hdr = ObjHeader::new(size as u32, type_id, flags);
                unsafe { ptr::write(self.buffer.as_mut_ptr().add(off).cast::<ObjHeader>(), hdr); }
                return unsafe { self.buffer.as_mut_ptr().add(off + HEADER) };
            }
        }

        // Bump allocate.
        if self.bump + aligned > self.buffer.len() { return ptr::null_mut(); }
        let off = self.bump;
        self.bump += aligned;
        let hdr = ObjHeader::new(size as u32, type_id, flags);
        unsafe { ptr::write(self.buffer.as_mut_ptr().add(off).cast::<ObjHeader>(), hdr); }
        unsafe { self.buffer.as_mut_ptr().add(off + HEADER) }
    }

    /// True when `payload` is a non-freed object inside this arena.
    #[inline]
    pub fn contains_ptr(&self, payload: *const u8) -> bool {
        let base = self.buffer.as_ptr() as usize;
        let addr = payload as usize;
        if addr < base + HEADER || addr >= base + self.bump { return false; }
        // Quick free check — avoids treating freed slots as live objects.
        let hdr = unsafe { ptr::read((addr - HEADER) as *const ObjHeader) };
        hdr.gc_flags & GC_FREE == 0
    }

    /// Read the header of an object payload in this arena.
    #[inline]
    pub fn header_of(&self, payload: *const u8) -> Option<ObjHeader> {
        let base = self.buffer.as_ptr() as usize;
        let addr = payload as usize;
        if addr < base + HEADER || addr >= base + self.bump { return None; }
        let hdr = unsafe { ptr::read((addr - HEADER) as *const ObjHeader) };
        if hdr.gc_flags & GC_FREE != 0 { None } else { Some(hdr) }
    }
}

// ── Full heap state ───────────────────────────────────────────────────────────
#[allow(dead_code)]
pub struct HeapState {
    pub config: GcConfig,

    // ── Old generation ────────────────────────────────────────────────────────
    /// Contiguous old-gen arena (regular objects).
    pub old:           OldArena,
    /// Large objects (≥ large_threshold) live here; infrequent.
    pub large_objects: HashMap<usize, Box<[u8]>>,

    // ── Type descriptor table ─────────────────────────────────────────────────
    pub type_descriptors: HashMap<u16, TypeDescriptor>,

    // ── Root set (explicit protect() calls only) ──────────────────────────────
    pub roots: HashMap<usize, usize>,

    // ── Promotion forwarding table ─────────────────────────────────────────────
    /// Maps old young-arena payload address → current old-gen payload address.
    /// Allows callers holding pre-promotion raw pointers to resolve the object.
    /// Entries are pruned when the promoted object is later collected.
    pub young_forwarding: HashMap<usize, usize>,

    // ── Write-barrier bookkeeping ─────────────────────────────────────────────
    pub remembered_set: Vec<usize>,
    pub card_table:     CardTable,

    // ── Incremental major GC ──────────────────────────────────────────────────
    pub mark_stack:      Vec<usize>,
    pub is_marking:      bool,
    pub mark_slice_size: usize,

    // ── Statistics ────────────────────────────────────────────────────────────
    pub minor_cycles:    u64,
    pub major_cycles:    u64,
    pub bytes_allocated: u64,
    pub live_bytes:      usize,
    /// Sum of live bytes in old + large spaces; maintained in O(1).
    pub old_bytes:       usize,
}

impl HeapState {
    pub fn new(config: GcConfig) -> Self {
        let config = config.normalized();
        let card_table = CardTable::new(config.max_heap);
        Self {
            old:              OldArena::new(config.old_size),
            large_objects:    HashMap::new(),
            type_descriptors: HashMap::new(),
            roots:            HashMap::new(),
            young_forwarding: HashMap::new(),
            remembered_set:   Vec::new(),
            card_table,
            mark_stack:      Vec::new(),
            is_marking:      false,
            mark_slice_size: 256,
            minor_cycles:    0,
            major_cycles:    0,
            bytes_allocated: 0,
            live_bytes:      0,
            old_bytes:       0,
            config,
        }
    }

    // ── Type registration ─────────────────────────────────────────────────────

    pub fn register_type(&mut self, type_id: u16, size: u32, offsets: &[u32]) {
        self.type_descriptors.insert(type_id, TypeDescriptor {
            size,
            pointer_offsets: offsets.to_vec().into_boxed_slice(),
        });
    }

    // ── Allocation ────────────────────────────────────────────────────────────

    pub fn alloc_slow(&mut self, size: usize, type_id: u16) -> *mut u8 {
        if size >= self.config.large_threshold { return self.alloc_large(size, type_id); }
        self.alloc_old(size, type_id)
    }

    pub fn alloc_array(&mut self, elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
        let Some(size) = elem_size.checked_mul(len) else { return ptr::null_mut(); };
        self.alloc_slow(size, type_id)
    }

    pub(crate) fn alloc_old(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let aligned = (HEADER + size + 7) & !7;
        let payload = self.old.alloc(size, GC_OLD, type_id);
        if !payload.is_null() {
            self.bytes_allocated += aligned as u64;
            self.live_bytes += aligned;
            self.old_bytes  += aligned;
        }
        payload
    }

    pub(crate) fn alloc_large(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let total = HEADER + size;
        let mut bytes = vec![0u8; total].into_boxed_slice();
        let hdr = ObjHeader::new(size as u32, type_id, GC_OLD | GC_LARGE);
        unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        let payload = unsafe { bytes.as_mut_ptr().add(HEADER) };
        let payload_addr = payload as usize;
        self.large_objects.insert(payload_addr, bytes);
        self.bytes_allocated += total as u64;
        self.live_bytes += total;
        self.old_bytes  += total;
        payload
    }

    // ── Header / space access ─────────────────────────────────────────────────

    pub fn header_of(&self, pool: &YoungPool, payload: *mut u8) -> Option<ObjHeader> {
        let addr = payload as usize;
        // Check forwarding first: a promoted object's young-arena addr maps to
        // the current old-gen addr.
        if let Some(&new_addr) = self.young_forwarding.get(&addr) {
            return self.old.header_of(new_addr as *const u8)
                .or_else(|| {
                    self.large_objects.get(&new_addr)
                        .map(|b| unsafe { ptr::read(b.as_ptr().cast::<ObjHeader>()) })
                });
        }
        if pool.contains_ptr(payload as *const u8) {
            return Some(unsafe { pool.read_header(payload) });
        }
        if let Some(hdr) = self.old.header_of(payload as *const u8) {
            return Some(hdr);
        }
        if let Some(bytes) = self.large_objects.get(&addr) {
            return Some(unsafe { ptr::read(bytes.as_ptr().cast::<ObjHeader>()) });
        }
        None
    }

    pub(crate) fn set_header_old(&mut self, payload: *mut u8, hdr: ObjHeader) {
        let addr = payload as usize;
        if self.old.contains_ptr(payload as *const u8) {
            unsafe { ptr::write((addr - HEADER) as *mut ObjHeader, hdr); }
        } else if let Some(bytes) = self.large_objects.get_mut(&addr) {
            unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        }
    }

    pub fn space_of(&self, pool: &YoungPool, payload: *mut u8) -> Option<HeapSpace> {
        let addr = payload as usize;
        if self.young_forwarding.contains_key(&addr)        { return Some(HeapSpace::Old);   }
        if pool.contains_ptr(payload as *const u8)          { return Some(HeapSpace::Young); }
        if self.old.contains_ptr(payload as *const u8)      { return Some(HeapSpace::Old);   }
        if self.large_objects.contains_key(&addr)           { return Some(HeapSpace::Large); }
        None
    }

    // ── Pin / unpin ───────────────────────────────────────────────────────────

    pub fn pin(&mut self, pool: &YoungPool, payload: *mut u8) {
        if pool.contains_ptr(payload as *const u8) {
            let mut hdr = unsafe { pool.read_header(payload) };
            hdr.gc_flags |= GC_PINNED;
            unsafe { pool.write_header(payload, hdr); }
        } else if let Some(hdr) = self.header_of(pool, payload) {
            let mut h = hdr;
            h.gc_flags |= GC_PINNED;
            self.set_header_old(payload, h);
        }
    }

    pub fn unpin(&mut self, pool: &YoungPool, payload: *mut u8) {
        if pool.contains_ptr(payload as *const u8) {
            let mut hdr = unsafe { pool.read_header(payload) };
            hdr.gc_flags &= !GC_PINNED;
            unsafe { pool.write_header(payload, hdr); }
        } else if let Some(hdr) = self.header_of(pool, payload) {
            let mut h = hdr;
            h.gc_flags &= !GC_PINNED;
            self.set_header_old(payload, h);
        }
    }

    // ── Explicit root management ──────────────────────────────────────────────

    pub fn protect(&mut self, payload: *mut u8) {
        let addr = payload as usize;
        let canonical = self.young_forwarding.get(&addr).copied().unwrap_or(addr);
        *self.roots.entry(canonical).or_insert(0) += 1;
    }

    pub fn release(&mut self, payload: *mut u8) {
        let addr = payload as usize;
        let canonical = self.young_forwarding.get(&addr).copied().unwrap_or(addr);
        if let Some(counter) = self.roots.get_mut(&canonical) {
            if *counter > 1 { *counter -= 1; } else { self.roots.remove(&canonical); }
        }
    }

    // ── Utilities ─────────────────────────────────────────────────────────────

    pub fn old_usage(&self) -> usize { self.old_bytes }

    pub unsafe fn read_ptr_field(payload: *const u8, offset: u32) -> *mut u8 {
        let field = payload.add(offset as usize) as *const *mut u8;
        ptr::read(field)
    }
}

// ── GC runtime handle ─────────────────────────────────────────────────────────

pub struct GcRuntime {
    /// Per-thread lock-free young-gen pool. Allocation never requires the heap lock.
    pub pool: YoungPool,
    /// Old gen + large-object space + GC metadata.
    pub heap: Mutex<HeapState>,
    /// Cached from config for the lock-free alloc hot path.
    pub large_threshold: AtomicUsize,
    pub young_size:      AtomicUsize,
}

impl GcRuntime {
    pub fn new(config: GcConfig) -> Self {
        let config = config.normalized();
        Self {
            large_threshold: AtomicUsize::new(config.large_threshold),
            young_size:      AtomicUsize::new(config.young_size),
            pool:            YoungPool::new(config.young_size),
            heap:            Mutex::new(HeapState::new(config)),
        }
    }
}
