use std::collections::HashMap;
use std::mem::size_of;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::config::GcConfig;

// ── LLVM shadow-stack types ────────────────────────────────────────────────────
/// Per-function frame map: lives in read-only data, one per function.
/// Layout: { num_roots: i32, num_meta: i32, meta[num_meta]: *const () }
#[repr(C)]
pub struct FrameMap {
    pub num_roots: i32,
    pub num_meta:  i32,
    // metadata pointers follow in memory (we ignore them in a non-moving GC)
}

/// Per-call-frame shadow stack entry: allocated on the machine stack.
/// Layout: { next: *mut StackEntry, map: *const FrameMap, roots[num_roots]: *mut *mut u8 }
/// Each `roots[i]` is the address of the stack alloca slot (i.e. a `*mut *mut u8`).
/// The actual GC pointer is `*roots[i]`.
#[repr(C)]
pub struct StackEntry {
    pub next: *mut StackEntry,
    pub map:  *const FrameMap,
    // root slots follow in memory
}

// ── Flags ─────────────────────────────────────────────────────────────────────
pub const GC_MARKED:  u8 = 1 << 0; // reachable in the current GC cycle
pub const GC_OLD:     u8 = 1 << 1; // lives in old gen or large-object space
pub const GC_PINNED:  u8 = 1 << 2; // must not be collected regardless of roots
pub const GC_LARGE:   u8 = 1 << 3; // allocated in the large-object space

/// Card-table granularity: one dirty bit covers this many bytes of heap.
pub const CARD_BYTES: usize = 512;

// ── Object header (8 bytes) ───────────────────────────────────────────────────
/// Header stored immediately before every managed object payload.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjHeader {
    /// Payload size in bytes.
    pub size:     u32,
    /// Index into the type-descriptor table; 0 means "opaque / no pointer fields".
    pub type_id:  u16,
    /// GC status flags: GC_MARKED | GC_OLD | GC_PINNED | GC_LARGE.
    pub gc_flags: u8,
    /// Number of minor GC cycles this object has survived (diagnostic only).
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
pub enum HeapSpace {
    Young,
    Old,
    Large,
}

// ── Young generation ──────────────────────────────────────────────────────────
/// Lock-free bump-pointer arena for newly allocated objects.
///
/// All state mutations (other than the bump CAS) are done only during a
/// stop-the-world GC pause, so no additional synchronisation is needed for
/// header reads/writes during collection.
pub struct YoungArena {
    pub buffer:     Vec<u8>,
    /// Atomic bump pointer — CAS gives each allocating thread an exclusive slice.
    pub bump:       AtomicUsize,
    /// Number of objects that are logically live in this arena.
    pub live_count: AtomicUsize,
}

// SAFETY: YoungArena is `Sync` because:
// - bump and live_count are AtomicUsize (inherently Sync).
// - buffer bytes are written to non-overlapping regions (guaranteed by CAS) or
//   exclusively during a stop-the-world GC pause.
unsafe impl Sync for YoungArena {}

pub const HEADER: usize = size_of::<ObjHeader>(); // 8 bytes

impl YoungArena {
    pub fn new(size: usize) -> Self {
        Self {
            buffer:     vec![0u8; size],
            bump:       AtomicUsize::new(0),
            live_count: AtomicUsize::new(0),
        }
    }

    /// Lock-free bump-pointer allocation.
    ///
    /// Uses `fetch_add` to atomically reserve `[old, old+aligned)` in the buffer.
    /// Two threads can allocate concurrently; each gets an exclusive sub-range.
    pub fn try_alloc(&self, size: usize, type_id: u16) -> Option<*mut u8> {
        let aligned = (HEADER + size + 7) & !7;
        let old = self.bump.fetch_add(aligned, Ordering::AcqRel);
        if old + aligned > self.buffer.len() {
            // Roll back; arena is full.
            self.bump.fetch_sub(aligned, Ordering::AcqRel);
            return None;
        }
        // Write the header into the exclusively reserved region.
        // SAFETY: [old, old+aligned) is owned by this thread via the CAS above.
        let hdr_ptr = self.buffer.as_ptr().wrapping_add(old) as *mut ObjHeader;
        unsafe { ptr::write(hdr_ptr, ObjHeader::new(size as u32, type_id, 0)); }
        self.live_count.fetch_add(1, Ordering::Relaxed);
        Some(self.buffer.as_ptr().wrapping_add(old + HEADER) as *mut u8)
    }

    /// Reset the arena.  Only call during a stop-the-world GC pause.
    pub fn reset(&self) {
        self.bump.store(0, Ordering::Release);
        self.live_count.store(0, Ordering::Release);
    }

    /// Reset if no objects remain live.
    pub fn try_reset(&self) {
        if self.live_count.load(Ordering::Relaxed) == 0 {
            self.reset();
        }
    }

    /// Returns `true` when `ptr` is a valid payload in the live (allocated) region.
    pub fn contains_ptr(&self, ptr: *const u8) -> bool {
        let base = self.buffer.as_ptr() as usize;
        let addr = ptr as usize;
        let bump = self.bump.load(Ordering::Relaxed);
        addr >= base + HEADER && addr < base + bump
    }

    /// Read the header given the payload address.
    ///
    /// # Safety
    /// `payload` must have been returned by `try_alloc` on this arena and the
    /// caller must have exclusive access (GC pause or freshly allocated slot).
    pub unsafe fn read_header(&self, payload: *const u8) -> ObjHeader {
        ptr::read(payload.sub(HEADER).cast::<ObjHeader>())
    }

    /// Write the header given the payload address.
    ///
    /// # Safety
    /// Same as `read_header`; caller must have exclusive access.
    pub unsafe fn write_header(&self, payload: *mut u8, hdr: ObjHeader) {
        ptr::write(payload.sub(HEADER).cast::<ObjHeader>(), hdr);
    }
}

// ── Card table ────────────────────────────────────────────────────────────────
pub struct CardTable {
    cards: Vec<u8>,
}

impl CardTable {
    pub fn new(max_heap_bytes: usize) -> Self {
        let n = (max_heap_bytes / CARD_BYTES) + 2;
        Self { cards: vec![0u8; n] }
    }

    #[inline]
    fn index(addr: usize) -> usize { addr / CARD_BYTES }

    #[inline]
    pub fn mark_dirty(&mut self, addr: usize) {
        let i = Self::index(addr);
        if i < self.cards.len() { self.cards[i] = 1; }
    }

    #[inline]
    pub fn is_dirty(&self, addr: usize) -> bool {
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

// ── Full heap state ───────────────────────────────────────────────────────────
#[allow(dead_code)]
pub struct HeapState {
    pub config: GcConfig,

    // ── Old generation ────────────────────────────────────────────────────────
    pub old_objects:  HashMap<usize, Box<[u8]>>,
    pub large_objects: HashMap<usize, Box<[u8]>>,

    // ── Type descriptor table ─────────────────────────────────────────────────
    pub type_descriptors: HashMap<u16, TypeDescriptor>,

    // ── Root set (explicit protect() calls only) ──────────────────────────────
    pub roots: HashMap<usize, usize>,

    // ── Promotion forwarding table ─────────────────────────────────────────────
    /// Maps old young-arena payload address → new old-gen payload address.
    /// Allows callers holding pre-promotion raw pointers to resolve the object.
    /// Entries are pruned when the promoted object is later collected.
    pub young_forwarding: HashMap<usize, usize>,

    // ── Write-barrier bookkeeping ─────────────────────────────────────────────
    /// Old-gen parent addresses that may have pointer fields into young gen.
    /// Using a Vec (not HashSet): append is O(1) with no hash overhead;
    /// duplicates are deduplicated at the start of collect_minor.
    pub remembered_set: Vec<usize>,
    pub card_table: CardTable,

    // ── Incremental major GC ──────────────────────────────────────────────────
    pub mark_stack: Vec<usize>,
    pub is_marking: bool,
    pub mark_slice_size: usize,

    // ── Statistics ────────────────────────────────────────────────────────────
    pub minor_cycles:    u64,
    pub major_cycles:    u64,
    pub bytes_allocated: u64,
    pub live_bytes:      usize,
    pub old_bytes:       usize, // sum of old_objects + large_objects sizes, O(1)
}

impl HeapState {
    pub fn new(config: GcConfig) -> Self {
        let config = config.normalized();
        let card_table = CardTable::new(config.max_heap);
        Self {
            old_objects:      HashMap::new(),
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

    // ── Allocation (old gen / large object only) ──────────────────────────────
    // Young-gen allocation is handled by GcRuntime::young.try_alloc() without
    // acquiring the heap lock.

    /// Allocate in the appropriate space (not young gen).
    /// Called when the lock-free young fast path failed.
    pub fn alloc_slow(&mut self, size: usize, type_id: u16) -> *mut u8 {
        if size >= self.config.large_threshold {
            return self.alloc_large(size, type_id);
        }
        self.alloc_old(size, type_id)
    }

    pub fn alloc_array(&mut self, elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
        let Some(size) = elem_size.checked_mul(len) else { return ptr::null_mut(); };
        self.alloc_slow(size, type_id)
    }

    pub(crate) fn alloc_old(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let total = HEADER + size;
        let mut bytes = vec![0u8; total].into_boxed_slice();
        let hdr = ObjHeader::new(size as u32, type_id, GC_OLD);
        unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        let payload = unsafe { bytes.as_mut_ptr().add(HEADER) };
        let payload_addr = payload as usize;
        self.old_objects.insert(payload_addr, bytes);
        self.bytes_allocated += total as u64;
        self.live_bytes += total;
        self.old_bytes  += total;
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

    /// Look up the object header.  `young` is the live young-gen arena.
    pub fn header_of(&self, young: &YoungArena, payload: *mut u8) -> Option<ObjHeader> {
        let addr = payload as usize;
        // Forwarding must be checked FIRST: a promoted object's old arena bytes
        // may still be in the buffer, so contains_ptr would return true.
        if let Some(&new_addr) = self.young_forwarding.get(&addr) {
            if let Some(bytes) = self.old_objects.get(&new_addr) {
                return Some(unsafe { ptr::read(bytes.as_ptr().cast::<ObjHeader>()) });
            }
        }
        if young.contains_ptr(payload as *const u8) {
            return Some(unsafe { young.read_header(payload) });
        }
        if let Some(bytes) = self.old_objects.get(&addr).or_else(|| self.large_objects.get(&addr)) {
            return Some(unsafe { ptr::read(bytes.as_ptr().cast::<ObjHeader>()) });
        }
        None
    }

    pub(crate) fn set_header_old(&mut self, payload: *mut u8, hdr: ObjHeader) {
        let addr = payload as usize;
        if let Some(bytes) = self.old_objects.get_mut(&addr)
            .or_else(|| self.large_objects.get_mut(&addr))
        {
            unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        }
    }

    pub fn space_of(&self, young: &YoungArena, payload: *mut u8) -> Option<HeapSpace> {
        let addr = payload as usize;
        if self.young_forwarding.contains_key(&addr) { return Some(HeapSpace::Old); }
        if young.contains_ptr(payload as *const u8)  { return Some(HeapSpace::Young); }
        if self.old_objects.contains_key(&addr)       { return Some(HeapSpace::Old);   }
        if self.large_objects.contains_key(&addr)     { return Some(HeapSpace::Large); }
        None
    }

    // ── Pin / unpin ───────────────────────────────────────────────────────────

    pub fn pin(&mut self, young: &YoungArena, payload: *mut u8) {
        if young.contains_ptr(payload as *const u8) {
            let mut hdr = unsafe { young.read_header(payload) };
            hdr.gc_flags |= GC_PINNED;
            unsafe { young.write_header(payload, hdr); }
        } else if let Some(hdr) = self.header_of(young, payload) {
            let mut hdr = hdr;
            hdr.gc_flags |= GC_PINNED;
            self.set_header_old(payload, hdr);
        }
    }

    pub fn unpin(&mut self, young: &YoungArena, payload: *mut u8) {
        if young.contains_ptr(payload as *const u8) {
            let mut hdr = unsafe { young.read_header(payload) };
            hdr.gc_flags &= !GC_PINNED;
            unsafe { young.write_header(payload, hdr); }
        } else if let Some(hdr) = self.header_of(young, payload) {
            let mut hdr = hdr;
            hdr.gc_flags &= !GC_PINNED;
            self.set_header_old(payload, hdr);
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

    pub fn old_usage(&self)   -> usize { self.old_bytes }

    /// Read a pointer field from a managed object payload.
    ///
    /// # Safety
    /// `payload` must be a valid managed object and `offset` within bounds.
    pub unsafe fn read_ptr_field(payload: *const u8, offset: u32) -> *mut u8 {
        let field = payload.add(offset as usize) as *const *mut u8;
        ptr::read(field)
    }
}

// ── GC runtime handle ─────────────────────────────────────────────────────────

pub struct GcRuntime {
    /// Lock-free young-gen arena.  Allocation never requires the heap lock.
    pub young: YoungArena,
    /// Old gen + large-object space + GC metadata.
    pub heap:  Mutex<HeapState>,
    /// Cached from config for the lock-free alloc hot path.
    /// Updated atomically when `gc::configure()` is called.
    pub large_threshold: AtomicUsize,
    pub young_size:      AtomicUsize,
}

impl GcRuntime {
    pub fn new(config: GcConfig) -> Self {
        let config = config.normalized();
        Self {
            large_threshold: AtomicUsize::new(config.large_threshold),
            young_size:      AtomicUsize::new(config.young_size),
            young:           YoungArena::new(config.young_size),
            heap:            Mutex::new(HeapState::new(config)),
        }
    }
}
