use std::path::Path;

use anyhow::Result;

use crate::config::DratonConfig;

pub(crate) fn run(project_root: &Path, pkg: &str) -> Result<()> {
    let mut config = DratonConfig::load(project_root)?;
    config.dependencies.remove(pkg);
    config.dev_dependencies.remove(pkg);
    config.save(project_root)
}
