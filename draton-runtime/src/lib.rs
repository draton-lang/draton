//! Draton runtime: garbage collector, scheduler, channels and panic entrypoints.

pub mod gc;
pub mod panic;
pub mod scheduler;

use std::ptr;
use std::slice;

use gc::config::GcConfig;
use scheduler::channel::RawChan;

#[repr(C)]
pub struct DratonString {
    pub len: i64,
    pub ptr: *mut libc::c_char,
}

fn string_bytes(value: DratonString) -> &'static [u8] {
    if value.ptr.is_null() || value.len <= 0 {
        &[]
    } else {
        // SAFETY: The compiler only passes Draton strings created from valid UTF-8
        // buffers with at least `len` initialized bytes. These helpers treat the
        // payload as an immutable byte slice for the duration of the call.
        unsafe { slice::from_raw_parts(value.ptr.cast::<u8>(), value.len as usize) }
    }
}

fn owned_string(bytes: Vec<u8>) -> DratonString {
    let len = bytes.len();
    let mut raw = bytes;
    raw.push(0);
    let boxed = raw.into_boxed_slice();
    let ptr = Box::into_raw(boxed).cast::<u8>();
    DratonString {
        len: len as i64,
        ptr: ptr.cast::<libc::c_char>(),
    }
}

/// Initializes the global runtime scheduler.
#[no_mangle]
pub extern "C" fn draton_runtime_init(n_threads: usize) {
    scheduler::init_global(n_threads.max(1));
}

/// Shuts down the global runtime scheduler and joins worker threads.
#[no_mangle]
pub extern "C" fn draton_runtime_shutdown() {
    scheduler::shutdown_global();
}

/// Spawns a new coreroutine on the global scheduler.
#[no_mangle]
pub extern "C" fn draton_spawn(
    fn_ptr: extern "C" fn(*mut libc::c_void),
    arg: *mut libc::c_void,
) -> u64 {
    scheduler::spawn_raw(fn_ptr, arg)
}

/// Cooperatively yields the current OS thread.
#[no_mangle]
pub extern "C" fn draton_yield() {
    scheduler::yield_now();
}

/// Creates a raw byte channel used by FFI.
#[no_mangle]
pub extern "C" fn draton_chan_new(elem_size: usize, capacity: usize) -> *mut RawChan {
    scheduler::channel::into_raw(RawChan::new(elem_size, capacity))
}

/// Sends a value to a raw byte channel.
#[no_mangle]
pub extern "C" fn draton_chan_send(chan: *mut RawChan, val: *const libc::c_void) {
    scheduler::channel::ffi_send(chan, val.cast::<u8>());
}

/// Receives a value from a raw byte channel.
#[no_mangle]
pub extern "C" fn draton_chan_recv(chan: *mut RawChan, out: *mut libc::c_void) {
    scheduler::channel::ffi_recv(chan, out.cast::<u8>());
}

/// Drops a raw byte channel.
#[no_mangle]
pub extern "C" fn draton_chan_drop(chan: *mut RawChan) {
    scheduler::channel::ffi_drop(chan);
}

/// Allocates a GC-managed object payload.
#[no_mangle]
pub extern "C" fn draton_gc_alloc(size: usize, type_id: u16) -> *mut libc::c_void {
    gc::alloc(size, type_id).cast::<libc::c_void>()
}

/// Allocates a GC-managed array payload.
#[no_mangle]
pub extern "C" fn draton_gc_alloc_array(
    elem_size: usize,
    len: usize,
    type_id: u16,
) -> *mut libc::c_void {
    gc::alloc_array(elem_size, len, type_id).cast::<libc::c_void>()
}

/// Applies the GC write barrier for a pointer store.
#[no_mangle]
pub extern "C" fn draton_gc_write_barrier(
    obj: *mut libc::c_void,
    field: *mut *mut libc::c_void,
    new_val: *mut libc::c_void,
) {
    let field_ptr = if field.is_null() {
        ptr::null_mut()
    } else {
        field.cast::<u8>()
    };
    gc::write_barrier(obj.cast::<u8>(), field_ptr, new_val.cast::<u8>());
}

/// Triggers a manual GC cycle.
#[no_mangle]
pub extern "C" fn draton_gc_collect() {
    gc::collect();
}

/// Pins an object so it is not moved by collection.
#[no_mangle]
pub extern "C" fn draton_gc_pin(obj: *mut libc::c_void) {
    gc::pin(obj.cast::<u8>());
}

/// Unpins a previously pinned object.
#[no_mangle]
pub extern "C" fn draton_gc_unpin(obj: *mut libc::c_void) {
    gc::unpin(obj.cast::<u8>());
}

/// Configures GC thresholds and heap limits.
#[no_mangle]
pub extern "C" fn draton_gc_configure(
    heap_size: usize,
    max_heap: usize,
    gc_threshold: f64,
    pause_target_ns: u64,
) {
    gc::configure(GcConfig {
        heap_size,
        max_heap,
        gc_threshold,
        pause_target_ns,
    });
}

/// Initializes the global GC runtime.
#[no_mangle]
pub extern "C" fn draton_gc_init() {
    gc::init();
}

/// Shuts the global GC runtime down and frees all tracked objects.
#[no_mangle]
pub extern "C" fn draton_gc_shutdown() {
    gc::shutdown();
}

/// Runtime panic entrypoint used by generated code.
#[no_mangle]
pub extern "C" fn draton_panic(
    msg: *const libc::c_char,
    file: *const libc::c_char,
    line: u32,
) -> ! {
    panic::draton_panic(msg, file, line)
}

/// Returns a newly allocated substring of a Draton string.
#[no_mangle]
pub extern "C" fn draton_str_slice(value: DratonString, start: i64, end: i64) -> DratonString {
    let bytes = string_bytes(value);
    let len = bytes.len() as i64;
    let start = start.clamp(0, len) as usize;
    let end = end.clamp(start as i64, len) as usize;
    owned_string(bytes[start..end].to_vec())
}

/// Concatenates two Draton strings into a newly allocated string.
#[no_mangle]
pub extern "C" fn draton_str_concat(lhs: DratonString, rhs: DratonString) -> DratonString {
    let lhs_bytes = string_bytes(lhs);
    let rhs_bytes = string_bytes(rhs);
    let mut out = Vec::with_capacity(lhs_bytes.len() + rhs_bytes.len());
    out.extend_from_slice(lhs_bytes);
    out.extend_from_slice(rhs_bytes);
    owned_string(out)
}

/// Converts an integer to a Draton string.
#[no_mangle]
pub extern "C" fn draton_int_to_string(value: i64) -> DratonString {
    owned_string(value.to_string().into_bytes())
}

/// Converts a single ASCII byte value to a one-character Draton string.
#[no_mangle]
pub extern "C" fn draton_ascii_char(value: i64) -> DratonString {
    let byte = value.clamp(0, 255) as u8;
    owned_string(vec![byte])
}
