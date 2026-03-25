mod config;
mod fmt;
mod tooling;
mod commands {
    pub mod add;
    pub mod ast_dump;
    pub mod build;
    pub mod doc;
    pub mod fmt;
    pub mod init;
    pub mod lex_dump;
    pub mod lint;
    pub mod lsp;
    pub mod publish;
    pub mod remove;
    pub mod repl;
    pub mod run;
    pub mod selfhost_stage0;
    pub mod task;
    pub mod test;
    pub mod type_dump;
    pub mod update;
}

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::commands::build::{BuildRequest, Profile};

#[derive(Debug, Parser)]
#[command(name = "drat", version, about = "Draton compiler CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init { name: Option<String> },
    Build(BuildFlags),
    AstDump { path: PathBuf },
    TypeDump { path: PathBuf },
    LexDump { path: PathBuf },
    Run(RunFlags),
    Test,
    Fmt(FmtFlags),
    Lint(LintFlags),
    Task(TaskFlags),
    Doc,
    Lsp,
    Repl,
    Add { pkg: String },
    Remove { pkg: String },
    Update { subject: Option<String> },
    Publish,
    #[command(hide = true, name = "selfhost-stage0")]
    SelfhostStage0(SelfhostStage0Flags),
}

#[derive(Debug, Clone, Args)]
struct BuildFlags {
    input: Option<PathBuf>,
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    size: bool,
    #[arg(long)]
    fast: bool,
    #[arg(long)]
    target: Option<String>,
    #[arg(long = "strict-syntax", alias = "deny-deprecated-syntax")]
    strict_syntax: bool,
}

#[derive(Debug, Clone, Args)]
struct RunFlags {
    input: Option<PathBuf>,
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    size: bool,
    #[arg(long)]
    fast: bool,
    #[arg(long)]
    target: Option<String>,
    #[arg(long = "strict-syntax", alias = "deny-deprecated-syntax")]
    strict_syntax: bool,
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[derive(Debug, Clone, Args)]
struct FmtFlags {
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Clone, Args)]
struct LintFlags {
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct TaskFlags {
    name: Option<String>,
}

#[derive(Debug, Clone, Args)]
struct SelfhostStage0Flags {
    #[command(subcommand)]
    command: SelfhostStage0Subcommand,
}

#[derive(Debug, Clone, Subcommand)]
enum SelfhostStage0Subcommand {
    Lex {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Parse {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Typeck {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long = "strict-syntax", alias = "deny-deprecated-syntax")]
        strict_syntax: bool,
    },
    Build {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
        #[arg(long)]
        release: bool,
        #[arg(long)]
        size: bool,
        #[arg(long)]
        fast: bool,
        #[arg(long)]
        target: Option<String>,
        #[arg(long = "strict-syntax", alias = "deny-deprecated-syntax")]
        strict_syntax: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    match cli.command {
        Command::Init { name } => commands::init::run(&cwd, name.as_deref()),
        Command::Build(flags) => {
            let request = BuildRequest {
                profile: Profile::from_flags(flags.release, flags.size, flags.fast)?,
                target: flags.target.clone(),
                strict_syntax: flags.strict_syntax,
            };
            let output = match flags.input.as_deref() {
                Some(input) => {
                    commands::build::run_file(&cwd, input, flags.output.as_deref(), &request)?
                }
                None => commands::build::run(&cwd, &request)?,
            };
            println!("{}", output.binary_path.display());
            println!("{}", output.object_path.display());
            println!("{}", output.ir_path.display());
            Ok(())
        }
        Command::AstDump { path } => commands::ast_dump::run(&path),
        Command::TypeDump { path } => commands::type_dump::run(&path),
        Command::LexDump { path } => commands::lex_dump::run(&path),
        Command::Run(flags) => {
            let request = BuildRequest {
                profile: Profile::from_flags(flags.release, flags.size, flags.fast)?,
                target: flags.target,
                strict_syntax: flags.strict_syntax,
            };
            match flags.input.as_deref() {
                Some(input) => commands::run::run_file(
                    &cwd,
                    input,
                    flags.output.as_deref(),
                    &request,
                    &flags.args,
                ),
                None => commands::run::run(&cwd, &request, &flags.args),
            }
        }
        Command::Test => commands::test::run(&cwd),
        Command::Fmt(flags) => commands::fmt::run(&cwd, &flags.paths, flags.check),
        Command::Lint(flags) => commands::lint::run(&cwd, &flags.paths),
        Command::Task(flags) => commands::task::run(&cwd, flags.name.as_deref()),
        Command::Doc => commands::doc::run(&cwd),
        Command::Lsp => commands::lsp::run(),
        Command::Repl => commands::repl::run(),
        Command::Add { pkg } => commands::add::run(&cwd, &pkg),
        Command::Remove { pkg } => commands::remove::run(&cwd, &pkg),
        Command::Update { subject } => commands::update::run(&cwd, subject.as_deref()),
        Command::Publish => commands::publish::run(&cwd),
        Command::SelfhostStage0(flags) => {
            let command = match flags.command {
                SelfhostStage0Subcommand::Lex { path, json } => {
                    commands::selfhost_stage0::SelfhostStage0Command::Lex { path, json }
                }
                SelfhostStage0Subcommand::Parse { path, json } => {
                    commands::selfhost_stage0::SelfhostStage0Command::Parse { path, json }
                }
                SelfhostStage0Subcommand::Typeck {
                    path,
                    json,
                    strict_syntax,
                } => commands::selfhost_stage0::SelfhostStage0Command::Typeck {
                    path,
                    json,
                    strict_syntax,
                },
                SelfhostStage0Subcommand::Build {
                    path,
                    json,
                    output,
                    release,
                    size,
                    fast,
                    target,
                    strict_syntax,
                } => commands::selfhost_stage0::SelfhostStage0Command::Build {
                    path,
                    json,
                    output,
                    request: BuildRequest {
                        profile: Profile::from_flags(release, size, fast)?,
                        target,
                        strict_syntax,
                    },
                },
            };
            commands::selfhost_stage0::run(&cwd, command)
        }
    }
}
