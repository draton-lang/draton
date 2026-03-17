use std::mem::size_of;
use std::time::Instant;

use super::heap::{GcRuntime, HeapState, ObjHeader, YoungArena,
                  GC_MARKED, GC_OLD, GC_PINNED, HEADER};

// ── Tracing helpers ───────────────────────────────────────────────────────────

/// Push all GC-managed pointer children of `payload` onto the mark stack.
fn enqueue_children(
    young: &YoungArena,
    heap:  &HeapState,
    payload: usize,
    mark_stack: &mut Vec<usize>,
) {
    let ptr = payload as *mut u8;
    let Some(hdr) = heap.header_of(young, ptr) else { return };
    let Some(desc) = heap.type_descriptors.get(&hdr.type_id) else { return };

    for &offset in desc.pointer_offsets.iter() {
        let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
        if child.is_null() { continue; }
        let child_addr = child as usize;
        if let Some(child_hdr) = heap.header_of(young, child) {
            if child_hdr.gc_flags & GC_MARKED == 0 {
                mark_stack.push(child_addr);
            }
        }
    }
}

/// Set GC_MARKED on an object regardless of which space it lives in.
fn set_marked(young: &YoungArena, heap: &mut HeapState, payload: usize) {
    let ptr = payload as *mut u8;
    if young.contains_ptr(ptr as *const u8) {
        let mut hdr = unsafe { young.read_header(ptr) };
        hdr.gc_flags |= GC_MARKED;
        unsafe { young.write_header(ptr, hdr); }
    } else if let Some(bytes) = heap.old_objects.get_mut(&payload)
        .or_else(|| heap.large_objects.get_mut(&payload))
    {
        let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            hdr.gc_flags |= GC_MARKED;
            std::ptr::write(hdr_ptr, hdr);
        }
    }
}

// ── GcRuntime impl ────────────────────────────────────────────────────────────

impl GcRuntime {
    // ── Minor GC ─────────────────────────────────────────────────────────────

    /// Collect unreachable young-generation objects.
    ///
    /// Algorithm (promote-all-survivors):
    /// 1. Seed mark stack from roots in young gen (shadow stack + explicit roots).
    /// 2. Trace remembered-set members (old→young cross-gen pointers).
    /// 3. Transitive closure — mark all reachable young objects.
    /// 4. Linear arena scan: promote ALL marked/pinned objects to old gen.
    /// 5. Fix up pointer fields in old-gen objects and shadow-stack slots so no
    ///    dangling references to old young-arena addresses remain.
    /// 6. Reset young arena (live_count → 0 after promoting every survivor).
    pub fn collect_minor(&self) {
        let Ok(mut heap) = self.heap.lock() else { return };
        heap.minor_cycles = heap.minor_cycles.saturating_add(1);

        // Dedup remembered set before processing (Vec may contain duplicates
        // from rapid write-barrier traffic).
        heap.remembered_set.sort_unstable();
        heap.remembered_set.dedup();

        let shadow_roots = unsafe { super::shadow_stack_roots() };
        let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();

        // Phase 1: seed from roots in the young arena.
        let mut mark_stack: Vec<usize> = Vec::new();
        for &addr in shadow_roots.iter().chain(explicit_roots.iter()) {
            if self.young.contains_ptr(addr as *const u8) {
                set_marked(&self.young, &mut heap, addr);
                enqueue_children(&self.young, &heap, addr, &mut mark_stack);
            }
        }

        // Phase 2: trace pointer fields of remembered-set members.
        let rs_snapshot: Vec<usize> = heap.remembered_set.drain(..).collect();
        for old_addr in &rs_snapshot {
            let ptr = *old_addr as *mut u8;
            let Some(hdr) = heap.header_of(&self.young, ptr) else { continue };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else { continue };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() { continue; }
                if self.young.contains_ptr(child as *const u8) {
                    if heap.header_of(&self.young, child)
                        .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                    {
                        set_marked(&self.young, &mut heap, child as usize);
                        mark_stack.push(child as usize);
                    }
                }
            }
        }

        // Phase 3: transitive closure over young objects.
        while let Some(addr) = mark_stack.pop() {
            let ptr = addr as *mut u8;
            let Some(hdr) = heap.header_of(&self.young, ptr) else { continue };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else { continue };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() { continue; }
                if self.young.contains_ptr(child as *const u8) {
                    if heap.header_of(&self.young, child)
                        .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                    {
                        set_marked(&self.young, &mut heap, child as usize);
                        mark_stack.push(child as usize);
                    }
                }
            }
        }

        // Phase 4: linear arena scan — promote ALL marked/pinned objects.
        // Dead objects cost nothing (bump-pointer arena, no free list).
        let bump = self.young.bump.load(std::sync::atomic::Ordering::Acquire);
        let mut dead_bytes = 0usize;
        let mut dead_count = 0usize;
        let mut promoted: Vec<(usize, usize)> = Vec::new();
        let mut offset = 0usize;

        while offset < bump {
            let hdr_ptr = self.young.buffer.as_ptr().wrapping_add(offset) as *const ObjHeader;
            let hdr = unsafe { std::ptr::read(hdr_ptr) };
            let size = hdr.size as usize;
            let aligned = (HEADER + size + 7) & !7;
            let payload_addr = self.young.buffer.as_ptr().wrapping_add(offset + HEADER) as usize;

            if hdr.gc_flags & GC_MARKED != 0 || hdr.gc_flags & GC_PINNED != 0 {
                promoted.push((payload_addr, size));
            } else {
                dead_bytes += aligned;
                dead_count += 1;
            }
            offset += aligned;
        }

        heap.live_bytes = heap.live_bytes.saturating_sub(dead_bytes);
        self.young.live_count.fetch_sub(dead_count, std::sync::atomic::Ordering::Relaxed);

        // Phase 5: copy survivors to old gen and record forwarding.
        for (addr, size) in promoted {
            let ptr = addr as *mut u8;
            let hdr = unsafe { self.young.read_header(ptr) };
            let total = HEADER + size;
            let aligned = (HEADER + size + 7) & !7;
            let mut bytes = vec![0u8; total].into_boxed_slice();
            let new_hdr = ObjHeader {
                gc_flags: GC_OLD | (hdr.gc_flags & GC_PINNED),
                age: hdr.age.saturating_add(1),
                ..hdr
            };
            unsafe {
                std::ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), new_hdr);
                std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr().add(HEADER), size);
            }
            let new_payload = unsafe { bytes.as_mut_ptr().add(HEADER) } as usize;

            if let Some(rc) = heap.roots.remove(&addr) {
                heap.roots.insert(new_payload, rc);
            }
            heap.old_objects.insert(new_payload, bytes);
            heap.old_bytes += total;
            heap.young_forwarding.insert(addr, new_payload);
            heap.live_bytes = heap.live_bytes.saturating_sub(aligned);
            heap.live_bytes += total;
            self.young.live_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Phase 6: fix up stale pointers so generated code sees new addresses.
        //
        // 6a. Update shadow-stack alloca slots.
        unsafe { fix_shadow_stack_slots(&heap); }
        //
        // 6b. Update pointer fields inside remembered-set parent objects.
        //     (Restores correct field values; rs_snapshot is the pre-drain list.)
        fix_old_gen_fields(&mut heap, &rs_snapshot);

        // Phase 7: all survivors are in old gen; reset the young arena.
        self.young.try_reset();
        // The remembered set is empty after promote-all: there are no young
        // objects, so no old→young cross-gen pointers can exist.
        // (rs_snapshot was already drained from heap.remembered_set above.)
    }

    // ── Major GC (incremental, stop-the-world per slice) ─────────────────────

    pub fn collect_major_slice(&self) {
        let Ok(mut heap) = self.heap.lock() else { return };

        if !heap.is_marking {
            heap.major_cycles = heap.major_cycles.saturating_add(1);
            heap.is_marking = true;
            heap.mark_stack.clear();

            let shadow_roots = unsafe { super::shadow_stack_roots() };
            let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();
            for addr in shadow_roots.into_iter().chain(explicit_roots.into_iter()) {
                if heap.header_of(&self.young, addr as *mut u8).is_some() {
                    set_marked(&self.young, &mut heap, addr);
                    heap.mark_stack.push(addr);
                }
            }
        }

        let t0 = Instant::now();
        let slice = heap.mark_slice_size;

        for _ in 0..slice {
            let Some(addr) = heap.mark_stack.pop() else { break };
            let mut children: Vec<usize> = Vec::new();
            enqueue_children(&self.young, &heap, addr, &mut children);
            for child_addr in children {
                if heap.header_of(&self.young, child_addr as *mut u8)
                    .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                {
                    set_marked(&self.young, &mut heap, child_addr);
                    heap.mark_stack.push(child_addr);
                }
            }
        }

        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        let target = heap.config.pause_target_ns;
        if elapsed_ns > target {
            heap.mark_slice_size = (heap.mark_slice_size * 3 / 4).max(32);
        } else if elapsed_ns < target / 2 {
            heap.mark_slice_size = (heap.mark_slice_size * 5 / 4).min(65536);
        }

        if heap.mark_stack.is_empty() {
            heap.is_marking = false;
            Self::sweep_old(&mut heap);
            Self::sweep_large(&mut heap);
        }
    }

    fn sweep_old(heap: &mut HeapState) {
        let dead: Vec<usize> = heap.old_objects.iter()
            .filter(|(_, bytes)| {
                let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
                hdr.gc_flags & GC_MARKED == 0 && hdr.gc_flags & GC_PINNED == 0
            })
            .map(|(&addr, _)| addr)
            .collect();

        for addr in dead {
            if let Some(bytes) = heap.old_objects.remove(&addr) {
                heap.live_bytes = heap.live_bytes.saturating_sub(bytes.len());
                heap.old_bytes  = heap.old_bytes.saturating_sub(bytes.len());
            }
        }

        heap.young_forwarding.retain(|_old, new| heap.old_objects.contains_key(new));

        for (_, bytes) in heap.old_objects.iter_mut() {
            let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
            unsafe {
                let mut hdr = std::ptr::read(hdr_ptr);
                hdr.gc_flags &= !GC_MARKED;
                std::ptr::write(hdr_ptr, hdr);
            }
        }
    }

    fn sweep_large(heap: &mut HeapState) {
        let dead: Vec<usize> = heap.large_objects.iter()
            .filter(|(_, bytes)| {
                let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
                hdr.gc_flags & GC_MARKED == 0 && hdr.gc_flags & GC_PINNED == 0
            })
            .map(|(&addr, _)| addr)
            .collect();

        for addr in dead {
            if let Some(bytes) = heap.large_objects.remove(&addr) {
                heap.live_bytes = heap.live_bytes.saturating_sub(bytes.len());
                heap.old_bytes  = heap.old_bytes.saturating_sub(bytes.len());
            }
        }

        for (_, bytes) in heap.large_objects.iter_mut() {
            let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
            unsafe {
                let mut hdr = std::ptr::read(hdr_ptr);
                hdr.gc_flags &= !GC_MARKED;
                std::ptr::write(hdr_ptr, hdr);
            }
        }
    }

    // ── Full collection ───────────────────────────────────────────────────────

    pub fn collect_full(&self) {
        self.collect_minor();
        loop {
            self.collect_major_slice();
            let Ok(heap) = self.heap.lock() else { break };
            if !heap.is_marking { break; }
        }
    }
}

// ── Post-promotion pointer fixups ─────────────────────────────────────────────

/// Walk LLVM shadow-stack alloca slots and update any that point to a
/// young-arena address that has since been promoted.
///
/// # Safety
/// Must be called during a stop-the-world GC pause.
unsafe fn fix_shadow_stack_slots(heap: &HeapState) {
    use super::heap::StackEntry;
    let mut entry = crate::llvm_gc_root_chain;
    while !entry.is_null() {
        let num_roots = (*(*entry).map).num_roots as usize;
        let base = (entry as *const u8).add(size_of::<StackEntry>())
                   as *const *mut *mut u8;
        for i in 0..num_roots {
            let alloca = *base.add(i);
            if alloca.is_null() { continue; }
            let val = *alloca;
            if val.is_null() { continue; }
            if let Some(&new_addr) = heap.young_forwarding.get(&(val as usize)) {
                *alloca = new_addr as *mut u8;
            }
        }
        entry = (*entry).next;
    }
}

/// For each old-gen parent in `rs_snapshot`, update any pointer fields that
/// still contain old young-arena addresses (now stale) to the promoted address.
///
/// This corrects the view that generated code has when it reads a pointer field
/// from an old-gen object: it gets the current (old-gen) address, not the
/// stale young-arena address.
fn fix_old_gen_fields(heap: &mut HeapState, rs_snapshot: &[usize]) {
    if heap.young_forwarding.is_empty() { return; }

    for &parent_addr in rs_snapshot {
        // Get payload pointer inside the Box<[u8]>.
        let Some(bytes) = heap.old_objects.get_mut(&parent_addr) else { continue };
        let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
        let type_id = hdr.type_id;
        let payload_ptr = unsafe { bytes.as_mut_ptr().add(HEADER) };

        let Some(desc) = heap.type_descriptors.get(&type_id).cloned() else { continue };
        for &offset in desc.pointer_offsets.iter() {
            let field = unsafe { payload_ptr.add(offset as usize) as *mut *mut u8 };
            let child = unsafe { std::ptr::read(field) };
            if child.is_null() { continue; }
            if let Some(&new_addr) = heap.young_forwarding.get(&(child as usize)) {
                unsafe { std::ptr::write(field, new_addr as *mut u8); }
            }
        }
    }
}
