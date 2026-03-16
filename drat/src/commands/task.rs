use std::path::Path;

use anyhow::Result;

use crate::tooling::tasks;

pub(crate) fn run(cwd: &Path, task_name: Option<&str>) -> Result<()> {
    match task_name {
        Some(name) => tasks::run_named_task(cwd, name),
        None => {
            println!("{}", tasks::render_task_list(cwd)?);
            Ok(())
        }
    }
}
