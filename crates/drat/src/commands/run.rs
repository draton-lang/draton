use std::path::Path;
use std::process::{self, Command};

use anyhow::{bail, Context, Result};

use crate::commands::build::{self, BuildRequest};

pub(crate) fn run(project_root: &Path, request: &BuildRequest, args: &[String]) -> Result<()> {
    let output = build::run(project_root, request)?;
    run_binary(&output.binary_path, args)
}

pub(crate) fn run_file(
    cwd: &Path,
    input_path: &Path,
    output_path: Option<&Path>,
    request: &BuildRequest,
    args: &[String],
) -> Result<()> {
    let output = build::run_file(cwd, input_path, output_path, request)?;
    run_binary(&output.binary_path, args)
}

fn run_binary(binary_path: &Path, args: &[String]) -> Result<()> {
    let status = Command::new(binary_path)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {}", binary_path.display()))?;
    if let Some(code) = status.code() {
        process::exit(code);
    }
    bail!("program terminated abnormally")
}
