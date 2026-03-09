use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::commands::build::{self, BuildRequest};

pub(crate) fn run(project_root: &Path, request: &BuildRequest, args: &[String]) -> Result<()> {
    let output = build::run(project_root, request)?;
    let status = Command::new(&output.binary_path)
        .args(args)
        .status()
        .with_context(|| format!("khong the chay {}", output.binary_path.display()))?;
    if !status.success() {
        bail!("chuong trinh thoat voi ma {:?}", status.code());
    }
    Ok(())
}
