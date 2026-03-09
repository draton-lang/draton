use std::sync::{Mutex, MutexGuard, OnceLock};

use draton_runtime::gc;
use draton_runtime::gc::heap::{GC_OLD, GC_PINNED};

fn gc_test_guard() -> MutexGuard<'static, ()> {
    static GC_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = GC_TEST_LOCK.get_or_init(|| Mutex::new(()));
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[test]
fn alloc_collect_keeps_live_objects_valid() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(32, 1);
    assert!(!ptr.is_null());
    gc::collect();
    let header = gc::header_of(ptr).expect("header");
    assert_eq!(header.size, 32);
}

#[test]
fn write_barrier_keeps_child_object_reachable() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let parent = gc::alloc(16, 1);
    let child = gc::alloc(16, 2);
    gc::release(child);
    gc::write_barrier(parent, std::ptr::null_mut(), child);
    gc::collect();
    assert!(gc::header_of(child).is_some());
}

#[test]
fn promotion_moves_survivor_to_old_generation() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(24, 7);
    gc::collect();
    gc::collect();
    let header = gc::header_of(ptr).expect("header");
    assert_ne!(header.gc_flags & GC_OLD, 0);
}

#[test]
fn large_object_uses_large_object_space() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 128, 9);
    assert_eq!(gc::space_of(ptr), Some(gc::HeapSpace::Large));
}

#[test]
fn gc_config_applies_custom_heap_size() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        heap_size: 128 * 1024 * 1024,
        max_heap: 256 * 1024 * 1024,
        gc_threshold: 0.6,
        pause_target_ns: 50_000,
    });
    let config = gc::current_config();
    assert_eq!(config.heap_size, 128 * 1024 * 1024);
    assert_eq!(config.max_heap, 256 * 1024 * 1024);
}

#[test]
fn pin_and_unpin_toggle_pinned_flag() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(48, 3);
    gc::pin(ptr);
    let pinned = gc::header_of(ptr).expect("pinned header");
    assert_ne!(pinned.gc_flags & GC_PINNED, 0);
    gc::unpin(ptr);
    let unpinned = gc::header_of(ptr).expect("unpinned header");
    assert_eq!(unpinned.gc_flags & GC_PINNED, 0);
}
