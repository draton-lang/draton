use std::time::Instant;

use super::heap::{GcRuntime, HeapState, ObjHeader, GC_MARKED, GC_OLD, GC_PINNED, HEADER};

// ── Tracing helpers ───────────────────────────────────────────────────────────

/// Push all GC-managed pointer children of `payload` onto the mark stack.
fn enqueue_children(heap: &HeapState, payload: usize, mark_stack: &mut Vec<usize>) {
    let ptr = payload as *mut u8;
    let Some(hdr) = heap.header_of(ptr) else { return };
    let Some(desc) = heap.type_descriptors.get(&hdr.type_id) else { return };

    for &offset in desc.pointer_offsets.iter() {
        let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
        if child.is_null() {
            continue;
        }
        let child_addr = child as usize;
        // Only enqueue objects we know about and haven't yet marked.
        if let Some(child_hdr) = heap.header_of(child) {
            if child_hdr.gc_flags & GC_MARKED == 0 {
                mark_stack.push(child_addr);
            }
        }
    }
}

/// Set GC_MARKED on an object regardless of which space it lives in.
fn set_marked(heap: &mut HeapState, payload: usize) {
    let ptr = payload as *mut u8;
    if heap.young_index.contains_key(&payload) {
        let mut hdr = unsafe { heap.young.read_header(ptr) };
        hdr.gc_flags |= GC_MARKED;
        unsafe { heap.young.write_header(ptr, hdr); }
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

/// Clear GC_MARKED on an object.
#[allow(dead_code)]
fn clear_marked(heap: &mut HeapState, payload: usize) {
    let ptr = payload as *mut u8;
    if heap.young_index.contains_key(&payload) {
        let mut hdr = unsafe { heap.young.read_header(ptr) };
        hdr.gc_flags &= !GC_MARKED;
        unsafe { heap.young.write_header(ptr, hdr); }
    } else if let Some(bytes) = heap.old_objects.get_mut(&payload)
        .or_else(|| heap.large_objects.get_mut(&payload))
    {
        let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            hdr.gc_flags &= !GC_MARKED;
            std::ptr::write(hdr_ptr, hdr);
        }
    }
}

// ── GcRuntime impl ────────────────────────────────────────────────────────────

impl GcRuntime {
    // ── Minor GC ─────────────────────────────────────────────────────────────

    /// Collect unreachable young-generation objects.
    ///
    /// Algorithm:
    /// 1. Seed mark stack from roots that are in the young gen.
    /// 2. Trace through remembered_set (old objects pointing into young gen).
    /// 3. Mark all reachable young objects.
    /// 4. Sweep: remove unreachable young objects from the index.
    /// 5. If young_index is empty after sweep: reset the bump pointer.
    pub fn collect_minor(&self) {
        let Ok(mut heap) = self.heap.lock() else { return };
        heap.minor_cycles = heap.minor_cycles.saturating_add(1);

        // Phase 1: seed from shadow-stack roots + explicit protect()-ed roots in young gen.
        // Shadow stack roots are walked while the heap lock is released momentarily;
        // we collect them before locking to avoid deadlock.
        let shadow_roots = unsafe { super::shadow_stack_roots() };
        let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();

        let mut mark_stack: Vec<usize> = Vec::new();
        for &addr in shadow_roots.iter().chain(explicit_roots.iter()) {
            if heap.young_index.contains_key(&addr) {
                set_marked(&mut heap, addr);
                enqueue_children(&heap, addr, &mut mark_stack);
            }
        }

        // Phase 2: trace pointer fields of remembered-set members.
        let rs_members: Vec<usize> = heap.remembered_set.iter().copied().collect();
        for old_addr in rs_members {
            let ptr = old_addr as *mut u8;
            let Some(hdr) = heap.header_of(ptr) else { continue };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else {
                continue
            };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() { continue; }
                let child_addr = child as usize;
                if heap.young_index.contains_key(&child_addr) {
                    if heap.header_of(child).map_or(false, |h| h.gc_flags & GC_MARKED == 0) {
                        set_marked(&mut heap, child_addr);
                        mark_stack.push(child_addr);
                    }
                }
            }
        }

        // Phase 3: transitive closure over young objects.
        while let Some(addr) = mark_stack.pop() {
            // Only follow pointers into young gen during minor GC.
            let ptr = addr as *mut u8;
            let Some(hdr) = heap.header_of(ptr) else { continue };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else {
                continue
            };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() { continue; }
                let child_addr = child as usize;
                if heap.young_index.contains_key(&child_addr) {
                    if heap.header_of(child).map_or(false, |h| h.gc_flags & GC_MARKED == 0) {
                        set_marked(&mut heap, child_addr);
                        mark_stack.push(child_addr);
                    }
                }
            }
        }

        // Phase 4: age live young objects; sweep dead ones.
        let live_young: Vec<usize> = heap.young_index.keys().copied().collect();
        let promotion_age = heap.config.promotion_age;
        let mut dead: Vec<usize> = Vec::new();
        let mut promoted: Vec<usize> = Vec::new();

        for addr in live_young {
            let ptr = addr as *mut u8;
            let mut hdr = unsafe { heap.young.read_header(ptr) };
            if hdr.gc_flags & GC_MARKED != 0 || hdr.gc_flags & GC_PINNED != 0 {
                // Alive: clear mark bit, increment age.
                hdr.gc_flags &= !GC_MARKED;
                hdr.age = hdr.age.saturating_add(1);
                if hdr.age >= promotion_age {
                    promoted.push(addr);
                } else {
                    unsafe { heap.young.write_header(ptr, hdr); }
                }
            } else {
                dead.push(addr);
            }
        }

        // Remove dead young objects.
        for addr in dead {
            if let Some((_, aligned)) = heap.young_index.remove(&addr) {
                heap.live_bytes = heap.live_bytes.saturating_sub(aligned);
                heap.young.live_count = heap.young.live_count.saturating_sub(1);
            }
            // Do not auto-remove from roots — roots only contains explicit protect() entries.
        }

        // Promote long-lived young objects to old gen in-place.
        // We allocate a fresh Box, copy the payload, then update the root entry.
        // Young objects with no registered type descriptor are promoted opaquely.
        for addr in promoted {
            let ptr = addr as *mut u8;
            let hdr = unsafe { heap.young.read_header(ptr) };
            let size = hdr.size as usize;
            let total = HEADER + size;
            let mut bytes = vec![0u8; total].into_boxed_slice();
            let new_hdr = ObjHeader { gc_flags: GC_OLD, age: hdr.age, ..hdr };
            unsafe {
                std::ptr::write(bytes.as_mut_ptr().cast::<ObjHeader>(), new_hdr);
                std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr().add(HEADER), size);
            }
            let new_payload = unsafe { bytes.as_mut_ptr().add(HEADER) } as usize;

            // Update roots and remembered set to use the new address.
            if let Some(rc) = heap.roots.remove(&addr) {
                heap.roots.insert(new_payload, rc);
            }
            if heap.remembered_set.remove(&addr) {
                heap.remembered_set.insert(new_payload);
            }
            heap.old_objects.insert(new_payload, bytes);
            heap.old_bytes += total;
            // Record forwarding so callers holding the old pointer can still
            // resolve the object via header_of / space_of / release.
            heap.young_forwarding.insert(addr, new_payload);

            // Remove from young index and decrement young live count.
            if let Some((_, aligned)) = heap.young_index.remove(&addr) {
                // live_bytes stays the same (same data, now in old gen; account for
                // the slight size difference due to alignment rounding — use total).
                heap.live_bytes = heap.live_bytes.saturating_sub(aligned);
                heap.live_bytes += total;
                heap.young.live_count = heap.young.live_count.saturating_sub(1);
            }
        }

        // Phase 5: reset young arena if completely empty.
        heap.try_reset_young();
    }

    // ── Major GC (incremental, stop-the-world per slice) ─────────────────────

    /// Begin or continue an incremental major GC cycle.
    ///
    /// Processes up to `mark_slice_size` objects per call and adapts the slice
    /// size based on measured elapsed time vs `pause_target_ns`.
    pub fn collect_major_slice(&self) {
        let Ok(mut heap) = self.heap.lock() else { return };

        // Start a new cycle if we are not already marking.
        if !heap.is_marking {
            heap.major_cycles = heap.major_cycles.saturating_add(1);
            heap.is_marking = true;
            heap.mark_stack.clear();

            // Seed mark stack from shadow-stack roots + explicit protect()-ed roots.
            let shadow_roots = unsafe { super::shadow_stack_roots() };
            let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();
            for addr in shadow_roots.into_iter().chain(explicit_roots.into_iter()) {
                if heap.header_of(addr as *mut u8).is_some() {
                    set_marked(&mut heap, addr);
                    heap.mark_stack.push(addr);
                }
            }
        }

        let t0 = Instant::now();
        let slice = heap.mark_slice_size;

        // Process up to `slice` items from the mark stack.
        for _ in 0..slice {
            let Some(addr) = heap.mark_stack.pop() else { break };
            let mut children: Vec<usize> = Vec::new();
            enqueue_children(&heap, addr, &mut children);
            for child_addr in children {
                if heap.header_of(child_addr as *mut u8)
                    .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                {
                    set_marked(&mut heap, child_addr);
                    heap.mark_stack.push(child_addr);
                }
            }
        }

        // Adapt slice size to stay within pause_target_ns.
        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        let target = heap.config.pause_target_ns;
        if elapsed_ns > target {
            heap.mark_slice_size = (heap.mark_slice_size * 3 / 4).max(32);
        } else if elapsed_ns < target / 2 {
            heap.mark_slice_size = (heap.mark_slice_size * 5 / 4).min(65536);
        }

        // If marking is complete, sweep dead objects.
        if heap.mark_stack.is_empty() {
            heap.is_marking = false;
            Self::sweep_old(&mut heap);
            Self::sweep_large(&mut heap);
        }
    }

    fn sweep_old(heap: &mut HeapState) {
        let dead: Vec<usize> = heap
            .old_objects
            .iter()
            .filter(|(&_addr, bytes)| {
                let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
                hdr.gc_flags & GC_MARKED == 0 && hdr.gc_flags & GC_PINNED == 0
            })
            .map(|(&addr, _)| addr)
            .collect();

        for addr in dead {
            if let Some(bytes) = heap.old_objects.remove(&addr) {
                heap.live_bytes = heap.live_bytes.saturating_sub(bytes.len());
                heap.old_bytes  = heap.old_bytes.saturating_sub(bytes.len());
                heap.remembered_set.remove(&addr);
                // roots contains only explicit protect()-ed entries; don't auto-remove.
            }
        }

        // Remove forwarding entries whose destination was just collected.
        heap.young_forwarding.retain(|_old, new| heap.old_objects.contains_key(new));

        // Clear mark bits on survivors.
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
        let dead: Vec<usize> = heap
            .large_objects
            .iter()
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
                // roots contains only explicit protect()-ed entries; don't auto-remove.
            }
        }

        // Clear mark bits on large survivors.
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
        // Run incremental slices until the full major cycle completes.
        loop {
            self.collect_major_slice();
            let Ok(heap) = self.heap.lock() else { break };
            if !heap.is_marking { break; }
        }
    }
}
