use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Parsed `draton.toml` project configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DratonConfig {
    pub project: ProjectConfig,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub build: BuildConfig,
}

/// Project metadata loaded from `draton.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProjectConfig {
    pub name: String,
    pub version: String,
    #[serde(default = "default_entry")]
    pub entry: String,
}

/// Build defaults loaded from `draton.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct BuildConfig {
    pub target: Option<String>,
}

fn default_entry() -> String {
    "src/main.dt".to_string()
}

impl DratonConfig {
    pub(crate) fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join("draton.toml");
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub(crate) fn save(&self, project_root: &Path) -> Result<()> {
        let path = project_root.join("draton.toml");
        let source = toml::to_string_pretty(self).context("failed to serialize draton.toml")?;
        fs::write(&path, source).with_context(|| format!("failed to write {}", path.display()))
    }

    pub(crate) fn entry_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.project.entry)
    }

    pub(crate) fn default_target(&self) -> Option<&str> {
        self.build.target.as_deref()
    }
}

impl ProjectConfig {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
            entry: default_entry(),
        }
    }
}

impl Default for DratonConfig {
    fn default() -> Self {
        Self {
            project: ProjectConfig::new("my-app"),
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
            build: BuildConfig::default(),
        }
    }
}
