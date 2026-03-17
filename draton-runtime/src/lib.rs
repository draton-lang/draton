//! Draton runtime: garbage collector, scheduler, channels and panic entrypoints.

pub mod gc;
pub mod panic;
pub mod scheduler;

// ── LLVM shadow-stack chain ────────────────────────────────────────────────────
// When Draton-generated code is linked against this runtime, LLVM's shadow-stack
// GC plugin provides its own definition of `llvm_gc_root_chain` and overrides
// this weak default. In pure-Rust test builds there is no LLVM-generated code,
// so the chain is always null and shadow_stack_roots() returns an empty Vec.
#[no_mangle]
pub static mut llvm_gc_root_chain: *mut gc::heap::StackEntry = std::ptr::null_mut();

// ── Safepoint mechanism ────────────────────────────────────────────────────────
// Generated code reads this flag at every safepoint poll (loop back-edges and
// after calls).  When non-zero, it jumps to draton_safepoint_slow().
// This is the authoritative definition; the codegen emits an External reference.
#[no_mangle]
pub static draton_safepoint_flag: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(0);

/// Safepoint slow-path called by generated code when `draton_safepoint_flag != 0`.
/// Resets the flag and drives GC collection.
#[no_mangle]
pub extern "C" fn draton_safepoint_slow() {
    draton_safepoint_flag.store(0, std::sync::atomic::Ordering::Release);
    gc::safepoint();
}

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::slice;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_stdlib as stdlib;
use draton_typeck::TypeChecker;
use gc::config::GcConfig;
use scheduler::channel::RawChan;

#[repr(C)]
pub struct DratonString {
    pub len: i64,
    pub ptr: *mut libc::c_char,
}

#[repr(C)]
pub struct DratonOptionI64 {
    pub is_some: bool,
    pub value: i64,
}

#[repr(C)]
pub struct DratonOptionF64 {
    pub is_some: bool,
    pub value: f64,
}

#[repr(C)]
pub struct DratonStringArray {
    pub len: i64,
    pub ptr: *mut DratonString,
}

#[repr(C)]
pub struct DratonIntArray {
    pub len: i64,
    pub ptr: *mut i64,
}

#[repr(C)]
pub struct DratonGcPauseStats {
    pub total_ns: u64,
    pub last_ns: u64,
    pub max_ns: u64,
}

#[repr(C)]
pub struct DratonGcStats {
    pub minor_cycles: u64,
    pub major_cycles: u64,
    pub major_slices: u64,
    pub full_cycles: u64,
    pub young_allocations: u64,
    pub old_allocations: u64,
    pub large_allocations: u64,
    pub array_allocations: u64,
    pub bytes_allocated: u64,
    pub bytes_promoted: u64,
    pub bytes_reclaimed_minor: u64,
    pub bytes_reclaimed_major: u64,
    pub bytes_reclaimed_large: u64,
    pub write_barrier_slow_calls: u64,
    pub major_work_requests: u64,
    pub major_work_threshold_requests: u64,
    pub major_work_continuation_requests: u64,
    pub major_mutator_assists: u64,
    pub major_background_slices: u64,
    pub major_autotune_adjustments: u64,
    pub major_work_budget: usize,
    pub major_work_budget_peak: usize,
    pub major_work_requested: bool,
    pub safepoint_rearms: u64,
    pub major_mark_barrier_traces: u64,
    pub remembered_set_entries_added: u64,
    pub remembered_set_entries_deduped: u64,
    pub young_usage_bytes: usize,
    pub old_usage_bytes: usize,
    pub heap_usage_bytes: usize,
    pub large_object_count: usize,
    pub large_free_pool_count: usize,
    pub large_free_bytes: usize,
    pub roots_count: usize,
    pub remembered_set_len: usize,
    pub old_free_slot_count: usize,
    pub old_free_bytes: usize,
    pub old_largest_free_slot: usize,
    pub current_mark_stack_len: usize,
    pub current_mark_slice_size: usize,
    pub current_gc_threshold_milli: u32,
    pub major_phase: u8,
    pub old_sweep_cursor: usize,
    pub large_sweep_pending: usize,
    pub minor_pause: DratonGcPauseStats,
    pub major_pause: DratonGcPauseStats,
    pub full_pause: DratonGcPauseStats,
}

fn export_gc_pause(stats: gc::GcPauseStats) -> DratonGcPauseStats {
    DratonGcPauseStats {
        total_ns: stats.total_ns,
        last_ns: stats.last_ns,
        max_ns: stats.max_ns,
    }
}

fn export_gc_stats(stats: gc::GcStats) -> DratonGcStats {
    DratonGcStats {
        minor_cycles: stats.minor_cycles,
        major_cycles: stats.major_cycles,
        major_slices: stats.major_slices,
        full_cycles: stats.full_cycles,
        young_allocations: stats.young_allocations,
        old_allocations: stats.old_allocations,
        large_allocations: stats.large_allocations,
        array_allocations: stats.array_allocations,
        bytes_allocated: stats.bytes_allocated,
        bytes_promoted: stats.bytes_promoted,
        bytes_reclaimed_minor: stats.bytes_reclaimed_minor,
        bytes_reclaimed_major: stats.bytes_reclaimed_major,
        bytes_reclaimed_large: stats.bytes_reclaimed_large,
        write_barrier_slow_calls: stats.write_barrier_slow_calls,
        major_work_requests: stats.major_work_requests,
        major_work_threshold_requests: stats.major_work_threshold_requests,
        major_work_continuation_requests: stats.major_work_continuation_requests,
        major_mutator_assists: stats.major_mutator_assists,
        major_background_slices: stats.major_background_slices,
        major_autotune_adjustments: stats.major_autotune_adjustments,
        major_work_budget: stats.major_work_budget,
        major_work_budget_peak: stats.major_work_budget_peak,
        major_work_requested: stats.major_work_requested,
        safepoint_rearms: stats.safepoint_rearms,
        major_mark_barrier_traces: stats.major_mark_barrier_traces,
        remembered_set_entries_added: stats.remembered_set_entries_added,
        remembered_set_entries_deduped: stats.remembered_set_entries_deduped,
        young_usage_bytes: stats.young_usage_bytes,
        old_usage_bytes: stats.old_usage_bytes,
        heap_usage_bytes: stats.heap_usage_bytes,
        large_object_count: stats.large_object_count,
        large_free_pool_count: stats.large_free_pool_count,
        large_free_bytes: stats.large_free_bytes,
        roots_count: stats.roots_count,
        remembered_set_len: stats.remembered_set_len,
        old_free_slot_count: stats.old_free_slot_count,
        old_free_bytes: stats.old_free_bytes,
        old_largest_free_slot: stats.old_largest_free_slot,
        current_mark_stack_len: stats.current_mark_stack_len,
        current_mark_slice_size: stats.current_mark_slice_size,
        current_gc_threshold_milli: stats.current_gc_threshold_milli,
        major_phase: stats.major_phase,
        old_sweep_cursor: stats.old_sweep_cursor,
        large_sweep_pending: stats.large_sweep_pending,
        minor_pause: export_gc_pause(stats.minor_pause),
        major_pause: export_gc_pause(stats.major_pause),
        full_pause: export_gc_pause(stats.full_pause),
    }
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

fn option_i64(value: Option<i64>) -> DratonOptionI64 {
    match value {
        Some(value) => DratonOptionI64 {
            is_some: true,
            value,
        },
        None => DratonOptionI64 {
            is_some: false,
            value: 0,
        },
    }
}

fn option_f64(value: Option<f64>) -> DratonOptionF64 {
    match value {
        Some(value) => DratonOptionF64 {
            is_some: true,
            value,
        },
        None => DratonOptionF64 {
            is_some: false,
            value: 0.0,
        },
    }
}

fn string_array_to_owned(values: DratonStringArray) -> Vec<String> {
    if values.ptr.is_null() || values.len <= 0 {
        return Vec::new();
    }
    let slice = unsafe { slice::from_raw_parts(values.ptr, values.len as usize) };
    slice
        .iter()
        .map(|value| {
            draton_string_to_owned(DratonString {
                len: value.len,
                ptr: value.ptr,
            })
        })
        .collect()
}

fn owned_string_array(values: Vec<String>) -> DratonStringArray {
    let len = values.len();
    let items = values
        .into_iter()
        .map(|value| owned_string(value.into_bytes()))
        .collect::<Vec<_>>();
    let boxed = items.into_boxed_slice();
    let ptr = Box::into_raw(boxed) as *mut DratonString;
    DratonStringArray {
        len: len as i64,
        ptr,
    }
}

fn int_array_to_owned(values: DratonIntArray) -> Vec<i64> {
    if values.ptr.is_null() || values.len <= 0 {
        return Vec::new();
    }
    let slice = unsafe { slice::from_raw_parts(values.ptr, values.len as usize) };
    slice.to_vec()
}

fn owned_int_array(values: Vec<i64>) -> DratonIntArray {
    let len = values.len();
    let boxed = values.into_boxed_slice();
    let ptr = Box::into_raw(boxed) as *mut i64;
    DratonIntArray {
        len: len as i64,
        ptr,
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
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
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
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
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

fn runtime_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn runtime_target_dir() -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| runtime_workspace_root().join("target"))
}

fn runtime_ensure_host_drat() -> Result<PathBuf, String> {
    let manifest = runtime_workspace_root().join("drat/Cargo.toml");
    let exe_name = if cfg!(windows) { "drat.exe" } else { "drat" };
    let path = runtime_target_dir().join("debug").join(exe_name);
    if path.exists() {
        return Ok(path);
    }
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("-p")
        .arg("drat")
        .arg("--manifest-path")
        .arg(manifest);
    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to build drat: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "drat build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !path.exists() {
        return Err(format!("host drat binary not found at {}", path.display()));
    }
    Ok(path)
}

fn runtime_profile_dir(mode: &str) -> &'static str {
    match mode {
        "Release" => "release",
        "Size" => "size",
        "Fast" => "fast",
        _ => "debug",
    }
}

fn runtime_find_project_root(source_path: &Path) -> Option<PathBuf> {
    for ancestor in source_path.ancestors() {
        if ancestor.join("draton.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn runtime_copy_dir_recursive(source: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|error| format!("failed to create {}: {error}", dest.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read directory {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if source_path.is_dir() {
            runtime_copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            fs::copy(&source_path, &dest_path).map_err(|error| {
                format!(
                    "failed to copy {} -> {}: {error}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn runtime_temp_project_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    runtime_workspace_root()
        .join("build")
        .join("phase5_host")
        .join(format!("tmp_{}_{}", std::process::id(), stamp))
}

fn runtime_build_mode_flag(mode: &str) -> Option<&'static str> {
    match mode {
        "Release" => Some("--release"),
        "Size" => Some("--size"),
        "Fast" => Some("--fast"),
        _ => None,
    }
}

fn runtime_prepare_temp_project(source_path: &Path, temp_root: &Path) -> Result<String, String> {
    fs::create_dir_all(temp_root)
        .map_err(|error| format!("failed to create {}: {error}", temp_root.display()))?;
    if let Some(project_root) = runtime_find_project_root(source_path) {
        let src_root = project_root.join("src");
        if source_path.starts_with(&src_root) {
            runtime_copy_dir_recursive(&src_root, &temp_root.join("src"))?;
            let rel = source_path
                .strip_prefix(&project_root)
                .map_err(|error| format!("failed to compute relative path: {error}"))?;
            return Ok(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    let temp_src = temp_root.join("src");
    fs::create_dir_all(&temp_src)
        .map_err(|error| format!("failed to create {}: {error}", temp_src.display()))?;
    let dest = temp_src.join("main.dt");
    fs::copy(source_path, &dest).map_err(|error| {
        format!(
            "failed to copy {} -> {}: {error}",
            source_path.display(),
            dest.display()
        )
    })?;
    Ok("src/main.dt".to_string())
}

fn runtime_copy_output(source: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::copy(source, dest).map_err(|error| {
        format!(
            "failed to copy {} -> {}: {error}",
            source.display(),
            dest.display()
        )
    })?;
    Ok(())
}

fn host_build_source_impl(
    source_path: &Path,
    ir_path: &Path,
    object_path: &Path,
    binary_path: &Path,
    mode: &str,
    emit_ir: bool,
) -> Result<(), String> {
    let host_drat = runtime_ensure_host_drat()?;
    let temp_root = runtime_temp_project_root();
    let entry = runtime_prepare_temp_project(source_path, &temp_root)?;
    let project_name = "phase5_bootstrap";
    let config_path = temp_root.join("draton.toml");
    let config =
        format!("[project]\nname = \"{project_name}\"\nversion = \"0.1.0\"\nentry = \"{entry}\"\n");
    fs::write(&config_path, config)
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;

    let mut command = Command::new(host_drat);
    command.current_dir(&temp_root).arg("build");
    command.env("DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS", "1");
    command.env("DRATON_DISABLE_GCROOT", "1");
    if let Some(flag) = runtime_build_mode_flag(mode) {
        command.arg(flag);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to run host drat build: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut message = String::new();
        if !stderr.trim().is_empty() {
            message.push_str(&stderr);
        }
        if !stdout.trim().is_empty() {
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str(&stdout);
        }
        if message.trim().is_empty() {
            message = "host drat build failed".to_string();
        }
        return Err(message);
    }

    let build_dir = temp_root.join("build").join(runtime_profile_dir(mode));
    let exe_name = if cfg!(windows) {
        format!("{project_name}.exe")
    } else {
        project_name.to_string()
    };
    let built_ir = build_dir.join(format!("{project_name}.ll"));
    let built_obj = build_dir.join(format!("{project_name}.o"));
    let built_bin = build_dir.join(exe_name);
    runtime_copy_output(&built_obj, object_path)?;
    runtime_copy_output(&built_bin, binary_path)?;
    if emit_ir {
        runtime_copy_output(&built_ir, ir_path)?;
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn draton_host_type_dump(path: DratonString) -> DratonString {
    let path_string = draton_string_to_owned(path);
    let output = host_type_dump_path(Path::new(&path_string)).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[no_mangle]
pub extern "C" fn draton_host_build_source(
    source_path: DratonString,
    ir_path: DratonString,
    object_path: DratonString,
    binary_path: DratonString,
    mode: DratonString,
    emit_ir: i64,
) -> DratonString {
    let source_path = draton_string_to_owned(source_path);
    let ir_path = draton_string_to_owned(ir_path);
    let object_path = draton_string_to_owned(object_path);
    let binary_path = draton_string_to_owned(binary_path);
    let mode = draton_string_to_owned(mode);
    let output = match host_build_source_impl(
        Path::new(&source_path),
        Path::new(&ir_path),
        Path::new(&object_path),
        Path::new(&binary_path),
        &mode,
        emit_ir != 0,
    ) {
        Ok(()) => String::new(),
        Err(message) => message,
    };
    owned_string(output.into_bytes())
}

#[no_mangle]
pub extern "C" fn draton_shell_run(cmd: DratonString) -> i64 {
    let cmd = draton_string_to_owned(cmd);
    let output = if cfg!(windows) {
        Command::new("cmd").args(["/C", &cmd]).status()
    } else {
        Command::new("sh").args(["-c", &cmd]).status()
    };
    match output {
        Ok(status) => status.code().unwrap_or(1) as i64,
        Err(_) => 1,
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

/// Returns a snapshot of GC telemetry counters and current heap state.
#[no_mangle]
pub extern "C" fn draton_gc_stats() -> DratonGcStats {
    export_gc_stats(gc::stats())
}

/// Resets GC telemetry counters without changing the current heap contents.
#[no_mangle]
pub extern "C" fn draton_gc_reset_stats() {
    gc::reset_stats();
}

/// Verifies internal GC heap invariants and returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn draton_gc_verify() -> i64 {
    if gc::verify().is_ok() {
        1
    } else {
        0
    }
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
/// `heap_size` maps to the old-generation budget; young-gen and large-object
/// thresholds retain their defaults unless changed via the full `GcConfig` API.
#[no_mangle]
pub extern "C" fn draton_gc_configure(
    heap_size: usize,
    max_heap: usize,
    gc_threshold: f64,
    pause_target_ns: u64,
) {
    gc::configure(GcConfig {
        old_size: heap_size,
        max_heap,
        gc_threshold,
        pause_target_ns,
        ..GcConfig::default()
    });
}

/// Registers a type descriptor with the GC so it can precisely trace pointer
/// fields inside objects of the given type.
///
/// # Safety
/// `offsets_ptr` must point to `num_offsets` valid `u32` values for the
/// duration of this call.
#[no_mangle]
pub unsafe extern "C" fn draton_gc_register_type(
    type_id: u16,
    size: u32,
    offsets_ptr: *const u32,
    num_offsets: u32,
) {
    let offsets = if offsets_ptr.is_null() || num_offsets == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(offsets_ptr, num_offsets as usize)
    };
    gc::register_type(type_id, size, offsets);
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

/// Returns true when `needle` occurs anywhere in `value`.
#[no_mangle]
pub extern "C" fn draton_str_contains(value: DratonString, needle: DratonString) -> bool {
    draton_string_to_owned(value).contains(&draton_string_to_owned(needle))
}

/// Returns true when `value` begins with `prefix`.
#[no_mangle]
pub extern "C" fn draton_str_starts_with(value: DratonString, prefix: DratonString) -> bool {
    draton_string_to_owned(value).starts_with(&draton_string_to_owned(prefix))
}

/// Returns true when two Draton strings are byte-equal.
#[no_mangle]
pub extern "C" fn draton_str_eq(lhs: DratonString, rhs: DratonString) -> bool {
    string_bytes(lhs) == string_bytes(rhs)
}

/// Replaces all occurrences of `from` with `to`.
#[no_mangle]
pub extern "C" fn draton_str_replace(
    value: DratonString,
    from: DratonString,
    to: DratonString,
) -> DratonString {
    owned_string(
        draton_string_to_owned(value)
            .replace(&draton_string_to_owned(from), &draton_string_to_owned(to))
            .into_bytes(),
    )
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
    draton_string_to_owned(value).parse::<f64>().unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn __draton_std_string_split(
    value: DratonString,
    sep: DratonString,
) -> DratonStringArray {
    owned_string_array(stdlib::string::split(
        draton_string_to_owned(value),
        draton_string_to_owned(sep),
    ))
}

#[no_mangle]
pub extern "C" fn __draton_std_string_trim(value: DratonString) -> DratonString {
    owned_string(stdlib::string::trim(draton_string_to_owned(value)).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_trim_start(value: DratonString) -> DratonString {
    owned_string(stdlib::string::trim_start(draton_string_to_owned(value)).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_trim_end(value: DratonString) -> DratonString {
    owned_string(stdlib::string::trim_end(draton_string_to_owned(value)).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_to_upper(value: DratonString) -> DratonString {
    owned_string(stdlib::string::upper(draton_string_to_owned(value)).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_to_lower(value: DratonString) -> DratonString {
    owned_string(stdlib::string::lower(draton_string_to_owned(value)).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_parse_int(value: DratonString) -> DratonOptionI64 {
    option_i64(stdlib::string::to_int(draton_string_to_owned(value)).ok())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_parse_float(value: DratonString) -> DratonOptionF64 {
    option_f64(stdlib::string::to_float(draton_string_to_owned(value)).ok())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_join(
    parts: DratonStringArray,
    sep: DratonString,
) -> DratonString {
    owned_string(
        stdlib::string::join(&string_array_to_owned(parts), draton_string_to_owned(sep))
            .into_bytes(),
    )
}

#[no_mangle]
pub extern "C" fn __draton_std_string_repeat(value: DratonString, n: i64) -> DratonString {
    owned_string(stdlib::string::repeat(draton_string_to_owned(value), n).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_string_index_of(value: DratonString, sub: DratonString) -> i64 {
    stdlib::string::index_of(draton_string_to_owned(value), draton_string_to_owned(sub))
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn __draton_std_string_ends_with(value: DratonString, suffix: DratonString) -> bool {
    stdlib::string::ends_with(
        draton_string_to_owned(value),
        draton_string_to_owned(suffix),
    )
}

#[no_mangle]
pub extern "C" fn __draton_std_string_contains(value: DratonString, sub: DratonString) -> bool {
    stdlib::string::contains(draton_string_to_owned(value), draton_string_to_owned(sub))
}

#[no_mangle]
pub extern "C" fn __draton_std_string_starts_with(
    value: DratonString,
    prefix: DratonString,
) -> bool {
    stdlib::string::starts_with(
        draton_string_to_owned(value),
        draton_string_to_owned(prefix),
    )
}

#[no_mangle]
pub extern "C" fn __draton_std_string_replace(
    value: DratonString,
    from: DratonString,
    to: DratonString,
) -> DratonString {
    owned_string(
        stdlib::string::replace(
            draton_string_to_owned(value),
            draton_string_to_owned(from),
            draton_string_to_owned(to),
        )
        .into_bytes(),
    )
}

#[no_mangle]
pub extern "C" fn __draton_std_string_slice(
    value: DratonString,
    start: i64,
    end: i64,
) -> DratonString {
    owned_string(stdlib::string::slice(draton_string_to_owned(value), start, end).into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_int_to_string(value: i64) -> DratonString {
    draton_int_to_string(value)
}

#[no_mangle]
pub extern "C" fn __draton_std_float_to_string(value: f64) -> DratonString {
    owned_string(value.to_string().into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_io_eprintln(value: DratonString) {
    stdlib::io::eprintln(draton_string_to_owned(value));
}

#[no_mangle]
pub extern "C" fn __draton_std_io_read_line() -> DratonString {
    owned_string(stdlib::io::readline().into_bytes())
}

#[no_mangle]
pub extern "C" fn __draton_std_io_read_file(path: DratonString) -> DratonString {
    match stdlib::fs::read(draton_string_to_owned(path)) {
        Ok(content) => owned_string(content.into_bytes()),
        Err(_) => owned_string(Vec::new()),
    }
}

#[no_mangle]
pub extern "C" fn __draton_std_io_write_file(path: DratonString, content: DratonString) -> bool {
    stdlib::fs::write(
        draton_string_to_owned(path),
        draton_string_to_owned(content),
    )
    .is_ok()
}

#[no_mangle]
pub extern "C" fn __draton_std_io_append_file(path: DratonString, content: DratonString) -> bool {
    stdlib::fs::append(
        draton_string_to_owned(path),
        draton_string_to_owned(content),
    )
    .is_ok()
}

#[no_mangle]
pub extern "C" fn __draton_std_io_file_exists(path: DratonString) -> bool {
    stdlib::fs::exists(draton_string_to_owned(path))
}

#[no_mangle]
pub extern "C" fn __draton_std_collections_sum(values: DratonIntArray) -> i64 {
    int_array_to_owned(values).into_iter().sum()
}

#[no_mangle]
pub extern "C" fn __draton_std_collections_product(values: DratonIntArray) -> i64 {
    int_array_to_owned(values).into_iter().product()
}

#[no_mangle]
pub extern "C" fn __draton_std_collections_reverse_int(values: DratonIntArray) -> DratonIntArray {
    let mut values = int_array_to_owned(values);
    values.reverse();
    owned_int_array(values)
}

#[no_mangle]
pub extern "C" fn __draton_std_collections_sort_int(values: DratonIntArray) -> DratonIntArray {
    let mut values = int_array_to_owned(values);
    values.sort();
    owned_int_array(values)
}

#[no_mangle]
pub extern "C" fn __draton_std_collections_unique_int(values: DratonIntArray) -> DratonIntArray {
    let mut values = int_array_to_owned(values);
    values.sort();
    values.dedup();
    owned_int_array(values)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_sqrt(x: f64) -> f64 {
    stdlib::math::sqrt(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_pow(base: f64, exp: f64) -> f64 {
    stdlib::math::pow(base, exp)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_abs(x: f64) -> f64 {
    stdlib::math::abs(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_floor(x: f64) -> f64 {
    stdlib::math::floor(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_ceil(x: f64) -> f64 {
    stdlib::math::ceil(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_round(x: f64) -> f64 {
    stdlib::math::round(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_sin(x: f64) -> f64 {
    stdlib::math::sin(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_cos(x: f64) -> f64 {
    stdlib::math::cos(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_tan(x: f64) -> f64 {
    stdlib::math::tan(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_log(x: f64) -> f64 {
    stdlib::math::log(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_log2(x: f64) -> f64 {
    stdlib::math::log2(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_log10(x: f64) -> f64 {
    stdlib::math::log10(x)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_min(a: f64, b: f64) -> f64 {
    stdlib::math::min(a, b)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_max(a: f64, b: f64) -> f64 {
    stdlib::math::max(a, b)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_clamp(x: f64, lo: f64, hi: f64) -> f64 {
    stdlib::math::clamp(x, lo, hi)
}

#[no_mangle]
pub extern "C" fn __draton_std_math_pi() -> f64 {
    stdlib::math::pi()
}

#[no_mangle]
pub extern "C" fn __draton_std_math_e() -> f64 {
    stdlib::math::e()
}

#[no_mangle]
pub extern "C" fn __draton_std_math_checked_add(a: i64, b: i64) -> DratonOptionI64 {
    option_i64(stdlib::math::checked_add(a, b))
}

#[no_mangle]
pub extern "C" fn __draton_std_math_checked_sub(a: i64, b: i64) -> DratonOptionI64 {
    option_i64(stdlib::math::checked_sub(a, b))
}

#[no_mangle]
pub extern "C" fn __draton_std_math_checked_mul(a: i64, b: i64) -> DratonOptionI64 {
    option_i64(stdlib::math::checked_mul(a, b))
}

#[no_mangle]
pub extern "C" fn __draton_std_math_checked_div(a: i64, b: i64) -> DratonOptionI64 {
    option_i64(stdlib::math::checked_div(a, b))
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
