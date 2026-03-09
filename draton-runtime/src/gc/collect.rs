use super::heap::{GcRuntime, HeapSpace, GC_MARKED, GC_OLD, GC_PINNED};

const PROMOTION_AGE: u8 = 2;

impl GcRuntime {
    pub fn collect_minor(&self) {
        let Ok(mut heap) = self.heap.lock() else {
            return;
        };
        heap.minor_cycles = heap.minor_cycles.saturating_add(1);
        for record in &mut heap.objects {
            if matches!(
                record.space,
                HeapSpace::YoungEden | HeapSpace::YoungSurvivor
            ) {
                let mut header = record.header();
                if record.protected || (header.gc_flags & GC_PINNED) != 0 {
                    record.minor_survivals = record.minor_survivals.saturating_add(1);
                    if record.minor_survivals >= PROMOTION_AGE {
                        record.space = HeapSpace::Old;
                        header.gc_flags |= GC_OLD;
                    } else {
                        record.space = HeapSpace::YoungSurvivor;
                    }
                    header.gc_flags |= GC_MARKED;
                    record.set_header(header);
                } else {
                    header.gc_flags &= !GC_MARKED;
                    record.set_header(header);
                }
            }
        }
    }

    pub fn collect_major(&self) {
        let Ok(mut heap) = self.heap.lock() else {
            return;
        };
        heap.major_cycles = heap.major_cycles.saturating_add(1);
        heap.objects.retain_mut(|record| {
            let mut header = record.header();
            let keep = record.protected || (header.gc_flags & GC_PINNED) != 0;
            if keep {
                header.gc_flags |= GC_MARKED;
                record.set_header(header);
            }
            keep
        });
    }

    pub fn collect_full(&self) {
        self.collect_minor();
        self.collect_major();
    }
}
