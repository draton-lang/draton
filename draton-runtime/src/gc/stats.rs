use std::sync::atomic::{AtomicU64, Ordering};

use super::heap::{GcRuntime, HeapState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcPauseStats {
    pub total_ns: u64,
    pub last_ns: u64,
    pub max_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcStats {
    pub minor_cycles: u64,
    pub major_cycles: u64,
    pub major_slices: u64,
    pub full_cycles: u64,
    pub young_allocations: u64,
    pub old_allocations: u64,
    pub large_allocations: u64,
    pub array_allocations: u64,
    pub bytes_allocated: u64,
    pub bytes_promoted: u64,
    pub bytes_reclaimed_minor: u64,
    pub bytes_reclaimed_major: u64,
    pub bytes_reclaimed_large: u64,
    pub write_barrier_slow_calls: u64,
    pub remembered_set_entries_added: u64,
    pub remembered_set_entries_deduped: u64,
    pub young_usage_bytes: usize,
    pub old_usage_bytes: usize,
    pub heap_usage_bytes: usize,
    pub large_object_count: usize,
    pub roots_count: usize,
    pub remembered_set_len: usize,
    pub old_free_slot_count: usize,
    pub old_free_bytes: usize,
    pub old_largest_free_slot: usize,
    pub current_mark_stack_len: usize,
    pub current_mark_slice_size: usize,
    pub major_phase: u8,
    pub old_sweep_cursor: usize,
    pub large_sweep_pending: usize,
    pub minor_pause: GcPauseStats,
    pub major_pause: GcPauseStats,
    pub full_pause: GcPauseStats,
}

#[derive(Debug)]
pub struct GcTelemetry {
    minor_cycles: AtomicU64,
    major_cycles: AtomicU64,
    major_slices: AtomicU64,
    full_cycles: AtomicU64,
    young_allocations: AtomicU64,
    old_allocations: AtomicU64,
    large_allocations: AtomicU64,
    array_allocations: AtomicU64,
    bytes_allocated: AtomicU64,
    bytes_promoted: AtomicU64,
    bytes_reclaimed_minor: AtomicU64,
    bytes_reclaimed_major: AtomicU64,
    bytes_reclaimed_large: AtomicU64,
    write_barrier_slow_calls: AtomicU64,
    remembered_set_entries_added: AtomicU64,
    remembered_set_entries_deduped: AtomicU64,
    minor_pause_total_ns: AtomicU64,
    minor_pause_last_ns: AtomicU64,
    minor_pause_max_ns: AtomicU64,
    major_pause_total_ns: AtomicU64,
    major_pause_last_ns: AtomicU64,
    major_pause_max_ns: AtomicU64,
    full_pause_total_ns: AtomicU64,
    full_pause_last_ns: AtomicU64,
    full_pause_max_ns: AtomicU64,
}

impl GcTelemetry {
    pub fn new() -> Self {
        Self {
            minor_cycles: AtomicU64::new(0),
            major_cycles: AtomicU64::new(0),
            major_slices: AtomicU64::new(0),
            full_cycles: AtomicU64::new(0),
            young_allocations: AtomicU64::new(0),
            old_allocations: AtomicU64::new(0),
            large_allocations: AtomicU64::new(0),
            array_allocations: AtomicU64::new(0),
            bytes_allocated: AtomicU64::new(0),
            bytes_promoted: AtomicU64::new(0),
            bytes_reclaimed_minor: AtomicU64::new(0),
            bytes_reclaimed_major: AtomicU64::new(0),
            bytes_reclaimed_large: AtomicU64::new(0),
            write_barrier_slow_calls: AtomicU64::new(0),
            remembered_set_entries_added: AtomicU64::new(0),
            remembered_set_entries_deduped: AtomicU64::new(0),
            minor_pause_total_ns: AtomicU64::new(0),
            minor_pause_last_ns: AtomicU64::new(0),
            minor_pause_max_ns: AtomicU64::new(0),
            major_pause_total_ns: AtomicU64::new(0),
            major_pause_last_ns: AtomicU64::new(0),
            major_pause_max_ns: AtomicU64::new(0),
            full_pause_total_ns: AtomicU64::new(0),
            full_pause_last_ns: AtomicU64::new(0),
            full_pause_max_ns: AtomicU64::new(0),
        }
    }

    pub fn reset(&self) {
        for counter in [
            &self.minor_cycles,
            &self.major_cycles,
            &self.major_slices,
            &self.full_cycles,
            &self.young_allocations,
            &self.old_allocations,
            &self.large_allocations,
            &self.array_allocations,
            &self.bytes_allocated,
            &self.bytes_promoted,
            &self.bytes_reclaimed_minor,
            &self.bytes_reclaimed_major,
            &self.bytes_reclaimed_large,
            &self.write_barrier_slow_calls,
            &self.remembered_set_entries_added,
            &self.remembered_set_entries_deduped,
            &self.minor_pause_total_ns,
            &self.minor_pause_last_ns,
            &self.minor_pause_max_ns,
            &self.major_pause_total_ns,
            &self.major_pause_last_ns,
            &self.major_pause_max_ns,
            &self.full_pause_total_ns,
            &self.full_pause_last_ns,
            &self.full_pause_max_ns,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn record_young_alloc(&self, aligned_bytes: usize) {
        self.young_allocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_allocated.fetch_add(aligned_bytes as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_old_alloc(&self, aligned_bytes: usize) {
        self.old_allocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_allocated.fetch_add(aligned_bytes as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_large_alloc(&self, total_bytes: usize) {
        self.large_allocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_allocated.fetch_add(total_bytes as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_array_alloc(&self) {
        self.array_allocations.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_minor_cycle(&self, pause_ns: u64, promoted_bytes: usize, reclaimed_bytes: usize) {
        self.minor_cycles.fetch_add(1, Ordering::Relaxed);
        self.bytes_promoted.fetch_add(promoted_bytes as u64, Ordering::Relaxed);
        self.bytes_reclaimed_minor.fetch_add(reclaimed_bytes as u64, Ordering::Relaxed);
        self.record_pause(
            &self.minor_pause_total_ns,
            &self.minor_pause_last_ns,
            &self.minor_pause_max_ns,
            pause_ns,
        );
    }

    #[inline]
    pub fn record_major_cycle_start(&self) {
        self.major_cycles.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_major_slice(&self, pause_ns: u64, reclaimed_old: usize, reclaimed_large: usize) {
        self.major_slices.fetch_add(1, Ordering::Relaxed);
        self.bytes_reclaimed_major.fetch_add(reclaimed_old as u64, Ordering::Relaxed);
        self.bytes_reclaimed_large.fetch_add(reclaimed_large as u64, Ordering::Relaxed);
        self.record_pause(
            &self.major_pause_total_ns,
            &self.major_pause_last_ns,
            &self.major_pause_max_ns,
            pause_ns,
        );
    }

    #[inline]
    pub fn record_full_cycle(&self, pause_ns: u64) {
        self.full_cycles.fetch_add(1, Ordering::Relaxed);
        self.record_pause(
            &self.full_pause_total_ns,
            &self.full_pause_last_ns,
            &self.full_pause_max_ns,
            pause_ns,
        );
    }

    #[inline]
    pub fn record_write_barrier_slow(&self) {
        self.write_barrier_slow_calls.fetch_add(1, Ordering::Relaxed);
        self.remembered_set_entries_added.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_remembered_set_deduped(&self, entries: usize) {
        self.remembered_set_entries_deduped
            .fetch_add(entries as u64, Ordering::Relaxed);
    }

    pub fn snapshot(&self, runtime: &GcRuntime, heap: &HeapState) -> GcStats {
        let young_usage = runtime.pool.used_bytes();
        let old_usage = heap.old_bytes;
        GcStats {
            minor_cycles: self.minor_cycles.load(Ordering::Relaxed),
            major_cycles: self.major_cycles.load(Ordering::Relaxed),
            major_slices: self.major_slices.load(Ordering::Relaxed),
            full_cycles: self.full_cycles.load(Ordering::Relaxed),
            young_allocations: self.young_allocations.load(Ordering::Relaxed),
            old_allocations: self.old_allocations.load(Ordering::Relaxed),
            large_allocations: self.large_allocations.load(Ordering::Relaxed),
            array_allocations: self.array_allocations.load(Ordering::Relaxed),
            bytes_allocated: self.bytes_allocated.load(Ordering::Relaxed),
            bytes_promoted: self.bytes_promoted.load(Ordering::Relaxed),
            bytes_reclaimed_minor: self.bytes_reclaimed_minor.load(Ordering::Relaxed),
            bytes_reclaimed_major: self.bytes_reclaimed_major.load(Ordering::Relaxed),
            bytes_reclaimed_large: self.bytes_reclaimed_large.load(Ordering::Relaxed),
            write_barrier_slow_calls: self.write_barrier_slow_calls.load(Ordering::Relaxed),
            remembered_set_entries_added: self.remembered_set_entries_added.load(Ordering::Relaxed),
            remembered_set_entries_deduped: self.remembered_set_entries_deduped.load(Ordering::Relaxed),
            young_usage_bytes: young_usage,
            old_usage_bytes: old_usage,
            heap_usage_bytes: young_usage.saturating_add(old_usage),
            large_object_count: heap.large_objects.len(),
            roots_count: heap.roots.len(),
            remembered_set_len: heap.remembered_set.len(),
            old_free_slot_count: heap.old.free_slot_count(),
            old_free_bytes: heap.old.free_bytes(),
            old_largest_free_slot: heap.old.largest_free_slot(),
            current_mark_stack_len: heap.mark_stack.len(),
            current_mark_slice_size: heap.mark_slice_size,
            major_phase: match heap.major_phase {
                super::heap::MajorPhase::Idle => 0,
                super::heap::MajorPhase::Mark => 1,
                super::heap::MajorPhase::SweepOld => 2,
                super::heap::MajorPhase::SweepLarge => 3,
            },
            old_sweep_cursor: heap.old_sweep_cursor,
            large_sweep_pending: heap.large_sweep_pending.len(),
            minor_pause: GcPauseStats {
                total_ns: self.minor_pause_total_ns.load(Ordering::Relaxed),
                last_ns: self.minor_pause_last_ns.load(Ordering::Relaxed),
                max_ns: self.minor_pause_max_ns.load(Ordering::Relaxed),
            },
            major_pause: GcPauseStats {
                total_ns: self.major_pause_total_ns.load(Ordering::Relaxed),
                last_ns: self.major_pause_last_ns.load(Ordering::Relaxed),
                max_ns: self.major_pause_max_ns.load(Ordering::Relaxed),
            },
            full_pause: GcPauseStats {
                total_ns: self.full_pause_total_ns.load(Ordering::Relaxed),
                last_ns: self.full_pause_last_ns.load(Ordering::Relaxed),
                max_ns: self.full_pause_max_ns.load(Ordering::Relaxed),
            },
        }
    }

    fn record_pause(
        &self,
        total: &AtomicU64,
        last: &AtomicU64,
        max: &AtomicU64,
        pause_ns: u64,
    ) {
        total.fetch_add(pause_ns, Ordering::Relaxed);
        last.store(pause_ns, Ordering::Relaxed);
        let _ = max.fetch_max(pause_ns, Ordering::Relaxed);
    }
}
