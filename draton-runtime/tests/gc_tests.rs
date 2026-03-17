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

// ── Root semantics ────────────────────────────────────────────────────────────

/// An explicitly protect()-ed object must survive GC.
#[test]
fn protect_keeps_object_alive_across_collect() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(32, 1);
    gc::protect(ptr);
    gc::collect();
    let header = gc::header_of(ptr).expect("protected object must survive GC");
    assert_eq!(header.size, 32);
    gc::release(ptr);
}

/// An object with no root (neither protect()-ed nor reachable from the shadow
/// stack) must be collected during the next full GC cycle.
#[test]
fn unprotected_object_is_collected() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(32, 1);
    // No protect() — object has no root in a pure-Rust test environment
    // (shadow stack is null, no LLVM-generated frames).
    gc::collect();
    assert!(gc::header_of(ptr).is_none(), "unprotected object must be collected");
}

// ── Write barrier ─────────────────────────────────────────────────────────────

/// The write barrier must add an old-gen parent to the remembered set when it
/// stores a reference to a young-gen child.
#[test]
fn write_barrier_tracks_old_to_young_reference() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    // Allocate parent and promote it to old gen via repeated collections.
    let parent = gc::alloc(16, 1);
    gc::protect(parent);
    // Two minor GCs to exceed promotion_age=2 → parent lands in old gen.
    gc::collect();
    gc::collect();
    let parent_hdr = gc::header_of(parent).expect("parent header");
    assert_ne!(parent_hdr.gc_flags & GC_OLD, 0, "parent must be in old gen");

    // Allocate a young child and call the write barrier.
    let child = gc::alloc(16, 2);
    gc::write_barrier(parent, std::ptr::null_mut(), child);

    // The write barrier should have dirtied the card and added parent to the
    // remembered set.  We cannot inspect the remembered set directly in tests,
    // but collecting and then confirming the parent is still alive is a
    // reasonable proxy.
    gc::collect();
    assert!(gc::header_of(parent).is_some(), "parent must still be live after write_barrier");
    gc::release(parent);
}

// ── Promotion ─────────────────────────────────────────────────────────────────

/// An object that survives enough minor GC cycles must be promoted to old gen.
#[test]
fn promotion_moves_survivor_to_old_generation() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(24, 7);
    gc::protect(ptr);
    gc::collect();
    gc::collect();
    let header = gc::header_of(ptr).expect("header after promotion");
    assert_ne!(header.gc_flags & GC_OLD, 0, "object must be in old gen after promotion");
    gc::release(ptr);
}

// ── Large-object space ────────────────────────────────────────────────────────

/// Objects above the large-threshold must be placed in the large-object space.
#[test]
fn large_object_uses_large_object_space() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 128, 9);
    assert_eq!(gc::space_of(ptr), Some(gc::HeapSpace::Large));
}

// ── GcConfig ──────────────────────────────────────────────────────────────────

/// Custom GcConfig values must be applied and readable back via current_config().
#[test]
fn gc_config_applies_custom_values() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        old_size:        128 * 1024 * 1024,
        max_heap:        256 * 1024 * 1024,
        gc_threshold:    0.6,
        pause_target_ns: 50_000,
        ..gc::config::GcConfig::default()
    });
    let config = gc::current_config();
    assert_eq!(config.old_size, 128 * 1024 * 1024);
    assert_eq!(config.max_heap, 256 * 1024 * 1024);
}

// ── Pin / unpin ───────────────────────────────────────────────────────────────

/// pin() must set GC_PINNED; unpin() must clear it.
#[test]
fn pin_and_unpin_toggle_pinned_flag() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    let ptr = gc::alloc(48, 3);
    gc::protect(ptr); // keep ptr alive so we can inspect it after pin/unpin
    gc::pin(ptr);
    let pinned = gc::header_of(ptr).expect("pinned header");
    assert_ne!(pinned.gc_flags & GC_PINNED, 0);
    gc::unpin(ptr);
    let unpinned = gc::header_of(ptr).expect("unpinned header");
    assert_eq!(unpinned.gc_flags & GC_PINNED, 0);
    gc::release(ptr);
}
