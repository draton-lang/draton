//! Draton runtime: ownership-mode runtime ABI, scheduler, channels and panic entrypoints.

pub mod panic;
pub mod platform;
#[cfg(feature = "scheduler")]
pub mod scheduler;

#[cfg(feature = "coop-scheduler")]
#[path = "scheduler/coop.rs"]
mod coop_scheduler;

#[cfg(feature = "host-compiler")]
use std::env;
#[cfg(any(feature = "std-io", feature = "host-compiler"))]
use std::fs;
#[cfg(feature = "host-compiler")]
use std::path::Path;
#[cfg(feature = "host-compiler")]
use std::path::PathBuf;
#[cfg(feature = "host-compiler")]
use std::process::Command;
use std::slice;
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "host-compiler")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
use coop_scheduler::RawChan;
#[cfg(feature = "host-compiler")]
use draton_lexer::{LexResult, Lexer};
#[cfg(feature = "host-compiler")]
use draton_parser::{ParseResult, ParseWarning, Parser};
use draton_stdlib as stdlib;
#[cfg(feature = "host-compiler")]
use draton_typeck::{DeprecatedSyntaxMode, TypeCheckResult, TypeChecker};
use platform::DratonPlatform;
#[cfg(feature = "scheduler")]
use scheduler::channel::RawChan;
#[cfg(feature = "host-compiler")]
use serde_json::json;

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

pub fn set_platform(p: Box<dyn DratonPlatform>) {
    let _ = platform::registry().set(p);
}

pub(crate) fn platform() -> &'static dyn DratonPlatform {
    if let Some(existing) = platform::registry().get() {
        return existing.as_ref();
    }
    if let Some(default_platform) = platform::default_platform() {
        let _ = platform::registry().set(default_platform);
    }
    platform::registry()
        .get()
        .map(|platform| platform.as_ref())
        .expect("draton platform not initialized")
}

#[cfg(feature = "std-io")]
fn trim_line_endings(mut bytes: Vec<u8>) -> Vec<u8> {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        let _ = bytes.pop();
    }
    bytes
}

fn stdout_bytes(bytes: &[u8]) {
    if !bytes.is_empty() {
        platform().write_stdout(bytes);
    }
}

#[cfg(feature = "std-io")]
fn stderr_bytes(bytes: &[u8]) {
    if !bytes.is_empty() {
        platform().write_stderr(bytes);
    }
}

#[cfg(feature = "host-compiler")]
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

#[cfg(feature = "host-compiler")]
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

#[cfg(feature = "host-compiler")]
fn host_lex_result(path: &Path) -> Result<LexResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(Lexer::new(&source).tokenize())
}

#[cfg(feature = "host-compiler")]
pub fn host_lex_json_path(path: &Path) -> Result<String, String> {
    let lexed = host_lex_result(path)?;
    serde_json::to_string(&lexed)
        .map_err(|error| format!("failed to serialize lex result: {error}"))
}

#[cfg(feature = "host-compiler")]
pub fn host_parse_json_path(path: &Path) -> Result<String, String> {
    let lexed = host_lex_result(path)?;
    let parse_result: Option<ParseResult> = if lexed.errors.is_empty() {
        Some(Parser::new(lexed.tokens.clone()).parse())
    } else {
        None
    };
    serde_json::to_string(&json!({
        "lex_errors": lexed.errors,
        "parse_result": parse_result,
    }))
    .map_err(|error| format!("failed to serialize parse result: {error}"))
}

#[cfg(feature = "host-compiler")]
pub fn host_type_json_path(path: &Path, strict_syntax: bool) -> Result<String, String> {
    let lexed = host_lex_result(path)?;
    let mut parse_errors = Vec::new();
    let mut parse_warnings: Vec<ParseWarning> = Vec::new();
    let mut typecheck_result: Option<TypeCheckResult> = None;
    if lexed.errors.is_empty() {
        let parsed = Parser::new(lexed.tokens.clone()).parse();
        parse_errors = parsed.errors.clone();
        parse_warnings = parsed.warnings.clone();
        if parse_errors.is_empty() {
            typecheck_result = Some(
                TypeChecker::new()
                    .with_deprecated_syntax_mode(if strict_syntax {
                        DeprecatedSyntaxMode::Deny
                    } else {
                        DeprecatedSyntaxMode::Warn
                    })
                    .check(parsed.program),
            );
        }
    }
    serde_json::to_string(&json!({
        "lex_errors": lexed.errors,
        "parse_errors": parse_errors,
        "parse_warnings": parse_warnings,
        "typecheck_result": typecheck_result,
    }))
    .map_err(|error| format!("failed to serialize typecheck result: {error}"))
}

#[cfg(feature = "host-compiler")]
fn runtime_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(feature = "host-compiler")]
fn runtime_target_dir() -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| runtime_workspace_root().join("target"))
}

#[cfg(feature = "host-compiler")]
fn runtime_ensure_host_drat() -> Result<PathBuf, String> {
    let manifest = runtime_workspace_root().join("crates/drat/Cargo.toml");
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

#[cfg(feature = "host-compiler")]
fn runtime_profile_dir(mode: &str) -> &'static str {
    match mode {
        "Release" => "release",
        "Size" => "size",
        "Fast" => "fast",
        _ => "debug",
    }
}

#[cfg(feature = "host-compiler")]
fn runtime_find_project_root(source_path: &Path) -> Option<PathBuf> {
    for ancestor in source_path.ancestors() {
        if ancestor.join("draton.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

#[cfg(feature = "host-compiler")]
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

#[cfg(feature = "host-compiler")]
fn runtime_temp_project_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    env::temp_dir()
        .join("draton")
        .join("phase5_host")
        .join(format!("tmp_{}_{}", std::process::id(), stamp))
}

#[cfg(feature = "host-compiler")]
fn runtime_build_mode_flag(mode: &str) -> Option<&'static str> {
    match mode {
        "Release" => Some("--release"),
        "Size" => Some("--size"),
        "Fast" => Some("--fast"),
        _ => None,
    }
}

#[cfg(feature = "host-compiler")]
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

#[cfg(feature = "host-compiler")]
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

#[cfg(feature = "host-compiler")]
fn host_build_source_impl(
    source_path: &Path,
    ir_path: &Path,
    object_path: &Path,
    binary_path: &Path,
    mode: &str,
    strict_syntax: bool,
    target: Option<&str>,
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
    if let Some(flag) = runtime_build_mode_flag(mode) {
        command.arg(flag);
    }
    if strict_syntax {
        command.arg("--strict-syntax");
    }
    if let Some(target) = target.filter(|value| !value.is_empty()) {
        command.arg("--target").arg(target);
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

#[cfg(feature = "host-compiler")]
pub fn host_build_json_path(
    source_path: &Path,
    output_path: Option<&Path>,
    mode: &str,
    strict_syntax: bool,
    target: Option<&str>,
) -> Result<String, String> {
    let final_binary = match output_path {
        Some(path) => path.to_path_buf(),
        None => {
            let cwd = env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?;
            let stem = source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("output");
            if cfg!(windows) {
                cwd.join(format!("{stem}.exe"))
            } else {
                cwd.join(stem)
            }
        }
    };
    let final_object = final_binary.with_extension("o");
    let final_ir = final_binary.with_extension("ll");
    let result = host_build_source_impl(
        source_path,
        &final_ir,
        &final_object,
        &final_binary,
        mode,
        strict_syntax,
        target,
        true,
    );
    let envelope = match result {
        Ok(()) => json!({
            "ok": true,
            "output": {
                "binary_path": final_binary.display().to_string(),
                "object_path": final_object.display().to_string(),
                "ir_path": final_ir.display().to_string(),
            },
            "error": null,
        }),
        Err(error) => json!({
            "ok": false,
            "output": null,
            "error": error,
        }),
    };
    serde_json::to_string(&envelope)
        .map_err(|error| format!("failed to serialize build result: {error}"))
}

#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_type_dump(path: DratonString) -> DratonString {
    let path_string = draton_string_to_owned(path);
    let output = host_type_dump_path(Path::new(&path_string)).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
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
        false,
        None,
        emit_ir != 0,
    ) {
        Ok(()) => String::new(),
        Err(message) => message,
    };
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_lex_json(path: DratonString) -> DratonString {
    let path = draton_string_to_owned(path);
    let output = host_lex_json_path(Path::new(&path)).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_parse_json(path: DratonString) -> DratonString {
    let path = draton_string_to_owned(path);
    let output = host_parse_json_path(Path::new(&path)).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_type_json(path: DratonString, strict_syntax: i64) -> DratonString {
    let path = draton_string_to_owned(path);
    let output =
        host_type_json_path(Path::new(&path), strict_syntax != 0).unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_build_json(
    source_path: DratonString,
    output_path: DratonString,
    mode: DratonString,
    strict_syntax: i64,
    target: DratonString,
) -> DratonString {
    let source_path = draton_string_to_owned(source_path);
    let output_path = draton_string_to_owned(output_path);
    let mode = draton_string_to_owned(mode);
    let target = draton_string_to_owned(target);
    let output = host_build_json_path(
        Path::new(&source_path),
        if output_path.is_empty() {
            None
        } else {
            Some(Path::new(&output_path))
        },
        &mode,
        strict_syntax != 0,
        if target.is_empty() {
            None
        } else {
            Some(target.as_str())
        },
    )
    .unwrap_or_else(|message| message);
    owned_string(output.into_bytes())
}

#[cfg(feature = "host-compiler")]
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
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_runtime_init(n_threads: usize) {
    let _ = n_threads;
    #[cfg(feature = "std-io")]
    let _ = platform();
    #[cfg(feature = "scheduler")]
    scheduler::init_global(n_threads.max(1));
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::init_global();
}

/// Shuts down the global runtime scheduler and joins worker threads.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_runtime_shutdown() {
    #[cfg(feature = "scheduler")]
    scheduler::shutdown_global();
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::shutdown_global();
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
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_spawn(
    fn_ptr: extern "C" fn(*mut libc::c_void),
    arg: *mut libc::c_void,
) -> u64 {
    #[cfg(feature = "scheduler")]
    {
        return scheduler::spawn_raw(fn_ptr, arg);
    }
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    {
        return coop_scheduler::spawn_raw(fn_ptr, arg);
    }
}

/// Cooperatively yields the current OS thread.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_yield() {
    #[cfg(feature = "scheduler")]
    scheduler::yield_now();
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::yield_now();
}

/// Creates a raw byte channel used by FFI.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_chan_new(elem_size: usize, capacity: usize) -> *mut RawChan {
    #[cfg(feature = "scheduler")]
    {
        return scheduler::channel::into_raw(RawChan::new(elem_size, capacity));
    }
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    {
        return coop_scheduler::into_raw(RawChan::new(elem_size, capacity));
    }
}

/// Sends a value to a raw byte channel.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_chan_send(chan: *mut RawChan, val: *const libc::c_void) {
    #[cfg(feature = "scheduler")]
    scheduler::channel::ffi_send(chan, val.cast::<u8>());
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::ffi_send(chan, val.cast::<u8>());
}

/// Receives a value from a raw byte channel.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_chan_recv(chan: *mut RawChan, out: *mut libc::c_void) {
    #[cfg(feature = "scheduler")]
    scheduler::channel::ffi_recv(chan, out.cast::<u8>());
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::ffi_recv(chan, out.cast::<u8>());
}

/// Drops a raw byte channel.
#[cfg(any(feature = "scheduler", feature = "coop-scheduler"))]
#[no_mangle]
pub extern "C" fn draton_chan_drop(chan: *mut RawChan) {
    #[cfg(feature = "scheduler")]
    scheduler::channel::ffi_drop(chan);
    #[cfg(all(feature = "coop-scheduler", not(feature = "scheduler")))]
    coop_scheduler::ffi_drop(chan);
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

#[no_mangle]
pub extern "C" fn draton_print(value: DratonString) {
    stdout_bytes(string_bytes(value));
}

#[no_mangle]
pub extern "C" fn draton_println(value: DratonString) {
    stdout_bytes(string_bytes(value));
    stdout_bytes(b"\n");
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

/// Prints a prompt and reads a single line from stdin.
#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn draton_input(prompt: DratonString) -> DratonString {
    stdout_bytes(string_bytes(prompt));
    owned_string(trim_line_endings(platform().read_line()))
}

#[cfg(feature = "std-io")]
#[used]
static DRATON_INPUT_KEEP: extern "C" fn(DratonString) -> DratonString = draton_input;

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
#[cfg(feature = "std-io")]
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

#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn __draton_std_io_eprintln(value: DratonString) {
    stderr_bytes(string_bytes(value));
    stderr_bytes(b"\n");
}

#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn __draton_std_io_read_line() -> DratonString {
    owned_string(trim_line_endings(platform().read_line()))
}

#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn __draton_std_io_read_file(path: DratonString) -> DratonString {
    match stdlib::fs::read(draton_string_to_owned(path)) {
        Ok(content) => owned_string(content.into_bytes()),
        Err(_) => owned_string(Vec::new()),
    }
}

#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn __draton_std_io_write_file(path: DratonString, content: DratonString) -> bool {
    stdlib::fs::write(
        draton_string_to_owned(path),
        draton_string_to_owned(content),
    )
    .is_ok()
}

#[cfg(feature = "std-io")]
#[no_mangle]
pub extern "C" fn __draton_std_io_append_file(path: DratonString, content: DratonString) -> bool {
    stdlib::fs::append(
        draton_string_to_owned(path),
        draton_string_to_owned(content),
    )
    .is_ok()
}

#[cfg(feature = "std-io")]
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
#[cfg(feature = "host-compiler")]
#[no_mangle]
pub extern "C" fn draton_host_ast_dump(path: DratonString) -> DratonString {
    let path = draton_string_to_owned(path);
    match host_ast_dump_path(Path::new(&path)) {
        Ok(dump) => owned_string(dump.into_bytes()),
        Err(message) => owned_string(message.into_bytes()),
    }
}
