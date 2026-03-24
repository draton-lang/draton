use std::path::Path;

use anyhow::Result;

use crate::config::DratonConfig;

pub(crate) fn run(project_root: &Path, pkg: &str) -> Result<()> {
    let mut config = DratonConfig::load(project_root)?;
    let (name, value) = parse_package(pkg);
    config.dependencies.insert(name, value);
    config.save(project_root)
}

fn parse_package(pkg: &str) -> (String, String) {
    let (base, version) = pkg
        .split_once('@')
        .map_or((pkg, "latest"), |(name, ver)| (name, ver));
    let name = base.rsplit('/').next().unwrap_or(base).to_string();
    let value = if base.contains("://") || base.contains("github.com/") {
        format!("{base}@{version}")
    } else if base.starts_with("stdlib@") {
        base.to_string()
    } else {
        format!("github.com/{base}@{version}")
    };
    (name, value)
}
