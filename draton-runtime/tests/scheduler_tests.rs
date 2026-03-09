use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use draton_runtime::scheduler::{current_worker_id, Scheduler};

#[test]
fn spawn_many_coreroutines_all_complete() {
    let scheduler = Scheduler::new(4);
    let completed = Arc::new(AtomicUsize::new(0));
    for _ in 0..128 {
        let completed = Arc::clone(&completed);
        let _ = scheduler.spawn(move || {
            completed.fetch_add(1, Ordering::SeqCst);
        });
    }
    assert!(scheduler.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(completed.load(Ordering::SeqCst), 128);
    scheduler.shutdown();
}

#[test]
fn work_stealing_spreads_tasks_across_workers() {
    let scheduler = Scheduler::new(4);
    let seen = Arc::new((0..4).map(|_| AtomicBool::new(false)).collect::<Vec<_>>());
    for _ in 0..64 {
        let seen = Arc::clone(&seen);
        let _ = scheduler.spawn(move || {
            if let Some(worker) = current_worker_id() {
                seen[worker].store(true, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(2));
        });
    }
    assert!(scheduler.wait_for_idle(Duration::from_secs(2)));
    let active_workers = seen
        .iter()
        .filter(|flag| flag.load(Ordering::SeqCst))
        .count();
    assert!(active_workers >= 2);
    scheduler.shutdown();
}

#[test]
fn preemption_like_progress_allows_other_work_to_run() {
    let scheduler = Scheduler::new(2);
    let quick = Arc::new(AtomicBool::new(false));
    let slow_done = Arc::new(AtomicBool::new(false));
    {
        let slow_done = Arc::clone(&slow_done);
        let _ = scheduler.spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            slow_done.store(true, Ordering::SeqCst);
        });
    }
    {
        let quick = Arc::clone(&quick);
        let _ = scheduler.spawn(move || {
            quick.store(true, Ordering::SeqCst);
        });
    }
    assert!(scheduler.wait_for_idle(Duration::from_secs(2)));
    assert!(quick.load(Ordering::SeqCst));
    assert!(slow_done.load(Ordering::SeqCst));
    scheduler.shutdown();
}

#[test]
fn context_switch_preserves_values_across_yield() {
    let scheduler = Scheduler::new(1);
    let value = Arc::new(AtomicUsize::new(0));
    {
        let value = Arc::clone(&value);
        let _ = scheduler.spawn(move || {
            let local = 41usize;
            std::thread::yield_now();
            value.store(local + 1, Ordering::SeqCst);
        });
    }
    assert!(scheduler.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(value.load(Ordering::SeqCst), 42);
    scheduler.shutdown();
}

#[test]
fn deadlock_detection_reports_cycle() {
    let scheduler = Scheduler::new(1);
    scheduler.record_wait(1, 2);
    scheduler.record_wait(2, 3);
    scheduler.record_wait(3, 1);
    assert!(scheduler.detect_deadlock());
    scheduler.shutdown();
}
