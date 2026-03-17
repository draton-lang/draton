use super::heap::{GcRuntime, HeapState, ObjHeader, GC_MARKED, GC_OLD, HEADER};

impl GcRuntime {
    /// Write barrier invoked by generated code on every GC-pointer store.
    ///
    /// Fast path (completely lock-free — two atomic reads):
    ///   ① Read `GC_OLD` from the parent header (no lock; flag is immutable
    ///      after allocation in Draton's stop-the-world model).
    ///      Young parent → no cross-gen pointer possible → return.
    ///   ② Check whether child is in the young arena via `young.contains_ptr()`
    ///      which reads only the atomic bump pointer (no lock).
    ///      Child not in young gen → no old→young reference → return.
    ///
    /// Slow path (heap lock acquired; only old parent + young child):
    ///   Append parent to the remembered-set Vec (O(1), no hashing) and dirty
    ///   its card-table entry.  During incremental major GC also re-mark the
    ///   parent to preserve the tri-color invariant.
    pub fn write_barrier(&self, parent: *mut u8, _field: *mut u8, child: *mut u8) {
        if parent.is_null() { return; }

        // Fast path ①: is parent in old gen?
        let gc_flags = unsafe {
            (*parent.sub(HEADER).cast::<ObjHeader>()).gc_flags
        };
        if gc_flags & GC_OLD == 0 { return; }

        if child.is_null() { return; }

        // Fast path ②: is child in the young arena?
        // Lock-free: contains_ptr uses only an atomic Relaxed load of bump.
        if !self.young.contains_ptr(child as *const u8) { return; }

        // Slow path: old→young pointer — record it.
        let Ok(mut heap) = self.heap.lock() else { return };
        let parent_addr = parent as usize;

        heap.remembered_set.push(parent_addr); // Vec append, O(1), no hashing
        heap.card_table.mark_dirty(parent_addr);

        if heap.is_marking {
            mark_old_object(&mut heap, parent_addr);
        }
    }
}

/// Set GC_MARKED on an old-gen or large-object-space object.
fn mark_old_object(heap: &mut HeapState, addr: usize) {
    if let Some(bytes) = heap.old_objects.get_mut(&addr) {
        unsafe { (*bytes.as_mut_ptr().cast::<ObjHeader>()).gc_flags |= GC_MARKED; }
    } else if let Some(bytes) = heap.large_objects.get_mut(&addr) {
        unsafe { (*bytes.as_mut_ptr().cast::<ObjHeader>()).gc_flags |= GC_MARKED; }
    }
}
