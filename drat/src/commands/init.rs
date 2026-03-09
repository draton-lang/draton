use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::{DratonConfig, ProjectConfig};

pub(crate) fn run(cwd: &Path, name: Option<&str>) -> Result<()> {
    let project_name = name
        .map(ToOwned::to_owned)
        .or_else(|| {
            cwd.file_name()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "my-app".to_string());
    let project_root = if let Some(name) = name {
        cwd.join(name)
    } else {
        cwd.to_path_buf()
    };
    scaffold(&project_root, &project_name)
}

fn scaffold(project_root: &Path, project_name: &str) -> Result<()> {
    if project_root.exists() {
        let config_path = project_root.join("draton.toml");
        let main_path = project_root.join("src/main.dt");
        if config_path.exists() || main_path.exists() {
            bail!("du an da ton tai o {}", project_root.display());
        }
    }

    fs::create_dir_all(project_root.join("src"))
        .with_context(|| format!("khong the tao {}", project_root.join("src").display()))?;

    let config = DratonConfig {
        project: ProjectConfig::new(project_name),
        ..DratonConfig::default()
    };
    config.save(project_root)?;

    fs::write(
        project_root.join("src/main.dt"),
        format!("fn main() {{\n    print(\"hello from {project_name}!\")\n}}\n"),
    )
    .with_context(|| {
        format!(
            "khong the ghi {}",
            project_root.join("src/main.dt").display()
        )
    })?;

    fs::write(project_root.join(".gitignore"), "build/\n.drat/\n").with_context(|| {
        format!(
            "khong the ghi {}",
            project_root.join(".gitignore").display()
        )
    })?;

    Ok(())
}

#[allow(dead_code)]
fn _project_root(cwd: &Path, name: Option<&str>) -> PathBuf {
    name.map_or_else(|| cwd.to_path_buf(), |name| cwd.join(name))
}
