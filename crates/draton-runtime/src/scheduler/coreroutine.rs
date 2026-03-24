use std::cell::Cell;
use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Runtime state of a coreroutine.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    Ready,
    Running,
    Blocked,
    Dead,
}

/// Opaque waiting token used by channels/debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitToken(pub u64);

/// Public coreroutine descriptor shared with generated code.
#[repr(C)]
#[derive(Debug)]
pub struct Coreroutine {
    pub stack_ptr: *mut u8,
    pub stack_base: *mut u8,
    pub stack_size: usize,
    pub stack_limit: *mut u8,
    pub state: CoreState,
    pub id: u64,
    pub thread_id: usize,
    pub priority: u8,
    pub saved_regs: [u64; 16],
    pub waiting_on: Option<*mut crate::scheduler::channel::RawChan>,
}

// SAFETY: `Coreroutine` stores opaque runtime pointers and plain scalar state.
// Ownership and dereferencing are controlled externally by the runtime.
unsafe impl Send for Coreroutine {}

// SAFETY: Shared access to coreroutine state is synchronized via surrounding
// runtime mutexes; this type does not provide interior mutation on its own.
unsafe impl Sync for Coreroutine {}

/// Internal runtime task wrapper.
pub struct RuntimeTask {
    pub core: Arc<Mutex<Coreroutine>>,
    task: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
    done: Condvar,
    finished: Mutex<bool>,
}

impl RuntimeTask {
    pub fn new<F>(id: u64, task: F) -> Arc<Self>
    where
        F: FnOnce() + Send + 'static,
    {
        Arc::new(Self {
            core: Arc::new(Mutex::new(Coreroutine {
                stack_ptr: ptr::null_mut(),
                stack_base: ptr::null_mut(),
                stack_size: 0,
                stack_limit: ptr::null_mut(),
                state: CoreState::Ready,
                id,
                thread_id: 0,
                priority: 0,
                saved_regs: [0; 16],
                waiting_on: None,
            })),
            task: Mutex::new(Some(Box::new(task))),
            done: Condvar::new(),
            finished: Mutex::new(false),
        })
    }

    pub fn run(&self, worker_id: usize) {
        {
            let mut core = match self.core.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            core.thread_id = worker_id;
            core.state = CoreState::Running;
        }
        if let Some(task) = match self.task.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        } {
            task();
        }
        {
            let mut core = match self.core.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            core.state = CoreState::Dead;
        }
        let mut finished = match self.finished.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *finished = true;
        self.done.notify_all();
    }

    pub fn wait(&self) {
        let mut finished = match self.finished.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        while !*finished {
            finished = match self.done.wait(finished) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

thread_local! {
    static CURRENT_WORKER: Cell<usize> = const { Cell::new(usize::MAX) };
}

static NEXT_CORE_ID: AtomicU64 = AtomicU64::new(1);

/// Returns the next runtime coreroutine id.
pub fn next_core_id() -> u64 {
    NEXT_CORE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Sets the current worker id for the thread-local runtime context.
pub fn set_current_worker_id(id: usize) {
    CURRENT_WORKER.with(|slot| slot.set(id));
}

/// Returns the current worker id if running on a scheduler thread.
pub fn current_worker_id() -> Option<usize> {
    CURRENT_WORKER.with(|slot| {
        let id = slot.get();
        (id != usize::MAX).then_some(id)
    })
}

/// Converts an FFI C string to an owned Rust string.
pub(crate) fn c_string(ptr: *const libc::c_char) -> String {
    if ptr.is_null() {
        return "<null>".to_string();
    }
    // SAFETY: The caller promises `ptr` points to a valid NUL-terminated C
    // string for the duration of this conversion.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Copies bytes from a raw pointer into an owned buffer.
pub(crate) fn read_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; len];
    // SAFETY: `out` is allocated for `len` bytes and `ptr` is treated as a
    // readable byte buffer of exactly `len` bytes by the FFI contract.
    unsafe {
        ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), len);
    }
    out
}

/// Copies a byte slice into a raw output pointer.
pub(crate) fn write_bytes(ptr: *mut u8, bytes: &[u8]) {
    if ptr.is_null() || bytes.is_empty() {
        return;
    }
    // SAFETY: The caller guarantees `ptr` is writable for `bytes.len()` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
}

/// Drops a boxed pointer allocated by `Box::into_raw`.
pub(crate) fn drop_boxed<T>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: The pointer comes from `Box::into_raw` and is dropped exactly once.
    unsafe {
        drop(Box::from_raw(ptr));
    }
}

/// Returns a shared reference to a raw channel allocated by `Box::into_raw`.
pub(crate) fn raw_chan_ref<'a>(
    ptr: *mut crate::scheduler::channel::RawChan,
) -> Option<&'a crate::scheduler::channel::RawChan> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: The pointer originates from `Box::into_raw` and remains valid
    // until the matching `drop_boxed` call.
    Some(unsafe { &*ptr })
}
