//! Generational, incrementally-collected GC runtime surface used by generated code.

pub mod barrier;
pub mod collect;
pub mod config;
pub mod heap;

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::Ordering;

use config::GcConfig;
pub use heap::{GcRuntime, HeapSpace, ObjHeader, CARD_BYTES};

/// Objects larger than this are allocated directly in the large-object space.
/// Matches `GcConfig::default().large_threshold`.
pub const LARGE_OBJECT_THRESHOLD: usize = 32 * 1024;

// ── Shadow-stack root scanning ─────────────────────────────────────────────────

/// Walk LLVM's shadow-stack frame list and return the payload address of every
/// non-null GC root reachable from it.
///
/// # Safety
/// Must only be called while the mutator is stopped (i.e. from within a GC
/// cycle).  In Draton's current stop-the-world model this is always satisfied.
pub unsafe fn shadow_stack_roots() -> Vec<usize> {
    use std::mem::size_of;
    use heap::StackEntry;
    use crate::llvm_gc_root_chain;

    let mut roots = Vec::new();
    let mut entry = llvm_gc_root_chain;
    while !entry.is_null() {
        let num_roots = (*(*entry).map).num_roots as usize;
        let base = (entry as *const u8).add(size_of::<StackEntry>())
                   as *const *mut *mut u8;
        for i in 0..num_roots {
            let alloca = *base.add(i);
            if alloca.is_null() { continue; }
            let val = *alloca;
            if !val.is_null() { roots.push(val as usize); }
        }
        entry = (*entry).next;
    }
    roots
}

static GC_RUNTIME: OnceLock<Mutex<Option<Arc<GcRuntime>>>> = OnceLock::new();

fn global_slot() -> &'static Mutex<Option<Arc<GcRuntime>>> {
    GC_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn runtime() -> Arc<GcRuntime> {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(rt) = guard.as_ref() { return Arc::clone(rt); }
    let rt = Arc::new(GcRuntime::new(GcConfig::default()));
    *guard = Some(Arc::clone(&rt));
    rt
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

pub fn init()     { let _ = runtime(); }

pub fn shutdown() {
    let slot = global_slot();
    let mut guard = match slot.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    *guard = None;
}

/// Applies a new configuration to the GC.
pub fn configure(config: GcConfig) {
    let rt = runtime();
    let norm = config.normalized();
    rt.large_threshold.store(norm.large_threshold, Ordering::Relaxed);
    rt.young_size.store(norm.young_size, Ordering::Relaxed);
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.config = norm;
}

// ── Type registration ─────────────────────────────────────────────────────────

pub fn register_type(type_id: u16, size: u32, offsets: &[u32]) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.register_type(type_id, size, offsets);
}

// ── Allocation ────────────────────────────────────────────────────────────────

/// Allocates a GC-managed object payload.
///
/// Fast path (lock-free): bumps the calling thread's young-pool arena pointer.
/// Slow path: acquires the heap lock for old-gen / large-object allocation, or
/// when the thread's arena is full and needs to be collected first.
pub fn alloc(size: usize, type_id: u16) -> *mut u8 {
    let rt = runtime();

    // ── Fast path: lock-free per-thread young-gen bump ────────────────────────
    let large_threshold = rt.large_threshold.load(Ordering::Relaxed);
    if size < large_threshold {
        if let Some(payload) = rt.pool.try_alloc(size, type_id) {
            if rt.pool.current_slot_nearly_full() { signal_gc_flag(); }
            return payload;
        }

        // Thread's arena full — collect before returning a valid pointer.
        rt.collect_minor();

        if let Some(payload) = rt.pool.try_alloc(size, type_id) {
            return payload;
        }
        // If still full (tiny young_size), fall through to old-gen.
    }

    // ── Slow path: old-gen / large-object allocation ──────────────────────────
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let payload = heap.alloc_slow(size, type_id);
    let needs_major = heap.old_usage()
        >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    drop(heap);

    if needs_major { signal_gc_flag(); }
    payload
}

/// Allocates a GC-managed array payload.
pub fn alloc_array(elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let payload = heap.alloc_array(elem_size, len, type_id);
    let needs_major = heap.old_usage()
        >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    drop(heap);
    if needs_major { signal_gc_flag(); }
    payload
}

/// Request GC at the next safepoint (non-test) or fall through (test).
#[inline]
fn signal_gc_flag() {
    #[cfg(not(test))]
    { crate::draton_safepoint_flag.store(1, Ordering::Release); }
}

// ── Safepoint slow path ────────────────────────────────────────────────────────

/// Called by the runtime safepoint slow path when `draton_safepoint_flag != 0`.
pub fn safepoint() {
    let rt = runtime();
    let needs_minor = rt.pool.current_slot_nearly_full();
    let needs_major = {
        let heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
        heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize
    };
    if needs_minor { rt.collect_minor(); }
    if needs_major { rt.collect_major_slice(); }
}

// ── Write barrier ─────────────────────────────────────────────────────────────

pub fn write_barrier(obj: *mut u8, field: *mut u8, new_val: *mut u8) {
    runtime().write_barrier(obj, field, new_val);
}

// ── Manual collection ─────────────────────────────────────────────────────────

pub fn collect() { runtime().collect_full(); }

// ── Pinning ───────────────────────────────────────────────────────────────────

pub fn pin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.pin(&rt.pool, obj);
}

pub fn unpin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.unpin(&rt.pool, obj);
}

// ── Explicit root management ──────────────────────────────────────────────────

pub fn protect(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.protect(obj);
}

pub fn release(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.release(obj);
}

// ── Inspection (tests / diagnostics) ─────────────────────────────────────────

pub fn header_of(obj: *mut u8) -> Option<ObjHeader> {
    let rt = runtime();
    let heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.header_of(&rt.pool, obj)
}

pub fn space_of(obj: *mut u8) -> Option<HeapSpace> {
    let rt = runtime();
    let heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.space_of(&rt.pool, obj)
}

pub fn current_config() -> GcConfig {
    let rt = runtime();
    let heap = match rt.heap.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    heap.config
}
