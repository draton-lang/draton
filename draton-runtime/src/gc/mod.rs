//! Incremental/generational GC runtime surface used by generated code.

pub mod barrier;
pub mod collect;
pub mod config;
pub mod heap;

use std::sync::{Arc, Mutex, OnceLock};

use config::GcConfig;
pub use heap::{GcRuntime, HeapSpace, ObjHeader, LARGE_OBJECT_THRESHOLD};

static GC_RUNTIME: OnceLock<Mutex<Option<Arc<GcRuntime>>>> = OnceLock::new();

fn global_slot() -> &'static Mutex<Option<Arc<GcRuntime>>> {
    GC_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn runtime() -> Arc<GcRuntime> {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(runtime) = guard.as_ref() {
        return Arc::clone(runtime);
    }
    let runtime = Arc::new(GcRuntime::new(GcConfig::default()));
    *guard = Some(Arc::clone(&runtime));
    runtime
}

/// Initializes the global GC runtime.
pub fn init() {
    let _ = runtime();
}

/// Shuts the GC down and frees tracked heap state.
pub fn shutdown() {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = None;
}

/// Applies a new configuration to the GC.
pub fn configure(config: GcConfig) {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.config = config.normalized();
}

/// Allocates a GC-managed object payload.
pub fn alloc(size: usize, type_id: u16) -> *mut u8 {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let payload = heap.alloc(size, type_id);
    if heap.current_usage() >= heap.config.heap_size {
        drop(heap);
        runtime.collect_minor();
    }
    payload
}

/// Allocates a GC-managed array payload.
pub fn alloc_array(elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let payload = heap.alloc_array(elem_size, len, type_id);
    if heap.current_usage() >= heap.config.heap_size {
        drop(heap);
        runtime.collect_minor();
    }
    payload
}

/// Applies the write barrier for a pointer store.
pub fn write_barrier(obj: *mut u8, field: *mut u8, new_val: *mut u8) {
    runtime().write_barrier(obj, field, new_val);
}

/// Triggers a full collection.
pub fn collect() {
    runtime().collect_full();
}

/// Pins the object against movement.
pub fn pin(obj: *mut u8) {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.pin(obj);
}

/// Unpins the object.
pub fn unpin(obj: *mut u8) {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.unpin(obj);
}

/// Protects an object as live for tests or host integration.
pub fn protect(obj: *mut u8) {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.protect(obj);
}

/// Releases a protected object.
pub fn release(obj: *mut u8) {
    let runtime = runtime();
    let mut heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.release(obj);
}

/// Returns the current object header for tests.
pub fn header_of(obj: *mut u8) -> Option<ObjHeader> {
    let runtime = runtime();
    let heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.header_of(obj)
}

/// Returns the space classification for a payload pointer.
pub fn space_of(obj: *mut u8) -> Option<HeapSpace> {
    let runtime = runtime();
    let heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.space_of(obj)
}

/// Returns the normalized active GC config.
pub fn current_config() -> GcConfig {
    let runtime = runtime();
    let heap = match runtime.heap.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    heap.config
}
