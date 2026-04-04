use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::commands::build::{self, BuildRequest, Profile};

#[derive(Debug, Clone)]
pub(crate) enum SelfhostStage0Command {
    Lex {
        path: PathBuf,
        json: bool,
    },
    Parse {
        path: PathBuf,
        json: bool,
    },
    Typeck {
        path: PathBuf,
        json: bool,
        strict_syntax: bool,
    },
    Build {
        path: PathBuf,
        json: bool,
        output: Option<PathBuf>,
        request: BuildRequest,
    },
}

pub(crate) fn run(cwd: &Path, command: SelfhostStage0Command) -> Result<()> {
    let stage0_binary = build_stage0_binary(&command)?;
    let pretty = wants_pretty_json(&command);
    let args = stage0_args(cwd, &command);
    let input_path = stage0_input_path(cwd, &command);
    let output = Command::new(&stage0_binary)
        .current_dir(cwd)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run {}", stage0_binary.display()))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut message = format!(
            "selfhost stage0 binary exited with {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string())
        );
        if !stderr.trim().is_empty() {
            message.push_str("\nstderr:\n");
            message.push_str(&stderr);
        }
        if !stdout.trim().is_empty() {
            message.push_str("\nstdout:\n");
            message.push_str(&stdout);
        }
        bail!(message);
    }

    let payload = parse_stage0_json(&output.stdout)?;
    let json = stage0_envelope(&command, &input_path, payload)?;
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}

fn parse_stage0_json(stdout: &[u8]) -> Result<Value> {
    let mut value: Value = serde_json::from_slice(stdout)
        .map_err(|error| anyhow!("selfhost stage0 returned invalid JSON: {error}"))?;
    for depth in 0..=2 {
        match value {
            Value::String(inner) => {
                let trimmed = inner.trim();
                if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                    bail!("selfhost stage0 returned a JSON string instead of an object/array");
                }
                value = serde_json::from_str(trimmed).map_err(|error| {
                    anyhow!(
                        "selfhost stage0 returned nested JSON string at depth {} that failed to parse: {}",
                        depth + 1,
                        error
                    )
                })?;
            }
            other => return Ok(other),
        }
    }

    bail!("selfhost stage0 returned excessively nested JSON string payloads")
}

fn stage0_envelope(
    command: &SelfhostStage0Command,
    input_path: &Path,
    payload: Value,
) -> Result<Value> {
    let (success, result) = match command {
        SelfhostStage0Command::Lex { .. } => normalize_lex_payload(payload)?,
        SelfhostStage0Command::Parse { .. } => normalize_parse_payload(payload)?,
        SelfhostStage0Command::Typeck { .. } => normalize_typeck_payload(payload)?,
        SelfhostStage0Command::Build { .. } => normalize_build_payload(payload)?,
    };
    Ok(json!({
        "schema": "draton.selfhost.stage0/v1",
        "stage": stage_name(command),
        "input_path": input_path.display().to_string(),
        "bridge": stage0_bridge(command),
        "success": success,
        "result": result,
        "error": Value::Null,
    }))
}

fn normalize_lex_payload(payload: Value) -> Result<(bool, Value)> {
    let payload = expect_object(payload, "lex payload")?;
    let tokens = expect_array_field(&payload, "lex", "tokens")?;
    let errors = expect_array_field(&payload, "lex", "errors")?;
    let success = errors.is_empty();
    Ok((
        success,
        json!({
            "tokens": tokens,
            "errors": errors,
        }),
    ))
}

fn normalize_parse_payload(payload: Value) -> Result<(bool, Value)> {
    let payload = expect_object(payload, "parse payload")?;
    let lex_errors = expect_array_field(&payload, "parse", "lex_errors")?;
    let parse_result = optional_object_field(&payload, "parse", "parse_result")?;
    let (parse_errors, parse_warnings, program) = match parse_result {
        Some(parse_result) => (
            expect_array_field(&parse_result, "parse", "parse_result.errors")?,
            expect_array_field(&parse_result, "parse", "parse_result.warnings")?,
            expect_optional_field(&parse_result, "program"),
        ),
        None => (Vec::new(), Vec::new(), Value::Null),
    };
    let success = lex_errors.is_empty() && parse_errors.is_empty();
    Ok((
        success,
        json!({
            "lex_errors": lex_errors,
            "parse_errors": parse_errors,
            "parse_warnings": parse_warnings,
            "program": program,
        }),
    ))
}

fn normalize_typeck_payload(payload: Value) -> Result<(bool, Value)> {
    let payload = expect_object(payload, "typeck payload")?;
    let lex_errors = expect_array_field(&payload, "typeck", "lex_errors")?;
    let parse_errors = expect_array_field(&payload, "typeck", "parse_errors")?;
    let parse_warnings = expect_array_field(&payload, "typeck", "parse_warnings")?;
    let typecheck_result = optional_object_field(&payload, "typeck", "typecheck_result")?;
    let (type_errors, type_warnings, typed_program) = match typecheck_result {
        Some(typecheck_result) => (
            expect_array_field(&typecheck_result, "typeck", "typecheck_result.errors")?,
            expect_array_field(&typecheck_result, "typeck", "typecheck_result.warnings")?,
            expect_optional_field(&typecheck_result, "typed_program"),
        ),
        None => (Vec::new(), Vec::new(), Value::Null),
    };
    let success = lex_errors.is_empty() && parse_errors.is_empty() && type_errors.is_empty();
    Ok((
        success,
        json!({
            "lex_errors": lex_errors,
            "parse_errors": parse_errors,
            "parse_warnings": parse_warnings,
            "type_errors": type_errors,
            "type_warnings": type_warnings,
            "typed_program": typed_program,
        }),
    ))
}

fn normalize_build_payload(payload: Value) -> Result<(bool, Value)> {
    let payload = expect_object(payload, "build payload")?;
    let success = expect_bool_field(&payload, "build", "ok")?;
    let output = match payload.get("output") {
        Some(Value::Object(_)) => payload.get("output").cloned().unwrap_or(Value::Null),
        Some(Value::Null) | None => Value::Null,
        Some(other) => bail!(
            "selfhost stage0 build contract break: expected output to be object or null, found {}",
            json_kind(other)
        ),
    };
    let error = match payload.get("error") {
        Some(Value::Null) | None => Value::Null,
        Some(Value::String(message)) => json!({
            "kind": "build_failed",
            "message": message,
        }),
        Some(other) => bail!(
            "selfhost stage0 build contract break: expected error to be string or null, found {}",
            json_kind(other)
        ),
    };
    if success && output.is_null() {
        bail!("selfhost stage0 build contract break: ok=true requires output payload");
    }
    if success && !error.is_null() {
        bail!("selfhost stage0 build contract break: ok=true requires error=null");
    }
    if !success && error.is_null() {
        bail!("selfhost stage0 build contract break: ok=false requires error payload");
    }
    Ok((
        success,
        json!({
            "output": output,
            "error": error,
        }),
    ))
}

fn expect_object(value: Value, context: &str) -> Result<serde_json::Map<String, Value>> {
    match value {
        Value::Object(object) => Ok(object),
        other => bail!(
            "selfhost stage0 contract break in {context}: expected object, found {}",
            json_kind(&other)
        ),
    }
}

fn expect_array_field(
    object: &serde_json::Map<String, Value>,
    stage: &str,
    field: &str,
) -> Result<Vec<Value>> {
    match object.get(field) {
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(other) => bail!(
            "selfhost stage0 {stage} contract break: expected {field} to be array, found {}",
            json_kind(other)
        ),
        None => bail!("selfhost stage0 {stage} contract break: missing {field}"),
    }
}

fn expect_bool_field(
    object: &serde_json::Map<String, Value>,
    stage: &str,
    field: &str,
) -> Result<bool> {
    match object.get(field) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(other) => bail!(
            "selfhost stage0 {stage} contract break: expected {field} to be bool, found {}",
            json_kind(other)
        ),
        None => bail!("selfhost stage0 {stage} contract break: missing {field}"),
    }
}

fn optional_object_field(
    object: &serde_json::Map<String, Value>,
    stage: &str,
    field: &str,
) -> Result<Option<serde_json::Map<String, Value>>> {
    match object.get(field) {
        Some(Value::Object(value)) => Ok(Some(value.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(other) => bail!(
            "selfhost stage0 {stage} contract break: expected {field} to be object or null, found {}",
            json_kind(other)
        ),
    }
}

fn expect_optional_field(object: &serde_json::Map<String, Value>, field: &str) -> Value {
    object.get(field).cloned().unwrap_or(Value::Null)
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn stage_name(command: &SelfhostStage0Command) -> &'static str {
    match command {
        SelfhostStage0Command::Lex { .. } => "lex",
        SelfhostStage0Command::Parse { .. } => "parse",
        SelfhostStage0Command::Typeck { .. } => "typeck",
        SelfhostStage0Command::Build { .. } => "build",
    }
}

fn stage0_bridge(command: &SelfhostStage0Command) -> Value {
    match command {
        SelfhostStage0Command::Lex { .. } => json!({
            "kind": "selfhost",
            "builtin": Value::Null,
        }),
        SelfhostStage0Command::Parse { .. } => json!({
            "kind": "host",
            "builtin": "host_parse_json",
        }),
        SelfhostStage0Command::Typeck { .. } => json!({
            "kind": "host",
            "builtin": "host_type_json",
        }),
        SelfhostStage0Command::Build { .. } => json!({
            "kind": "host",
            "builtin": "host_build_json",
        }),
    }
}

fn wants_pretty_json(command: &SelfhostStage0Command) -> bool {
    match command {
        SelfhostStage0Command::Lex { json, .. }
        | SelfhostStage0Command::Parse { json, .. }
        | SelfhostStage0Command::Typeck { json, .. }
        | SelfhostStage0Command::Build { json, .. } => *json,
    }
}

fn stage0_args(cwd: &Path, command: &SelfhostStage0Command) -> Vec<String> {
    match command {
        SelfhostStage0Command::Lex { path, .. } => {
            vec![
                "lex".to_string(),
                resolve_path(cwd, path).display().to_string(),
            ]
        }
        SelfhostStage0Command::Parse { path, .. } => {
            vec![
                "parse".to_string(),
                resolve_path(cwd, path).display().to_string(),
            ]
        }
        SelfhostStage0Command::Typeck {
            path,
            strict_syntax,
            ..
        } => vec![
            "typeck".to_string(),
            resolve_path(cwd, path).display().to_string(),
            bool_flag(*strict_syntax),
        ],
        SelfhostStage0Command::Build {
            path,
            output,
            request,
            ..
        } => vec![
            "build".to_string(),
            resolve_path(cwd, path).display().to_string(),
            output
                .as_ref()
                .map(|value| resolve_path(cwd, value).display().to_string())
                .unwrap_or_default(),
            build_mode_name(request.profile).to_string(),
            bool_flag(request.strict_syntax),
            request.target.clone().unwrap_or_default(),
        ],
    }
}

fn stage0_input_path(cwd: &Path, command: &SelfhostStage0Command) -> PathBuf {
    match command {
        SelfhostStage0Command::Lex { path, .. }
        | SelfhostStage0Command::Parse { path, .. }
        | SelfhostStage0Command::Typeck { path, .. }
        | SelfhostStage0Command::Build { path, .. } => resolve_path(cwd, path),
    }
}

struct Stage0Layout {
    cache_key: &'static str,
    entries: &'static [&'static str],
}

fn stage0_layout(command: &SelfhostStage0Command) -> Stage0Layout {
    match command {
        _ if env::var_os("DRATON_SELFHOST_STAGE0_MINIMAL").is_some() => Stage0Layout {
            cache_key: "minimal",
            entries: &["main.dt", "driver/pipeline.dt"],
        },
        SelfhostStage0Command::Lex { .. } => Stage0Layout {
            cache_key: "lex",
            entries: &["main.dt", "driver/pipeline.dt"],
        },
        SelfhostStage0Command::Parse { .. }
        | SelfhostStage0Command::Typeck { .. }
        | SelfhostStage0Command::Build { .. } => Stage0Layout {
            cache_key: "semantic",
            entries: &["main.dt", "driver", "ast", "lexer", "parser", "typeck"],
        },
    }
}

fn build_stage0_binary(command: &SelfhostStage0Command) -> Result<PathBuf> {
    let repo_root = repo_root();
    let compiler_root = repo_root.join("compiler");
    let compiler_entry = compiler_root.join("main.dt");
    let pipeline_src = compiler_root.join("driver").join("pipeline.dt");
    let layout = stage0_layout(command);
    if !compiler_entry.exists() {
        bail!(
            "selfhost entrypoint not found at {}",
            compiler_entry.display()
        );
    }
    if !pipeline_src.exists() {
        bail!("selfhost pipeline not found at {}", pipeline_src.display());
    }

    let temp_root = stage0_cache_root(&compiler_root, &layout)?;
    let cached_binary = temp_root
        .join("build")
        .join("debug")
        .join(stage0_binary_name());
    if cached_binary.exists() {
        return Ok(cached_binary);
    }
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to reset {}", temp_root.display()))?;
    }
    copy_compiler_entries(&compiler_root, &temp_root, &layout)?;
    let config_path = temp_root.join("draton.toml");
    fs::write(
        &config_path,
        "[project]\nname = \"draton-selfhost-stage0\"\nversion = \"0.1.0\"\nentry = \"main.dt\"\n",
    )
    .with_context(|| format!("failed to write {}", config_path.display()))?;

    let built = build::run(
        &temp_root,
        &BuildRequest {
            profile: Profile::Debug,
            target: None,
            strict_syntax: false,
        },
    )?;
    Ok(built.binary_path)
}

fn stage0_cache_root(compiler_root: &Path, layout: &Stage0Layout) -> Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    "draton-selfhost-stage0".hash(&mut hasher);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    layout.cache_key.hash(&mut hasher);
    for relative in layout.entries {
        hash_compiler_path(compiler_root, &compiler_root.join(relative), &mut hasher)?;
    }
    Ok(std::env::temp_dir()
        .join("draton")
        .join("selfhost_stage0_cache")
        .join(format!("{:016x}", hasher.finish())))
}

fn hash_compiler_path(root: &Path, current: &Path, hasher: &mut DefaultHasher) -> Result<()> {
    if !current.exists() {
        bail!("missing compiler dependency {}", current.display());
    }
    let relative = current
        .strip_prefix(root)
        .with_context(|| format!("failed to relativize {}", current.display()))?;
    relative.hash(hasher);
    if current.is_file() {
        fs::read(current)
            .with_context(|| format!("failed to read {}", current.display()))?
            .hash(hasher);
        return Ok(());
    }
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read {}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to iterate {}", current.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {}", path.display()))?;
        if file_type.is_dir() {
            hash_compiler_path(root, &path, hasher)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .with_context(|| format!("failed to relativize {}", path.display()))?;
            relative.hash(hasher);
            fs::read(&path)
                .with_context(|| format!("failed to read {}", path.display()))?
                .hash(hasher);
        }
    }
    Ok(())
}

fn copy_compiler_entries(
    compiler_root: &Path,
    temp_root: &Path,
    layout: &Stage0Layout,
) -> Result<()> {
    fs::create_dir_all(temp_root)
        .with_context(|| format!("failed to create {}", temp_root.display()))?;
    for relative in layout.entries {
        let source = compiler_root.join(relative);
        let dest = temp_root.join(relative);
        if source.is_dir() {
            copy_directory_recursive(&source, &dest)?;
        } else if source.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::copy(&source, &dest).with_context(|| {
                format!(
                    "failed to copy {} into {}",
                    source.display(),
                    dest.display()
                )
            })?;
        } else {
            bail!("missing compiler dependency {}", source.display());
        }
    }
    Ok(())
}

fn copy_directory_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("failed to create {}", dest.display()))?;
    let mut entries = fs::read_dir(source)
        .with_context(|| format!("failed to read {}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to iterate {}", source.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {}", source_path.display()))?;
        if file_type.is_dir() {
            copy_directory_recursive(&source_path, &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} into {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn stage0_binary_name() -> &'static str {
    if cfg!(windows) {
        "draton-selfhost-stage0.exe"
    } else {
        "draton-selfhost-stage0"
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn bool_flag(value: bool) -> String {
    if value {
        return "1".to_string();
    }
    "0".to_string()
}

fn build_mode_name(profile: Profile) -> &'static str {
    match profile {
        Profile::Debug => "debug",
        Profile::Release => "release",
        Profile::Size => "size",
        Profile::Fast => "fast",
    }
}
