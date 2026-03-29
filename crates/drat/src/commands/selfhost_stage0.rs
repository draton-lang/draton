use std::collections::hash_map::DefaultHasher;
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
    let stage0_binary = build_stage0_binary()?;
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

fn build_stage0_binary() -> Result<PathBuf> {
    let repo_root = repo_root();
    let compiler_root = repo_root.join("compiler");
    let compiler_entry = compiler_root.join("main.dt");
    let pipeline_src = compiler_root.join("driver").join("pipeline.dt");
    if !compiler_entry.exists() {
        bail!(
            "selfhost entrypoint not found at {}",
            compiler_entry.display()
        );
    }
    if !pipeline_src.exists() {
        bail!("selfhost pipeline not found at {}", pipeline_src.display());
    }

    let temp_root = stage0_cache_root(&compiler_entry, &pipeline_src)?;
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
    let temp_driver = temp_root.join("driver");
    fs::create_dir_all(&temp_driver)
        .with_context(|| format!("failed to create {}", temp_driver.display()))?;
    fs::copy(&compiler_entry, temp_root.join("main.dt")).with_context(|| {
        format!(
            "failed to copy {} into {}",
            compiler_entry.display(),
            temp_root.display()
        )
    })?;
    fs::copy(&pipeline_src, temp_driver.join("pipeline.dt")).with_context(|| {
        format!(
            "failed to copy {} into {}",
            pipeline_src.display(),
            temp_driver.display()
        )
    })?;
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

fn stage0_cache_root(compiler_entry: &Path, pipeline_src: &Path) -> Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    "draton-selfhost-stage0".hash(&mut hasher);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    fs::read(compiler_entry)
        .with_context(|| format!("failed to read {}", compiler_entry.display()))?
        .hash(&mut hasher);
    fs::read(pipeline_src)
        .with_context(|| format!("failed to read {}", pipeline_src.display()))?
        .hash(&mut hasher);
    Ok(std::env::temp_dir()
        .join("draton")
        .join("selfhost_stage0_cache")
        .join(format!("{:016x}", hasher.finish())))
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
