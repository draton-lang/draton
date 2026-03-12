mod config;
mod fmt;
mod commands {
    pub mod add;
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
    pub mod test;
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
    LexDump { path: PathBuf },
    Run(RunFlags),
    Test,
    Fmt,
    Lint,
    Doc,
    Lsp,
    Repl,
    Add { pkg: String },
    Remove { pkg: String },
    Update { subject: Option<String> },
    Publish,
}

#[derive(Debug, Clone, Args)]
struct BuildFlags {
    #[arg(long)]
    release: bool,
    #[arg(long)]
    size: bool,
    #[arg(long)]
    fast: bool,
    #[arg(long)]
    target: Option<String>,
}

#[derive(Debug, Clone, Args)]
struct RunFlags {
    #[command(flatten)]
    build: BuildFlags,
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    match cli.command {
        Command::Init { name } => commands::init::run(&cwd, name.as_deref()),
        Command::Build(flags) => {
            let request = BuildRequest {
                profile: Profile::from_flags(flags.release, flags.size, flags.fast)?,
                target: flags.target,
            };
            let output = commands::build::run(&cwd, &request)?;
            println!("{}", output.binary_path.display());
            println!("{}", output.object_path.display());
            println!("{}", output.ir_path.display());
            Ok(())
        }
        Command::LexDump { path } => commands::lex_dump::run(&path),
        Command::Run(flags) => {
            let request = BuildRequest {
                profile: Profile::from_flags(
                    flags.build.release,
                    flags.build.size,
                    flags.build.fast,
                )?,
                target: flags.build.target,
            };
            commands::run::run(&cwd, &request, &flags.args)
        }
        Command::Test => commands::test::run(&cwd),
        Command::Fmt => commands::fmt::run(&cwd),
        Command::Lint => commands::lint::run(&cwd),
        Command::Doc => commands::doc::run(&cwd),
        Command::Lsp => commands::lsp::run(),
        Command::Repl => commands::repl::run(),
        Command::Add { pkg } => commands::add::run(&cwd, &pkg),
        Command::Remove { pkg } => commands::remove::run(&cwd, &pkg),
        Command::Update { subject } => commands::update::run(&cwd, subject.as_deref()),
        Command::Publish => commands::publish::run(&cwd),
    }
}
