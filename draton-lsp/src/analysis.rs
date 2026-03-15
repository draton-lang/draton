use draton_ast::{
    Block, ClassMember, DestructureBinding, ElseBranch, Expr, FStrPart, FnDef, Item, MatchArmBody,
    Param, Program, Span, Stmt, TypeMember,
};
use draton_lexer::{LexError, Lexer};
use draton_parser::{ParseError, Parser};
use draton_typeck::{
    typed_ast::{TypedElseBranch, TypedSpawnBody, TypedTypeMember},
    TypeCheckResult, TypeChecker, TypedBlock, TypedExpr, TypedExprKind, TypedFStrPart, TypedFnDef,
    TypedItem, TypedMatchArmBody, TypedProgram, TypedStmt, TypedStmtKind,
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
    pub start_offset: usize,
    pub end_offset: usize,
    pub ref_line: usize,
    pub ref_col: usize,
    pub ref_end_line: usize,
    pub ref_end_col: usize,
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

    let ast_program = parse_result.program.clone();

    let TypeCheckResult {
        typed_program,
        errors,
        ..
    } = TypeChecker::new().check(parse_result.program);

    let span_type_map = build_span_type_map(text, &typed_program);
    let def_map = build_def_map(text, &ast_program);

    AnalysisResult {
        lex_errors: Vec::new(),
        parse_errors: Vec::new(),
        type_errors: errors,
        typed_program: Some(typed_program),
        span_type_map,
        def_map,
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
        TypedItem::Enum(_) | TypedItem::Error(_) | TypedItem::Import(_) => {}
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
        TypedStmtKind::AsmBlock(_) | TypedStmtKind::TypeBlock(_) => {}
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
        TypedExprKind::Array(items) | TypedExprKind::Set(items) | TypedExprKind::Tuple(items) => {
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
        TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
            collect_expr_spans(text, lhs, out);
            collect_expr_spans(text, rhs, out);
        }
        TypedExprKind::UnOp(_, inner)
        | TypedExprKind::Cast(inner, _)
        | TypedExprKind::Ok(inner)
        | TypedExprKind::Err(inner) => collect_expr_spans(text, inner, out),
        TypedExprKind::Call(callee, args) | TypedExprKind::MethodCall(callee, _, args) => {
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

fn build_def_map(text: &str, program: &Program) -> Vec<DefEntry> {
    let mut collector = DefCollector::new(text);
    collector.predeclare(program);
    collector.collect_program(program);
    collector.entries
}

#[derive(Clone)]
struct DefLocation {
    span: Span,
}

struct DefCollector<'a> {
    text: &'a str,
    globals: std::collections::HashMap<String, DefLocation>,
    scopes: Vec<std::collections::HashMap<String, DefLocation>>,
    entries: Vec<DefEntry>,
}

impl<'a> DefCollector<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            globals: std::collections::HashMap::new(),
            scopes: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn predeclare(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Fn(def) | Item::PanicHandler(def) | Item::OomHandler(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Class(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Interface(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Enum(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Error(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Const(def) => {
                    self.globals
                        .insert(def.name.clone(), DefLocation { span: def.span });
                }
                Item::Import(_) | Item::Extern(_) | Item::TypeBlock(_) => {}
            }
        }
    }

    fn collect_program(&mut self, program: &Program) {
        for item in &program.items {
            self.collect_item(item);
        }
    }

    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::Fn(def) | Item::PanicHandler(def) | Item::OomHandler(def) => {
                self.collect_fn(def);
            }
            Item::Class(def) => {
                for member in &def.members {
                    if let ClassMember::Method(method) = member {
                        self.collect_fn(method);
                    } else if let ClassMember::Layer(layer) = member {
                        for method in &layer.methods {
                            self.collect_fn(method);
                        }
                    }
                }
            }
            Item::Interface(def) => {
                for method in &def.methods {
                    self.collect_fn(method);
                }
            }
            Item::Const(def) => self.collect_expr(&def.value),
            Item::Extern(def) => {
                for function in &def.functions {
                    self.collect_fn(function);
                }
            }
            Item::TypeBlock(def) => {
                for member in &def.members {
                    if let TypeMember::Function(function) = member {
                        self.collect_fn(function);
                    }
                }
            }
            Item::Enum(_) | Item::Error(_) | Item::Import(_) => {}
        }
    }

    fn collect_fn(&mut self, def: &FnDef) {
        self.push_scope();
        for Param { name, span, .. } in &def.params {
            self.define(name.clone(), *span);
        }
        if let Some(body) = &def.body {
            self.collect_block(body);
        }
        self.pop_scope();
    }

    fn collect_block(&mut self, block: &Block) {
        self.push_scope();
        for stmt in &block.stmts {
            self.collect_stmt(stmt);
        }
        self.pop_scope();
    }

    fn collect_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(inner) => {
                if let Some(value) = &inner.value {
                    self.collect_expr(value);
                }
                self.define(inner.name.clone(), inner.span);
            }
            Stmt::LetDestructure(inner) => {
                self.collect_expr(&inner.value);
                for binding in &inner.names {
                    if let DestructureBinding::Name(name) = binding {
                        self.define(name.clone(), inner.span);
                    }
                }
            }
            Stmt::Assign(inner) => {
                self.collect_expr(&inner.target);
                if let Some(value) = &inner.value {
                    self.collect_expr(value);
                }
            }
            Stmt::Return(inner) => {
                if let Some(value) = &inner.value {
                    self.collect_expr(value);
                }
            }
            Stmt::Expr(expr) => self.collect_expr(expr),
            Stmt::If(inner) => {
                self.collect_expr(&inner.condition);
                self.collect_block(&inner.then_branch);
                if let Some(else_branch) = &inner.else_branch {
                    self.collect_else(else_branch);
                }
            }
            Stmt::For(inner) => {
                self.collect_expr(&inner.iter);
                self.push_scope();
                self.define(inner.name.clone(), inner.span);
                self.collect_block(&inner.body);
                self.pop_scope();
            }
            Stmt::While(inner) => {
                self.collect_expr(&inner.condition);
                self.collect_block(&inner.body);
            }
            Stmt::Spawn(inner) => match &inner.body {
                draton_ast::SpawnBody::Expr(expr) => self.collect_expr(expr),
                draton_ast::SpawnBody::Block(block) => self.collect_block(block),
            },
            Stmt::Block(block)
            | Stmt::UnsafeBlock(block)
            | Stmt::PointerBlock(block)
            | Stmt::ComptimeBlock(block) => self.collect_block(block),
            Stmt::IfCompile(inner) => {
                self.collect_expr(&inner.condition);
                self.collect_block(&inner.body);
            }
            Stmt::GcConfig(inner) => {
                for entry in &inner.entries {
                    self.collect_expr(&entry.value);
                }
            }
            Stmt::AsmBlock(_, _) | Stmt::TypeBlock(_) => {}
        }
    }

    fn collect_else(&mut self, branch: &ElseBranch) {
        match branch {
            ElseBranch::If(inner) => {
                self.collect_expr(&inner.condition);
                self.collect_block(&inner.then_branch);
                if let Some(else_branch) = &inner.else_branch {
                    self.collect_else(else_branch);
                }
            }
            ElseBranch::Block(block) => self.collect_block(block),
        }
    }

    fn collect_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name, span) => self.record_ref(name, *span),
            Expr::Array(items, _) | Expr::Set(items, _) | Expr::Tuple(items, _) => {
                for item in items {
                    self.collect_expr(item);
                }
            }
            Expr::Map(entries, _) => {
                for (key, value) in entries {
                    self.collect_expr(key);
                    self.collect_expr(value);
                }
            }
            Expr::BinOp(lhs, _, rhs, _) | Expr::Nullish(lhs, rhs, _) => {
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            Expr::UnOp(_, inner, _) | Expr::Ok(inner, _) | Expr::Err(inner, _) => {
                self.collect_expr(inner);
            }
            Expr::Call(callee, args, _) => {
                self.collect_expr(callee);
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            Expr::MethodCall(target, _, args, _) => {
                self.collect_expr(target);
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            Expr::Field(target, _, _) => self.collect_expr(target),
            Expr::Index(target, index, _) => {
                self.collect_expr(target);
                self.collect_expr(index);
            }
            Expr::Lambda(params, body, _) => {
                self.push_scope();
                for name in params {
                    self.define(name.clone(), body.span());
                }
                self.collect_expr(body);
                self.pop_scope();
            }
            Expr::Cast(inner, _, _) => self.collect_expr(inner),
            Expr::Match(subject, arms, _) => {
                self.collect_expr(subject);
                for arm in arms {
                    self.collect_expr(&arm.pattern);
                    match &arm.body {
                        MatchArmBody::Expr(expr) => self.collect_expr(expr),
                        MatchArmBody::Block(block) => self.collect_block(block),
                    }
                }
            }
            Expr::FStrLit(parts, _) => {
                for part in parts {
                    if let FStrPart::Interp(expr) = part {
                        self.collect_expr(expr);
                    }
                }
            }
            Expr::IntLit(_, _)
            | Expr::FloatLit(_, _)
            | Expr::StrLit(_, _)
            | Expr::BoolLit(_, _)
            | Expr::NoneLit(_)
            | Expr::Chan(_, _) => {}
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(std::collections::HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, span: Span) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, DefLocation { span });
        }
    }

    fn record_ref(&mut self, name: &str, span: Span) {
        let Some(location) = self.lookup(name).cloned() else {
            return;
        };
        let (ref_line, ref_col) = offset_to_position(self.text, span.start);
        let ref_end = span.end.max(span.start + 1).min(self.text.len());
        let (ref_end_line, ref_end_col) = offset_to_position(self.text, ref_end);
        let (def_line, def_col) = offset_to_position(self.text, location.span.start);
        self.entries.push(DefEntry {
            start_offset: span.start,
            end_offset: ref_end,
            ref_line,
            ref_col,
            ref_end_line,
            ref_end_col,
            def_line,
            def_col,
            def_uri: String::new(),
        });
    }

    fn lookup(&self, name: &str) -> Option<&DefLocation> {
        for scope in self.scopes.iter().rev() {
            if let Some(location) = scope.get(name) {
                return Some(location);
            }
        }
        self.globals.get(name)
    }
}
