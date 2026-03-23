use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::typed_ast::{
    TypedBlock, TypedElseBranch, TypedExpr, TypedExprKind, TypedItem, TypedMatchArmBody,
    TypedProgram, TypedSpawnBody, TypedStmtKind,
};
use draton_typeck::{TypeCheckResult, TypeChecker, TypeError};
use inkwell::context::Context;

const SKIPPED_FIXTURES: &[&str] = &[
    "tests/programs/compile/37_inheritance_field.dt",
    "tests/programs/compile/53_lambda_apply.dt",
    "tests/programs/jit/23_nullish_result.dt",
    "tests/programs/jit/24_result_match.dt",
];

#[test]
fn passing_corpus_programs_remain_ownership_clean_and_gc_free() {
    let fixtures = collect_programs(programs_dir())
        .into_iter()
        .filter(|path| {
            let rel = path
                .strip_prefix(workspace_root())
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            !SKIPPED_FIXTURES.contains(&rel.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(fixtures.len(), 56, "expected 56 passing fixtures");

    for fixture in fixtures {
        let source = fs::read_to_string(&fixture)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture.display()));
        let typed = typecheck_program(&fixture, &source);
        assert!(
            typed.errors.is_empty(),
            "unexpected type or ownership errors in {}: {:?}",
            fixture.display(),
            typed.errors
        );
        assert!(
            typed.warnings.is_empty(),
            "unexpected warnings in {}: {:?}",
            fixture.display(),
            typed.warnings
        );

        let expects_malloc = program_contains_malloc_lowered_alloc(&typed.typed_program);
        let context = Context::create();
        let module = CodeGen::new(&context, BuildMode::Debug)
            .emit(&typed.typed_program)
            .unwrap_or_else(|err| panic!("codegen failed for {}: {err}", fixture.display()));
        let ir = module.print_to_string().to_string();
        assert!(
            !ir.contains("draton_gc_alloc"),
            "GC alloc surfaced again in {}\n{ir}",
            fixture.display()
        );
        if expects_malloc {
            assert!(
                ir.contains("call i8* @malloc"),
                "expected malloc-backed allocation in {}\n{ir}",
                fixture.display()
            );
        }
    }
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
    assert!(
        parsed.warnings.is_empty(),
        "parser warnings in {}: {:?}",
        path.display(),
        parsed.warnings
    );
    let typed = TypeChecker::new().check(parsed.program);
    assert!(
        !typed.errors
            .iter()
            .any(|error| matches!(error, TypeError::Ownership(_))),
        "ownership errors in {}: {:?}",
        path.display(),
        typed.errors
    );
    typed
}

fn program_contains_malloc_lowered_alloc(program: &TypedProgram) -> bool {
    let class_names = program
        .items
        .iter()
        .filter_map(|item| match item {
            TypedItem::Class(class_def) => Some(class_def.name.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    program
        .items
        .iter()
        .any(|item| item_contains_malloc_lowered_alloc(item, &class_names))
}

fn item_contains_malloc_lowered_alloc(item: &TypedItem, class_names: &HashSet<String>) -> bool {
    match item {
        TypedItem::Fn(function)
        | TypedItem::PanicHandler(function)
        | TypedItem::OomHandler(function) => function
            .body
            .as_ref()
            .is_some_and(|body| block_contains_malloc_lowered_alloc(body, class_names)),
        TypedItem::Class(class_def) => class_def
            .methods
            .iter()
            .any(|method| {
                method
                    .body
                    .as_ref()
                    .is_some_and(|body| block_contains_malloc_lowered_alloc(body, class_names))
            }),
        TypedItem::Extern(extern_block) => extern_block.functions.iter().any(|function| {
            function
                .body
                .as_ref()
                .is_some_and(|body| block_contains_malloc_lowered_alloc(body, class_names))
        }),
        TypedItem::Interface(interface_def) => interface_def.methods.iter().any(|method| {
            method
                .body
                .as_ref()
                .is_some_and(|body| block_contains_malloc_lowered_alloc(body, class_names))
        }),
        TypedItem::Const(const_def) => expr_contains_malloc_lowered_alloc(&const_def.value, class_names),
        TypedItem::Enum(_)
        | TypedItem::Error(_)
        | TypedItem::Import(_)
        | TypedItem::TypeBlock(_) => false,
    }
}

fn block_contains_malloc_lowered_alloc(block: &TypedBlock, class_names: &HashSet<String>) -> bool {
    block.stmts
        .iter()
        .any(|stmt| stmt_contains_malloc_lowered_alloc(&stmt.kind, class_names))
}

fn stmt_contains_malloc_lowered_alloc(kind: &TypedStmtKind, class_names: &HashSet<String>) -> bool {
    match kind {
        TypedStmtKind::Let(let_stmt) => let_stmt
            .value
            .as_ref()
            .is_some_and(|value| expr_contains_malloc_lowered_alloc(value, class_names)),
        TypedStmtKind::LetDestructure(stmt) => {
            expr_contains_malloc_lowered_alloc(&stmt.value, class_names)
        }
        TypedStmtKind::Assign(assign) => {
            expr_contains_malloc_lowered_alloc(&assign.target, class_names)
                || assign
                    .value
                    .as_ref()
                    .is_some_and(|value| expr_contains_malloc_lowered_alloc(value, class_names))
        }
        TypedStmtKind::Return(ret) => ret
            .value
            .as_ref()
            .is_some_and(|value| expr_contains_malloc_lowered_alloc(value, class_names)),
        TypedStmtKind::Expr(expr) => expr_contains_malloc_lowered_alloc(expr, class_names),
        TypedStmtKind::If(if_stmt) => {
            expr_contains_malloc_lowered_alloc(&if_stmt.condition, class_names)
                || block_contains_malloc_lowered_alloc(&if_stmt.then_branch, class_names)
                || if_stmt.else_branch.as_ref().is_some_and(|branch| match branch {
                    TypedElseBranch::If(inner) => {
                        expr_contains_malloc_lowered_alloc(&inner.condition, class_names)
                            || block_contains_malloc_lowered_alloc(&inner.then_branch, class_names)
                            || inner.else_branch.as_ref().is_some_and(|next| match next {
                                TypedElseBranch::If(next_if) => {
                                    block_contains_malloc_lowered_alloc(&next_if.then_branch, class_names)
                                }
                                TypedElseBranch::Block(block) => {
                                    block_contains_malloc_lowered_alloc(block, class_names)
                                }
                            })
                    }
                    TypedElseBranch::Block(block) => {
                        block_contains_malloc_lowered_alloc(block, class_names)
                    }
                })
        }
        TypedStmtKind::For(for_stmt) => {
            expr_contains_malloc_lowered_alloc(&for_stmt.iter, class_names)
                || block_contains_malloc_lowered_alloc(&for_stmt.body, class_names)
        }
        TypedStmtKind::While(while_stmt) => {
            expr_contains_malloc_lowered_alloc(&while_stmt.condition, class_names)
                || block_contains_malloc_lowered_alloc(&while_stmt.body, class_names)
        }
        TypedStmtKind::Spawn(spawn) => match &spawn.body {
            TypedSpawnBody::Expr(expr) => expr_contains_malloc_lowered_alloc(expr, class_names),
            TypedSpawnBody::Block(block) => block_contains_malloc_lowered_alloc(block, class_names),
        },
        TypedStmtKind::Block(block)
        | TypedStmtKind::UnsafeBlock(block)
        | TypedStmtKind::PointerBlock(block)
        | TypedStmtKind::ComptimeBlock(block) => block_contains_malloc_lowered_alloc(block, class_names),
        TypedStmtKind::IfCompile(if_compile) => {
            expr_contains_malloc_lowered_alloc(&if_compile.condition, class_names)
                || block_contains_malloc_lowered_alloc(&if_compile.body, class_names)
        }
        TypedStmtKind::GcConfig(gc) => gc
            .entries
            .iter()
            .any(|entry| expr_contains_malloc_lowered_alloc(&entry.value, class_names)),
        TypedStmtKind::TypeBlock(_) | TypedStmtKind::AsmBlock(_) => false,
    }
}

fn expr_contains_malloc_lowered_alloc(expr: &TypedExpr, class_names: &HashSet<String>) -> bool {
    match &expr.kind {
        TypedExprKind::Lambda(_, body) => expr_contains_malloc_lowered_alloc(body, class_names) || true,
        TypedExprKind::Call(callee, args) => {
            is_class_literal_call(callee, args, class_names)
                || expr_contains_malloc_lowered_alloc(callee, class_names)
                || args
                    .iter()
                    .any(|arg| expr_contains_malloc_lowered_alloc(arg, class_names))
        }
        TypedExprKind::MethodCall(target, _, args) => {
            expr_contains_malloc_lowered_alloc(target, class_names)
                || args
                    .iter()
                    .any(|arg| expr_contains_malloc_lowered_alloc(arg, class_names))
        }
        TypedExprKind::Array(items) | TypedExprKind::Set(items) | TypedExprKind::Tuple(items) => items
            .iter()
            .any(|item| expr_contains_malloc_lowered_alloc(item, class_names)),
        TypedExprKind::Map(entries) => entries.iter().any(|(key, value)| {
            expr_contains_malloc_lowered_alloc(key, class_names)
                || expr_contains_malloc_lowered_alloc(value, class_names)
        }),
        TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
            expr_contains_malloc_lowered_alloc(lhs, class_names)
                || expr_contains_malloc_lowered_alloc(rhs, class_names)
        }
        TypedExprKind::UnOp(_, inner)
        | TypedExprKind::Cast(inner, _)
        | TypedExprKind::Field(inner, _)
        | TypedExprKind::Index(inner, _)
        | TypedExprKind::Ok(inner)
        | TypedExprKind::Err(inner) => expr_contains_malloc_lowered_alloc(inner, class_names),
        TypedExprKind::Match(subject, arms) => {
            expr_contains_malloc_lowered_alloc(subject, class_names)
                || arms.iter().any(|arm| {
                    expr_contains_malloc_lowered_alloc(&arm.pattern, class_names)
                        || matches!(
                            &arm.body,
                            TypedMatchArmBody::Expr(expr)
                                if expr_contains_malloc_lowered_alloc(expr, class_names)
                        )
                })
        }
        TypedExprKind::FStrLit(parts) => parts.iter().any(|part| match part {
            draton_typeck::typed_ast::TypedFStrPart::Interp(expr) => {
                expr_contains_malloc_lowered_alloc(expr, class_names)
            }
            draton_typeck::typed_ast::TypedFStrPart::Literal(_) => false,
        }),
        TypedExprKind::IntLit(_)
        | TypedExprKind::FloatLit(_)
        | TypedExprKind::StrLit(_)
        | TypedExprKind::BoolLit(_)
        | TypedExprKind::NoneLit
        | TypedExprKind::Ident(_)
        | TypedExprKind::Chan(_) => false,
    }
}

fn is_class_literal_call(
    callee: &TypedExpr,
    args: &[TypedExpr],
    class_names: &HashSet<String>,
) -> bool {
    matches!(
        (&callee.kind, args),
        (TypedExprKind::Ident(name), [TypedExpr { kind: TypedExprKind::Map(_), .. }])
            if class_names.contains(name)
    )
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
