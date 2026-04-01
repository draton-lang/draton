use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

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

    let json: Value = parse_stage0_json(&output.stdout)?;
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}

fn parse_stage0_json(stdout: &[u8]) -> Result<Value> {
    match serde_json::from_slice(stdout) {
        Ok(json) => Ok(json),
        Err(primary_error) => {
            let normalized = normalize_stage0_json(stdout);
            serde_json::from_slice(&normalized).map_err(|normalized_error| {
                anyhow!(
                    "selfhost stage0 returned invalid JSON: {primary_error}; normalized parse also failed: {normalized_error}"
                )
            })
        }
    }
}

fn normalize_stage0_json(stdout: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(stdout.len());
    let mut index = 0;
    while index < stdout.len() {
        if stdout[index] == b'\\' && index + 1 < stdout.len() && stdout[index + 1] == b'"' {
            normalized.push(b'"');
            index += 2;
        } else {
            normalized.push(stdout[index]);
            index += 1;
        }
    }
    normalized
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
