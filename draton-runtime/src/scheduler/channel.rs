use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

use super::coreroutine;

/// Buffered/unbuffered channel used by the runtime.
#[derive(Debug)]
pub struct Chan<T> {
    state: Mutex<ChanState<T>>,
    send_cv: Condvar,
    recv_cv: Condvar,
}

#[derive(Debug)]
struct ChanState<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    blocked_senders: usize,
    blocked_receivers: usize,
}

impl<T> Chan<T> {
    /// Creates a new channel with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(ChanState {
                buffer: VecDeque::new(),
                capacity,
                blocked_senders: 0,
                blocked_receivers: 0,
            }),
            send_cv: Condvar::new(),
            recv_cv: Condvar::new(),
        }
    }

    /// Sends a value into the channel, blocking when full.
    pub fn send(&self, value: T) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let effective_capacity = state.capacity.max(1);
        while state.buffer.len() >= effective_capacity {
            state.blocked_senders += 1;
            state = match self.send_cv.wait(state) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.blocked_senders = state.blocked_senders.saturating_sub(1);
        }
        state.buffer.push_back(value);
        self.recv_cv.notify_one();
    }

    /// Receives a value from the channel, blocking when empty.
    pub fn recv(&self) -> T {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        loop {
            while state.buffer.is_empty() {
                state.blocked_receivers += 1;
                state = match self.recv_cv.wait(state) {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                state.blocked_receivers = state.blocked_receivers.saturating_sub(1);
            }
            if let Some(value) = state.buffer.pop_front() {
                self.send_cv.notify_one();
                return value;
            }
        }
    }

    /// Returns the current buffered length.
    pub fn len(&self) -> usize {
        match self.state.lock() {
            Ok(guard) => guard.buffer.len(),
            Err(poisoned) => poisoned.into_inner().buffer.len(),
        }
    }

    /// Returns whether the channel currently buffers no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// FFI-safe raw byte channel.
#[derive(Debug)]
pub struct RawChan {
    elem_size: usize,
    inner: Chan<Vec<u8>>,
}

impl RawChan {
    /// Creates a new byte channel.
    pub fn new(elem_size: usize, capacity: usize) -> Self {
        Self {
            elem_size,
            inner: Chan::new(capacity),
        }
    }

    /// Sends a byte buffer into the channel.
    pub fn send_bytes(&self, value: Vec<u8>) {
        self.inner.send(value);
    }

    /// Receives a byte buffer from the channel.
    pub fn recv_bytes(&self) -> Vec<u8> {
        self.inner.recv()
    }
}

/// Converts the channel into an owned raw pointer for FFI.
pub fn into_raw(chan: RawChan) -> *mut RawChan {
    Box::into_raw(Box::new(chan))
}

/// Sends bytes from an FFI pointer into the raw channel.
pub fn ffi_send(chan: *mut RawChan, value_ptr: *const u8) {
    let Some(chan_ref) = coreroutine::raw_chan_ref(chan) else {
        return;
    };
    let bytes = coreroutine::read_bytes(value_ptr, chan_ref.elem_size);
    chan_ref.send_bytes(bytes);
}

/// Receives bytes from the raw channel into an FFI output buffer.
pub fn ffi_recv(chan: *mut RawChan, out_ptr: *mut u8) {
    let Some(chan_ref) = coreroutine::raw_chan_ref(chan) else {
        return;
    };
    let value = chan_ref.recv_bytes();
    coreroutine::write_bytes(out_ptr, &value);
}

/// Drops the raw channel pointer.
pub fn ffi_drop(chan: *mut RawChan) {
    coreroutine::drop_boxed(chan);
}
