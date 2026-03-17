//! Generational, incrementally-collected GC runtime surface used by generated code.

pub mod barrier;
pub mod collect;
pub mod config;
pub mod heap;

use std::sync::{Arc, Mutex, OnceLock};

use config::GcConfig;
pub use heap::{GcRuntime, HeapSpace, ObjHeader, CARD_BYTES};

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

    // Trigger collection heuristics after the alloc.
    let old_usage = heap.old_usage();
    let threshold = (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    let needs_major = old_usage >= threshold;
    let needs_minor = heap.young_usage() >= heap.config.young_size.saturating_sub(64);
    drop(heap);

    if needs_minor {
        rt.collect_minor();
    }
    if needs_major {
        rt.collect_major_slice(); // incremental — bounded pause
    }

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

    if needs_major {
        rt.collect_major_slice();
    }

    payload
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
