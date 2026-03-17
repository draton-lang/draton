//! Generational, incrementally-collected GC runtime surface used by generated code.

pub mod barrier;
pub mod collect;
pub mod config;
pub mod heap;
pub mod stats;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use config::GcConfig;
pub use heap::{GcRuntime, HeapSpace, ObjHeader, CARD_BYTES};
use heap::{HeapState, MajorPhase};
pub use stats::{GcPauseStats, GcStats};

/// Objects larger than this are allocated directly in the large-object space.
/// Matches `GcConfig::default().large_threshold`.
pub const LARGE_OBJECT_THRESHOLD: usize = 32 * 1024;
const MAX_MAJOR_WORK_BUDGET: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MajorWorkReason {
    Threshold,
    Continuation,
}

// ── Shadow-stack root scanning ─────────────────────────────────────────────────

/// Walk LLVM's shadow-stack frame list and return the payload address of every
/// non-null GC root reachable from it.
///
/// # Safety
/// Must only be called while the mutator is stopped (i.e. from within a GC
/// cycle).  In Draton's current stop-the-world model this is always satisfied.
pub unsafe fn shadow_stack_roots() -> Vec<usize> {
    use crate::llvm_gc_root_chain;
    use heap::StackEntry;
    use std::mem::size_of;

    let mut roots = Vec::new();
    let mut entry = llvm_gc_root_chain;
    while !entry.is_null() {
        let num_roots = (*(*entry).map).num_roots as usize;
        let base = (entry as *const u8).add(size_of::<StackEntry>()) as *const *mut *mut u8;
        for i in 0..num_roots {
            let alloca = *base.add(i);
            if alloca.is_null() {
                continue;
            }
            let val = *alloca;
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
    GcRuntime::start_major_worker(&rt);
    *guard = Some(Arc::clone(&rt));
    rt
}

#[inline]
pub(super) fn clear_major_work_state(rt: &GcRuntime) {
    rt.major_work_requested.store(false, Ordering::Release);
    rt.major_work_budget.store(0, Ordering::Release);
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

pub fn init() {
    let _ = runtime();
}

pub fn shutdown() {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(rt) = guard.take() {
        rt.stop_major_worker();
    }
}

/// Applies a new configuration to the GC.
pub fn configure(config: GcConfig) {
    let rt = runtime();
    let norm = config.normalized();
    rt.large_threshold
        .store(norm.large_threshold, Ordering::Relaxed);
    rt.young_size.store(norm.young_size, Ordering::Relaxed);
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.config = norm;
    let request =
        major_work_reason(&heap).map(|reason| (reason, desired_major_work_budget(&heap, reason)));
    let should_request_major = sync_major_work_request(&rt, &heap);
    drop(heap);
    if should_request_major {
        if let Some((reason, desired_budget)) = request {
            request_major_work_for_reason(&rt, reason, desired_budget);
        }
    }
}

// ── Type registration ─────────────────────────────────────────────────────────

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
///
/// Fast path (lock-free): bumps the calling thread's young-pool arena pointer.
/// Slow path: acquires the heap lock for old-gen / large-object allocation, or
/// when the thread's arena is full and needs to be collected first.
pub fn alloc(size: usize, type_id: u16) -> *mut u8 {
    let rt = runtime();
    let aligned = (heap::HEADER + size + 7) & !7;

    // ── Fast path: lock-free per-thread young-gen bump ────────────────────────
    let large_threshold = rt.large_threshold.load(Ordering::Relaxed);
    if size < large_threshold {
        if let Some(payload) = rt.pool.try_alloc(size, type_id) {
            rt.telemetry.record_young_alloc(aligned);
            if rt.pool.current_slot_nearly_full() {
                signal_gc_flag();
            }
            return payload;
        }

        // Thread's arena full — collect before returning a valid pointer.
        rt.collect_minor();
        assist_major_work_if_requested(&rt);

        if let Some(payload) = rt.pool.try_alloc(size, type_id) {
            rt.telemetry.record_young_alloc(aligned);
            return payload;
        }
        // If still full (tiny young_size), fall through to old-gen.
    }

    // ── Slow path: old-gen / large-object allocation ──────────────────────────
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let payload = heap.alloc_slow(size, type_id);
    if !payload.is_null() {
        if size >= heap.config.large_threshold {
            rt.telemetry.record_large_alloc(heap::HEADER + size);
        } else {
            rt.telemetry.record_old_alloc(aligned);
        }
    }
    let needs_major =
        heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    drop(heap);

    if needs_major {
        request_major_work(&rt);
    }
    assist_major_work_if_requested(&rt);
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
    if !payload.is_null() {
        rt.telemetry.record_array_alloc();
        let size = elem_size.saturating_mul(len);
        if size >= heap.config.large_threshold {
            rt.telemetry.record_large_alloc(heap::HEADER + size);
        } else {
            let aligned = (heap::HEADER + size + 7) & !7;
            rt.telemetry.record_old_alloc(aligned);
        }
    }
    let needs_major =
        heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize;
    drop(heap);
    if needs_major {
        request_major_work(&rt);
    }
    assist_major_work_if_requested(&rt);
    payload
}

/// Request GC at the next safepoint (non-test) or fall through (test).
#[inline]
pub(super) fn signal_gc_flag() {
    #[cfg(not(test))]
    {
        crate::draton_safepoint_flag.store(1, Ordering::Release);
    }
}

#[inline]
fn rearm_safepoint_flag(rt: &GcRuntime) {
    rt.telemetry.record_safepoint_rearm();
    signal_gc_flag();
}

#[inline]
pub(super) fn major_work_needed(heap: &HeapState) -> bool {
    heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize
        || heap.major_phase != MajorPhase::Idle
}

#[inline]
pub(super) fn sync_major_work_request(rt: &GcRuntime, heap: &HeapState) -> bool {
    let requested = major_work_reason(heap).is_some();
    rt.major_work_requested.store(requested, Ordering::Release);
    if !requested {
        rt.major_work_budget.store(0, Ordering::Release);
    }
    requested
}

#[inline]
fn major_work_reason(heap: &HeapState) -> Option<MajorWorkReason> {
    if heap.major_phase != MajorPhase::Idle {
        Some(MajorWorkReason::Continuation)
    } else if heap.old_usage() >= (heap.config.old_size as f64 * heap.config.gc_threshold) as usize
    {
        Some(MajorWorkReason::Threshold)
    } else {
        None
    }
}

#[inline]
fn desired_major_work_budget(heap: &HeapState, reason: MajorWorkReason) -> usize {
    match reason {
        MajorWorkReason::Threshold => {
            let threshold =
                ((heap.config.old_size as f64 * heap.config.gc_threshold) as usize).max(1);
            let usage = heap.old_usage();
            let pressure_steps = usage
                .saturating_sub(threshold)
                .saturating_mul(4)
                .checked_div(threshold)
                .unwrap_or(0)
                .min(4);
            (2 + pressure_steps).min(MAX_MAJOR_WORK_BUDGET)
        }
        MajorWorkReason::Continuation => match heap.major_phase {
            MajorPhase::Mark => {
                let slice = heap.mark_slice_size.max(1);
                let backlog = heap.mark_stack.len().saturating_add(slice - 1) / slice;
                (2 + backlog.min(4)).min(MAX_MAJOR_WORK_BUDGET)
            }
            MajorPhase::SweepOld => {
                let total = heap.old.bump.max(1);
                let remaining = total.saturating_sub(heap.old_sweep_cursor);
                let steps = remaining
                    .saturating_mul(4)
                    .checked_div(total)
                    .unwrap_or(0)
                    .min(4);
                (2 + steps).min(MAX_MAJOR_WORK_BUDGET)
            }
            MajorPhase::SweepLarge => {
                let slice = heap.mark_slice_size.max(1);
                let backlog = heap.large_sweep_pending.len().saturating_add(slice - 1) / slice;
                (2 + backlog.min(4)).min(MAX_MAJOR_WORK_BUDGET)
            }
            MajorPhase::Idle => 1,
        },
    }
}

#[inline]
fn raise_major_work_budget(rt: &GcRuntime, desired: usize) -> usize {
    let desired = desired.clamp(1, MAX_MAJOR_WORK_BUDGET);
    let mut current = rt.major_work_budget.load(Ordering::Acquire);
    loop {
        let next = current.max(desired);
        match rt.major_work_budget.compare_exchange(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return next,
            Err(observed) => current = observed,
        }
    }
}

#[inline]
fn request_major_work_for_reason(rt: &GcRuntime, reason: MajorWorkReason, desired_budget: usize) {
    rt.telemetry.record_major_work_request();
    match reason {
        MajorWorkReason::Threshold => rt.telemetry.record_major_work_threshold_request(),
        MajorWorkReason::Continuation => rt.telemetry.record_major_work_continuation_request(),
    }
    let budget = raise_major_work_budget(rt, desired_budget);
    rt.telemetry.record_major_work_budget_peak(budget);
    rt.major_work_requested.store(true, Ordering::Release);
    if matches!(reason, MajorWorkReason::Continuation) {
        rt.major_worker_signal.notify_one();
    }
    signal_gc_flag();
}

#[inline]
pub(super) fn request_major_work(rt: &GcRuntime) {
    let work = {
        let heap = match rt.heap.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        major_work_reason(&heap).map(|reason| (reason, desired_major_work_budget(&heap, reason)))
    };
    if let Some((reason, desired_budget)) = work {
        request_major_work_for_reason(rt, reason, desired_budget);
    }
}

#[inline]
fn assist_major_work_if_requested(rt: &GcRuntime) {
    if try_take_major_work_budget(rt) {
        rt.telemetry.record_major_mutator_assist();
        rt.collect_major_slice();
    }
}

#[inline]
fn try_take_major_work_budget(rt: &GcRuntime) -> bool {
    let mut current = rt.major_work_budget.load(Ordering::Acquire);
    while current != 0 {
        match rt.major_work_budget.compare_exchange(
            current,
            current - 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
    false
}

impl GcRuntime {
    fn start_major_worker(this: &Arc<Self>) {
        if this.major_worker_running.swap(true, Ordering::AcqRel) {
            return;
        }
        this.major_worker_stop.store(false, Ordering::Release);
        let weak = Arc::downgrade(this);
        let handle = thread::Builder::new()
            .name("draton-gc-major".to_string())
            .spawn(move || major_worker_loop(weak))
            .expect("failed to spawn draton gc major worker");
        let mut slot = match this.major_worker_handle.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        *slot = Some(handle);
    }

    fn stop_major_worker(&self) {
        self.major_worker_stop.store(true, Ordering::Release);
        self.major_worker_signal.notify_all();
        let mut slot = match self.major_worker_handle.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(handle) = slot.take() {
            let _ = handle.join();
        }
        self.major_worker_running.store(false, Ordering::Release);
    }
}

fn major_worker_loop(weak: std::sync::Weak<GcRuntime>) {
    loop {
        let Some(rt) = weak.upgrade() else {
            break;
        };

        let mut guard = match rt.major_worker_lock.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        while !rt.major_worker_stop.load(Ordering::Acquire)
            && (rt.major_work_budget.load(Ordering::Acquire) == 0
                || !background_major_cycle_active(&rt))
        {
            guard = match rt.major_worker_signal.wait(guard) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
        let should_stop = rt.major_worker_stop.load(Ordering::Acquire);
        drop(guard);
        if should_stop {
            break;
        }

        while background_major_cycle_active(&rt) && try_take_major_work_budget(&rt) {
            rt.telemetry.record_major_background_slice();
            rt.collect_major_slice();
            if rt.major_worker_stop.load(Ordering::Acquire) {
                break;
            }
        }
    }
}

fn background_major_cycle_active(rt: &GcRuntime) -> bool {
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.major_phase != MajorPhase::Idle
}

// ── Safepoint slow path ────────────────────────────────────────────────────────

/// Called by the runtime safepoint slow path when `draton_safepoint_flag != 0`.
pub fn safepoint() {
    let rt = runtime();
    let needs_minor = rt.pool.current_slot_nearly_full();
    let needs_major = try_take_major_work_budget(&rt);
    if needs_minor {
        rt.collect_minor();
    }
    if needs_major {
        rt.collect_major_slice();
    }

    let major_still_pending = {
        let heap = match rt.heap.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        sync_major_work_request(&rt, &heap)
    };
    let should_rearm = rt.pool.current_slot_nearly_full() || major_still_pending;
    if should_rearm {
        rearm_safepoint_flag(&rt);
    } else {
        clear_major_work_state(&rt);
    }
}

// ── Write barrier ─────────────────────────────────────────────────────────────

pub fn write_barrier(obj: *mut u8, field: *mut u8, new_val: *mut u8) {
    runtime().write_barrier(obj, field, new_val);
}

// ── Manual collection ─────────────────────────────────────────────────────────

pub fn collect() {
    runtime().collect_full();
}

pub fn stats() -> GcStats {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    rt.telemetry.snapshot(&rt, &heap)
}

pub fn reset_stats() {
    let rt = runtime();
    rt.telemetry.reset();
}

pub fn verify() -> Result<(), String> {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.verify_invariants(&rt.pool)
}

// ── Pinning ───────────────────────────────────────────────────────────────────

pub fn pin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.pin(&rt.pool, obj);
}

pub fn unpin(obj: *mut u8) {
    let rt = runtime();
    let mut heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.unpin(&rt.pool, obj);
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
    heap.header_of(&rt.pool, obj)
}

pub fn space_of(obj: *mut u8) -> Option<HeapSpace> {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.space_of(&rt.pool, obj)
}

pub fn current_config() -> GcConfig {
    let rt = runtime();
    let heap = match rt.heap.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    heap.config
}
