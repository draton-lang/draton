use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::heap::{
    FreeSlot, GcRuntime, HeapState, MajorPhase, ObjHeader, YoungPool, GC_FREE, GC_MARKED, GC_OLD,
    GC_PINNED, HEADER, MAX_THREADS,
};
use super::{major_work_needed, request_major_work, sync_major_work_request};

// ── Tracing helpers ───────────────────────────────────────────────────────────

fn enqueue_children(
    pool: &YoungPool,
    heap: &HeapState,
    payload: usize,
    mark_stack: &mut Vec<usize>,
) {
    let ptr = payload as *mut u8;
    let Some(hdr) = heap.header_of(pool, ptr) else {
        return;
    };
    let Some(desc) = heap.type_descriptors.get(&hdr.type_id) else {
        return;
    };

    for &offset in desc.pointer_offsets.iter() {
        let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
        if child.is_null() {
            continue;
        }
        let child_addr = child as usize;
        if let Some(child_hdr) = heap.header_of(pool, child) {
            if child_hdr.gc_flags & GC_MARKED == 0 {
                mark_stack.push(child_addr);
            }
        }
    }
}

/// Set GC_MARKED on an object regardless of which space it lives in.
fn set_marked(pool: &YoungPool, heap: &mut HeapState, payload: usize) {
    let ptr = payload as *mut u8;
    if pool.contains_ptr(ptr as *const u8) {
        let mut hdr = unsafe { pool.read_header(ptr) };
        hdr.gc_flags |= GC_MARKED;
        unsafe {
            pool.write_header(ptr, hdr);
        }
    } else if heap.old.contains_ptr(ptr as *const u8) {
        let hdr_ptr = (payload - HEADER) as *mut ObjHeader;
        unsafe {
            let mut hdr = std::ptr::read(hdr_ptr);
            hdr.gc_flags |= GC_MARKED;
            std::ptr::write(hdr_ptr, hdr);
        }
    } else if let Some(bytes) = heap.large_objects.get_mut(&payload) {
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

    /// Collect unreachable young-generation objects (promote-all-survivors).
    pub fn collect_minor(&self) {
        let t0 = Instant::now();
        let Ok(mut heap) = self.heap.lock() else {
            return;
        };
        heap.minor_cycles = heap.minor_cycles.saturating_add(1);

        let remembered_before = heap.remembered_set.len();
        heap.remembered_set.sort_unstable();
        heap.remembered_set.dedup();
        let deduped = remembered_before.saturating_sub(heap.remembered_set.len());
        if deduped != 0 {
            self.telemetry.record_remembered_set_deduped(deduped);
        }

        let shadow_roots = unsafe { super::shadow_stack_roots() };
        let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();

        // Phase 1: seed from roots that live in the young pool.
        let mut mark_stack: Vec<usize> = Vec::new();
        for &addr in shadow_roots.iter().chain(explicit_roots.iter()) {
            if self.pool.contains_ptr(addr as *const u8) {
                set_marked(&self.pool, &mut heap, addr);
                enqueue_children(&self.pool, &heap, addr, &mut mark_stack);
            }
        }

        // Phase 2: trace pointer fields of remembered-set members.
        let rs_snapshot: Vec<usize> = heap.remembered_set.drain(..).collect();
        for &old_addr in &rs_snapshot {
            let ptr = old_addr as *mut u8;
            let Some(hdr) = heap.header_of(&self.pool, ptr) else {
                continue;
            };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else {
                continue;
            };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() {
                    continue;
                }
                if self.pool.contains_ptr(child as *const u8) {
                    if heap
                        .header_of(&self.pool, child)
                        .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                    {
                        set_marked(&self.pool, &mut heap, child as usize);
                        mark_stack.push(child as usize);
                    }
                }
            }
        }

        // Phase 3: transitive closure over young objects.
        while let Some(addr) = mark_stack.pop() {
            let ptr = addr as *mut u8;
            let Some(hdr) = heap.header_of(&self.pool, ptr) else {
                continue;
            };
            let Some(desc) = heap.type_descriptors.get(&hdr.type_id).cloned() else {
                continue;
            };
            for &offset in desc.pointer_offsets.iter() {
                let child = unsafe { HeapState::read_ptr_field(ptr as *const u8, offset) };
                if child.is_null() {
                    continue;
                }
                if self.pool.contains_ptr(child as *const u8) {
                    if heap
                        .header_of(&self.pool, child)
                        .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                    {
                        set_marked(&self.pool, &mut heap, child as usize);
                        mark_stack.push(child as usize);
                    }
                }
            }
        }

        // Phase 4: linear scan over all pool slots — collect survivors.
        let mut dead_bytes = 0usize;
        let mut promoted: Vec<(usize, usize)> = Vec::new(); // (payload_addr, payload_size)

        for slot_idx in 0..MAX_THREADS {
            let slot_bump = self.pool.slots[slot_idx]
                .bump
                .load(std::sync::atomic::Ordering::Acquire);
            if slot_bump == 0 {
                continue;
            }
            let slot_base = self
                .pool
                .buffer
                .as_ptr()
                .wrapping_add(slot_idx * self.pool.per_thread_size);
            let mut offset = 0usize;
            while offset < slot_bump {
                let hdr_ptr = slot_base.wrapping_add(offset) as *const ObjHeader;
                let hdr = unsafe { std::ptr::read(hdr_ptr) };
                let size = hdr.size as usize;
                let aligned = (HEADER + size + 7) & !7;
                let payload_addr = slot_base.wrapping_add(offset + HEADER) as usize;
                if hdr.gc_flags & GC_MARKED != 0 || hdr.gc_flags & GC_PINNED != 0 {
                    promoted.push((payload_addr, size));
                } else {
                    dead_bytes += aligned;
                }
                offset += aligned;
            }
        }
        heap.live_bytes = heap.live_bytes.saturating_sub(dead_bytes);

        // Phase 5: copy survivors to old gen and record forwarding.
        let mut promoted_bytes = 0usize;
        for (addr, size) in promoted {
            let ptr = addr as *mut u8;
            let hdr = unsafe { self.pool.read_header(ptr) };
            let aligned = (HEADER + size + 7) & !7;
            let new_hdr = ObjHeader {
                gc_flags: GC_OLD | (hdr.gc_flags & GC_PINNED),
                age: hdr.age.saturating_add(1),
                ..hdr
            };
            let new_payload = heap.old.alloc(size, new_hdr.gc_flags, new_hdr.type_id);
            if new_payload.is_null() {
                // OldArena full — leave in young gen; will be retried next cycle.
                continue;
            }
            // Fix up the age field (alloc wrote the initial header with age=0).
            unsafe {
                std::ptr::write(new_payload.sub(HEADER).cast::<ObjHeader>(), new_hdr);
                std::ptr::copy_nonoverlapping(ptr, new_payload, size);
            }
            let new_payload_addr = new_payload as usize;
            if let Some(rc) = heap.roots.remove(&addr) {
                heap.roots.insert(new_payload_addr, rc);
            }
            heap.old_bytes += (HEADER + size + 7) & !7;
            heap.live_bytes = heap.live_bytes.saturating_sub(aligned);
            heap.live_bytes += aligned;
            heap.young_forwarding.insert(addr, new_payload_addr);
            promoted_bytes += aligned;
        }

        // Phase 6: fix up stale pointers in shadow stack and remembered-set parents.
        unsafe {
            fix_shadow_stack_slots(&heap);
        }
        fix_old_gen_fields(&mut heap, &rs_snapshot);

        // Phase 7: reset all pool slots (all survivors are in old gen now).
        self.pool.reset_all();
        self.telemetry.record_minor_cycle(
            t0.elapsed().as_nanos() as u64,
            promoted_bytes,
            dead_bytes,
        );
        let should_request_major = major_work_needed(&heap);
        drop(heap);
        if should_request_major {
            request_major_work(self);
        } else {
            self.major_work_requested.store(false, Ordering::Release);
        }
    }

    // ── Major GC (incremental, stop-the-world per slice) ─────────────────────

    pub fn collect_major_slice(&self) {
        let Ok(mut heap) = self.heap.lock() else {
            return;
        };

        if heap.major_phase == MajorPhase::Idle {
            Self::begin_major_cycle(&self.pool, &mut heap);
            self.telemetry.record_major_cycle_start();
        }

        let t0 = Instant::now();
        let mut reclaimed_old = 0usize;
        let mut reclaimed_large = 0usize;

        match heap.major_phase {
            MajorPhase::Idle => {}
            MajorPhase::Mark => {
                Self::drain_mark_slice(&self.pool, &mut heap);
                if heap.mark_stack.is_empty() {
                    heap.major_phase = MajorPhase::SweepOld;
                    heap.old.clear_free_lists();
                    heap.old_sweep_cursor = 0;
                    heap.old_sweep_pending = None;
                }
            }
            MajorPhase::SweepOld => {
                reclaimed_old = Self::sweep_old_slice(&mut heap);
                if heap.old_sweep_cursor >= heap.old.bump {
                    Self::flush_pending_old_free(&mut heap);
                    heap.major_phase = MajorPhase::SweepLarge;
                    heap.large_sweep_pending = heap.large_objects.keys().copied().collect();
                }
            }
            MajorPhase::SweepLarge => {
                reclaimed_large = Self::sweep_large_slice(&mut heap);
                if heap.large_sweep_pending.is_empty() {
                    Self::finish_major_cycle(&mut heap);
                }
            }
        }
        self.major_mark_active
            .store(heap.major_phase == MajorPhase::Mark, Ordering::Relaxed);
        let major_still_pending = sync_major_work_request(self, &heap);

        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        let target = heap.config.pause_target_ns;
        if elapsed_ns > target {
            heap.mark_slice_size = (heap.mark_slice_size * 3 / 4).max(32);
        } else if elapsed_ns < target / 2 {
            heap.mark_slice_size = (heap.mark_slice_size * 5 / 4).min(65536);
        }
        self.telemetry
            .record_major_slice(elapsed_ns, reclaimed_old, reclaimed_large);
        drop(heap);
        if major_still_pending {
            request_major_work(self);
        }
    }

    fn begin_major_cycle(pool: &YoungPool, heap: &mut HeapState) {
        heap.major_cycles = heap.major_cycles.saturating_add(1);
        heap.major_phase = MajorPhase::Mark;
        heap.mark_stack.clear();
        heap.old_sweep_cursor = 0;
        heap.old_sweep_pending = None;
        heap.large_sweep_pending.clear();

        let shadow_roots = unsafe { super::shadow_stack_roots() };
        let explicit_roots: Vec<usize> = heap.roots.keys().copied().collect();
        for addr in shadow_roots.into_iter().chain(explicit_roots.into_iter()) {
            if heap.header_of(pool, addr as *mut u8).is_some() {
                set_marked(pool, heap, addr);
                heap.mark_stack.push(addr);
            }
        }
    }

    fn drain_mark_slice(pool: &YoungPool, heap: &mut HeapState) {
        let slice = heap.mark_slice_size;
        for _ in 0..slice {
            let Some(addr) = heap.mark_stack.pop() else {
                break;
            };
            let mut children: Vec<usize> = Vec::new();
            enqueue_children(pool, heap, addr, &mut children);
            for child_addr in children {
                if heap
                    .header_of(pool, child_addr as *mut u8)
                    .map_or(false, |h| h.gc_flags & GC_MARKED == 0)
                {
                    set_marked(pool, heap, child_addr);
                    heap.mark_stack.push(child_addr);
                }
            }
        }
    }

    /// Incremental sweep of the contiguous old-gen arena.
    fn sweep_old_slice(heap: &mut HeapState) -> usize {
        let mut reclaimed = 0usize;
        let mut swept = 0usize;
        while heap.old_sweep_cursor < heap.old.bump && swept < heap.mark_slice_size {
            let offset = heap.old_sweep_cursor;
            let hdr_ptr = heap.old.buffer.as_ptr().wrapping_add(offset) as *mut ObjHeader;
            let hdr = unsafe { std::ptr::read(hdr_ptr) };
            let size = hdr.size as usize;
            let aligned = (HEADER + size + 7) & !7;

            if hdr.gc_flags & GC_FREE != 0 {
                Self::extend_pending_old_free(heap, offset, aligned);
                heap.old_sweep_cursor += aligned;
                swept += 1;
                continue;
            }

            if hdr.gc_flags & GC_MARKED == 0 && hdr.gc_flags & GC_PINNED == 0 {
                // Dead: mark free and coalesce adjacent runs as we sweep.
                let mut dead_hdr = hdr;
                dead_hdr.gc_flags = GC_FREE | GC_OLD;
                unsafe {
                    std::ptr::write(hdr_ptr, dead_hdr);
                }
                Self::extend_pending_old_free(heap, offset, aligned);
                heap.old_bytes = heap.old_bytes.saturating_sub(aligned);
                heap.live_bytes = heap.live_bytes.saturating_sub(aligned);
                reclaimed += aligned;
            } else {
                Self::flush_pending_old_free(heap);
                if hdr.gc_flags & GC_MARKED != 0 {
                    // Live: clear the mark bit for next cycle.
                    let mut new_hdr = hdr;
                    new_hdr.gc_flags &= !GC_MARKED;
                    unsafe {
                        std::ptr::write(hdr_ptr, new_hdr);
                    }
                }
            }
            heap.old_sweep_cursor += aligned;
            swept += 1;
        }
        reclaimed
    }

    fn extend_pending_old_free(heap: &mut HeapState, offset: usize, total: usize) {
        if heap.old_sweep_pending.is_some() {
            let (run_offset, run_total) = {
                let run = heap.old_sweep_pending.as_mut().expect("pending free run");
                debug_assert_eq!(run.offset + run.total, offset);
                run.total += total;
                (run.offset, run.total)
            };
            Self::write_free_slot_header(heap, run_offset, run_total);
            return;
        }
        heap.old_sweep_pending = Some(FreeSlot { offset, total });
        Self::write_free_slot_header(heap, offset, total);
    }

    fn flush_pending_old_free(heap: &mut HeapState) {
        if let Some(run) = heap.old_sweep_pending.take() {
            Self::write_free_slot_header(heap, run.offset, run.total);
            heap.old.push_free_slot(run);
        }
    }

    fn write_free_slot_header(heap: &mut HeapState, offset: usize, total: usize) {
        let hdr = ObjHeader::new((total - HEADER) as u32, 0, GC_FREE | GC_OLD);
        unsafe {
            std::ptr::write(
                heap.old.buffer.as_mut_ptr().add(offset).cast::<ObjHeader>(),
                hdr,
            );
        }
    }

    fn sweep_large_slice(heap: &mut HeapState) -> usize {
        let mut reclaimed = 0usize;
        let budget = heap.mark_slice_size.max(1);
        let mut processed = 0usize;

        while processed < budget {
            let Some(addr) = heap.large_sweep_pending.pop() else {
                break;
            };
            processed += 1;
            let is_dead = heap
                .large_objects
                .get(&addr)
                .map(|bytes| {
                    let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
                    hdr.gc_flags & GC_MARKED == 0 && hdr.gc_flags & GC_PINNED == 0
                })
                .unwrap_or(false);

            if is_dead {
                if let Some(bytes) = heap.large_objects.remove(&addr) {
                    heap.live_bytes = heap.live_bytes.saturating_sub(bytes.len());
                    heap.old_bytes = heap.old_bytes.saturating_sub(bytes.len());
                    reclaimed += bytes.len();
                }
            } else if let Some(bytes) = heap.large_objects.get_mut(&addr) {
                let hdr_ptr = bytes.as_mut_ptr().cast::<ObjHeader>();
                unsafe {
                    let mut hdr = std::ptr::read(hdr_ptr);
                    hdr.gc_flags &= !GC_MARKED;
                    std::ptr::write(hdr_ptr, hdr);
                }
            }
        }
        reclaimed
    }

    fn finish_major_cycle(heap: &mut HeapState) {
        heap.major_phase = MajorPhase::Idle;
        heap.old_sweep_cursor = 0;
        heap.old_sweep_pending = None;
        heap.large_sweep_pending.clear();

        // Prune forwarding entries whose promoted object has since been freed.
        let to_remove: Vec<usize> = heap
            .young_forwarding
            .iter()
            .filter_map(|(&old_addr, &new_addr)| {
                // new_addr is a payload in OldArena; if now GC_FREE → stale.
                let hdr = unsafe { std::ptr::read((new_addr - HEADER) as *const ObjHeader) };
                if hdr.gc_flags & GC_FREE != 0 {
                    Some(old_addr)
                } else {
                    None
                }
            })
            .collect();
        for k in to_remove {
            heap.young_forwarding.remove(&k);
        }
    }

    fn major_cycle_active(heap: &HeapState) -> bool {
        heap.major_phase != MajorPhase::Idle
    }

    #[allow(dead_code)]
    fn sweep_old(heap: &mut HeapState) -> usize {
        let mut reclaimed = 0usize;
        while heap.major_phase != MajorPhase::Idle {
            match heap.major_phase {
                MajorPhase::SweepOld => reclaimed += Self::sweep_old_slice(heap),
                MajorPhase::SweepLarge => reclaimed += Self::sweep_large_slice(heap),
                _ => break,
            }
            if heap.major_phase == MajorPhase::SweepOld && heap.old_sweep_cursor >= heap.old.bump {
                Self::flush_pending_old_free(heap);
                heap.major_phase = MajorPhase::SweepLarge;
                heap.large_sweep_pending = heap.large_objects.keys().copied().collect();
            }
            if heap.major_phase == MajorPhase::SweepLarge && heap.large_sweep_pending.is_empty() {
                Self::finish_major_cycle(heap);
            }
        }
        reclaimed
    }

    // ── Full collection ───────────────────────────────────────────────────────

    pub fn collect_full(&self) {
        let t0 = Instant::now();
        self.collect_minor();
        loop {
            self.collect_major_slice();
            let Ok(heap) = self.heap.lock() else { break };
            if !Self::major_cycle_active(&heap) {
                break;
            }
        }
        self.telemetry
            .record_full_cycle(t0.elapsed().as_nanos() as u64);
    }
}

// ── Post-promotion pointer fixups ─────────────────────────────────────────────

/// Walk LLVM shadow-stack alloca slots and update any that point to a
/// young-pool address that has since been promoted.
///
/// # Safety
/// Must be called during a stop-the-world GC pause.
unsafe fn fix_shadow_stack_slots(heap: &HeapState) {
    use super::heap::StackEntry;
    let mut entry = crate::llvm_gc_root_chain;
    while !entry.is_null() {
        let num_roots = (*(*entry).map).num_roots as usize;
        let base = (entry as *const u8).add(size_of::<StackEntry>()) as *const *mut *mut u8;
        for i in 0..num_roots {
            let alloca = *base.add(i);
            if alloca.is_null() {
                continue;
            }
            let val = *alloca;
            if val.is_null() {
                continue;
            }
            if let Some(&new_addr) = heap.young_forwarding.get(&(val as usize)) {
                *alloca = new_addr as *mut u8;
            }
        }
        entry = (*entry).next;
    }
}

/// For each old-gen parent in `rs_snapshot`, update pointer fields that still
/// contain stale young-arena addresses to the promoted old-gen address.
fn fix_old_gen_fields(heap: &mut HeapState, rs_snapshot: &[usize]) {
    if heap.young_forwarding.is_empty() {
        return;
    }

    for &parent_addr in rs_snapshot {
        let type_id;
        if heap.old.contains_ptr(parent_addr as *const u8) {
            let hdr = unsafe { std::ptr::read((parent_addr - HEADER) as *const ObjHeader) };
            if hdr.gc_flags & GC_FREE != 0 {
                continue;
            }
            type_id = hdr.type_id;
        } else if let Some(bytes) = heap.large_objects.get(&parent_addr) {
            let hdr = unsafe { std::ptr::read(bytes.as_ptr().cast::<ObjHeader>()) };
            type_id = hdr.type_id;
        } else {
            continue;
        }

        let Some(desc) = heap.type_descriptors.get(&type_id).cloned() else {
            continue;
        };
        for &offset in desc.pointer_offsets.iter() {
            let field = (parent_addr + offset as usize) as *mut *mut u8;
            let child = unsafe { std::ptr::read(field) };
            if child.is_null() {
                continue;
            }
            if let Some(&new_addr) = heap.young_forwarding.get(&(child as usize)) {
                unsafe {
                    std::ptr::write(field, new_addr as *mut u8);
                }
            }
        }
    }
}
