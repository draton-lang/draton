use std::env;
use std::time::Instant;

use draton_runtime::gc;

fn main() {
    let args: Vec<String> = env::args().collect();
    let scenario = args.get(1).map(String::as_str).unwrap_or("young-burst");
    let iterations = args
        .get(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);

    gc::shutdown();
    gc::init();
    gc::reset_stats();

    let started = Instant::now();
    let status = match scenario {
        "young-burst" => run_young_burst(iterations),
        "promotion-chain" => run_promotion_chain(iterations),
        "barrier-churn" => run_barrier_churn(iterations),
        "old-reuse-churn" => run_old_reuse_churn(iterations),
        "large-object-burst" => run_large_object_burst(iterations),
        other => {
            eprintln!("unknown scenario: {other}");
            std::process::exit(2);
        }
    };
    let elapsed_ns = started.elapsed().as_nanos() as u64;
    let stats = gc::stats();

    println!(
        concat!(
            "{{",
            "\"scenario\":\"{}\",",
            "\"iterations\":{},",
            "\"elapsed_ns\":{},",
            "\"status\":\"{}\",",
            "\"stats\":{{",
            "\"minor_cycles\":{},",
            "\"major_cycles\":{},",
            "\"major_slices\":{},",
            "\"full_cycles\":{},",
            "\"young_allocations\":{},",
            "\"old_allocations\":{},",
            "\"large_allocations\":{},",
            "\"array_allocations\":{},",
            "\"bytes_allocated\":{},",
            "\"bytes_promoted\":{},",
            "\"bytes_reclaimed_minor\":{},",
            "\"bytes_reclaimed_major\":{},",
            "\"bytes_reclaimed_large\":{},",
            "\"write_barrier_slow_calls\":{},",
            "\"major_work_requests\":{},",
            "\"major_work_threshold_requests\":{},",
            "\"major_work_continuation_requests\":{},",
            "\"major_mutator_assists\":{},",
            "\"major_work_requested\":{},",
            "\"safepoint_rearms\":{},",
            "\"major_mark_barrier_traces\":{},",
            "\"remembered_set_entries_added\":{},",
            "\"remembered_set_entries_deduped\":{},",
            "\"young_usage_bytes\":{},",
            "\"old_usage_bytes\":{},",
            "\"heap_usage_bytes\":{},",
            "\"large_object_count\":{},",
            "\"roots_count\":{},",
            "\"remembered_set_len\":{},",
            "\"old_free_slot_count\":{},",
            "\"old_free_bytes\":{},",
            "\"old_largest_free_slot\":{},",
            "\"current_mark_stack_len\":{},",
            "\"current_mark_slice_size\":{},",
            "\"major_phase\":{},",
            "\"old_sweep_cursor\":{},",
            "\"large_sweep_pending\":{},",
            "\"minor_pause_total_ns\":{},",
            "\"minor_pause_max_ns\":{},",
            "\"major_pause_total_ns\":{},",
            "\"major_pause_max_ns\":{},",
            "\"full_pause_total_ns\":{},",
            "\"full_pause_max_ns\":{}",
            "}}",
            "}}"
        ),
        scenario,
        iterations,
        elapsed_ns,
        status,
        stats.minor_cycles,
        stats.major_cycles,
        stats.major_slices,
        stats.full_cycles,
        stats.young_allocations,
        stats.old_allocations,
        stats.large_allocations,
        stats.array_allocations,
        stats.bytes_allocated,
        stats.bytes_promoted,
        stats.bytes_reclaimed_minor,
        stats.bytes_reclaimed_major,
        stats.bytes_reclaimed_large,
        stats.write_barrier_slow_calls,
        stats.major_work_requests,
        stats.major_work_threshold_requests,
        stats.major_work_continuation_requests,
        stats.major_mutator_assists,
        if stats.major_work_requested {
            "true"
        } else {
            "false"
        },
        stats.safepoint_rearms,
        stats.major_mark_barrier_traces,
        stats.remembered_set_entries_added,
        stats.remembered_set_entries_deduped,
        stats.young_usage_bytes,
        stats.old_usage_bytes,
        stats.heap_usage_bytes,
        stats.large_object_count,
        stats.roots_count,
        stats.remembered_set_len,
        stats.old_free_slot_count,
        stats.old_free_bytes,
        stats.old_largest_free_slot,
        stats.current_mark_stack_len,
        stats.current_mark_slice_size,
        stats.major_phase,
        stats.old_sweep_cursor,
        stats.large_sweep_pending,
        stats.minor_pause.total_ns,
        stats.minor_pause.max_ns,
        stats.major_pause.total_ns,
        stats.major_pause.max_ns,
        stats.full_pause.total_ns,
        stats.full_pause.max_ns,
    );
}

fn run_young_burst(iterations: usize) -> &'static str {
    let mut roots = Vec::new();
    for index in 0..iterations {
        let ptr = gc::alloc(64, 1);
        if index % 256 == 0 {
            gc::protect(ptr);
            roots.push(ptr);
        }
    }
    gc::collect();
    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
    "ok"
}

fn run_promotion_chain(iterations: usize) -> &'static str {
    let mut roots = Vec::new();
    for _ in 0..iterations {
        let ptr = gc::alloc(48, 2);
        gc::protect(ptr);
        roots.push(ptr);
    }
    gc::collect();
    gc::collect();
    gc::collect();
    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
    "ok"
}

fn run_barrier_churn(iterations: usize) -> &'static str {
    let parent = gc::alloc(gc::LARGE_OBJECT_THRESHOLD + 256, 3);
    gc::protect(parent);
    let mut retained_children = Vec::new();
    for index in 0..iterations {
        let child = gc::alloc(32, 4);
        gc::write_barrier(parent, std::ptr::null_mut(), child);
        if index % 128 == 0 {
            gc::protect(child);
            retained_children.push(child);
        }
    }
    gc::collect();
    for ptr in retained_children {
        gc::release(ptr);
    }
    gc::release(parent);
    gc::collect();
    "ok"
}

fn run_large_object_burst(iterations: usize) -> &'static str {
    let mut roots = Vec::new();
    let size = gc::LARGE_OBJECT_THRESHOLD + 8 * 1024;
    for index in 0..iterations.max(16) {
        let ptr = gc::alloc(size, 5);
        if index % 8 == 0 {
            gc::protect(ptr);
            roots.push(ptr);
        }
    }
    gc::collect();
    for ptr in roots {
        gc::release(ptr);
    }
    gc::collect();
    "ok"
}

fn run_old_reuse_churn(iterations: usize) -> &'static str {
    let count = iterations.max(512);
    let mut first_batch = Vec::new();
    for _ in 0..count {
        let ptr = gc::alloc(48, 6);
        gc::protect(ptr);
        first_batch.push(ptr);
    }
    gc::collect();
    gc::collect();
    for ptr in first_batch {
        gc::release(ptr);
    }
    gc::collect();

    let mut second_batch = Vec::new();
    for _ in 0..count {
        let ptr = gc::alloc(48, 6);
        gc::protect(ptr);
        second_batch.push(ptr);
    }
    gc::collect();
    gc::collect();
    for ptr in second_batch {
        gc::release(ptr);
    }
    gc::collect();
    "ok"
}
