use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use super::files::find_upwards;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TaskFile {
    #[serde(default)]
    pub tasks: BTreeMap<String, TaskDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TaskDef {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(rename = "run")]
    pub command: TaskCommand,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum TaskCommand {
    One(String),
    Many(Vec<String>),
}

impl TaskCommand {
    pub(crate) fn lines(&self) -> Vec<&str> {
        match self {
            Self::One(command) => vec![command.as_str()],
            Self::Many(commands) => commands.iter().map(String::as_str).collect(),
        }
    }
}

impl TaskFile {
    pub(crate) fn load(cwd: &Path) -> Result<(PathBuf, Self)> {
        let path = find_upwards(cwd, "drat.tasks")
            .ok_or_else(|| anyhow!("could not find drat.tasks from {}", cwd.display()))?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let tasks = toml::from_str(&text)
            .with_context(|| format!("cannot parse {}", path.display()))?;
        Ok((path, tasks))
    }
}

pub(crate) fn run_named_task(cwd: &Path, task_name: &str) -> Result<()> {
    let (path, task_file) = TaskFile::load(cwd)?;
    if task_file.tasks.is_empty() {
        bail!("{} does not define any tasks", path.display());
    }
    let project_root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    run_task_recursive(
        &project_root,
        &task_file.tasks,
        task_name,
        &mut active,
        &mut completed,
    )
}

pub(crate) fn render_task_list(cwd: &Path) -> Result<String> {
    let (path, task_file) = TaskFile::load(cwd)?;
    if task_file.tasks.is_empty() {
        return Ok(format!("{} defines no tasks", path.display()));
    }
    let mut lines = vec![format!("tasks in {}:", path.display())];
    for (name, task) in &task_file.tasks {
        match task.description.as_deref() {
            Some(description) => lines.push(format!("  {name:<16} {description}")),
            None => lines.push(format!("  {name}")),
        }
    }
    Ok(lines.join("\n"))
}

fn run_task_recursive(
    project_root: &Path,
    tasks: &BTreeMap<String, TaskDef>,
    task_name: &str,
    active: &mut BTreeSet<String>,
    completed: &mut BTreeSet<String>,
) -> Result<()> {
    if completed.contains(task_name) {
        return Ok(());
    }
    let Some(task) = tasks.get(task_name) else {
        bail!("task '{}' is not defined", task_name);
    };
    if !active.insert(task_name.to_string()) {
        bail!("task dependency cycle detected at '{}'", task_name);
    }

    for dependency in &task.deps {
        run_task_recursive(project_root, tasks, dependency, active, completed)?;
    }

    for command in task.command.lines() {
        run_shell_command(project_root, task, command)?;
    }

    active.remove(task_name);
    completed.insert(task_name.to_string());
    Ok(())
}

fn run_shell_command(project_root: &Path, task: &TaskDef, command: &str) -> Result<()> {
    let task_cwd = task
        .cwd
        .as_deref()
        .map(|value| project_root.join(value))
        .unwrap_or_else(|| project_root.to_path_buf());
    println!("$ {}", command);

    let mut child = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };

    let status = child
        .current_dir(&task_cwd)
        .envs(&task.env)
        .status()
        .with_context(|| format!("failed to run task command in {}", task_cwd.display()))?;
    if status.success() {
        Ok(())
    } else {
        bail!("task command failed: {}", command)
    }
}
