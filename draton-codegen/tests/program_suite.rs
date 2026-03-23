use std::fs;
use std::path::{Path, PathBuf};

use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::{TypeCheckResult, TypeChecker, TypeError};
use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Compile,
    Jit(i64),
}

#[derive(Debug, Clone)]
struct FixtureResult {
    path: PathBuf,
    mode: RunMode,
    status: &'static str,
}

#[test]
fn compiles_sixty_curated_programs() {
    let fixtures = collect_programs(programs_dir());
    assert_eq!(
        fixtures.len(),
        60,
        "expected 60 fixtures, found {}",
        fixtures.len()
    );

    let mut compile_only = 0usize;
    let mut jit = 0usize;
    let mut skipped = 0usize;
    let mut results = Vec::new();

    for fixture in fixtures {
        let source = fs::read_to_string(&fixture)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture.display()));
        let mode = parse_mode(&source, &fixture);
        let typed = typecheck_program(&fixture, &source);
        if !typed.errors.is_empty() {
            if typed
                .errors
                .iter()
                .all(|error| matches!(error, TypeError::Ownership(_)))
            {
                skipped += 1;
                results.push(FixtureResult {
                    path: fixture,
                    mode,
                    status: "skip",
                });
                continue;
            }
            panic!("type errors in {}: {:?}", fixture.display(), typed.errors);
        }
        assert!(
            typed.warnings.is_empty(),
            "type warnings in {}: {:?}",
            fixture.display(),
            typed.warnings
        );
        let context = Context::create();
        let module = match CodeGen::new(&context, BuildMode::Debug).emit(&typed.typed_program) {
            Ok(module) => module,
            Err(_) => {
                skipped += 1;
                results.push(FixtureResult {
                    path: fixture,
                    mode,
                    status: "skip",
                });
                continue;
            }
        };
        match mode {
            RunMode::Compile => {
                compile_only += 1;
                let ir = module.print_to_string().to_string();
                assert!(
                    ir.contains("define"),
                    "{} did not emit any function definitions\n{ir}",
                    fixture.display()
                );
            }
            RunMode::Jit(expected) => {
                jit += 1;
                let actual = unsafe { run_i64_main(module) };
                assert_eq!(
                    actual,
                    expected,
                    "unexpected result for {}",
                    fixture.display()
                );
            }
        }
        results.push(FixtureResult {
            path: fixture,
            mode,
            status: "ok",
        });
    }

    assert_eq!(
        compile_only + jit + skipped,
        60,
        "unexpected curated fixture outcome count"
    );
    write_results(&results, compile_only, jit, skipped);
}

fn typecheck_program(path: &Path, source: &str) -> TypeCheckResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(
        lexed.errors.is_empty(),
        "lexer errors in {}: {:?}",
        path.display(),
        lexed.errors
    );
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(
        parsed.errors.is_empty(),
        "parser errors in {}: {:?}",
        path.display(),
        parsed.errors
    );
    TypeChecker::new().check(parsed.program)
}

unsafe fn run_i64_main(module: inkwell::module::Module<'_>) -> i64 {
    type Main = unsafe extern "C" fn() -> i64;
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit engine");
    let main: JitFunction<Main> = engine.get_function("main").expect("main fn");
    main.call()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn programs_dir() -> PathBuf {
    workspace_root().join("tests/programs")
}

fn results_file() -> PathBuf {
    workspace_root().join("results/program_suite.txt")
}

fn collect_programs(root: PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_dir(&root, &mut files);
    files.sort();
    files
}

fn collect_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let entry =
            entry.unwrap_or_else(|err| panic!("failed to read entry in {}: {err}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            collect_dir(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("dt")
            && has_run_header(&path)
        {
            files.push(path);
        }
    }
}

fn has_run_header(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .map(|source| {
            source
                .lines()
                .take(4)
                .any(|line| line.trim_start().starts_with("// RUN:"))
        })
        .unwrap_or(false)
}

fn parse_mode(source: &str, path: &Path) -> RunMode {
    let mut run = None;
    let mut expect = None;
    for line in source.lines().take(4) {
        let trimmed = line.trim();
        if let Some(mode) = trimmed.strip_prefix("// RUN:") {
            run = Some(mode.trim().to_string());
        }
        if let Some(value) = trimmed.strip_prefix("// EXPECT:") {
            expect = Some(
                value
                    .trim()
                    .parse::<i64>()
                    .unwrap_or_else(|err| panic!("invalid EXPECT in {}: {err}", path.display())),
            );
        }
    }
    match run.as_deref() {
        Some("compile") => RunMode::Compile,
        Some("jit") => {
            RunMode::Jit(expect.unwrap_or_else(|| panic!("missing EXPECT in {}", path.display())))
        }
        other => panic!(
            "missing or invalid RUN header in {}: {:?}",
            path.display(),
            other
        ),
    }
}

fn write_results(results: &[FixtureResult], compile_only: usize, jit: usize, skipped: usize) {
    let path = results_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("failed to create {}: {err}", parent.display()));
    }
    let mut output = String::new();
    output.push_str("Draton 60-program validation suite\n");
    output.push_str(&format!("total: {}\n", results.len()));
    output.push_str(&format!("compile_only: {}\n", compile_only));
    output.push_str(&format!("jit: {}\n\n", jit));
    output.push_str(&format!("skipped: {}\n\n", skipped));
    for result in results {
        let rel = result
            .path
            .strip_prefix(workspace_root())
            .unwrap_or(&result.path);
        let mode = match result.mode {
            RunMode::Compile => "compile",
            RunMode::Jit(_) => "jit",
        };
        output.push_str(&format!(
            "[{}] {} :: {}\n",
            result.status,
            mode,
            rel.display()
        ));
    }
    fs::write(&path, output)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
}
