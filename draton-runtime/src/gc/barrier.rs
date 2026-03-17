use super::heap::{GcRuntime, HeapState, ObjHeader, GC_FREE, GC_MARKED, GC_OLD, HEADER};

impl GcRuntime {
    /// Write barrier invoked by generated code on every GC-pointer store.
    ///
    /// Fast path (completely lock-free — one header read + one range check):
    ///   ① Read `GC_OLD` from the parent header.
    ///      Young parent → no cross-gen pointer possible → return.
    ///   ② Check whether child is in any young-pool slot via `pool.contains_ptr()`
    ///      (single range check on the pool's contiguous buffer — no lock).
    ///      Child not in young gen → no old→young reference → return.
    ///
    /// Slow path (heap lock acquired; only old parent + young child):
    ///   Append parent to the remembered-set Vec and dirty its card-table entry.
    pub fn write_barrier(&self, parent: *mut u8, _field: *mut u8, child: *mut u8) {
        if parent.is_null() { return; }

        // Fast path ①: is parent in old gen?
        let gc_flags = unsafe {
            (*parent.sub(HEADER).cast::<ObjHeader>()).gc_flags
        };
        if gc_flags & GC_OLD == 0 { return; }

        if child.is_null() { return; }

        // Fast path ②: is child in the young pool?
        // Lock-free: contains_ptr is a single range check on the pool buffer.
        if !self.pool.contains_ptr(child as *const u8) { return; }

        // Slow path: old→young pointer — record it.
        let Ok(mut heap) = self.heap.lock() else { return };
        let parent_addr = parent as usize;

        heap.remembered_set.push(parent_addr);
        heap.card_table.mark_dirty(parent_addr);

        if heap.is_marking {
            mark_old_object(&mut heap, parent_addr);
        }
    }
}

/// Set GC_MARKED on an old-gen or large-object-space object.
fn mark_old_object(heap: &mut HeapState, addr: usize) {
    if heap.old.contains_ptr(addr as *const u8) {
        let hdr_ptr = (addr - HEADER) as *mut ObjHeader;
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            if hdr.gc_flags & GC_FREE == 0 {
                hdr.gc_flags |= GC_MARKED;
                std::ptr::write(hdr_ptr, hdr);
            }
        }
    } else if let Some(bytes) = heap.large_objects.get_mut(&addr) {
        unsafe { (*bytes.as_mut_ptr().cast::<ObjHeader>()).gc_flags |= GC_MARKED; }
    }
}
