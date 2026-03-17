//! Generational, incrementally-collected GC runtime surface used by generated code.

pub mod barrier;
pub mod collect;
pub mod config;
pub mod heap;

use std::sync::{Arc, Mutex, OnceLock};

use config::GcConfig;
pub use heap::{GcRuntime, HeapSpace, ObjHeader, CARD_BYTES};

/// Objects larger than this are allocated directly in the large-object space.
/// Matches `GcConfig::default().large_threshold`.
pub const LARGE_OBJECT_THRESHOLD: usize = 32 * 1024;

// ── Shadow-stack root scanning ─────────────────────────────────────────────────

/// Walk LLVM's shadow-stack frame list and return the payload address of every
/// non-null GC root reachable from it.
///
/// Each `StackEntry.roots[i]` is an **alloca address** (`*mut *mut u8`): the
/// address of the stack slot that holds the GC pointer.  We dereference once to
/// get the actual managed-object payload pointer.
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
        // Root slots start immediately after the StackEntry header.
        let base = (entry as *const u8).add(size_of::<StackEntry>())
                   as *const *mut *mut u8; // &[alloca_ptr; num_roots]
        for i in 0..num_roots {
            let alloca = *base.add(i); // pointer to the stack slot
            if alloca.is_null() { continue; }
            let val = *alloca;         // current value of the GC root
            if !val.is_null() {
                roots.push(val as usize);
            }
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
    if let Some(rt) = guard.as_ref() {
        return Arc::clone(rt);
    }
    let rt = Arc::new(GcRuntime::new(GcConfig::default()));
    *guard = Some(Arc::clone(&rt));
    rt
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

/// Initialises the global GC runtime (idempotent).
pub fn init() {
    let _ = runtime();
}

/// Shuts the GC down and frees all tracked heap state.
pub fn shutdown() {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    *guard = None;
}

/// Applies a new configuration to the GC.
pub fn configure(config: GcConfig) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.config = config.normalized();
}

// ── Type registration ─────────────────────────────────────────────────────────

/// Register a type descriptor so the GC can precisely trace pointer fields.
///
/// Must be called before any allocations of `type_id`.
/// `offsets` is a slice of byte offsets from the payload start where GC-managed
/// pointer fields reside.
pub fn register_type(type_id: u16, size: u32, offsets: &[u32]) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.register_type(type_id, size, offsets);
}

// ── Allocation ────────────────────────────────────────────────────────────────

/// Allocates a GC-managed object payload.
pub fn alloc(size: usize, type_id: u16) -> *mut u8 {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let payload = heap.alloc(size, type_id);

    // Compute whether collection is needed before releasing the lock.
    let old_usage = heap.old_usage();
    let threshold = (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    let needs_major = old_usage >= threshold;
    let needs_minor = heap.young_usage() >= heap.config.young_size.saturating_sub(64);
    drop(heap);

    signal_gc(rt, needs_minor, needs_major);
    payload
}

/// Allocates a GC-managed array payload.
pub fn alloc_array(elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let payload = heap.alloc_array(elem_size, len, type_id);
    let old_usage = heap.old_usage();
    let threshold = (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    let needs_major = old_usage >= threshold;
    drop(heap);

    signal_gc(rt, false, needs_major);
    payload
}

/// Decide how to request a GC cycle depending on build context.
///
/// In non-test builds, generated code contains safepoint polls that read
/// `draton_safepoint_flag`; setting it to 1 requests collection at the next
/// poll without blocking the alloc hot path.
///
/// In test builds there are no generated safepoint polls (pure Rust test
/// driver), so we fall back to direct in-line collection.
#[inline]
fn signal_gc(rt: Arc<GcRuntime>, needs_minor: bool, needs_major: bool) {
    #[cfg(not(test))]
    {
        if needs_minor || needs_major {
            use std::sync::atomic::Ordering;
            crate::draton_safepoint_flag.store(1, Ordering::Release);
        }
    }
    #[cfg(test)]
    {
        if needs_minor { rt.collect_minor(); }
        if needs_major { rt.collect_major_slice(); }
    }
    // Suppress unused-variable warning in non-test builds.
    #[cfg(not(test))]
    { let _ = (rt, needs_minor, needs_major); }
}

// ── Safepoint slow path ────────────────────────────────────────────────────────

/// Called by the runtime safepoint slow path when `draton_safepoint_flag != 0`.
/// Clears the flag, then runs whichever collection passes are needed.
pub fn safepoint() {
    let rt = runtime();
    let (needs_minor, needs_major) = {
        let heap = match rt.heap.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let nm = heap.young_usage() >= heap.config.young_size.saturating_sub(64);
        let nj = heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
        (nm, nj)
    };
    if needs_minor { rt.collect_minor(); }
    if needs_major { rt.collect_major_slice(); }
}

// ── Write barrier ─────────────────────────────────────────────────────────────

/// Applies the write barrier for a pointer store.
pub fn write_barrier(obj: *mut u8, field: *mut u8, new_val: *mut u8) {
    runtime().write_barrier(obj, field, new_val);
}

// ── Manual collection ─────────────────────────────────────────────────────────

/// Triggers a full (minor + complete major) GC cycle.
pub fn collect() {
    runtime().collect_full();
}

// ── Pinning ───────────────────────────────────────────────────────────────────

pub fn pin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.pin(obj);
}

pub fn unpin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.unpin(obj);
}

// ── Explicit root management ──────────────────────────────────────────────────

pub fn protect(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.protect(obj);
}

pub fn release(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.release(obj);
}

// ── Inspection (tests / diagnostics) ─────────────────────────────────────────

pub fn header_of(obj: *mut u8) -> Option<ObjHeader> {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.header_of(obj)
}

pub fn space_of(obj: *mut u8) -> Option<HeapSpace> {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.space_of(obj)
}

pub fn current_config() -> GcConfig {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.config
}
