use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::sync::Mutex;

use super::config::GcConfig;

// ── Flags ─────────────────────────────────────────────────────────────────────
pub const GC_MARKED:  u8 = 1 << 0; // reachable in the current GC cycle
pub const GC_OLD:     u8 = 1 << 1; // lives in old gen or large-object space
pub const GC_PINNED:  u8 = 1 << 2; // must not be collected regardless of roots
pub const GC_LARGE:   u8 = 1 << 3; // allocated in the large-object space

/// Card-table granularity: one dirty bit covers this many bytes of heap.
pub const CARD_BYTES: usize = 512;

// ── Object header (8 bytes) ───────────────────────────────────────────────────
/// Header stored immediately before every managed object payload.
/// Reduced from 16 bytes (old design) to 8 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjHeader {
    /// Payload size in bytes.
    pub size:     u32,
    /// Index into the type-descriptor table; 0 means "opaque / no pointer fields".
    pub type_id:  u16,
    /// GC status flags: GC_MARKED | GC_OLD | GC_PINNED | GC_LARGE.
    pub gc_flags: u8,
    /// Number of minor GC cycles this object has survived (used for promotion).
    pub age:      u8,
}

impl ObjHeader {
    fn new(size: u32, type_id: u16, flags: u8) -> Self {
        Self { size, type_id, gc_flags: flags, age: 0 }
    }
}

// ── Type descriptor ───────────────────────────────────────────────────────────
/// Describes the GC-visible pointer layout of a heap-allocated type.
/// Registered at program startup via `draton_gc_register_type`.
#[derive(Debug, Clone)]
pub struct TypeDescriptor {
    pub size:            u32,
    /// Byte offsets (from payload start) of fields that hold GC-managed pointers.
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
/// Bump-pointer arena for newly allocated objects.
///
/// Layout inside `buffer`:
///   offset o   : ObjHeader (8 bytes)
///   offset o+8 : payload   (header.size bytes, 8-byte aligned)
///
/// An object's payload address is `buffer.as_ptr() + o + HEADER`.
pub struct YoungArena {
    pub buffer:     Vec<u8>,
    pub bump:       usize,
    /// Number of objects physically residing in this arena that are still live
    /// (either logically young or promoted-in-place).  When this hits 0 the
    /// arena can be reset.
    pub live_count: usize,
}

pub const HEADER: usize = size_of::<ObjHeader>(); // 8 bytes

impl YoungArena {
    pub fn new(size: usize) -> Self {
        Self { buffer: vec![0u8; size], bump: 0, live_count: 0 }
    }

    /// Attempt a bump-pointer allocation of `size` payload bytes.
    /// Returns the payload pointer on success, `None` if the arena is full.
    pub fn try_alloc(&mut self, size: usize, type_id: u16) -> Option<*mut u8> {
        let aligned = (HEADER + size + 7) & !7;
        if self.bump + aligned > self.buffer.len() {
            return None;
        }
        let header_ptr = unsafe {
            NonNull::new(self.buffer.as_mut_ptr().add(self.bump))?
        };
        let payload_ptr = unsafe { header_ptr.as_ptr().add(HEADER) };
        let hdr = ObjHeader::new(size as u32, type_id, 0);
        unsafe { ptr::write(header_ptr.as_ptr().cast::<ObjHeader>(), hdr); }
        self.bump += aligned;
        self.live_count += 1;
        Some(payload_ptr)
    }

    /// Reset the arena (only call when `live_count == 0`).
    pub fn reset(&mut self) {
        self.bump = 0;
        self.live_count = 0;
    }

    /// Returns true when `ptr` points into this arena's buffer.
    pub fn contains_ptr(&self, ptr: *const u8) -> bool {
        let base = self.buffer.as_ptr() as usize;
        let addr = ptr as usize;
        addr >= base && addr < base + self.buffer.len()
    }

    /// Read the header of a young object given its *payload* address.
    ///
    /// # Safety
    /// `payload` must have been returned by `try_alloc` on this arena.
    pub unsafe fn read_header(&self, payload: *const u8) -> ObjHeader {
        ptr::read(payload.sub(HEADER).cast::<ObjHeader>())
    }

    /// Write the header of a young object given its *payload* address.
    ///
    /// # Safety
    /// `payload` must have been returned by `try_alloc` on this arena.
    pub unsafe fn write_header(&mut self, payload: *mut u8, hdr: ObjHeader) {
        ptr::write(payload.sub(HEADER).cast::<ObjHeader>(), hdr);
    }
}

// ── Card table ────────────────────────────────────────────────────────────────
/// One dirty bit per `CARD_BYTES` of address space.
/// Indexed as `addr / CARD_BYTES`.  The table is sized for `max_heap` at init.
pub struct CardTable {
    cards: Vec<u8>,
}

impl CardTable {
    pub fn new(max_heap_bytes: usize) -> Self {
        let n = (max_heap_bytes / CARD_BYTES) + 2;
        Self { cards: vec![0u8; n] }
    }

    #[inline]
    fn index(addr: usize) -> usize {
        addr / CARD_BYTES
    }

    #[inline]
    pub fn mark_dirty(&mut self, addr: usize) {
        let i = Self::index(addr);
        if i < self.cards.len() {
            self.cards[i] = 1;
        }
    }

    #[inline]
    pub fn is_dirty(&self, addr: usize) -> bool {
        let i = Self::index(addr);
        i < self.cards.len() && self.cards[i] != 0
    }

    pub fn clear(&mut self, addr: usize) {
        let i = Self::index(addr);
        if i < self.cards.len() {
            self.cards[i] = 0;
        }
    }

    pub fn iter_dirty(&self) -> impl Iterator<Item = usize> + '_ {
        self.cards
            .iter()
            .enumerate()
            .filter(|(_, &v)| v != 0)
            .map(|(i, _)| i * CARD_BYTES)
    }
}

// ── Full heap state ───────────────────────────────────────────────────────────
#[allow(dead_code)]
pub struct HeapState {
    pub config: GcConfig,

    // ── Young generation ──────────────────────────────────────────────────────
    pub young: YoungArena,
    /// Payload address → (arena_byte_offset_of_header, aligned_total_bytes).
    /// Entries are removed when an object is collected or promoted out.
    pub young_index: HashMap<usize, (usize, usize)>,

    // ── Old generation ────────────────────────────────────────────────────────
    /// Payload address → Box<[u8]> (ObjHeader + payload).
    pub old_objects: HashMap<usize, Box<[u8]>>,

    // ── Large-object space ────────────────────────────────────────────────────
    /// Same layout as old_objects; allocated outside the young arena.
    pub large_objects: HashMap<usize, Box<[u8]>>,

    // ── Type descriptor table ─────────────────────────────────────────────────
    /// Registered via `draton_gc_register_type` before any allocations.
    pub type_descriptors: HashMap<u16, TypeDescriptor>,

    // ── Root set ──────────────────────────────────────────────────────────────
    /// Payload address → explicit root refcount.
    /// Decremented via `release()`; when 0 the object is eligible for collection
    /// unless still reachable through another root.
    pub roots: HashMap<usize, usize>,

    // ── Write-barrier bookkeeping ─────────────────────────────────────────────
    /// Old-gen objects that may contain pointers into the young gen.
    /// Populated by the write barrier; consulted during minor GC.
    pub remembered_set: HashSet<usize>,
    /// One dirty byte per `CARD_BYTES` of address space.
    pub card_table: CardTable,

    // ── Incremental major GC ──────────────────────────────────────────────────
    pub mark_stack: Vec<usize>,   // payload addresses yet to be traced
    pub is_marking: bool,
    /// Adaptive mark-slice size (number of objects per incremental step).
    pub mark_slice_size: usize,

    // ── Statistics ────────────────────────────────────────────────────────────
    pub minor_cycles:    u64,
    pub major_cycles:    u64,
    pub bytes_allocated: u64,
    pub live_bytes:      usize, // maintained as an O(1) counter
    pub old_bytes:       usize, // sum of old_objects + large_objects sizes, O(1)
}

impl HeapState {
    pub fn new(config: GcConfig) -> Self {
        let config = config.normalized();
        let card_table = CardTable::new(config.max_heap);
        Self {
            young:           YoungArena::new(config.young_size),
            young_index:     HashMap::new(),
            old_objects:     HashMap::new(),
            large_objects:   HashMap::new(),
            type_descriptors: HashMap::new(),
            roots:           HashMap::new(),
            remembered_set:  HashSet::new(),
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
        self.type_descriptors.insert(
            type_id,
            TypeDescriptor {
                size,
                pointer_offsets: offsets.to_vec().into_boxed_slice(),
            },
        );
    }

    // ── Allocation ────────────────────────────────────────────────────────────

    pub fn alloc(&mut self, size: usize, type_id: u16) -> *mut u8 {
        if size >= self.config.large_threshold {
            return self.alloc_large(size, type_id);
        }

        // Fast path: bump-pointer in young arena.
        if let Some(payload) = self.young.try_alloc(size, type_id) {
            let aligned = (HEADER + size + 7) & !7;
            let arena_offset = self.young.bump - aligned;
            self.young_index.insert(payload as usize, (arena_offset, aligned));
            self.roots.insert(payload as usize, 1);
            self.bytes_allocated += aligned as u64;
            self.live_bytes += aligned;
            return payload;
        }

        // Young arena full: promote all promotable young objects, then retry or
        // fall through to old-gen allocation.
        self.try_reset_young();
        if let Some(payload) = self.young.try_alloc(size, type_id) {
            let aligned = (HEADER + size + 7) & !7;
            let arena_offset = self.young.bump - aligned;
            self.young_index.insert(payload as usize, (arena_offset, aligned));
            self.roots.insert(payload as usize, 1);
            self.bytes_allocated += aligned as u64;
            self.live_bytes += aligned;
            return payload;
        }

        // Fallback: allocate directly in old gen.
        self.alloc_old(size, type_id)
    }

    pub fn alloc_array(&mut self, elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
        let Some(size) = elem_size.checked_mul(len) else {
            return ptr::null_mut();
        };
        self.alloc(size, type_id)
    }

    fn alloc_old(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let total = HEADER + size;
        let mut bytes = vec![0u8; total].into_boxed_slice();
        let hdr = ObjHeader::new(size as u32, type_id, GC_OLD);
        unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        let payload = unsafe { bytes.as_mut_ptr().add(HEADER) };
        let payload_addr = payload as usize;
        self.old_objects.insert(payload_addr, bytes);
        self.roots.insert(payload_addr, 1);
        self.bytes_allocated += total as u64;
        self.live_bytes += total;
        self.old_bytes   += total;
        payload
    }

    fn alloc_large(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let total = HEADER + size;
        let mut bytes = vec![0u8; total].into_boxed_slice();
        let hdr = ObjHeader::new(size as u32, type_id, GC_OLD | GC_LARGE);
        unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        let payload = unsafe { bytes.as_mut_ptr().add(HEADER) };
        let payload_addr = payload as usize;
        self.large_objects.insert(payload_addr, bytes);
        self.roots.insert(payload_addr, 1);
        self.bytes_allocated += total as u64;
        self.live_bytes += total;
        self.old_bytes   += total;
        payload
    }

    // ── Header access (all O(1)) ──────────────────────────────────────────────

    pub fn header_of(&self, payload: *mut u8) -> Option<ObjHeader> {
        let addr = payload as usize;
        // Young gen: header is HEADER bytes before the payload in the arena.
        if self.young_index.contains_key(&addr) {
            return Some(unsafe { self.young.read_header(payload) });
        }
        // Old gen / large: header is HEADER bytes before payload in the Box.
        if let Some(bytes) = self.old_objects.get(&addr).or_else(|| self.large_objects.get(&addr)) {
            return Some(unsafe {
                ptr::read(bytes.as_ptr().cast::<ObjHeader>())
            });
        }
        None
    }

    fn set_header_old(&mut self, payload: *mut u8, hdr: ObjHeader) {
        let addr = payload as usize;
        if let Some(bytes) = self.old_objects.get_mut(&addr)
            .or_else(|| self.large_objects.get_mut(&addr))
        {
            unsafe { ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), hdr); }
        }
    }

    pub fn space_of(&self, payload: *mut u8) -> Option<HeapSpace> {
        let addr = payload as usize;
        if self.young_index.contains_key(&addr) { return Some(HeapSpace::Young); }
        if self.old_objects.contains_key(&addr)  { return Some(HeapSpace::Old);   }
        if self.large_objects.contains_key(&addr) { return Some(HeapSpace::Large); }
        None
    }

    // ── Pin / unpin ───────────────────────────────────────────────────────────

    pub fn pin(&mut self, payload: *mut u8) {
        let addr = payload as usize;
        if self.young_index.contains_key(&addr) {
            let mut hdr = unsafe { self.young.read_header(payload) };
            hdr.gc_flags |= GC_PINNED;
            unsafe { self.young.write_header(payload, hdr); }
        } else {
            if let Some(hdr) = self.header_of(payload) {
                let mut hdr = hdr;
                hdr.gc_flags |= GC_PINNED;
                self.set_header_old(payload, hdr);
            }
        }
    }

    pub fn unpin(&mut self, payload: *mut u8) {
        let addr = payload as usize;
        if self.young_index.contains_key(&addr) {
            let mut hdr = unsafe { self.young.read_header(payload) };
            hdr.gc_flags &= !GC_PINNED;
            unsafe { self.young.write_header(payload, hdr); }
        } else {
            if let Some(hdr) = self.header_of(payload) {
                let mut hdr = hdr;
                hdr.gc_flags &= !GC_PINNED;
                self.set_header_old(payload, hdr);
            }
        }
    }

    // ── Explicit root management ──────────────────────────────────────────────

    pub fn protect(&mut self, payload: *mut u8) {
        let counter = self.roots.entry(payload as usize).or_insert(0);
        *counter += 1;
    }

    pub fn release(&mut self, payload: *mut u8) {
        let addr = payload as usize;
        if let Some(counter) = self.roots.get_mut(&addr) {
            if *counter > 1 {
                *counter -= 1;
            } else {
                self.roots.remove(&addr);
            }
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// If all young-arena residents are dead (live_count == 0), reset the arena
    /// so new allocations can reuse the memory.
    pub fn try_reset_young(&mut self) {
        if self.young.live_count == 0 {
            self.young.reset();
        }
    }

    pub fn current_usage(&self) -> usize {
        self.live_bytes
    }

    pub fn old_usage(&self) -> usize {
        self.old_bytes
    }

    pub fn young_usage(&self) -> usize {
        self.young.bump
    }

    /// Read a pointer field from a managed object payload.
    ///
    /// # Safety
    /// `payload` must be a valid managed object and `offset` must be within bounds.
    pub unsafe fn read_ptr_field(payload: *const u8, offset: u32) -> *mut u8 {
        let field = payload.add(offset as usize) as *const *mut u8;
        ptr::read(field)
    }
}

// ── GC runtime handle ─────────────────────────────────────────────────────────

pub struct GcRuntime {
    pub heap: Mutex<HeapState>,
}

impl GcRuntime {
    pub fn new(config: GcConfig) -> Self {
        Self { heap: Mutex::new(HeapState::new(config)) }
    }
}
