use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        // SAFETY: The spin lock guarantees exclusive mutable access.
        let result = f(unsafe { &mut *self.value.get() });
        self.locked.store(false, Ordering::Release);
        result
    }
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

#[derive(Clone, Copy)]
struct CoopTask {
    id: u64,
    fn_ptr: extern "C" fn(*mut libc::c_void),
    arg_bits: usize,
}

struct CoopScheduler {
    queue: SpinLock<VecDeque<CoopTask>>,
    next_task_id: AtomicU64,
    running: AtomicBool,
}

impl CoopScheduler {
    fn new() -> Self {
        Self {
            queue: SpinLock::new(VecDeque::new()),
            next_task_id: AtomicU64::new(1),
            running: AtomicBool::new(false),
        }
    }

    fn spawn_raw(&self, fn_ptr: extern "C" fn(*mut libc::c_void), arg: *mut libc::c_void) -> u64 {
        let id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        self.queue.with_mut(|queue| {
            queue.push_back(CoopTask {
                id,
                fn_ptr,
                arg_bits: arg as usize,
            });
        });
        if !self.running.swap(true, Ordering::AcqRel) {
            self.run_to_completion();
        }
        id
    }

    fn run_to_completion(&self) {
        while self.run_one_pending() {}
        self.running.store(false, Ordering::Release);
        if !self.queue.with_mut(|queue| queue.is_empty())
            && !self.running.swap(true, Ordering::AcqRel)
        {
            self.run_to_completion();
        }
    }

    fn run_one_pending(&self) -> bool {
        let task = self.queue.with_mut(|queue| queue.pop_front());
        if let Some(task) = task {
            let _ = task.id;
            (task.fn_ptr)(task.arg_bits as *mut libc::c_void);
            true
        } else {
            false
        }
    }

    fn yield_now(&self) {
        let _ = self.run_one_pending();
    }

    fn clear(&self) {
        self.queue.with_mut(|queue| queue.clear());
        self.running.store(false, Ordering::Release);
    }
}

fn scheduler() -> &'static CoopScheduler {
    static GLOBAL: OnceLock<CoopScheduler> = OnceLock::new();
    GLOBAL.get_or_init(CoopScheduler::new)
}

pub fn init_global() {
    let _ = scheduler();
}

pub fn shutdown_global() {
    scheduler().clear();
}

pub fn spawn_raw(fn_ptr: extern "C" fn(*mut libc::c_void), arg: *mut libc::c_void) -> u64 {
    scheduler().spawn_raw(fn_ptr, arg)
}

pub fn yield_now() {
    scheduler().yield_now();
}

struct CoopChanState {
    buffer: VecDeque<Vec<u8>>,
    capacity: usize,
}

struct CoopChan {
    state: SpinLock<CoopChanState>,
}

impl CoopChan {
    fn new(capacity: usize) -> Self {
        Self {
            state: SpinLock::new(CoopChanState {
                buffer: VecDeque::new(),
                capacity: capacity.max(1),
            }),
        }
    }

    fn send(&self, value: Vec<u8>) {
        let mut value = Some(value);
        loop {
            let sent = self.state.with_mut(|state| {
                if state.buffer.len() >= state.capacity {
                    false
                } else {
                    state
                        .buffer
                        .push_back(value.take().expect("cooperative send value missing"));
                    true
                }
            });
            if sent {
                return;
            }
            yield_now();
            spin_loop();
        }
    }

    fn recv(&self) -> Vec<u8> {
        loop {
            if let Some(value) = self.state.with_mut(|state| state.buffer.pop_front()) {
                return value;
            }
            yield_now();
            spin_loop();
        }
    }
}

/// FFI-safe raw byte channel for the cooperative scheduler.
pub struct RawChan {
    elem_size: usize,
    inner: CoopChan,
}

impl RawChan {
    pub fn new(elem_size: usize, capacity: usize) -> Self {
        Self {
            elem_size,
            inner: CoopChan::new(capacity),
        }
    }

    fn send_bytes(&self, value: Vec<u8>) {
        self.inner.send(value);
    }

    fn recv_bytes(&self) -> Vec<u8> {
        self.inner.recv()
    }
}

pub fn into_raw(chan: RawChan) -> *mut RawChan {
    Box::into_raw(Box::new(chan))
}

pub fn ffi_send(chan: *mut RawChan, value_ptr: *const u8) {
    let Some(chan_ref) = raw_chan_ref(chan) else {
        return;
    };
    let bytes = read_bytes(value_ptr, chan_ref.elem_size);
    chan_ref.send_bytes(bytes);
}

pub fn ffi_recv(chan: *mut RawChan, out_ptr: *mut u8) {
    let Some(chan_ref) = raw_chan_ref(chan) else {
        return;
    };
    let value = chan_ref.recv_bytes();
    write_bytes(out_ptr, &value);
}

pub fn ffi_drop(chan: *mut RawChan) {
    drop_boxed(chan);
}

fn read_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; len];
    // SAFETY: The caller guarantees the buffer is readable for `len` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), len);
    }
    out
}

fn write_bytes(ptr: *mut u8, bytes: &[u8]) {
    if ptr.is_null() || bytes.is_empty() {
        return;
    }
    // SAFETY: The caller guarantees the output buffer is writable for `bytes.len()` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
}

fn drop_boxed<T>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: The pointer originates from `Box::into_raw` and is dropped once.
    unsafe {
        drop(Box::from_raw(ptr));
    }
}

fn raw_chan_ref<'a>(ptr: *mut RawChan) -> Option<&'a RawChan> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: The pointer remains valid until the matching ffi_drop call.
    Some(unsafe { &*ptr })
}
