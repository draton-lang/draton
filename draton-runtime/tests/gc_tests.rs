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

#[test]
fn gc_stats_capture_alloc_collect_and_barrier_activity() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::reset_stats();

    let parent = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 96, 1);
    gc::protect(parent);

    let child = gc::alloc(24, 2);
    gc::protect(child);
    gc::write_barrier(parent, std::ptr::null_mut(), child);
    let large = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 64, 3);
    gc::protect(large);
    let _array = gc::alloc_array(8, 4, 4);
    gc::collect();

    let stats = gc::stats();
    assert!(stats.young_allocations >= 1, "young allocations should be tracked: {stats:?}");
    assert!(stats.large_allocations >= 1, "large allocations should be tracked: {stats:?}");
    assert!(stats.array_allocations >= 1, "array allocations should be tracked");
    assert!(stats.minor_cycles >= 1, "minor collections should be tracked");
    assert!(stats.full_cycles >= 1, "full collections should be tracked");
    assert!(stats.bytes_allocated > 0, "allocated bytes should accumulate");
    assert!(stats.bytes_promoted > 0, "promotion bytes should accumulate");
    assert!(stats.write_barrier_slow_calls >= 1, "barrier slow path should be counted");
    assert!(stats.minor_pause.total_ns > 0, "minor pause telemetry should be recorded");
    assert!(stats.full_pause.total_ns > 0, "full pause telemetry should be recorded");
    assert!(stats.heap_usage_bytes >= stats.old_usage_bytes);

    gc::release(parent);
    gc::release(child);
    gc::release(large);
}

#[test]
fn gc_reset_stats_clears_counters_without_disrupting_heap() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::reset_stats();

    let ptr = gc::alloc(32, 5);
    gc::protect(ptr);
    gc::collect();
    assert!(gc::stats().bytes_allocated > 0);

    gc::reset_stats();
    let stats = gc::stats();
    assert_eq!(stats.bytes_allocated, 0);
    assert_eq!(stats.minor_cycles, 0);
    assert_eq!(stats.major_slices, 0);
    assert!(gc::header_of(ptr).is_some(), "resetting stats must not free live objects");

    gc::release(ptr);
}

#[test]
fn gc_heap_verifier_accepts_live_and_reclaimed_objects() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();

    let mut roots = Vec::new();
    for index in 0..256 {
        let ptr = gc::alloc(40 + (index % 3) * 8, 8);
        if index % 4 == 0 {
            gc::protect(ptr);
            roots.push(ptr);
        }
    }
    gc::collect();
    gc::collect();
    assert!(gc::verify().is_ok(), "heap verifier should accept promoted live state");

    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
    assert!(gc::verify().is_ok(), "heap verifier should accept reclaimed old-gen state");
}

#[test]
fn old_generation_reuses_swept_slots() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::reset_stats();

    let mut first_batch = Vec::new();
    for _ in 0..1024 {
        let ptr = gc::alloc(48, 6);
        gc::protect(ptr);
        first_batch.push(ptr);
    }
    gc::collect();
    gc::collect();
    for ptr in &first_batch {
        gc::release(*ptr);
    }
    gc::collect();

    let after_free = gc::stats();
    assert!(after_free.old_free_slot_count > 0, "sweeping old gen should create reusable slots");
    assert!(after_free.old_free_bytes > 0, "old gen should report reusable bytes");

    let mut second_batch = Vec::new();
    for _ in 0..1024 {
        let ptr = gc::alloc(48, 6);
        gc::protect(ptr);
        second_batch.push(ptr);
    }
    gc::collect();
    gc::collect();

    let after_reuse = gc::stats();
    assert!(
        after_reuse.old_free_bytes < after_free.old_free_bytes,
        "promoted objects should consume old free bytes instead of leaving fragmentation unchanged"
    );

    for ptr in &second_batch {
        gc::release(*ptr);
    }
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
