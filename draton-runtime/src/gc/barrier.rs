use super::heap::{GcRuntime, ObjHeader, GC_MARKED};

impl GcRuntime {
    /// Write barrier invoked by generated code on every GC-pointer store.
    ///
    /// Replaces the old `protect(child)` approach (which permanently pinned the
    /// child and caused memory leaks) with a correct card-table + remembered-set
    /// implementation.
    ///
    /// Responsibilities:
    /// 1. Mark the parent object as live (it is being written to, so reachable).
    /// 2. If parent is in old gen and child is in young gen: add parent to the
    ///    remembered set and dirty its card — O(1), no linear scan.
    pub fn write_barrier(&self, parent: *mut u8, _field: *mut u8, child: *mut u8) {
        if parent.is_null() {
            return;
        }
        let Ok(mut heap) = self.heap.lock() else { return };

        let parent_addr = parent as usize;

        // Mark the parent as live.
        let mark_old = heap.old_objects.contains_key(&parent_addr);
        let mark_large = !mark_old && heap.large_objects.contains_key(&parent_addr);
        if mark_old {
            let bytes = heap.old_objects.get_mut(&parent_addr).unwrap();
            let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
            unsafe {
                let mut hdr = std::ptr::read(hdr_ptr);
                hdr.gc_flags |= GC_MARKED;
                std::ptr::write(hdr_ptr, hdr);
            }
        } else if mark_large {
            let bytes = heap.large_objects.get_mut(&parent_addr).unwrap();
            let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
            unsafe {
                let mut hdr = std::ptr::read(hdr_ptr);
                hdr.gc_flags |= GC_MARKED;
                std::ptr::write(hdr_ptr, hdr);
            }
        }

        if child.is_null() {
            return;
        }
        // Track old→young cross-generation pointers via remembered set + card table.
        let parent_is_old = heap.old_objects.contains_key(&parent_addr)
            || heap.large_objects.contains_key(&parent_addr);
        let child_is_young = heap.young.contains_ptr(child as *const u8);

        if parent_is_old && child_is_young {
            heap.remembered_set.insert(parent_addr);
            heap.card_table.mark_dirty(parent_addr); // O(1) — single array store
        }
    }
}
