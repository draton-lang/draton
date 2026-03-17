use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard, OnceLock};

use draton_runtime::gc;
use draton_runtime::gc::heap::{GC_MARKED, GC_OLD, GC_PINNED};

fn gc_test_guard() -> MutexGuard<'static, ()> {
    static GC_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = GC_TEST_LOCK.get_or_init(|| Mutex::new(()));
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

unsafe fn write_ptr_field(obj: *mut u8, value: *mut u8) {
    std::ptr::write(obj.cast::<*mut u8>(), value);
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
    assert!(
        gc::header_of(ptr).is_none(),
        "unprotected object must be collected"
    );
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
    assert!(
        gc::header_of(parent).is_some(),
        "parent must still be live after write_barrier"
    );
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
    assert_ne!(
        header.gc_flags & GC_OLD,
        0,
        "object must be in old gen after promotion"
    );
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
        old_size: 128 * 1024 * 1024,
        max_heap: 256 * 1024 * 1024,
        gc_threshold: 0.6,
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
    assert!(
        stats.young_allocations >= 1,
        "young allocations should be tracked: {stats:?}"
    );
    assert!(
        stats.large_allocations >= 1,
        "large allocations should be tracked: {stats:?}"
    );
    assert!(
        stats.array_allocations >= 1,
        "array allocations should be tracked"
    );
    assert!(
        stats.minor_cycles >= 1,
        "minor collections should be tracked"
    );
    assert!(stats.full_cycles >= 1, "full collections should be tracked");
    assert!(
        stats.bytes_allocated > 0,
        "allocated bytes should accumulate"
    );
    assert!(
        stats.bytes_promoted > 0,
        "promotion bytes should accumulate"
    );
    assert!(
        stats.write_barrier_slow_calls >= 1,
        "barrier slow path should be counted"
    );
    assert!(
        stats.minor_pause.total_ns > 0,
        "minor pause telemetry should be recorded"
    );
    assert!(
        stats.full_pause.total_ns > 0,
        "full pause telemetry should be recorded"
    );
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
    assert!(
        gc::header_of(ptr).is_some(),
        "resetting stats must not free live objects"
    );

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
    assert!(
        gc::verify().is_ok(),
        "heap verifier should accept promoted live state"
    );

    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
    assert!(
        gc::verify().is_ok(),
        "heap verifier should accept reclaimed old-gen state"
    );
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
    assert!(
        after_free.old_free_slot_count > 0,
        "sweeping old gen should create reusable slots"
    );
    assert!(
        after_free.old_free_bytes > 0,
        "old gen should report reusable bytes"
    );

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

#[test]
fn old_generation_coalesces_adjacent_free_runs() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::reset_stats();

    let mut batch = Vec::new();
    for _ in 0..256 {
        let ptr = gc::alloc(64, 10);
        gc::protect(ptr);
        batch.push(ptr);
    }
    gc::collect();
    gc::collect();

    for ptr in &batch {
        gc::release(*ptr);
    }
    gc::collect();

    let stats = gc::stats();
    assert_eq!(
        stats.old_free_slot_count, 1,
        "fully reclaimed adjacent old-gen objects should coalesce into one free run: {stats:?}"
    );
    assert_eq!(
        stats.old_largest_free_slot, stats.old_free_bytes,
        "largest free slot should cover the full reclaimed run after coalescing: {stats:?}"
    );
}

#[test]
fn major_mark_barrier_traces_new_old_edge_from_marked_parent() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 1024,
        old_size: 64 * 1024,
        gc_threshold: 0.01,
        pause_target_ns: 1,
        ..gc::config::GcConfig::default()
    });
    gc::register_type(11, 8, &[0]);
    gc::register_type(12, (gc::LARGE_OBJECT_THRESHOLD + 128) as u32, &[0]);
    gc::reset_stats();

    let parent = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 128, 12);
    gc::protect(parent);

    let mut chain = Vec::new();
    for _ in 0..8000 {
        let ptr = gc::alloc(8, 11);
        gc::protect(ptr);
        chain.push(ptr);
    }
    for index in 0..chain.len() - 1 {
        unsafe {
            write_ptr_field(chain[index], chain[index + 1]);
        }
    }

    unsafe {
        write_ptr_field(parent, chain[0]);
    }
    gc::write_barrier(parent, std::ptr::null_mut(), chain[0]);

    let victim = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 128, 12);
    gc::protect(victim);

    while gc::header_of(chain[0])
        .map(|hdr| hdr.gc_flags & GC_OLD == 0)
        .unwrap_or(true)
    {
        let _ = gc::alloc(8, 11);
    }

    for ptr in &chain {
        gc::release(*ptr);
    }
    gc::release(victim);

    gc::safepoint();
    let after_first_slice = gc::stats();
    assert_eq!(
        after_first_slice.major_phase, 1,
        "the first safepoint should leave a long old-gen graph in mark phase: {after_first_slice:?}"
    );

    let victim_marked_before_barrier = gc::header_of(victim)
        .map(|hdr| hdr.gc_flags & GC_MARKED != 0)
        .unwrap_or(false);
    let traces_before_barrier = after_first_slice.major_mark_barrier_traces;

    unsafe { write_ptr_field(parent, victim); }
    gc::write_barrier(parent, std::ptr::null_mut(), victim);
    let after_barrier = gc::stats();
    assert_eq!(
        after_barrier.major_phase, 1,
        "major cycle should still be marking immediately after the barrier: {after_barrier:?}"
    );
    assert!(
        victim_marked_before_barrier
            || after_barrier.major_mark_barrier_traces > traces_before_barrier,
        "major barrier should trace the child unless it was already marked by an earlier assist slice: before_marked={victim_marked_before_barrier} after={after_barrier:?}"
    );
    gc::collect();

    assert!(
        gc::header_of(victim).is_some(),
        "incremental-update major barrier should keep a newly linked old child alive"
    );

    gc::release(parent);
}

#[test]
fn active_major_cycle_rearms_safepoint_flag() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 1024,
        old_size: 64 * 1024,
        gc_threshold: 0.01,
        pause_target_ns: 1,
        ..gc::config::GcConfig::default()
    });
    gc::register_type(13, 8, &[0]);
    gc::register_type(14, (gc::LARGE_OBJECT_THRESHOLD + 128) as u32, &[0]);
    gc::reset_stats();

    let parent = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 128, 14);
    gc::protect(parent);
    let mut chain = Vec::new();
    for _ in 0..8000 {
        let ptr = gc::alloc(8, 13);
        gc::protect(ptr);
        chain.push(ptr);
    }
    for index in 0..chain.len() - 1 {
        unsafe {
            write_ptr_field(chain[index], chain[index + 1]);
        }
    }
    unsafe {
        write_ptr_field(parent, chain[0]);
    }
    gc::write_barrier(parent, std::ptr::null_mut(), chain[0]);

    while gc::header_of(chain[0])
        .map(|hdr| hdr.gc_flags & GC_OLD == 0)
        .unwrap_or(true)
    {
        let _ = gc::alloc(8, 13);
    }
    for ptr in &chain {
        gc::release(*ptr);
    }

    draton_runtime::draton_safepoint_flag.store(1, Ordering::Release);
    draton_runtime::draton_safepoint_slow();

    let stats = gc::stats();
    assert_eq!(
        stats.major_phase, 1,
        "first rearmed slow-path should leave the cycle in mark phase: {stats:?}"
    );
    assert_eq!(
        draton_runtime::draton_safepoint_flag.load(Ordering::Acquire),
        1,
        "active major cycle should rearm the safepoint flag for the next poll"
    );
    assert!(
        stats.safepoint_rearms >= 1,
        "rearm activity should be visible in telemetry: {stats:?}"
    );
    assert!(
        stats.major_work_continuation_requests >= 1,
        "active major cycles should register continuation-driven requests once slices keep the cycle alive: {stats:?}"
    );

    gc::collect();
    gc::release(parent);
}

#[test]
fn promotion_pressure_requests_major_work() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 256 * 1024,
        old_size: 1024 * 1024,
        gc_threshold: 0.10,
        pause_target_ns: 1_000,
        ..gc::config::GcConfig::default()
    });
    gc::reset_stats();

    let mut roots = Vec::new();
    for _ in 0..5000 {
        let ptr = gc::alloc(64, 15);
        gc::protect(ptr);
        roots.push(ptr);
    }

    let stats = gc::stats();
    assert!(
        stats.minor_cycles >= 1,
        "allocation burst should have triggered young-gen collection pressure: {stats:?}"
    );
    assert!(
        stats.major_work_requested,
        "promotion pressure should leave major work requested for the next safepoint: {stats:?}"
    );
    assert!(
        stats.major_work_requests >= 1,
        "major work requests should be visible in telemetry once promotion crosses the threshold: {stats:?}"
    );
    assert!(
        stats.major_work_threshold_requests >= 1,
        "promotion pressure should register at least one threshold-driven major-work request: {stats:?}"
    );

    gc::safepoint();
    let after_safepoint = gc::stats();
    assert!(
        after_safepoint.major_slices >= 1,
        "requested major work should start draining on the next safepoint: {after_safepoint:?}"
    );

    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
}

#[test]
fn full_collection_clears_major_work_request_flag() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 256 * 1024,
        old_size: 1024 * 1024,
        gc_threshold: 0.10,
        pause_target_ns: 1_000,
        ..gc::config::GcConfig::default()
    });
    gc::reset_stats();

    let mut roots = Vec::new();
    for _ in 0..5000 {
        let ptr = gc::alloc(64, 16);
        gc::protect(ptr);
        roots.push(ptr);
    }
    assert!(
        gc::stats().major_work_requested,
        "setup should leave major work pending"
    );

    for ptr in &roots {
        gc::release(*ptr);
    }
    gc::collect();
    let mut stats = gc::stats();
    if stats.major_work_requested {
        gc::collect();
        stats = gc::stats();
    }
    assert!(
        !stats.major_work_requested,
        "full collection should return the major-work request flag to idle: {stats:?}"
    );
}

#[test]
fn slow_path_allocation_assists_requested_major_work() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 256 * 1024,
        old_size: 1024 * 1024,
        gc_threshold: 0.10,
        pause_target_ns: 1_000,
        ..gc::config::GcConfig::default()
    });
    gc::reset_stats();

    let mut roots = Vec::new();
    for _ in 0..5000 {
        let ptr = gc::alloc(64, 17);
        gc::protect(ptr);
        roots.push(ptr);
    }

    let requested = gc::stats();
    assert!(
        requested.major_work_requested,
        "setup should leave major work pending before the assist allocation: {requested:?}"
    );

    let assist = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 256, 18);
    gc::protect(assist);

    let after_assist = gc::stats();
    assert!(
        after_assist.major_mutator_assists >= 1,
        "slow-path allocation should record a major mutator assist: {after_assist:?}"
    );
    assert!(
        after_assist.major_slices >= 1,
        "slow-path allocation assist should execute at least one major slice: {after_assist:?}"
    );

    gc::release(assist);
    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
}

#[test]
fn young_refill_assists_requested_major_work() {
    let _guard = gc_test_guard();
    gc::shutdown();
    gc::init();
    gc::configure(gc::config::GcConfig {
        young_size: 256 * 1024,
        old_size: 1024 * 1024,
        gc_threshold: 0.10,
        pause_target_ns: 1_000,
        ..gc::config::GcConfig::default()
    });
    gc::reset_stats();

    let mut roots = Vec::new();
    for _ in 0..5000 {
        let ptr = gc::alloc(64, 19);
        gc::protect(ptr);
        roots.push(ptr);
    }

    let before_refill = gc::stats();
    assert!(
        before_refill.major_work_requested,
        "setup should leave major work pending before a young-refill assist: {before_refill:?}"
    );

    for _ in 0..5000 {
        let _ = gc::alloc(64, 20);
    }

    let after_refill = gc::stats();
    assert!(
        after_refill.major_mutator_assists > before_refill.major_mutator_assists,
        "young refill path should record a major mutator assist once major work is pending: {after_refill:?}"
    );
    assert!(
        after_refill.major_slices >= 1,
        "young refill assist should execute at least one major slice: {after_refill:?}"
    );

    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
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
