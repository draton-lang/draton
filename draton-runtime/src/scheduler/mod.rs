//! Coreroutine scheduler, channels and async runtime surface.

pub mod channel;
pub mod coreroutine;
pub mod io;
#[allow(clippy::module_inception)]
pub mod scheduler;

use std::sync::{Arc, Mutex, OnceLock};

pub use channel::{Chan, RawChan};
pub use coreroutine::{CoreState, Coreroutine, WaitToken};
pub use scheduler::{current_worker_id, Scheduler};

static GLOBAL_SCHEDULER: OnceLock<Mutex<Option<Arc<Scheduler>>>> = OnceLock::new();

fn global_slot() -> &'static Mutex<Option<Arc<Scheduler>>> {
    GLOBAL_SCHEDULER.get_or_init(|| Mutex::new(None))
}

/// Initializes the global scheduler if needed.
pub fn init_global(n_threads: usize) -> Arc<Scheduler> {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(existing) = guard.as_ref() {
        return Arc::clone(existing);
    }
    let scheduler = Scheduler::new(n_threads.max(1));
    *guard = Some(Arc::clone(&scheduler));
    scheduler
}

/// Returns the global scheduler if initialized.
pub fn global() -> Option<Arc<Scheduler>> {
    let slot = global_slot();
    let guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.as_ref().map(Arc::clone)
}

/// Shuts down the global scheduler.
pub fn shutdown_global() {
    let slot = global_slot();
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(runtime) = guard.take() {
        runtime.shutdown();
    }
}

/// Spawns a raw extern function onto the global scheduler.
pub fn spawn_raw(fn_ptr: extern "C" fn(*mut libc::c_void), arg: *mut libc::c_void) -> u64 {
    init_global(1).spawn_raw(fn_ptr, arg)
}

/// Cooperatively yields the current worker thread.
pub fn yield_now() {
    std::thread::yield_now();
}
