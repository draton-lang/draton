//! Draton runtime: garbage collector, scheduler, channels and panic entrypoints.

pub mod gc;
pub mod panic;
pub mod scheduler;

use std::fs;
use std::path::Path;
use std::ptr;
use std::slice;
use std::sync::{Mutex, OnceLock};

use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::TypeChecker;
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

fn cli_args_storage() -> &'static Mutex<Vec<String>> {
    static CLI_ARGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CLI_ARGS.get_or_init(|| Mutex::new(Vec::new()))
}

fn draton_string_to_owned(value: DratonString) -> String {
    String::from_utf8_lossy(string_bytes(value)).into_owned()
}

pub fn host_ast_dump_path(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("khong the doc {}: {error}", path.display()))?;
    let lexed = Lexer::new(&source).tokenize();
    if !lexed.errors.is_empty() {
        return Err(lexed
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let parsed = Parser::new(lexed.tokens).parse();
    if !parsed.errors.is_empty() {
        return Err(parsed
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    Ok(format!("{:#?}", parsed.program))
}

pub fn host_type_dump_path(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("khong the doc {}: {error}", path.display()))?;
    let lexed = Lexer::new(&source).tokenize();
    if !lexed.errors.is_empty() {
        return Err(lexed
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let parsed = Parser::new(lexed.tokens).parse();
    if !parsed.errors.is_empty() {
        return Err(parsed
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let typed = TypeChecker::new().check(parsed.program);
    if !typed.errors.is_empty() {
        return Err(typed
            .errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    Ok(format!("{:#?}", typed.typed_program))
}

#[no_mangle]
pub extern "C" fn draton_host_type_dump(path: DratonString) -> DratonString {
    let path_string = draton_string_to_owned(path);
    let output = host_type_dump_path(Path::new(&path_string)).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
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

/// Stores the process command-line arguments for self-host builtins.
#[no_mangle]
pub extern "C" fn draton_set_cli_args(argc: i32, argv: *const *const libc::c_char) {
    let mut args = Vec::new();
    if !argv.is_null() && argc > 0 {
        for index in 0..argc as isize {
            // SAFETY: `argv` follows the C main ABI and is valid for at least `argc` entries.
            let ptr = unsafe { *argv.offset(index) };
            if ptr.is_null() {
                args.push(String::new());
            } else {
                // SAFETY: Each argument pointer is a valid null-terminated C string.
                let value = unsafe { std::ffi::CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned();
                args.push(value);
            }
        }
    }
    if let Ok(mut guard) = cli_args_storage().lock() {
        *guard = args;
    }
}

/// Returns the stored process argument count.
#[no_mangle]
pub extern "C" fn draton_cli_argc() -> i64 {
    cli_args_storage()
        .lock()
        .map(|args| args.len() as i64)
        .unwrap_or(0)
}

/// Returns a stored process argument or an empty string when out of bounds.
#[no_mangle]
pub extern "C" fn draton_cli_arg(index: i64) -> DratonString {
    if index < 0 {
        return owned_string(Vec::new());
    }
    cli_args_storage()
        .lock()
        .ok()
        .and_then(|args| args.get(index as usize).cloned())
        .map(|value| owned_string(value.into_bytes()))
        .unwrap_or_else(|| owned_string(Vec::new()))
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

/// Reads a UTF-8 source file and returns its contents, or an empty string on failure.
#[no_mangle]
pub extern "C" fn draton_read_file(path: DratonString) -> DratonString {
    let path = draton_string_to_owned(path);
    match fs::read(path) {
        Ok(bytes) => owned_string(bytes),
        Err(_) => owned_string(Vec::new()),
    }
}

/// Parses a base-10 integer and returns 0 on failure.
#[no_mangle]
pub extern "C" fn draton_string_parse_int(value: DratonString) -> i64 {
    draton_string_to_owned(value).parse::<i64>().unwrap_or(0)
}

/// Parses an integer with the given radix and returns 0 on failure.
#[no_mangle]
pub extern "C" fn draton_string_parse_int_radix(value: DratonString, radix: i64) -> i64 {
    let radix = radix.clamp(2, 36) as u32;
    i64::from_str_radix(draton_string_to_owned(value).trim(), radix).unwrap_or(0)
}

/// Parses an f64 value and returns 0.0 on failure.
#[no_mangle]
pub extern "C" fn draton_string_parse_float(value: DratonString) -> f64 {
    draton_string_to_owned(value)
        .parse::<f64>()
        .unwrap_or(0.0)
}

/// Parses a source file with the host Rust frontend and returns its AST debug dump.
#[no_mangle]
pub extern "C" fn draton_host_ast_dump(path: DratonString) -> DratonString {
    let path = draton_string_to_owned(path);
    match host_ast_dump_path(Path::new(&path)) {
        Ok(dump) => owned_string(dump.into_bytes()),
        Err(message) => owned_string(message.into_bytes()),
    }
}
