use std::sync::atomic::Ordering;

use super::heap::{GcRuntime, HeapState, MajorPhase, ObjHeader, GC_FREE, GC_MARKED, GC_OLD, HEADER};

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

        let child_is_young = self.pool.contains_ptr(child as *const u8);

        // Fast path ②: if this is neither an old→young store nor a store during
        // an active major-mark phase, there is nothing to record.
        if !child_is_young && !self.major_mark_active.load(Ordering::Relaxed) {
            return;
        }

        let Ok(mut heap) = self.heap.lock() else { return };
        let parent_addr = parent as usize;
        let parent_marked = is_marked_object(&heap, parent_addr);

        // Slow path ①: old→young pointer — record it for minor GC.
        if child_is_young {
            if heap.remembered_set.last().copied() != Some(parent_addr) {
                heap.remembered_set.push(parent_addr);
                heap.card_table.mark_dirty(parent_addr);
                self.telemetry.record_write_barrier_slow();
            }
        }

        // Slow path ②: incremental-update barrier for active major marking.
        // If a marked old/large parent stores a pointer to an unmarked old/large
        // child while the major mark phase is active, mark and enqueue the child
        // so the next slice traces it before sweeping begins.
        if heap.major_phase == MajorPhase::Mark && parent_marked {
            if trace_major_mark_barrier_child(&self.pool, &mut heap, child as usize) {
                self.telemetry.record_major_mark_barrier_trace();
            }
        }
    }
}

fn is_marked_object(heap: &HeapState, addr: usize) -> bool {
    if heap.old.contains_ptr(addr as *const u8) {
        let hdr = unsafe { std::ptr::read((addr - HEADER) as *const ObjHeader) };
        return hdr.gc_flags & GC_MARKED != 0;
    }
    if let Some(bytes) = heap.large_objects.get(&addr) {
        let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
        return hdr.gc_flags & GC_MARKED != 0;
    }
    false
}

fn trace_major_mark_barrier_child(
    pool: &super::heap::YoungPool,
    heap: &mut HeapState,
    child_addr: usize,
) -> bool {
    let child = child_addr as *mut u8;
    if child.is_null() || pool.contains_ptr(child as *const u8) {
        return false;
    }

    if heap.old.contains_ptr(child as *const u8) {
        let hdr_ptr = (child_addr - HEADER) as *mut ObjHeader;
        let type_id;
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            if hdr.gc_flags & (GC_FREE | GC_MARKED) != 0 {
                return false;
            }
            type_id = hdr.type_id;
            hdr.gc_flags |= GC_MARKED;
            std::ptr::write(hdr_ptr, hdr);
        }
        if heap.type_has_pointers(type_id) {
            heap.mark_stack.push(child_addr);
        }
        return true;
    }

    if let Some(bytes) = heap.large_objects.get_mut(&child_addr) {
        let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
        let type_id;
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            if hdr.gc_flags & GC_MARKED != 0 {
                return false;
            }
            type_id = hdr.type_id;
            hdr.gc_flags |= GC_MARKED;
            std::ptr::write(hdr_ptr, hdr);
        }
        if heap.type_has_pointers(type_id) {
            heap.mark_stack.push(child_addr);
        }
        return true;
    }

    false
}
