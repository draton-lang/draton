use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam::deque::{Injector, Steal, Stealer, Worker};

use super::coreroutine::{
    current_worker_id as tls_worker_id, next_core_id, set_current_worker_id, RuntimeTask,
};
use super::io::IoDriver;

#[derive(Debug)]
struct WorkerSignal {
    parked: Mutex<bool>,
    cv: Condvar,
}

impl WorkerSignal {
    fn new() -> Self {
        Self {
            parked: Mutex::new(false),
            cv: Condvar::new(),
        }
    }
}

/// Runtime worker metadata.
#[derive(Debug)]
pub struct WorkerThread {
    pub id: usize,
    pub stealer: Stealer<Arc<RuntimeTask>>,
}

/// Work-stealing scheduler used by the runtime.
#[derive(Debug)]
pub struct Scheduler {
    injector: Injector<Arc<RuntimeTask>>,
    workers: Vec<WorkerThread>,
    handles: Mutex<Vec<JoinHandle<()>>>,
    signals: Vec<Arc<WorkerSignal>>,
    shutdown: AtomicBool,
    active_jobs: AtomicUsize,
    wait_graph: Mutex<HashMap<u64, Vec<u64>>>,
}

impl Scheduler {
    /// Creates and starts a scheduler with `n_threads` workers.
    pub fn new(n_threads: usize) -> Arc<Self> {
        let mut locals = Vec::new();
        let mut workers = Vec::new();
        let mut signals = Vec::new();
        for id in 0..n_threads {
            let local = Worker::new_fifo();
            workers.push(WorkerThread {
                id,
                stealer: local.stealer(),
            });
            locals.push(local);
            signals.push(Arc::new(WorkerSignal::new()));
        }
        let scheduler = Arc::new(Self {
            injector: Injector::new(),
            workers,
            handles: Mutex::new(Vec::new()),
            signals,
            shutdown: AtomicBool::new(false),
            active_jobs: AtomicUsize::new(0),
            wait_graph: Mutex::new(HashMap::new()),
        });
        for (id, local) in locals.into_iter().enumerate() {
            let runtime = Arc::clone(&scheduler);
            let signal = Arc::clone(&scheduler.signals[id]);
            let stealers = runtime
                .workers
                .iter()
                .map(|worker| worker.stealer.clone())
                .collect::<Vec<_>>();
            let handle = thread::spawn(move || runtime.run_worker(id, local, stealers, signal));
            let mut handles = match scheduler.handles.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            handles.push(handle);
        }
        scheduler
    }

    fn run_worker(
        self: Arc<Self>,
        worker_id: usize,
        local: Worker<Arc<RuntimeTask>>,
        stealers: Vec<Stealer<Arc<RuntimeTask>>>,
        signal: Arc<WorkerSignal>,
    ) {
        set_current_worker_id(worker_id);
        let mut io_driver = IoDriver::new();
        loop {
            if self.shutdown.load(Ordering::Acquire) {
                break;
            }
            if let Some(job) = Self::next_job(&local, &self.injector, &stealers) {
                job.run(worker_id);
                self.active_jobs.fetch_sub(1, Ordering::AcqRel);
                continue;
            }
            io_driver.poll_once();
            let mut parked = match signal.parked.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *parked = true;
            parked = match signal.cv.wait_timeout(parked, Duration::from_millis(10)) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
            *parked = false;
        }
        set_current_worker_id(usize::MAX);
    }

    fn next_job(
        local: &Worker<Arc<RuntimeTask>>,
        injector: &Injector<Arc<RuntimeTask>>,
        stealers: &[Stealer<Arc<RuntimeTask>>],
    ) -> Option<Arc<RuntimeTask>> {
        if let Some(job) = local.pop() {
            return Some(job);
        }
        if let Steal::Success(job) = injector.steal_batch_and_pop(local) {
            return Some(job);
        }
        for stealer in stealers {
            match stealer.steal_batch_and_pop(local) {
                Steal::Success(job) => return Some(job),
                Steal::Retry => continue,
                Steal::Empty => {}
            }
        }
        None
    }

    fn wake_all(&self) {
        for signal in &self.signals {
            signal.cv.notify_one();
        }
    }

    /// Spawns a Rust closure onto the scheduler.
    pub fn spawn<F>(&self, task: F) -> u64
    where
        F: FnOnce() + Send + 'static,
    {
        let id = next_core_id();
        let runtime_task = RuntimeTask::new(id, task);
        self.active_jobs.fetch_add(1, Ordering::AcqRel);
        self.injector.push(runtime_task);
        self.wake_all();
        id
    }

    /// Spawns an extern function pointer and argument.
    pub fn spawn_raw(
        &self,
        fn_ptr: extern "C" fn(*mut libc::c_void),
        arg: *mut libc::c_void,
    ) -> u64 {
        let arg_bits = arg as usize;
        self.spawn(move || fn_ptr(arg_bits as *mut libc::c_void))
    }

    /// Waits until no queued/running tasks remain or timeout is reached.
    pub fn wait_for_idle(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while self.active_jobs.load(Ordering::Acquire) != 0 {
            if start.elapsed() >= timeout {
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        true
    }

    /// Returns the count of currently active jobs.
    pub fn active_jobs(&self) -> usize {
        self.active_jobs.load(Ordering::Acquire)
    }

    /// Records a wait edge for debug deadlock detection.
    pub fn record_wait(&self, from: u64, to: u64) {
        let mut graph = match self.wait_graph.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        graph.entry(from).or_default().push(to);
    }

    /// Clears all outgoing waits for a coreroutine.
    pub fn clear_wait(&self, from: u64) {
        let mut graph = match self.wait_graph.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        graph.remove(&from);
    }

    /// Returns true when a cycle exists in the current wait graph.
    pub fn detect_deadlock(&self) -> bool {
        let graph = match self.wait_graph.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        for node in graph.keys().copied() {
            if has_cycle(node, node, &graph, &mut Vec::new()) {
                return true;
            }
        }
        false
    }

    /// Stops all workers and joins threads.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.wake_all();
        let mut handles = match self.handles.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn has_cycle(origin: u64, node: u64, graph: &HashMap<u64, Vec<u64>>, seen: &mut Vec<u64>) -> bool {
    if seen.contains(&node) {
        return node == origin;
    }
    seen.push(node);
    let result = graph
        .get(&node)
        .map(|edges| {
            edges
                .iter()
                .copied()
                .any(|next| has_cycle(origin, next, graph, seen))
        })
        .unwrap_or(false);
    seen.pop();
    result
}

/// Returns the current worker id if running inside a worker thread.
pub fn current_worker_id() -> Option<usize> {
    tls_worker_id()
}
