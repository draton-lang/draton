use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use draton_ast::{
    Block, ClassDef, ClassMember, ElseBranch, Expr, FnDef, ImportDef, InterfaceDef, Item,
    MatchArmBody, Program, SpawnBody, Stmt, TypeBlock, TypeExpr, TypeMember,
};
use draton_lexer::Lexer;
use draton_parser::{ParseError, Parser};

use crate::tooling::files::collect_draton_files;

pub(crate) fn run(cwd: &Path, paths: &[PathBuf]) -> Result<()> {
    let files = collect_draton_files(cwd, paths)?;
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;

    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let findings = lint_source(&source);
        for finding in findings {
            match finding.severity {
                Severity::Warning => {
                    total_warnings += 1;
                    println!(
                        "{} {}:{}:{} [{}] {}",
                        "warning".yellow().bold(),
                        file.display(),
                        finding.line,
                        finding.col,
                        finding.rule,
                        finding.message
                    );
                }
                Severity::Error => {
                    total_errors += 1;
                    println!(
                        "{} {}:{}:{} [{}] {}",
                        "error".red().bold(),
                        file.display(),
                        finding.line,
                        finding.col,
                        finding.rule,
                        finding.message
                    );
                }
            }
        }
    }

    if total_errors == 0 && total_warnings == 0 {
        println!("{}", "lint: no findings".green());
    } else {
        println!(
            "{} {} warning(s), {} error(s)",
            "lint".bold(),
            total_warnings,
            total_errors
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
struct LintFinding {
    severity: Severity,
    rule: &'static str,
    line: usize,
    col: usize,
    message: String,
}

#[derive(Debug, Clone)]
struct FunctionContract {
    arity: usize,
    return_type: TypeExpr,
}

fn lint_source(source: &str) -> Vec<LintFinding> {
    let lexed = Lexer::new(source).tokenize();
    if !lexed.errors.is_empty() {
        return lexed
            .errors
            .into_iter()
            .map(|error| match error {
                draton_lexer::LexError::UnexpectedChar { line, col, found } => LintFinding {
                    severity: Severity::Error,
                    rule: "syntax",
                    line,
                    col,
                    message: format!("unexpected character '{found}'"),
                },
                draton_lexer::LexError::UnterminatedString { line, col } => LintFinding {
                    severity: Severity::Error,
                    rule: "syntax",
                    line,
                    col,
                    message: "unterminated string literal".to_string(),
                },
                draton_lexer::LexError::UnterminatedBlockComment { line, col } => LintFinding {
                    severity: Severity::Error,
                    rule: "syntax",
                    line,
                    col,
                    message: "unterminated block comment".to_string(),
                },
                draton_lexer::LexError::InvalidNumericLiteral { line, col, lexeme } => {
                    LintFinding {
                        severity: Severity::Error,
                        rule: "syntax",
                        line,
                        col,
                        message: format!("invalid numeric literal '{lexeme}'"),
                    }
                }
            })
            .collect();
    }

    let parsed = Parser::new(lexed.tokens).parse();
    if !parsed.errors.is_empty() {
        return parsed
            .errors
            .into_iter()
            .map(|error| parse_error_to_finding(&error))
            .collect();
    }

    let program = parsed.program;
    let mut findings = Vec::new();
    lint_program(&program, &mut findings);
    findings.sort_by_key(|finding| (finding.line, finding.col, finding.rule));
    findings
}

fn lint_program(program: &Program, findings: &mut Vec<LintFinding>) {
    let mut used_names = BTreeSet::new();
    for item in &program.items {
        collect_item_uses(item, &mut used_names);
    }

    let file_contracts = collect_type_contracts(program.items.iter().filter_map(|item| match item {
        Item::TypeBlock(block) => Some(block),
        _ => None,
    }));

    for item in &program.items {
        match item {
            Item::Import(import_def) => lint_import(import_def, &used_names, findings),
            Item::Fn(function) | Item::PanicHandler(function) | Item::OomHandler(function) => {
                lint_function(function, &file_contracts, findings);
            }
            Item::Class(class_def) => lint_class(class_def, findings),
            Item::Interface(interface_def) => lint_interface(interface_def, findings),
            Item::Extern(extern_block) => {
                for function in &extern_block.functions {
                    lint_deprecated_fn_syntax(function, findings);
                }
            }
            Item::Error(error_def) => {
                for field in &error_def.fields {
                    collect_type_expr_uses(field.type_hint.as_ref(), &mut used_names.clone());
                }
            }
            Item::Const(const_def) => collect_expr_uses(&const_def.value, &mut used_names.clone()),
            Item::Enum(_) | Item::TypeBlock(_) => {}
        }
    }
}

fn lint_class(class_def: &ClassDef, findings: &mut Vec<LintFinding>) {
    let class_contracts = collect_type_contracts(class_def.type_blocks.iter());
    for member in &class_def.members {
        match member {
            ClassMember::Field(_) => {}
            ClassMember::Method(function) => lint_function(function, &class_contracts, findings),
            ClassMember::Layer(layer) => {
                let layer_contracts = collect_type_contracts(layer.type_blocks.iter());
                for function in &layer.methods {
                    let merged_contracts = merged_contracts(&class_contracts, &layer_contracts);
                    lint_function(function, &merged_contracts, findings);
                }
            }
        }
    }
}

fn lint_interface(interface_def: &InterfaceDef, findings: &mut Vec<LintFinding>) {
    let interface_contracts = collect_type_contracts(interface_def.type_blocks.iter());
    for function in &interface_def.methods {
        lint_deprecated_fn_syntax(function, findings);
        if let Some(contract) = interface_contracts.get(&function.name) {
            if function.params.len() != contract.arity {
                findings.push(LintFinding {
                    severity: Severity::Warning,
                    rule: "contract-mismatch",
                    line: function.span.line,
                    col: function.span.col,
                    message: format!(
                        "interface method '{}' has {} parameter(s) but @type contract expects {}",
                        function.name,
                        function.params.len(),
                        contract.arity
                    ),
                });
            }
        }
    }
}

fn lint_import(import_def: &ImportDef, used_names: &BTreeSet<String>, findings: &mut Vec<LintFinding>) {
    for item in &import_def.items {
        let visible_name = item.alias.as_deref().unwrap_or(&item.name);
        if !used_names.contains(visible_name) {
            findings.push(LintFinding {
                severity: Severity::Warning,
                rule: "unused-import",
                line: item.span.line,
                col: item.span.col,
                message: format!("import '{}' is not used", visible_name),
            });
        }
    }
}

fn lint_function(
    function: &FnDef,
    contracts: &BTreeMap<String, FunctionContract>,
    findings: &mut Vec<LintFinding>,
) {
    lint_deprecated_fn_syntax(function, findings);
    if let Some(body) = &function.body {
        lint_block(body, findings);
        if let Some(contract) = contracts.get(&function.name) {
            if function.params.len() != contract.arity {
                findings.push(LintFinding {
                    severity: Severity::Warning,
                    rule: "contract-mismatch",
                    line: function.span.line,
                    col: function.span.col,
                    message: format!(
                        "function '{}' has {} parameter(s) but @type contract expects {}",
                        function.name,
                        function.params.len(),
                        contract.arity
                    ),
                });
            }
            if !is_unit_type(&contract.return_type) && !block_definitely_returns(body) {
                findings.push(LintFinding {
                    severity: Severity::Warning,
                    rule: "missing-return",
                    line: function.span.line,
                    col: function.span.col,
                    message: format!(
                        "function '{}' has a non-Unit contract but can fall through without an explicit return",
                        function.name
                    ),
                });
            }
        }
    }
}

fn lint_deprecated_fn_syntax(function: &FnDef, findings: &mut Vec<LintFinding>) {
    for param in &function.params {
        if param.type_hint.is_some() {
            findings.push(LintFinding {
                severity: Severity::Warning,
                rule: "deprecated-syntax",
                line: param.span.line,
                col: param.span.col,
                message: format!(
                    "typed parameter '{}' is deprecated; move the contract into a nearby @type block",
                    param.name
                ),
            });
        }
    }
    if function.ret_type.is_some() {
        findings.push(LintFinding {
            severity: Severity::Warning,
            rule: "deprecated-syntax",
            line: function.span.line,
            col: function.span.col,
            message: format!(
                "inline return type on '{}' is deprecated; move the contract into a nearby @type block",
                function.name
            ),
        });
    }
}

fn lint_block(block: &Block, findings: &mut Vec<LintFinding>) {
    let mut return_seen = false;
    for stmt in &block.stmts {
        if return_seen {
            let span = stmt_span(stmt);
            findings.push(LintFinding {
                severity: Severity::Warning,
                rule: "unreachable-code",
                line: span.line,
                col: span.col,
                message: "statement is unreachable because a previous branch always returns"
                    .to_string(),
            });
        }
        lint_stmt(stmt, findings);
        if stmt_definitely_returns(stmt) {
            return_seen = true;
        }
    }
}

fn lint_stmt(stmt: &Stmt, findings: &mut Vec<LintFinding>) {
    match stmt {
        Stmt::Let(let_stmt) => {
            if let_stmt.type_hint.is_some() {
                findings.push(LintFinding {
                    severity: Severity::Warning,
                    rule: "deprecated-syntax",
                    line: let_stmt.span.line,
                    col: let_stmt.span.col,
                    message: format!(
                        "typed local binding '{}' is deprecated; use a function-scope @type block instead",
                        let_stmt.name
                    ),
                });
            }
            if let Some(value) = &let_stmt.value {
                lint_expr(value, findings);
            }
        }
        Stmt::LetDestructure(let_stmt) => lint_expr(&let_stmt.value, findings),
        Stmt::Assign(assign) => {
            lint_expr(&assign.target, findings);
            if let Some(value) = &assign.value {
                lint_expr(value, findings);
            }
        }
        Stmt::Return(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                lint_expr(value, findings);
            }
        }
        Stmt::Expr(expr) => lint_expr(expr, findings),
        Stmt::If(if_stmt) => {
            lint_expr(&if_stmt.condition, findings);
            lint_block(&if_stmt.then_branch, findings);
            if let Some(else_branch) = &if_stmt.else_branch {
                lint_else_branch(else_branch, findings);
            }
        }
        Stmt::For(for_stmt) => {
            lint_expr(&for_stmt.iter, findings);
            lint_block(&for_stmt.body, findings);
        }
        Stmt::While(while_stmt) => {
            lint_expr(&while_stmt.condition, findings);
            lint_block(&while_stmt.body, findings);
        }
        Stmt::Spawn(spawn_stmt) => match &spawn_stmt.body {
            SpawnBody::Expr(expr) => lint_expr(expr, findings),
            SpawnBody::Block(block) => lint_block(block, findings),
        },
        Stmt::Block(block)
        | Stmt::UnsafeBlock(block)
        | Stmt::PointerBlock(block)
        | Stmt::ComptimeBlock(block) => lint_block(block, findings),
        Stmt::IfCompile(if_compile) => {
            lint_expr(&if_compile.condition, findings);
            lint_block(&if_compile.body, findings);
        }
        Stmt::GcConfig(gc_config) => {
            for entry in &gc_config.entries {
                lint_expr(&entry.value, findings);
            }
        }
        Stmt::AsmBlock(_, _) | Stmt::TypeBlock(_) => {}
    }
}

fn lint_else_branch(branch: &ElseBranch, findings: &mut Vec<LintFinding>) {
    match branch {
        ElseBranch::If(if_stmt) => {
            lint_expr(&if_stmt.condition, findings);
            lint_block(&if_stmt.then_branch, findings);
            if let Some(else_branch) = &if_stmt.else_branch {
                lint_else_branch(else_branch, findings);
            }
        }
        ElseBranch::Block(block) => lint_block(block, findings),
    }
}

fn lint_expr(expr: &Expr, findings: &mut Vec<LintFinding>) {
    match expr {
        Expr::Array(items, _)
        | Expr::Set(items, _)
        | Expr::Tuple(items, _)
        | Expr::Call(_, items, _)
        | Expr::MethodCall(_, _, items, _) => {
            for item in items {
                lint_expr(item, findings);
            }
        }
        Expr::Map(entries, _) => {
            for (key, value) in entries {
                lint_expr(key, findings);
                lint_expr(value, findings);
            }
        }
        Expr::BinOp(lhs, _, rhs, _) | Expr::Nullish(lhs, rhs, _) => {
            lint_expr(lhs, findings);
            lint_expr(rhs, findings);
        }
        Expr::UnOp(_, inner, _)
        | Expr::Cast(inner, _, _)
        | Expr::Ok(inner, _)
        | Expr::Err(inner, _) => lint_expr(inner, findings),
        Expr::Field(target, _, _) => lint_expr(target, findings),
        Expr::Index(target, index, _) => {
            lint_expr(target, findings);
            lint_expr(index, findings);
        }
        Expr::Lambda(_, body, _) => lint_expr(body, findings),
        Expr::Match(subject, arms, _) => {
            lint_expr(subject, findings);
            for arm in arms {
                lint_expr(&arm.pattern, findings);
                match &arm.body {
                    MatchArmBody::Expr(expr) => lint_expr(expr, findings),
                    MatchArmBody::Block(block) => lint_block(block, findings),
                }
            }
        }
        Expr::FStrLit(parts, _) => {
            for part in parts {
                if let draton_ast::FStrPart::Interp(expr) = part {
                    lint_expr(expr, findings);
                }
            }
        }
        Expr::Chan(_, _)
        | Expr::IntLit(_, _)
        | Expr::FloatLit(_, _)
        | Expr::StrLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::NoneLit(_)
        | Expr::Ident(_, _) => {}
    }
}

fn stmt_definitely_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::Block(block)
        | Stmt::UnsafeBlock(block)
        | Stmt::PointerBlock(block)
        | Stmt::ComptimeBlock(block) => block_definitely_returns(block),
        Stmt::If(if_stmt) => {
            let then_returns = block_definitely_returns(&if_stmt.then_branch);
            let else_returns = if_stmt
                .else_branch
                .as_ref()
                .map(else_branch_definitely_returns)
                .unwrap_or(false);
            then_returns && else_returns
        }
        _ => false,
    }
}

fn else_branch_definitely_returns(branch: &ElseBranch) -> bool {
    match branch {
        ElseBranch::If(if_stmt) => {
            block_definitely_returns(&if_stmt.then_branch)
                && if_stmt
                    .else_branch
                    .as_ref()
                    .map(else_branch_definitely_returns)
                    .unwrap_or(false)
        }
        ElseBranch::Block(block) => block_definitely_returns(block),
    }
}

fn block_definitely_returns(block: &Block) -> bool {
    block.stmts.iter().any(stmt_definitely_returns)
}

fn stmt_span(stmt: &Stmt) -> draton_ast::Span {
    match stmt {
        Stmt::Let(inner) => inner.span,
        Stmt::LetDestructure(inner) => inner.span,
        Stmt::Assign(inner) => inner.span,
        Stmt::Return(inner) => inner.span,
        Stmt::Expr(expr) => expr.span(),
        Stmt::If(inner) => inner.span,
        Stmt::For(inner) => inner.span,
        Stmt::While(inner) => inner.span,
        Stmt::Spawn(inner) => inner.span,
        Stmt::Block(inner)
        | Stmt::UnsafeBlock(inner)
        | Stmt::PointerBlock(inner)
        | Stmt::ComptimeBlock(inner) => inner.span,
        Stmt::AsmBlock(_, span) => *span,
        Stmt::IfCompile(inner) => inner.span,
        Stmt::GcConfig(inner) => inner.span,
        Stmt::TypeBlock(inner) => inner.span,
    }
}

fn collect_type_contracts<'a, I>(blocks: I) -> BTreeMap<String, FunctionContract>
where
    I: Iterator<Item = &'a TypeBlock>,
{
    let mut out = BTreeMap::new();
    for block in blocks {
        for member in &block.members {
            match member {
                TypeMember::Binding {
                    name, type_expr, ..
                } => {
                    if let TypeExpr::Fn(params, ret, _) = type_expr {
                        out.insert(
                            name.clone(),
                            FunctionContract {
                                arity: params.len(),
                                return_type: (**ret).clone(),
                            },
                        );
                    }
                }
                TypeMember::Function(function) => {
                    if let Some(ret_type) = &function.ret_type {
                        out.insert(
                            function.name.clone(),
                            FunctionContract {
                                arity: function.params.len(),
                                return_type: ret_type.clone(),
                            },
                        );
                    }
                }
            }
        }
    }
    out
}

fn merged_contracts(
    parent: &BTreeMap<String, FunctionContract>,
    child: &BTreeMap<String, FunctionContract>,
) -> BTreeMap<String, FunctionContract> {
    let mut out = parent.clone();
    for (name, contract) in child {
        out.insert(name.clone(), contract.clone());
    }
    out
}

fn is_unit_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Named(name, _) if name == "Unit")
}

fn collect_item_uses(item: &Item, used_names: &mut BTreeSet<String>) {
    match item {
        Item::Fn(function) | Item::PanicHandler(function) | Item::OomHandler(function) => {
            collect_fn_uses(function, used_names);
        }
        Item::Class(class_def) => collect_class_uses(class_def, used_names),
        Item::Interface(interface_def) => {
            for function in &interface_def.methods {
                collect_fn_uses(function, used_names);
            }
            for block in &interface_def.type_blocks {
                collect_type_block_uses(block, used_names);
            }
        }
        Item::Const(const_def) => collect_expr_uses(&const_def.value, used_names),
        Item::Import(_) | Item::Enum(_) => {}
        Item::Error(error_def) => {
            for field in &error_def.fields {
                collect_type_expr_uses(field.type_hint.as_ref(), used_names);
            }
        }
        Item::Extern(extern_block) => {
            for function in &extern_block.functions {
                collect_fn_uses(function, used_names);
            }
        }
        Item::TypeBlock(block) => collect_type_block_uses(block, used_names),
    }
}

fn collect_class_uses(class_def: &ClassDef, used_names: &mut BTreeSet<String>) {
    for member in &class_def.members {
        match member {
            ClassMember::Field(field) => collect_type_expr_uses(field.type_hint.as_ref(), used_names),
            ClassMember::Method(function) => collect_fn_uses(function, used_names),
            ClassMember::Layer(layer) => {
                for function in &layer.methods {
                    collect_fn_uses(function, used_names);
                }
                for block in &layer.type_blocks {
                    collect_type_block_uses(block, used_names);
                }
            }
        }
    }
    for block in &class_def.type_blocks {
        collect_type_block_uses(block, used_names);
    }
}

fn collect_fn_uses(function: &FnDef, used_names: &mut BTreeSet<String>) {
    for param in &function.params {
        collect_type_expr_uses(param.type_hint.as_ref(), used_names);
    }
    collect_type_expr_uses(function.ret_type.as_ref(), used_names);
    if let Some(body) = &function.body {
        collect_block_uses(body, used_names);
    }
}

fn collect_block_uses(block: &Block, used_names: &mut BTreeSet<String>) {
    for stmt in &block.stmts {
        collect_stmt_uses(stmt, used_names);
    }
}

fn collect_stmt_uses(stmt: &Stmt, used_names: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Let(inner) => {
            collect_type_expr_uses(inner.type_hint.as_ref(), used_names);
            if let Some(value) = &inner.value {
                collect_expr_uses(value, used_names);
            }
        }
        Stmt::LetDestructure(inner) => collect_expr_uses(&inner.value, used_names),
        Stmt::Assign(inner) => {
            collect_expr_uses(&inner.target, used_names);
            if let Some(value) = &inner.value {
                collect_expr_uses(value, used_names);
            }
        }
        Stmt::Return(inner) => {
            if let Some(value) = &inner.value {
                collect_expr_uses(value, used_names);
            }
        }
        Stmt::Expr(expr) => collect_expr_uses(expr, used_names),
        Stmt::If(inner) => {
            collect_expr_uses(&inner.condition, used_names);
            collect_block_uses(&inner.then_branch, used_names);
            if let Some(else_branch) = &inner.else_branch {
                collect_else_uses(else_branch, used_names);
            }
        }
        Stmt::For(inner) => {
            collect_expr_uses(&inner.iter, used_names);
            collect_block_uses(&inner.body, used_names);
        }
        Stmt::While(inner) => {
            collect_expr_uses(&inner.condition, used_names);
            collect_block_uses(&inner.body, used_names);
        }
        Stmt::Spawn(inner) => match &inner.body {
            SpawnBody::Expr(expr) => collect_expr_uses(expr, used_names),
            SpawnBody::Block(block) => collect_block_uses(block, used_names),
        },
        Stmt::Block(block)
        | Stmt::UnsafeBlock(block)
        | Stmt::PointerBlock(block)
        | Stmt::ComptimeBlock(block) => collect_block_uses(block, used_names),
        Stmt::IfCompile(inner) => {
            collect_expr_uses(&inner.condition, used_names);
            collect_block_uses(&inner.body, used_names);
        }
        Stmt::GcConfig(inner) => {
            for entry in &inner.entries {
                collect_expr_uses(&entry.value, used_names);
            }
        }
        Stmt::TypeBlock(block) => collect_type_block_uses(block, used_names),
        Stmt::AsmBlock(_, _) => {}
    }
}

fn collect_else_uses(branch: &ElseBranch, used_names: &mut BTreeSet<String>) {
    match branch {
        ElseBranch::If(inner) => {
            collect_expr_uses(&inner.condition, used_names);
            collect_block_uses(&inner.then_branch, used_names);
            if let Some(else_branch) = &inner.else_branch {
                collect_else_uses(else_branch, used_names);
            }
        }
        ElseBranch::Block(block) => collect_block_uses(block, used_names),
    }
}

fn collect_expr_uses(expr: &Expr, used_names: &mut BTreeSet<String>) {
    match expr {
        Expr::Ident(name, _) => {
            used_names.insert(name.clone());
        }
        Expr::Array(items, _) | Expr::Set(items, _) | Expr::Tuple(items, _) => {
            for item in items {
                collect_expr_uses(item, used_names);
            }
        }
        Expr::Map(entries, _) => {
            for (key, value) in entries {
                collect_expr_uses(key, used_names);
                collect_expr_uses(value, used_names);
            }
        }
        Expr::BinOp(lhs, _, rhs, _) | Expr::Nullish(lhs, rhs, _) => {
            collect_expr_uses(lhs, used_names);
            collect_expr_uses(rhs, used_names);
        }
        Expr::UnOp(_, inner, _)
        | Expr::Ok(inner, _)
        | Expr::Err(inner, _)
        | Expr::Cast(inner, _, _) => collect_expr_uses(inner, used_names),
        Expr::Call(callee, args, _) | Expr::MethodCall(callee, _, args, _) => {
            collect_expr_uses(callee, used_names);
            for arg in args {
                collect_expr_uses(arg, used_names);
            }
        }
        Expr::Field(target, _, _) => collect_expr_uses(target, used_names),
        Expr::Index(target, index, _) => {
            collect_expr_uses(target, used_names);
            collect_expr_uses(index, used_names);
        }
        Expr::Lambda(_, body, _) => collect_expr_uses(body, used_names),
        Expr::Match(subject, arms, _) => {
            collect_expr_uses(subject, used_names);
            for arm in arms {
                collect_expr_uses(&arm.pattern, used_names);
                match &arm.body {
                    MatchArmBody::Expr(expr) => collect_expr_uses(expr, used_names),
                    MatchArmBody::Block(block) => collect_block_uses(block, used_names),
                }
            }
        }
        Expr::FStrLit(parts, _) => {
            for part in parts {
                if let draton_ast::FStrPart::Interp(expr) = part {
                    collect_expr_uses(expr, used_names);
                }
            }
        }
        Expr::Chan(type_expr, _) => collect_type_expr_uses(Some(type_expr), used_names),
        Expr::IntLit(_, _)
        | Expr::FloatLit(_, _)
        | Expr::StrLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::NoneLit(_) => {}
    }
}

fn collect_type_block_uses(block: &TypeBlock, used_names: &mut BTreeSet<String>) {
    for member in &block.members {
        match member {
            TypeMember::Binding { type_expr, .. } => {
                collect_type_expr_uses(Some(type_expr), used_names);
            }
            TypeMember::Function(function) => collect_fn_uses(function, used_names),
        }
    }
}

fn collect_type_expr_uses(type_expr: Option<&TypeExpr>, used_names: &mut BTreeSet<String>) {
    let Some(type_expr) = type_expr else {
        return;
    };
    match type_expr {
        TypeExpr::Named(name, _) => {
            used_names.insert(name.clone());
        }
        TypeExpr::Generic(name, args, _) => {
            used_names.insert(name.clone());
            for arg in args {
                collect_type_expr_uses(Some(arg), used_names);
            }
        }
        TypeExpr::Fn(params, ret, _) => {
            for param in params {
                collect_type_expr_uses(Some(param), used_names);
            }
            collect_type_expr_uses(Some(ret), used_names);
        }
        TypeExpr::Pointer(_) | TypeExpr::Infer(_) => {}
    }
}

fn parse_error_to_finding(error: &ParseError) -> LintFinding {
    match error {
        ParseError::UnexpectedToken {
            expected,
            found,
            line,
            col,
        } => LintFinding {
            severity: Severity::Error,
            rule: "syntax",
            line: *line,
            col: *col,
            message: format!("expected {expected}, found {found}"),
        },
        ParseError::UnexpectedEof {
            expected,
            line,
            col,
        } => LintFinding {
            severity: Severity::Error,
            rule: "syntax",
            line: *line,
            col: *col,
            message: format!("unexpected end of file; expected {expected}"),
        },
        ParseError::InvalidExpr { line, col } => LintFinding {
            severity: Severity::Error,
            rule: "syntax",
            line: *line,
            col: *col,
            message: "invalid expression".to_string(),
        },
        ParseError::NestedLayerNotAllowed { line, col } => LintFinding {
            severity: Severity::Error,
            rule: "syntax",
            line: *line,
            col: *col,
            message: "nested layer blocks are not allowed".to_string(),
        },
        ParseError::LayerOutsideClass { line, col } => LintFinding {
            severity: Severity::Error,
            rule: "syntax",
            line: *line,
            col: *col,
            message: "layer blocks are only valid inside class bodies".to_string(),
        },
    }
}
