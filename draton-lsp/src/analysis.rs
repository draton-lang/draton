use draton_ast::Span;
use draton_lexer::{LexError, Lexer};
use draton_parser::{ParseError, Parser};
use draton_typeck::{
    typed_ast::{TypedElseBranch, TypedSpawnBody, TypedTypeMember},
    TypeCheckResult, TypeChecker, TypedBlock, TypedExpr, TypedExprKind, TypedFStrPart,
    TypedFnDef, TypedItem, TypedMatchArmBody, TypedProgram, TypedStmt, TypedStmtKind,
};

#[derive(Debug)]
pub struct AnalysisResult {
    pub lex_errors: Vec<LexError>,
    pub parse_errors: Vec<ParseError>,
    pub type_errors: Vec<draton_typeck::TypeError>,
    pub typed_program: Option<TypedProgram>,
    pub span_type_map: Vec<SpanType>,
    pub def_map: Vec<DefEntry>,
}

#[derive(Debug, Clone)]
pub struct SpanType {
    pub start_offset: usize,
    pub end_offset: usize,
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub type_str: String,
}

#[derive(Debug, Clone)]
pub struct DefEntry {
    pub ref_line: usize,
    pub ref_col: usize,
    pub def_line: usize,
    pub def_col: usize,
    pub def_uri: String,
}

pub fn analyze(text: &str) -> AnalysisResult {
    let lex_result = Lexer::new(text).tokenize();
    if !lex_result.errors.is_empty() {
        return AnalysisResult {
            lex_errors: lex_result.errors,
            parse_errors: Vec::new(),
            type_errors: Vec::new(),
            typed_program: None,
            span_type_map: Vec::new(),
            def_map: Vec::new(),
        };
    }

    let parse_result = Parser::new(lex_result.tokens).parse();
    if !parse_result.errors.is_empty() {
        return AnalysisResult {
            lex_errors: Vec::new(),
            parse_errors: parse_result.errors,
            type_errors: Vec::new(),
            typed_program: None,
            span_type_map: Vec::new(),
            def_map: Vec::new(),
        };
    }

    let TypeCheckResult {
        typed_program,
        errors,
        ..
    } = TypeChecker::new().check(parse_result.program);

    let span_type_map = build_span_type_map(text, &typed_program);

    AnalysisResult {
        lex_errors: Vec::new(),
        parse_errors: Vec::new(),
        type_errors: errors,
        typed_program: Some(typed_program),
        span_type_map,
        def_map: Vec::new(),
    }
}

fn build_span_type_map(text: &str, program: &TypedProgram) -> Vec<SpanType> {
    let mut out = Vec::new();
    for item in &program.items {
        collect_item_spans(text, item, &mut out);
    }
    out
}

fn collect_item_spans(text: &str, item: &TypedItem, out: &mut Vec<SpanType>) {
    match item {
        TypedItem::Fn(def) | TypedItem::PanicHandler(def) | TypedItem::OomHandler(def) => {
            collect_fn_spans(text, def, out);
        }
        TypedItem::Class(def) => {
            for method in &def.methods {
                collect_fn_spans(text, method, out);
            }
        }
        TypedItem::Interface(def) => {
            for method in &def.methods {
                collect_fn_spans(text, method, out);
            }
        }
        TypedItem::Const(def) => collect_expr_spans(text, &def.value, out),
        TypedItem::Extern(def) => {
            for function in &def.functions {
                collect_fn_spans(text, function, out);
            }
        }
        TypedItem::TypeBlock(def) => {
            for member in &def.members {
                if let TypedTypeMember::Function(function) = member {
                    collect_fn_spans(text, function, out);
                }
            }
        }
        TypedItem::Enum(_)
        | TypedItem::Error(_)
        | TypedItem::Import(_) => {}
    }
}

fn collect_fn_spans(text: &str, def: &TypedFnDef, out: &mut Vec<SpanType>) {
    for param in &def.params {
        push_span_type(text, param.span, param.ty.to_string(), out);
    }
    if let Some(body) = &def.body {
        collect_block_spans(text, body, out);
    }
}

fn collect_block_spans(text: &str, block: &TypedBlock, out: &mut Vec<SpanType>) {
    for stmt in &block.stmts {
        collect_stmt_spans(text, stmt, out);
    }
}

fn collect_stmt_spans(text: &str, stmt: &TypedStmt, out: &mut Vec<SpanType>) {
    match &stmt.kind {
        TypedStmtKind::Let(inner) => {
            if let Some(value) = &inner.value {
                collect_expr_spans(text, value, out);
            }
        }
        TypedStmtKind::LetDestructure(inner) => {
            collect_expr_spans(text, &inner.value, out);
        }
        TypedStmtKind::Assign(inner) => {
            collect_expr_spans(text, &inner.target, out);
            if let Some(value) = &inner.value {
                collect_expr_spans(text, value, out);
            }
        }
        TypedStmtKind::Return(inner) => {
            if let Some(value) = &inner.value {
                collect_expr_spans(text, value, out);
            }
        }
        TypedStmtKind::Expr(expr) => collect_expr_spans(text, expr, out),
        TypedStmtKind::If(inner) => {
            collect_expr_spans(text, &inner.condition, out);
            collect_block_spans(text, &inner.then_branch, out);
            if let Some(else_branch) = &inner.else_branch {
                collect_else_spans(text, else_branch, out);
            }
        }
        TypedStmtKind::For(inner) => {
            collect_expr_spans(text, &inner.iter, out);
            collect_block_spans(text, &inner.body, out);
        }
        TypedStmtKind::While(inner) => {
            collect_expr_spans(text, &inner.condition, out);
            collect_block_spans(text, &inner.body, out);
        }
        TypedStmtKind::Spawn(inner) => match &inner.body {
            TypedSpawnBody::Expr(expr) => collect_expr_spans(text, expr, out),
            TypedSpawnBody::Block(block) => collect_block_spans(text, block, out),
        },
        TypedStmtKind::Block(block)
        | TypedStmtKind::UnsafeBlock(block)
        | TypedStmtKind::PointerBlock(block)
        | TypedStmtKind::ComptimeBlock(block) => collect_block_spans(text, block, out),
        TypedStmtKind::IfCompile(inner) => {
            collect_expr_spans(text, &inner.condition, out);
            collect_block_spans(text, &inner.body, out);
        }
        TypedStmtKind::GcConfig(inner) => {
            for entry in &inner.entries {
                collect_expr_spans(text, &entry.value, out);
            }
        }
        TypedStmtKind::AsmBlock(_) => {}
    }
}

fn collect_else_spans(text: &str, branch: &TypedElseBranch, out: &mut Vec<SpanType>) {
    match branch {
        TypedElseBranch::If(inner) => {
            collect_expr_spans(text, &inner.condition, out);
            collect_block_spans(text, &inner.then_branch, out);
            if let Some(else_branch) = &inner.else_branch {
                collect_else_spans(text, else_branch, out);
            }
        }
        TypedElseBranch::Block(block) => collect_block_spans(text, block, out),
    }
}

fn collect_expr_spans(text: &str, expr: &TypedExpr, out: &mut Vec<SpanType>) {
    push_span_type(text, expr.span, expr.ty.to_string(), out);
    match &expr.kind {
        TypedExprKind::FStrLit(parts) => {
            for part in parts {
                if let TypedFStrPart::Interp(inner) = part {
                    collect_expr_spans(text, inner, out);
                }
            }
        }
        TypedExprKind::Array(items)
        | TypedExprKind::Set(items)
        | TypedExprKind::Tuple(items) => {
            for item in items {
                collect_expr_spans(text, item, out);
            }
        }
        TypedExprKind::Map(entries) => {
            for (key, value) in entries {
                collect_expr_spans(text, key, out);
                collect_expr_spans(text, value, out);
            }
        }
        TypedExprKind::BinOp(lhs, _, rhs)
        | TypedExprKind::Nullish(lhs, rhs) => {
            collect_expr_spans(text, lhs, out);
            collect_expr_spans(text, rhs, out);
        }
        TypedExprKind::UnOp(_, inner)
        | TypedExprKind::Cast(inner, _)
        | TypedExprKind::Ok(inner)
        | TypedExprKind::Err(inner) => collect_expr_spans(text, inner, out),
        TypedExprKind::Call(callee, args)
        | TypedExprKind::MethodCall(callee, _, args) => {
            collect_expr_spans(text, callee, out);
            for arg in args {
                collect_expr_spans(text, arg, out);
            }
        }
        TypedExprKind::Field(target, _) => collect_expr_spans(text, target, out),
        TypedExprKind::Index(target, index) => {
            collect_expr_spans(text, target, out);
            collect_expr_spans(text, index, out);
        }
        TypedExprKind::Lambda(params, body) => {
            for param in params {
                push_span_type(text, param.span, param.ty.to_string(), out);
            }
            collect_expr_spans(text, body, out);
        }
        TypedExprKind::Match(subject, arms) => {
            collect_expr_spans(text, subject, out);
            for arm in arms {
                collect_expr_spans(text, &arm.pattern, out);
                match &arm.body {
                    TypedMatchArmBody::Expr(expr) => collect_expr_spans(text, expr, out),
                    TypedMatchArmBody::Block(block) => collect_block_spans(text, block, out),
                }
            }
        }
        TypedExprKind::IntLit(_)
        | TypedExprKind::FloatLit(_)
        | TypedExprKind::StrLit(_)
        | TypedExprKind::BoolLit(_)
        | TypedExprKind::NoneLit
        | TypedExprKind::Ident(_)
        | TypedExprKind::Chan(_) => {}
    }
}

fn push_span_type(text: &str, span: Span, type_str: String, out: &mut Vec<SpanType>) {
    let (line, col) = offset_to_position(text, span.start);
    let mut end = span.end.max(span.start + 1);
    if end > text.len() {
        end = text.len();
    }
    let (end_line, end_col) = offset_to_position(text, end);
    out.push(SpanType {
        start_offset: span.start,
        end_offset: end,
        line,
        col,
        end_line,
        end_col,
        type_str,
    });
}

fn offset_to_position(text: &str, offset: usize) -> (usize, usize) {
    let bounded = offset.min(text.len());
    let mut line = 0usize;
    let mut col = 0usize;
    for (index, ch) in text.char_indices() {
        if index >= bounded {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
