use std::path::Path;
use std::process::{self, Command};

use anyhow::{bail, Context, Result};

use crate::commands::build::{self, BuildRequest};

pub(crate) fn run(project_root: &Path, request: &BuildRequest, args: &[String]) -> Result<()> {
    let output = build::run(project_root, request)?;
    let status = Command::new(&output.binary_path)
        .args(args)
        .status()
        .with_context(|| format!("khong the chay {}", output.binary_path.display()))?;
    if let Some(code) = status.code() {
        process::exit(code);
    }
    bail!("chuong trinh ket thuc bat thuong")
}
