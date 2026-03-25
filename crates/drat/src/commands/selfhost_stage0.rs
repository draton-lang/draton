use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use draton_lexer::{LexError, LexResult, Lexer};
use draton_parser::{ParseError, ParseResult, Parser};
use draton_typeck::{DeprecatedSyntaxMode, TypeCheckResult, TypeChecker};
use serde::Serialize;

use crate::commands::build::{self, BuildOutput, BuildRequest};

#[derive(Debug, Clone)]
pub(crate) enum SelfhostStage0Command {
    Lex { path: PathBuf, json: bool },
    Parse { path: PathBuf, json: bool },
    Typeck {
        path: PathBuf,
        json: bool,
        strict_syntax: bool,
    },
    Build {
        path: PathBuf,
        json: bool,
        output: Option<PathBuf>,
        request: BuildRequest,
    },
}

#[derive(Debug, Serialize)]
struct ParseEnvelope {
    lex_errors: Vec<LexError>,
    parse_result: Option<ParseResult>,
}

#[derive(Debug, Serialize)]
struct TypecheckEnvelope {
    lex_errors: Vec<LexError>,
    parse_errors: Vec<ParseError>,
    parse_warnings: Vec<draton_parser::ParseWarning>,
    typecheck_result: Option<TypeCheckResult>,
}

#[derive(Debug, Serialize)]
struct BuildEnvelope {
    ok: bool,
    output: Option<SerializableBuildOutput>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SerializableBuildOutput {
    binary_path: String,
    object_path: String,
    ir_path: String,
}

pub(crate) fn run(cwd: &Path, command: SelfhostStage0Command) -> Result<()> {
    match command {
        SelfhostStage0Command::Lex { path, json } => {
            let lexed = lex_path(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&lexed)?);
            } else {
                println!("{}", serde_json::to_string(&lexed)?);
            }
            Ok(())
        }
        SelfhostStage0Command::Parse { path, json } => {
            let source = read_source(&path)?;
            let lexed = Lexer::new(&source).tokenize();
            let parse_result = if lexed.errors.is_empty() {
                Some(Parser::new(lexed.tokens.clone()).parse())
            } else {
                None
            };
            let envelope = ParseEnvelope {
                lex_errors: lexed.errors,
                parse_result,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("{}", serde_json::to_string(&envelope)?);
            }
            Ok(())
        }
        SelfhostStage0Command::Typeck {
            path,
            json,
            strict_syntax,
        } => {
            let source = read_source(&path)?;
            let lexed = Lexer::new(&source).tokenize();
            let mut parse_errors = Vec::new();
            let mut parse_warnings = Vec::new();
            let mut typecheck_result = None;
            if lexed.errors.is_empty() {
                let parsed = Parser::new(lexed.tokens.clone()).parse();
                parse_errors = parsed.errors.clone();
                parse_warnings = parsed.warnings.clone();
                if parse_errors.is_empty() {
                    typecheck_result = Some(
                        TypeChecker::new()
                            .with_deprecated_syntax_mode(if strict_syntax {
                                DeprecatedSyntaxMode::Deny
                            } else {
                                DeprecatedSyntaxMode::Warn
                            })
                            .check(parsed.program),
                    );
                }
            }
            let envelope = TypecheckEnvelope {
                lex_errors: lexed.errors,
                parse_errors,
                parse_warnings,
                typecheck_result,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("{}", serde_json::to_string(&envelope)?);
            }
            Ok(())
        }
        SelfhostStage0Command::Build {
            path,
            json,
            output,
            request,
        } => {
            let result = build::run_file(cwd, &path, output.as_deref(), &request);
            let envelope = match result {
                Ok(output) => BuildEnvelope {
                    ok: true,
                    output: Some(serialize_build_output(output)),
                    error: None,
                },
                Err(error) => BuildEnvelope {
                    ok: false,
                    output: None,
                    error: Some(error.to_string()),
                },
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("{}", serde_json::to_string(&envelope)?);
            }
            Ok(())
        }
    }
}

fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn lex_path(path: &Path) -> Result<LexResult> {
    let source = read_source(path)?;
    Ok(Lexer::new(&source).tokenize())
}

fn serialize_build_output(output: BuildOutput) -> SerializableBuildOutput {
    SerializableBuildOutput {
        binary_path: output.binary_path.display().to_string(),
        object_path: output.object_path.display().to_string(),
        ir_path: output.ir_path.display().to_string(),
    }
}
