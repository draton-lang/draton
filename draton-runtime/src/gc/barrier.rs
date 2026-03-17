use super::heap::{GcRuntime, HeapState, ObjHeader, GC_MARKED, GC_OLD, HEADER};

impl GcRuntime {
    /// Write barrier invoked by generated code on every GC-pointer store.
    ///
    /// Fast path (lock-free):
    ///   Read `GC_OLD` from the parent header without acquiring the heap lock.
    ///   This is safe in Draton's stop-the-world model: `GC_OLD` is set at
    ///   allocation time and never cleared; the GC and mutator never run
    ///   concurrently, so there is no data race on this field.
    ///   If the parent is in the young generation, no remembered-set action is
    ///   needed and we return immediately — same as OCaml's inline barrier.
    ///
    /// Slow path (lock acquired, old-gen parent only):
    ///   If parent is old and child is young, add parent to the remembered set
    ///   and dirty its card table entry.  If a major GC cycle is in progress,
    ///   also re-mark the parent to maintain the tri-color invariant.
    pub fn write_barrier(&self, parent: *mut u8, _field: *mut u8, child: *mut u8) {
        if parent.is_null() { return; }

        // ── Fast path ─────────────────────────────────────────────────────────
        // Read gc_flags directly from the header that precedes the payload.
        // Safe: see doc comment above.
        let gc_flags = unsafe {
            (*parent.sub(HEADER).cast::<ObjHeader>()).gc_flags
        };

        // Young parent writing any child: no cross-gen pointer to record.
        if gc_flags & GC_OLD == 0 { return; }
        if child.is_null() { return; }

        // ── Slow path ─────────────────────────────────────────────────────────
        let Ok(mut heap) = self.heap.lock() else { return };
        let parent_addr = parent as usize;

        // Track old→young cross-generation pointer.
        if heap.young.contains_ptr(child as *const u8) {
            heap.remembered_set.insert(parent_addr);
            heap.card_table.mark_dirty(parent_addr); // O(1) — single array store
        }

        // During incremental major GC marking: re-mark old parent (tri-color
        // invariant — if parent was black and now points to a white young child,
        // grey the parent so its new children are eventually traced).
        if heap.is_marking {
            mark_old_object(&mut heap, parent_addr);
        }
    }
}

/// Set GC_MARKED on an old-gen or large-object-space object.
fn mark_old_object(heap: &mut HeapState, addr: usize) {
    let mark_old = heap.old_objects.contains_key(&addr);
    if mark_old {
        let bytes = heap.old_objects.get_mut(&addr).unwrap();
        unsafe { (*bytes.as_mut_ptr().cast::<ObjHeader>()).gc_flags |= GC_MARKED; }
    } else if let Some(bytes) = heap.large_objects.get_mut(&addr) {
        unsafe { (*bytes.as_mut_ptr().cast::<ObjHeader>()).gc_flags |= GC_MARKED; }
    }
}
