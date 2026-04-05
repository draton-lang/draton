use std::collections::{HashMap, HashSet};

use draton_ast::{BinOp, Span};

use crate::error::OwnershipError;
use crate::typed_ast::{
    FnOwnershipSummary, OwnershipState, ParamOwnershipSummary, Type, TypedAssignStmt, TypedBlock,
    TypedClassDef, TypedElseBranch, TypedExpr, TypedExprKind, TypedFnDef, TypedForStmt,
    TypedGcConfigStmt, TypedIfStmt, TypedItem, TypedMatchArmBody, TypedParam, TypedProgram,
    TypedReturnStmt, TypedSpawnBody, TypedStmt, TypedStmtKind, TypedTypeBlock, TypedTypeMember,
    TypedWhileStmt, UseEffect,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowKind {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BindingState {
    name: String,
    ty: Type,
    state: OwnershipState,
    state_span: Span,
    origin: Option<usize>,
    is_mut: bool,
    is_param: bool,
    is_closure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BorrowRecord {
    kind: BorrowKind,
    span: Span,
    persistent: bool,
}

#[derive(Debug, Clone, Default)]
struct OwnershipEnv {
    bindings: HashMap<String, BindingState>,
    borrows: HashMap<String, Vec<BorrowRecord>>,
    origin_parents: HashMap<usize, usize>,
}

#[derive(Debug, Clone)]
struct FunctionRecord {
    _key: String,
    params: Vec<TypedParam>,
    body: Option<TypedBlock>,
    ret_type: Type,
    receiver_ty: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalFnSummary {
    summary: FnOwnershipSummary,
    receiver_effect: Option<UseEffect>,
}

#[derive(Debug, Clone, Default)]
struct FunctionIndex {
    records: HashMap<String, FunctionRecord>,
    top_level: HashSet<String>,
    methods: HashMap<(String, String), String>,
    enum_names: HashSet<String>,
    class_fields: HashMap<String, HashMap<String, Type>>,
    acyclic_classes: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClosureMeta {
    captures: HashSet<String>,
    exclusive_captures: HashSet<String>,
    escaping: bool,
    last_call: Option<Span>,
}

pub fn is_copy(ty: &Type) -> bool {
    match ty {
        Type::Bool
        | Type::Int
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Float
        | Type::Float32
        | Type::Float64
        | Type::Char
        | Type::Unit
        | Type::Never
        | Type::Pointer(_)
        | Type::Fn(_, _) => true,
        Type::Tuple(items) => items.len() <= 2 && items.iter().all(is_copy),
        Type::Option(inner) => is_copy(inner),
        Type::Result(ok, err) => is_copy(ok) && is_copy(err),
        Type::Row { fields, rest } => {
            fields.len() <= 2 && fields.values().all(is_copy) && rest.as_deref().is_none_or(is_copy)
        }
        Type::String
        | Type::Array(_)
        | Type::Map(_, _)
        | Type::Set(_)
        | Type::Chan(_)
        | Type::Named(_, _)
        | Type::Var(_) => false,
    }
}

pub struct OwnershipChecker {
    index: FunctionIndex,
    summaries: HashMap<String, InternalFnSummary>,
    free_points: HashMap<String, Vec<Span>>,
    next_origin: usize,
}

impl OwnershipChecker {
    pub fn new() -> Self {
        Self {
            index: FunctionIndex::default(),
            summaries: HashMap::new(),
            free_points: HashMap::new(),
            next_origin: 1,
        }
    }

    pub fn recorded_free_points(&self) -> &HashMap<String, Vec<Span>> {
        &self.free_points
    }

    pub fn check_program(&mut self, program: &mut TypedProgram) -> Vec<OwnershipError> {
        self.index = self.build_index(program);
        let mut errors = self.validate_acyclic_definitions(program);
        self.seed_builtin_summaries();
        self.infer_function_summaries(program);
        self.write_summaries(program);
        for item in &mut program.items {
            self.visit_item(item, &mut errors);
        }
        errors
    }

    fn build_index(&self, program: &TypedProgram) -> FunctionIndex {
        let mut index = FunctionIndex::default();
        for item in &program.items {
            self.collect_index_item(item, None, &mut index);
        }
        index
    }

    fn collect_index_item(
        &self,
        item: &TypedItem,
        current_class: Option<&str>,
        index: &mut FunctionIndex,
    ) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => {
                let key = function.name.clone();
                index.top_level.insert(function.name.clone());
                index.records.insert(
                    key.clone(),
                    FunctionRecord {
                        _key: key,
                        params: function.params.clone(),
                        body: function.body.clone(),
                        ret_type: function.ret_type.clone(),
                        receiver_ty: None,
                    },
                );
            }
            TypedItem::Class(class_def) => {
                index.class_fields.insert(
                    class_def.name.clone(),
                    class_def
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.clone()))
                        .collect(),
                );
                if self.class_is_acyclic(class_def) {
                    index.acyclic_classes.insert(class_def.name.clone());
                }
                for method in &class_def.methods {
                    let key = format!("{}::{}", class_def.name, method.name);
                    index
                        .methods
                        .insert((class_def.name.clone(), method.name.clone()), key.clone());
                    index.records.insert(
                        key.clone(),
                        FunctionRecord {
                            _key: key,
                            params: method.params.clone(),
                            body: method.body.clone(),
                            ret_type: method.ret_type.clone(),
                            receiver_ty: Some(Type::Named(class_def.name.clone(), Vec::new())),
                        },
                    );
                }
            }
            TypedItem::Interface(interface_def) => {
                for method in &interface_def.methods {
                    let key = format!("{}::{}", interface_def.name, method.name);
                    index.methods.insert(
                        (interface_def.name.clone(), method.name.clone()),
                        key.clone(),
                    );
                    index.records.insert(
                        key.clone(),
                        FunctionRecord {
                            _key: key,
                            params: method.params.clone(),
                            body: method.body.clone(),
                            ret_type: method.ret_type.clone(),
                            receiver_ty: Some(Type::Named(interface_def.name.clone(), Vec::new())),
                        },
                    );
                }
            }
            TypedItem::Enum(enum_def) => {
                index.enum_names.insert(enum_def.name.clone());
            }
            TypedItem::Extern(extern_block) => {
                for function in &extern_block.functions {
                    let key = function.name.clone();
                    index.records.insert(
                        key.clone(),
                        FunctionRecord {
                            _key: key,
                            params: function.params.clone(),
                            body: None,
                            ret_type: function.ret_type.clone(),
                            receiver_ty: None,
                        },
                    );
                }
            }
            TypedItem::TypeBlock(type_block) => {
                self.collect_type_block_functions(type_block, current_class, index);
            }
            TypedItem::Const(_) | TypedItem::Error(_) | TypedItem::Import(_) => {}
        }
    }

    fn collect_type_block_functions(
        &self,
        type_block: &TypedTypeBlock,
        current_class: Option<&str>,
        index: &mut FunctionIndex,
    ) {
        for member in &type_block.members {
            if let TypedTypeMember::Function(function) = member {
                let key = if let Some(class_name) = current_class {
                    format!("{class_name}::{}", function.name)
                } else {
                    function.name.clone()
                };
                index.records.entry(key.clone()).or_insert(FunctionRecord {
                    _key: key,
                    params: function.params.clone(),
                    body: function.body.clone(),
                    ret_type: function.ret_type.clone(),
                    receiver_ty: current_class
                        .map(|class_name| Type::Named(class_name.to_string(), Vec::new())),
                });
            }
        }
    }

    fn class_is_acyclic(&self, class_def: &TypedClassDef) -> bool {
        class_def.type_blocks.iter().any(|block| {
            block.members.iter().any(|member| match member {
                TypedTypeMember::Binding { name, .. } => name == "@acyclic",
                TypedTypeMember::Function(_) => false,
            })
        })
    }

    fn validate_acyclic_definitions(&self, program: &TypedProgram) -> Vec<OwnershipError> {
        let mut errors = Vec::new();
        for item in &program.items {
            if let TypedItem::Class(class_def) = item {
                if !self.index.acyclic_classes.contains(&class_def.name) {
                    continue;
                }
                for field in &class_def.fields {
                    if self.type_contains_named(&field.ty, &class_def.name) {
                        errors.push(OwnershipError::OwnershipCycle { span: field.span });
                    }
                }
            }
        }
        errors
    }

    fn type_contains_named(&self, ty: &Type, target: &str) -> bool {
        let _ = self;
        match ty {
            Type::Named(name, _) => name == target,
            Type::Array(inner) | Type::Option(inner) | Type::Chan(inner) | Type::Pointer(inner) => {
                self.type_contains_named(inner, target)
            }
            Type::Map(key, value) | Type::Result(key, value) => {
                self.type_contains_named(key, target) || self.type_contains_named(value, target)
            }
            Type::Set(inner) => self.type_contains_named(inner, target),
            Type::Tuple(items) => items
                .iter()
                .any(|item| self.type_contains_named(item, target)),
            Type::Row { fields, rest } => {
                fields
                    .values()
                    .any(|item| self.type_contains_named(item, target))
                    || rest
                        .as_deref()
                        .is_some_and(|rest| self.type_contains_named(rest, target))
            }
            Type::Bool
            | Type::Int
            | Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::UInt8
            | Type::UInt16
            | Type::UInt32
            | Type::UInt64
            | Type::Float
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Char
            | Type::Unit
            | Type::Never
            | Type::Fn(_, _)
            | Type::Var(_) => false,
        }
    }

    fn seed_builtin_summaries(&mut self) {
        self.insert_builtin("print", vec![UseEffect::BorrowShared], false);
        self.insert_builtin("println", vec![UseEffect::BorrowShared], false);
        self.insert_builtin("input", vec![UseEffect::BorrowShared], true);
        self.insert_builtin(
            "range",
            vec![UseEffect::Copy, UseEffect::Copy, UseEffect::Copy],
            true,
        );
        self.insert_builtin("str_len", vec![UseEffect::BorrowShared], false);
        self.insert_builtin(
            "str_byte_at",
            vec![UseEffect::BorrowShared, UseEffect::Copy],
            false,
        );
        self.insert_builtin(
            "str_slice",
            vec![UseEffect::BorrowShared, UseEffect::Copy, UseEffect::Copy],
            true,
        );
        self.insert_builtin(
            "str_concat",
            vec![UseEffect::BorrowShared, UseEffect::BorrowShared],
            true,
        );
        self.insert_builtin("read_file", vec![UseEffect::BorrowShared], true);
        self.insert_builtin("host_ast_dump", vec![UseEffect::BorrowShared], true);
        self.insert_builtin("host_type_dump", vec![UseEffect::BorrowShared], true);
        self.insert_builtin("host_lex_json", vec![UseEffect::BorrowShared], true);
        self.insert_builtin("host_parse_json", vec![UseEffect::BorrowShared], true);
        self.insert_builtin(
            "host_type_json",
            vec![UseEffect::BorrowShared, UseEffect::Copy],
            true,
        );
        self.insert_builtin(
            "host_build_json",
            vec![
                UseEffect::BorrowShared,
                UseEffect::BorrowShared,
                UseEffect::BorrowShared,
                UseEffect::Copy,
                UseEffect::BorrowShared,
            ],
            true,
        );
        self.insert_builtin("string_parse_int", vec![UseEffect::BorrowShared], false);
        self.insert_builtin(
            "string_parse_int_radix",
            vec![UseEffect::BorrowShared, UseEffect::Copy],
            false,
        );
        self.insert_builtin("string_parse_float", vec![UseEffect::BorrowShared], false);
        self.insert_builtin("int_to_string", vec![UseEffect::Copy], true);
        self.insert_builtin("ascii_char", vec![UseEffect::Copy], true);
        self.insert_builtin("cli_argc", Vec::new(), false);
        self.insert_builtin("cli_arg", vec![UseEffect::Copy], true);
        self.insert_builtin("Some", vec![UseEffect::Move], true);
        self.insert_builtin("Ok", vec![UseEffect::Move], true);
        self.insert_builtin("Err", vec![UseEffect::Move], true);
    }

    fn insert_builtin(&mut self, name: &str, params: Vec<UseEffect>, returns_owned: bool) {
        self.summaries.insert(
            name.to_string(),
            InternalFnSummary {
                summary: FnOwnershipSummary {
                    params: params
                        .into_iter()
                        .enumerate()
                        .map(|(param_index, effect)| ParamOwnershipSummary {
                            param_index,
                            effect,
                        })
                        .collect(),
                    returns_owned,
                },
                receiver_effect: None,
            },
        );
    }

    fn infer_function_summaries(&mut self, program: &TypedProgram) {
        let graph = self.call_graph(program);
        let sccs = self.tarjan_scc(&graph);
        for component in sccs.into_iter().rev() {
            let mut changed = true;
            while changed {
                changed = false;
                for key in &component {
                    let Some(record) = self.index.records.get(key) else {
                        continue;
                    };
                    let next = self.infer_summary_for_record(record);
                    let prev = self
                        .summaries
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| self.default_summary(record));
                    if next != prev {
                        self.summaries.insert(key.clone(), next);
                        changed = true;
                    }
                }
            }
        }
    }

    fn call_graph(&self, _program: &TypedProgram) -> HashMap<String, HashSet<String>> {
        let mut graph = HashMap::new();
        for (key, record) in &self.index.records {
            let mut edges = HashSet::new();
            if let Some(body) = &record.body {
                self.collect_call_edges_in_block(body, &mut edges);
            }
            graph.insert(key.clone(), edges);
        }
        graph
    }

    fn collect_call_edges_in_block(&self, block: &TypedBlock, edges: &mut HashSet<String>) {
        for stmt in &block.stmts {
            self.collect_call_edges_in_stmt(stmt, edges);
        }
    }

    fn collect_call_edges_in_stmt(&self, stmt: &TypedStmt, edges: &mut HashSet<String>) {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.collect_call_edges_in_expr(value, edges);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => {
                self.collect_call_edges_in_expr(&stmt.value, edges)
            }
            TypedStmtKind::Assign(assign) => {
                self.collect_call_edges_in_expr(&assign.target, edges);
                if let Some(value) = &assign.value {
                    self.collect_call_edges_in_expr(value, edges);
                }
            }
            TypedStmtKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_call_edges_in_expr(value, edges);
                }
            }
            TypedStmtKind::Expr(expr) => self.collect_call_edges_in_expr(expr, edges),
            TypedStmtKind::If(if_stmt) => {
                self.collect_call_edges_in_expr(&if_stmt.condition, edges);
                self.collect_call_edges_in_block(&if_stmt.then_branch, edges);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.collect_call_edges_in_else_branch(else_branch, edges);
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.collect_call_edges_in_expr(&for_stmt.iter, edges);
                self.collect_call_edges_in_block(&for_stmt.body, edges);
            }
            TypedStmtKind::While(while_stmt) => {
                self.collect_call_edges_in_expr(&while_stmt.condition, edges);
                self.collect_call_edges_in_block(&while_stmt.body, edges);
            }
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => self.collect_call_edges_in_expr(expr, edges),
                TypedSpawnBody::Block(block) => self.collect_call_edges_in_block(block, edges),
            },
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.collect_call_edges_in_block(block, edges),
            TypedStmtKind::IfCompile(if_compile) => {
                self.collect_call_edges_in_expr(&if_compile.condition, edges);
                self.collect_call_edges_in_block(&if_compile.body, edges);
            }
            TypedStmtKind::GcConfig(gc) => self.collect_call_edges_in_gc(gc, edges),
            TypedStmtKind::TypeBlock(type_block) => {
                for member in &type_block.members {
                    if let TypedTypeMember::Function(function) = member {
                        if let Some(body) = &function.body {
                            self.collect_call_edges_in_block(body, edges);
                        }
                    }
                }
            }
            TypedStmtKind::AsmBlock(_) => {}
        }
    }

    fn collect_call_edges_in_else_branch(
        &self,
        else_branch: &TypedElseBranch,
        edges: &mut HashSet<String>,
    ) {
        match else_branch {
            TypedElseBranch::If(if_stmt) => {
                self.collect_call_edges_in_expr(&if_stmt.condition, edges);
                self.collect_call_edges_in_block(&if_stmt.then_branch, edges);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.collect_call_edges_in_else_branch(else_branch, edges);
                }
            }
            TypedElseBranch::Block(block) => self.collect_call_edges_in_block(block, edges),
        }
    }

    fn collect_call_edges_in_gc(&self, gc: &TypedGcConfigStmt, edges: &mut HashSet<String>) {
        for entry in &gc.entries {
            self.collect_call_edges_in_expr(&entry.value, edges);
        }
    }

    fn collect_call_edges_in_expr(&self, expr: &TypedExpr, edges: &mut HashSet<String>) {
        match &expr.kind {
            TypedExprKind::Call(callee, args) => {
                if let Some(key) = self.resolve_direct_callee(callee) {
                    edges.insert(key);
                }
                self.collect_call_edges_in_expr(callee, edges);
                for arg in args {
                    self.collect_call_edges_in_expr(arg, edges);
                }
            }
            TypedExprKind::MethodCall(target, name, args) => {
                if let Some(key) = self.resolve_method_key(&target.ty, name) {
                    edges.insert(key);
                }
                self.collect_call_edges_in_expr(target, edges);
                for arg in args {
                    self.collect_call_edges_in_expr(arg, edges);
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.collect_call_edges_in_expr(item, edges);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.collect_call_edges_in_expr(key, edges);
                    self.collect_call_edges_in_expr(value, edges);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.collect_call_edges_in_expr(lhs, edges);
                self.collect_call_edges_in_expr(rhs, edges);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.collect_call_edges_in_expr(inner, edges),
            TypedExprKind::Index(target, index) => {
                self.collect_call_edges_in_expr(target, edges);
                self.collect_call_edges_in_expr(index, edges);
            }
            TypedExprKind::Lambda(_, body) => self.collect_call_edges_in_expr(body, edges),
            TypedExprKind::Match(subject, arms) => {
                self.collect_call_edges_in_expr(subject, edges);
                for arm in arms {
                    self.collect_call_edges_in_expr(&arm.pattern, edges);
                    match &arm.body {
                        TypedMatchArmBody::Expr(expr) => {
                            self.collect_call_edges_in_expr(expr, edges)
                        }
                        TypedMatchArmBody::Block(block) => {
                            self.collect_call_edges_in_block(block, edges)
                        }
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.collect_call_edges_in_expr(expr, edges);
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

    fn resolve_direct_callee(&self, callee: &TypedExpr) -> Option<String> {
        match &callee.kind {
            TypedExprKind::Ident(name) if self.index.records.contains_key(name) => {
                Some(name.clone())
            }
            _ => None,
        }
    }

    fn resolve_method_key(&self, target_ty: &Type, name: &str) -> Option<String> {
        match target_ty {
            Type::Named(class_name, _) => self
                .index
                .methods
                .get(&(class_name.clone(), name.to_string()))
                .cloned(),
            _ => None,
        }
    }

    fn tarjan_scc(&self, graph: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
        struct Tarjan<'a> {
            graph: &'a HashMap<String, HashSet<String>>,
            index: usize,
            stack: Vec<String>,
            on_stack: HashSet<String>,
            indices: HashMap<String, usize>,
            lowlink: HashMap<String, usize>,
            out: Vec<Vec<String>>,
        }

        impl<'a> Tarjan<'a> {
            fn strong_connect(&mut self, node: &str) {
                self.indices.insert(node.to_string(), self.index);
                self.lowlink.insert(node.to_string(), self.index);
                self.index += 1;
                self.stack.push(node.to_string());
                self.on_stack.insert(node.to_string());
                if let Some(edges) = self.graph.get(node) {
                    for edge in edges {
                        if !self.indices.contains_key(edge) {
                            self.strong_connect(edge);
                            let next = self.lowlink[edge];
                            let current = self.lowlink[node];
                            self.lowlink.insert(node.to_string(), current.min(next));
                        } else if self.on_stack.contains(edge) {
                            let current = self.lowlink[node];
                            let edge_index = self.indices[edge];
                            self.lowlink
                                .insert(node.to_string(), current.min(edge_index));
                        }
                    }
                }
                if self.lowlink[node] == self.indices[node] {
                    let mut component = Vec::new();
                    while let Some(popped) = self.stack.pop() {
                        self.on_stack.remove(&popped);
                        component.push(popped.clone());
                        if popped == node {
                            break;
                        }
                    }
                    self.out.push(component);
                }
            }
        }

        let mut tarjan = Tarjan {
            graph,
            index: 0,
            stack: Vec::new(),
            on_stack: HashSet::new(),
            indices: HashMap::new(),
            lowlink: HashMap::new(),
            out: Vec::new(),
        };
        for node in graph.keys() {
            if !tarjan.indices.contains_key(node) {
                tarjan.strong_connect(node);
            }
        }
        tarjan.out
    }

    fn infer_summary_for_record(&self, record: &FunctionRecord) -> InternalFnSummary {
        let mut walker = SummaryWalker::new(self, record);
        if let Some(body) = &record.body {
            walker.visit_block(body);
        }
        InternalFnSummary {
            summary: FnOwnershipSummary {
                params: record
                    .params
                    .iter()
                    .enumerate()
                    .map(|(param_index, param)| ParamOwnershipSummary {
                        param_index,
                        effect: if self.is_copy_type(&param.ty) {
                            UseEffect::Copy
                        } else {
                            walker
                                .param_effects
                                .get(&param.name)
                                .cloned()
                                .unwrap_or(UseEffect::Copy)
                        },
                    })
                    .collect(),
                returns_owned: !self.is_copy_type(&record.ret_type),
            },
            receiver_effect: record.receiver_ty.as_ref().map(|receiver_ty| {
                if self.is_copy_type(receiver_ty) {
                    UseEffect::Copy
                } else {
                    walker.receiver_effect.unwrap_or(UseEffect::Copy)
                }
            }),
        }
    }

    fn default_summary(&self, record: &FunctionRecord) -> InternalFnSummary {
        InternalFnSummary {
            summary: FnOwnershipSummary {
                params: record
                    .params
                    .iter()
                    .enumerate()
                    .map(|(param_index, _)| ParamOwnershipSummary {
                        param_index,
                        effect: UseEffect::Copy,
                    })
                    .collect(),
                returns_owned: !self.is_copy_type(&record.ret_type),
            },
            receiver_effect: record.receiver_ty.as_ref().map(|_| UseEffect::Copy),
        }
    }

    fn write_summaries(&self, program: &mut TypedProgram) {
        for item in &mut program.items {
            self.write_item_summary(item, None);
        }
    }

    fn write_item_summary(&self, item: &mut TypedItem, current_class: Option<&str>) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => {
                function.ownership_summary = self
                    .summaries
                    .get(&function.name)
                    .map(|summary| summary.summary.clone());
            }
            TypedItem::Class(class_def) => {
                for method in &mut class_def.methods {
                    let key = format!("{}::{}", class_def.name, method.name);
                    method.ownership_summary = self
                        .summaries
                        .get(&key)
                        .map(|summary| summary.summary.clone());
                }
                for type_block in &mut class_def.type_blocks {
                    self.write_type_block_summary(type_block, Some(&class_def.name));
                }
            }
            TypedItem::Interface(interface_def) => {
                for method in &mut interface_def.methods {
                    let key = format!("{}::{}", interface_def.name, method.name);
                    method.ownership_summary = self
                        .summaries
                        .get(&key)
                        .map(|summary| summary.summary.clone());
                }
                for type_block in &mut interface_def.type_blocks {
                    self.write_type_block_summary(type_block, Some(&interface_def.name));
                }
            }
            TypedItem::Extern(extern_block) => {
                for function in &mut extern_block.functions {
                    function.ownership_summary = self
                        .summaries
                        .get(&function.name)
                        .map(|summary| summary.summary.clone());
                }
            }
            TypedItem::TypeBlock(type_block) => {
                self.write_type_block_summary(type_block, current_class)
            }
            TypedItem::Enum(_)
            | TypedItem::Const(_)
            | TypedItem::Error(_)
            | TypedItem::Import(_) => {}
        }
    }

    fn write_type_block_summary(
        &self,
        type_block: &mut TypedTypeBlock,
        current_class: Option<&str>,
    ) {
        for member in &mut type_block.members {
            if let TypedTypeMember::Function(function) = member {
                let key = if let Some(class_name) = current_class {
                    format!("{class_name}::{}", function.name)
                } else {
                    function.name.clone()
                };
                function.ownership_summary = self
                    .summaries
                    .get(&key)
                    .map(|summary| summary.summary.clone());
            }
        }
    }

    fn visit_item(&mut self, item: &mut TypedItem, errors: &mut Vec<OwnershipError>) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => self.visit_fn(function, None, errors),
            TypedItem::Class(class_def) => {
                for method in &mut class_def.methods {
                    self.visit_fn(method, Some(&class_def.name), errors);
                }
            }
            TypedItem::Interface(interface_def) => {
                for method in &mut interface_def.methods {
                    self.visit_fn(method, Some(&interface_def.name), errors);
                }
            }
            TypedItem::Extern(_)
            | TypedItem::Enum(_)
            | TypedItem::Error(_)
            | TypedItem::Const(_)
            | TypedItem::Import(_)
            | TypedItem::TypeBlock(_) => {}
        }
    }

    fn visit_fn(
        &mut self,
        function: &mut TypedFnDef,
        current_class: Option<&str>,
        errors: &mut Vec<OwnershipError>,
    ) {
        let Some(body_ref) = function.body.as_ref() else {
            return;
        };
        let key = if let Some(class_name) = current_class {
            format!("{class_name}::{}", function.name)
        } else {
            function.name.clone()
        };
        let closures = self.collect_closures(function, current_class, body_ref);
        let body = function.body.as_mut().expect("checked above");
        let mut env = OwnershipEnv::default();
        if let Some(class_name) = current_class {
            env.bindings.insert(
                "self".to_string(),
                BindingState {
                    name: "self".to_string(),
                    ty: Type::Named(class_name.to_string(), Vec::new()),
                    state: OwnershipState::Owned,
                    state_span: function.span,
                    origin: Some(self.new_origin()),
                    is_mut: true,
                    is_param: true,
                    is_closure: false,
                },
            );
        }
        for param in &function.params {
            env.bindings.insert(
                param.name.clone(),
                BindingState {
                    name: param.name.clone(),
                    ty: param.ty.clone(),
                    state: OwnershipState::Owned,
                    state_span: param.span,
                    origin: (!self.is_copy_type(&param.ty)).then(|| self.new_origin()),
                    is_mut: true,
                    is_param: true,
                    is_closure: false,
                },
            );
        }
        let live_after = self.compute_live_after_block(body);
        self.analyze_block(body, &mut env, &key, &closures, &live_after, errors);
        self.free_owned_bindings(&key, &env, function.span);
    }

    fn collect_closures(
        &self,
        function: &TypedFnDef,
        current_class: Option<&str>,
        body: &TypedBlock,
    ) -> HashMap<String, ClosureMeta> {
        let mut locals = function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();
        if current_class.is_some() {
            locals.insert("self".to_string());
        }
        let mut closures = HashMap::new();
        self.collect_closures_in_block(body, &locals, &mut closures);
        closures
    }

    fn collect_closures_in_block(
        &self,
        block: &TypedBlock,
        locals: &HashSet<String>,
        closures: &mut HashMap<String, ClosureMeta>,
    ) {
        let mut scope = locals.clone();
        for stmt in &block.stmts {
            match &stmt.kind {
                TypedStmtKind::Let(let_stmt) => {
                    if let Some(value) = &let_stmt.value {
                        if let TypedExprKind::Lambda(params, body) = &value.kind {
                            let captures = self.collect_captures(params, body, &scope);
                            closures.insert(
                                let_stmt.name.clone(),
                                ClosureMeta {
                                    exclusive_captures: self.collect_exclusive_captures(
                                        params, body, &scope, &captures,
                                    ),
                                    captures,
                                    escaping: self.binding_escapes(&let_stmt.name, block),
                                    last_call: self.last_call(&let_stmt.name, block),
                                },
                            );
                        }
                    }
                    scope.insert(let_stmt.name.clone());
                }
                TypedStmtKind::LetDestructure(stmt) => {
                    for binding in &stmt.bindings {
                        if let crate::typed_ast::TypedDestructureBinding::Name { name, .. } =
                            binding
                        {
                            scope.insert(name.clone());
                        }
                    }
                }
                TypedStmtKind::If(if_stmt) => {
                    self.collect_closures_in_block(&if_stmt.then_branch, &scope, closures);
                    if let Some(else_branch) = &if_stmt.else_branch {
                        match else_branch {
                            TypedElseBranch::If(if_stmt) => self.collect_closures_in_block(
                                &if_stmt.then_branch,
                                &scope,
                                closures,
                            ),
                            TypedElseBranch::Block(block) => {
                                self.collect_closures_in_block(block, &scope, closures)
                            }
                        }
                    }
                }
                TypedStmtKind::For(for_stmt) => {
                    let mut inner = scope.clone();
                    inner.insert(for_stmt.name.clone());
                    self.collect_closures_in_block(&for_stmt.body, &inner, closures);
                }
                TypedStmtKind::While(while_stmt) => {
                    self.collect_closures_in_block(&while_stmt.body, &scope, closures);
                }
                TypedStmtKind::Block(block)
                | TypedStmtKind::UnsafeBlock(block)
                | TypedStmtKind::PointerBlock(block)
                | TypedStmtKind::ComptimeBlock(block) => {
                    self.collect_closures_in_block(block, &scope, closures);
                }
                TypedStmtKind::Spawn(spawn) => {
                    if let TypedSpawnBody::Block(block) = &spawn.body {
                        self.collect_closures_in_block(block, &scope, closures);
                    }
                }
                TypedStmtKind::IfCompile(if_compile) => {
                    self.collect_closures_in_block(&if_compile.body, &scope, closures)
                }
                TypedStmtKind::Assign(_)
                | TypedStmtKind::Return(_)
                | TypedStmtKind::Expr(_)
                | TypedStmtKind::AsmBlock(_)
                | TypedStmtKind::GcConfig(_)
                | TypedStmtKind::TypeBlock(_) => {}
            }
        }
    }

    fn collect_captures(
        &self,
        params: &[TypedParam],
        body: &TypedExpr,
        scope: &HashSet<String>,
    ) -> HashSet<String> {
        let locals = params
            .iter()
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();
        let mut captures = HashSet::new();
        self.collect_captures_in_expr(body, scope, &locals, &mut captures);
        captures
    }

    fn collect_captures_in_expr(
        &self,
        expr: &TypedExpr,
        outer: &HashSet<String>,
        locals: &HashSet<String>,
        captures: &mut HashSet<String>,
    ) {
        let _ = self;
        match &expr.kind {
            TypedExprKind::Ident(name) => {
                if outer.contains(name) && !locals.contains(name) {
                    captures.insert(name.clone());
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.collect_captures_in_expr(item, outer, locals, captures);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.collect_captures_in_expr(key, outer, locals, captures);
                    self.collect_captures_in_expr(value, outer, locals, captures);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.collect_captures_in_expr(lhs, outer, locals, captures);
                self.collect_captures_in_expr(rhs, outer, locals, captures);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => {
                self.collect_captures_in_expr(inner, outer, locals, captures)
            }
            TypedExprKind::Call(callee, args) => {
                self.collect_captures_in_expr(callee, outer, locals, captures);
                for arg in args {
                    self.collect_captures_in_expr(arg, outer, locals, captures);
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.collect_captures_in_expr(target, outer, locals, captures);
                for arg in args {
                    self.collect_captures_in_expr(arg, outer, locals, captures);
                }
            }
            TypedExprKind::Index(target, index) => {
                self.collect_captures_in_expr(target, outer, locals, captures);
                self.collect_captures_in_expr(index, outer, locals, captures);
            }
            TypedExprKind::Lambda(params, body) => {
                let mut nested = locals.clone();
                for param in params {
                    nested.insert(param.name.clone());
                }
                self.collect_captures_in_expr(body, outer, &nested, captures);
            }
            TypedExprKind::Match(subject, arms) => {
                self.collect_captures_in_expr(subject, outer, locals, captures);
                for arm in arms {
                    self.collect_captures_in_expr(&arm.pattern, outer, locals, captures);
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.collect_captures_in_expr(expr, outer, locals, captures);
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.collect_captures_in_expr(expr, outer, locals, captures);
                    }
                }
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn collect_exclusive_captures(
        &self,
        params: &[TypedParam],
        body: &TypedExpr,
        scope: &HashSet<String>,
        captures: &HashSet<String>,
    ) -> HashSet<String> {
        let mut locals = params
            .iter()
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();
        let mut exclusive = HashSet::new();
        self.collect_exclusive_captures_in_expr(body, scope, &mut locals, captures, &mut exclusive);
        exclusive
    }

    fn collect_exclusive_captures_in_expr(
        &self,
        expr: &TypedExpr,
        outer: &HashSet<String>,
        locals: &mut HashSet<String>,
        captures: &HashSet<String>,
        exclusive: &mut HashSet<String>,
    ) {
        match &expr.kind {
            TypedExprKind::MethodCall(target, name, args) => {
                if let Some(base) = self.base_ident_name(target) {
                    if outer.contains(&base)
                        && captures.contains(&base)
                        && !locals.contains(&base)
                        && self.method_receiver_effect(&target.ty, name)
                            == UseEffect::BorrowExclusive
                    {
                        exclusive.insert(base);
                    }
                }
                self.collect_exclusive_captures_in_expr(target, outer, locals, captures, exclusive);
                for arg in args {
                    self.collect_exclusive_captures_in_expr(
                        arg, outer, locals, captures, exclusive,
                    );
                }
            }
            TypedExprKind::Call(callee, args) => {
                if let Some(summary) = self.lookup_call_summary(callee) {
                    for (index, arg) in args.iter().enumerate() {
                        if summary
                            .summary
                            .params
                            .get(index)
                            .is_some_and(|param| param.effect == UseEffect::BorrowExclusive)
                        {
                            if let Some(base) = self.base_ident_name(arg) {
                                if outer.contains(&base)
                                    && captures.contains(&base)
                                    && !locals.contains(&base)
                                {
                                    exclusive.insert(base);
                                }
                            }
                        }
                    }
                }
                self.collect_exclusive_captures_in_expr(callee, outer, locals, captures, exclusive);
                for arg in args {
                    self.collect_exclusive_captures_in_expr(
                        arg, outer, locals, captures, exclusive,
                    );
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.collect_exclusive_captures_in_expr(
                        item, outer, locals, captures, exclusive,
                    );
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.collect_exclusive_captures_in_expr(
                        key, outer, locals, captures, exclusive,
                    );
                    self.collect_exclusive_captures_in_expr(
                        value, outer, locals, captures, exclusive,
                    );
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.collect_exclusive_captures_in_expr(lhs, outer, locals, captures, exclusive);
                self.collect_exclusive_captures_in_expr(rhs, outer, locals, captures, exclusive);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Index(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => {
                self.collect_exclusive_captures_in_expr(inner, outer, locals, captures, exclusive);
            }
            TypedExprKind::Lambda(params, body) => {
                let mut nested = locals.clone();
                for param in params {
                    nested.insert(param.name.clone());
                }
                self.collect_exclusive_captures_in_expr(
                    body,
                    outer,
                    &mut nested,
                    captures,
                    exclusive,
                );
            }
            TypedExprKind::Match(subject, arms) => {
                self.collect_exclusive_captures_in_expr(
                    subject, outer, locals, captures, exclusive,
                );
                for arm in arms {
                    self.collect_exclusive_captures_in_expr(
                        &arm.pattern,
                        outer,
                        locals,
                        captures,
                        exclusive,
                    );
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.collect_exclusive_captures_in_expr(
                            expr, outer, locals, captures, exclusive,
                        );
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.collect_exclusive_captures_in_expr(
                            expr, outer, locals, captures, exclusive,
                        );
                    }
                }
            }
            TypedExprKind::Ident(_)
            | TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn binding_escapes(&self, name: &str, block: &TypedBlock) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.binding_escapes_in_stmt(name, stmt))
    }

    fn binding_escapes_in_stmt(&self, name: &str, stmt: &TypedStmt) -> bool {
        match &stmt.kind {
            TypedStmtKind::Return(ret) => ret
                .value
                .as_ref()
                .is_some_and(|value| self.expr_mentions_ident(value, name)),
            TypedStmtKind::Assign(assign) => {
                !matches!(assign.target.kind, TypedExprKind::Ident(_))
                    && assign
                        .value
                        .as_ref()
                        .is_some_and(|value| self.expr_mentions_ident(value, name))
            }
            TypedStmtKind::Expr(expr) => self.expr_escapes_ident(expr, name),
            TypedStmtKind::If(if_stmt) => {
                self.binding_escapes(name, &if_stmt.then_branch)
                    || if_stmt
                        .else_branch
                        .as_ref()
                        .is_some_and(|else_branch| match else_branch {
                            TypedElseBranch::If(if_stmt) => {
                                self.binding_escapes(name, &if_stmt.then_branch)
                            }
                            TypedElseBranch::Block(block) => self.binding_escapes(name, block),
                        })
            }
            TypedStmtKind::For(for_stmt) => self.binding_escapes(name, &for_stmt.body),
            TypedStmtKind::While(while_stmt) => self.binding_escapes(name, &while_stmt.body),
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => self.expr_escapes_ident(expr, name),
                TypedSpawnBody::Block(block) => self.binding_escapes(name, block),
            },
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.binding_escapes(name, block),
            TypedStmtKind::Let(_)
            | TypedStmtKind::LetDestructure(_)
            | TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::GcConfig(_)
            | TypedStmtKind::IfCompile(_)
            | TypedStmtKind::TypeBlock(_) => false,
        }
    }

    fn expr_mentions_ident(&self, expr: &TypedExpr, name: &str) -> bool {
        let _ = self;
        match &expr.kind {
            TypedExprKind::Ident(candidate) => candidate == name,
            TypedExprKind::Array(items) | TypedExprKind::Set(items) | TypedExprKind::Tuple(items) => {
                items.iter().any(|item| self.expr_mentions_ident(item, name))
            }
            TypedExprKind::Map(entries) => entries.iter().any(|(key, value)| {
                self.expr_mentions_ident(key, name) || self.expr_mentions_ident(value, name)
            }),
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.expr_mentions_ident(lhs, name) || self.expr_mentions_ident(rhs, name)
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.expr_mentions_ident(inner, name),
            TypedExprKind::Call(callee, args) => {
                self.expr_mentions_ident(callee, name)
                    || args.iter().any(|arg| self.expr_mentions_ident(arg, name))
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.expr_mentions_ident(target, name)
                    || args.iter().any(|arg| self.expr_mentions_ident(arg, name))
            }
            TypedExprKind::Index(target, index) => {
                self.expr_mentions_ident(target, name) || self.expr_mentions_ident(index, name)
            }
            TypedExprKind::Lambda(_, body) => self.expr_mentions_ident(body, name),
            TypedExprKind::Match(subject, arms) => {
                self.expr_mentions_ident(subject, name)
                    || arms.iter().any(|arm| {
                        self.expr_mentions_ident(&arm.pattern, name)
                            || matches!(&arm.body, TypedMatchArmBody::Expr(expr) if self.expr_mentions_ident(expr, name))
                    })
            }
            TypedExprKind::FStrLit(parts) => parts.iter().any(|part| match part {
                crate::typed_ast::TypedFStrPart::Literal(_) => false,
                crate::typed_ast::TypedFStrPart::Interp(expr) => self.expr_mentions_ident(expr, name),
            }),
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => false,
        }
    }

    fn expr_escapes_ident(&self, expr: &TypedExpr, name: &str) -> bool {
        match &expr.kind {
            TypedExprKind::Call(callee, args) => {
                !matches!(&callee.kind, TypedExprKind::Ident(candidate) if candidate == name)
                    && args.iter().any(|arg| self.expr_mentions_ident(arg, name))
            }
            TypedExprKind::MethodCall(_, _, args) => {
                args.iter().any(|arg| self.expr_mentions_ident(arg, name))
            }
            TypedExprKind::Array(items) | TypedExprKind::Set(items) | TypedExprKind::Tuple(items) => {
                items.iter().any(|item| self.expr_mentions_ident(item, name))
            }
            TypedExprKind::Map(entries) => entries.iter().any(|(key, value)| {
                self.expr_mentions_ident(key, name) || self.expr_mentions_ident(value, name)
            }),
            TypedExprKind::Lambda(_, body) => self.expr_mentions_ident(body, name),
            TypedExprKind::Match(subject, arms) => {
                self.expr_mentions_ident(subject, name)
                    || arms.iter().any(|arm| {
                        self.expr_mentions_ident(&arm.pattern, name)
                            || matches!(&arm.body, TypedMatchArmBody::Expr(expr) if self.expr_mentions_ident(expr, name))
                    })
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.expr_mentions_ident(lhs, name) || self.expr_mentions_ident(rhs, name)
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Index(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.expr_mentions_ident(inner, name),
            TypedExprKind::FStrLit(parts) => parts.iter().any(|part| match part {
                crate::typed_ast::TypedFStrPart::Literal(_) => false,
                crate::typed_ast::TypedFStrPart::Interp(expr) => self.expr_mentions_ident(expr, name),
            }),
            TypedExprKind::Ident(_)
            | TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => false,
        }
    }

    fn last_call(&self, name: &str, block: &TypedBlock) -> Option<Span> {
        let mut last = None;
        for stmt in &block.stmts {
            self.last_call_in_stmt(name, stmt, &mut last);
        }
        last
    }

    fn last_call_in_stmt(&self, name: &str, stmt: &TypedStmt, last: &mut Option<Span>) {
        match &stmt.kind {
            TypedStmtKind::Expr(expr) => self.last_call_in_expr(name, expr, last),
            TypedStmtKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.last_call_in_expr(name, value, last);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => self.last_call_in_expr(name, &stmt.value, last),
            TypedStmtKind::Assign(assign) => {
                self.last_call_in_expr(name, &assign.target, last);
                if let Some(value) = &assign.value {
                    self.last_call_in_expr(name, value, last);
                }
            }
            TypedStmtKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.last_call_in_expr(name, value, last);
                }
            }
            TypedStmtKind::If(if_stmt) => {
                self.last_call_in_expr(name, &if_stmt.condition, last);
                for stmt in &if_stmt.then_branch.stmts {
                    self.last_call_in_stmt(name, stmt, last);
                }
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch {
                        TypedElseBranch::If(if_stmt) => {
                            for stmt in &if_stmt.then_branch.stmts {
                                self.last_call_in_stmt(name, stmt, last);
                            }
                        }
                        TypedElseBranch::Block(block) => {
                            for stmt in &block.stmts {
                                self.last_call_in_stmt(name, stmt, last);
                            }
                        }
                    }
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.last_call_in_expr(name, &for_stmt.iter, last);
                for stmt in &for_stmt.body.stmts {
                    self.last_call_in_stmt(name, stmt, last);
                }
            }
            TypedStmtKind::While(while_stmt) => {
                self.last_call_in_expr(name, &while_stmt.condition, last);
                for stmt in &while_stmt.body.stmts {
                    self.last_call_in_stmt(name, stmt, last);
                }
            }
            TypedStmtKind::Spawn(spawn) => {
                if let TypedSpawnBody::Expr(expr) = &spawn.body {
                    self.last_call_in_expr(name, expr, last);
                }
            }
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => {
                for stmt in &block.stmts {
                    self.last_call_in_stmt(name, stmt, last);
                }
            }
            TypedStmtKind::IfCompile(if_compile) => {
                self.last_call_in_expr(name, &if_compile.condition, last)
            }
            TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::GcConfig(_)
            | TypedStmtKind::TypeBlock(_) => {}
        }
    }

    fn last_call_in_expr(&self, name: &str, expr: &TypedExpr, last: &mut Option<Span>) {
        let _ = self;
        match &expr.kind {
            TypedExprKind::Call(callee, args) => {
                if matches!(&callee.kind, TypedExprKind::Ident(candidate) if candidate == name) {
                    *last = Some(expr.span);
                }
                self.last_call_in_expr(name, callee, last);
                for arg in args {
                    self.last_call_in_expr(name, arg, last);
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.last_call_in_expr(name, target, last);
                for arg in args {
                    self.last_call_in_expr(name, arg, last);
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.last_call_in_expr(name, item, last);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.last_call_in_expr(name, key, last);
                    self.last_call_in_expr(name, value, last);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.last_call_in_expr(name, lhs, last);
                self.last_call_in_expr(name, rhs, last);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Index(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.last_call_in_expr(name, inner, last),
            TypedExprKind::Lambda(_, body) => self.last_call_in_expr(name, body, last),
            TypedExprKind::Match(subject, arms) => {
                self.last_call_in_expr(name, subject, last);
                for arm in arms {
                    self.last_call_in_expr(name, &arm.pattern, last);
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.last_call_in_expr(name, expr, last);
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.last_call_in_expr(name, expr, last);
                    }
                }
            }
            TypedExprKind::Ident(_)
            | TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn compute_live_after_block(&self, block: &TypedBlock) -> Vec<HashSet<String>> {
        let mut live_after = vec![HashSet::new(); block.stmts.len()];
        let mut live = HashSet::new();
        for (index, stmt) in block.stmts.iter().enumerate().rev() {
            live_after[index] = live.clone();
            let used = self.stmt_used_bindings(stmt);
            let defined = self.stmt_defined_bindings(stmt);
            for binding in defined {
                live.remove(&binding);
            }
            live.extend(used);
        }
        live_after
    }

    fn stmt_used_bindings(&self, stmt: &TypedStmt) -> HashSet<String> {
        let mut used = HashSet::new();
        self.collect_used_bindings_in_stmt(stmt, &mut used);
        used
    }

    fn collect_used_bindings_in_stmt(&self, stmt: &TypedStmt, used: &mut HashSet<String>) {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.collect_used_bindings_in_expr(value, used);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => {
                self.collect_used_bindings_in_expr(&stmt.value, used)
            }
            TypedStmtKind::Assign(assign) => {
                self.collect_used_bindings_in_expr(&assign.target, used);
                if let Some(value) = &assign.value {
                    self.collect_used_bindings_in_expr(value, used);
                }
            }
            TypedStmtKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_used_bindings_in_expr(value, used);
                }
            }
            TypedStmtKind::Expr(expr) => self.collect_used_bindings_in_expr(expr, used),
            TypedStmtKind::If(if_stmt) => {
                self.collect_used_bindings_in_expr(&if_stmt.condition, used);
                for stmt in &if_stmt.then_branch.stmts {
                    self.collect_used_bindings_in_stmt(stmt, used);
                }
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch {
                        TypedElseBranch::If(if_stmt) => {
                            for stmt in &if_stmt.then_branch.stmts {
                                self.collect_used_bindings_in_stmt(stmt, used);
                            }
                        }
                        TypedElseBranch::Block(block) => {
                            for stmt in &block.stmts {
                                self.collect_used_bindings_in_stmt(stmt, used);
                            }
                        }
                    }
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.collect_used_bindings_in_expr(&for_stmt.iter, used);
                for stmt in &for_stmt.body.stmts {
                    self.collect_used_bindings_in_stmt(stmt, used);
                }
            }
            TypedStmtKind::While(while_stmt) => {
                self.collect_used_bindings_in_expr(&while_stmt.condition, used);
                for stmt in &while_stmt.body.stmts {
                    self.collect_used_bindings_in_stmt(stmt, used);
                }
            }
            TypedStmtKind::Spawn(spawn) => {
                if let TypedSpawnBody::Expr(expr) = &spawn.body {
                    self.collect_used_bindings_in_expr(expr, used);
                }
            }
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => {
                for stmt in &block.stmts {
                    self.collect_used_bindings_in_stmt(stmt, used);
                }
            }
            TypedStmtKind::IfCompile(if_compile) => {
                self.collect_used_bindings_in_expr(&if_compile.condition, used);
            }
            TypedStmtKind::GcConfig(gc) => {
                for entry in &gc.entries {
                    self.collect_used_bindings_in_expr(&entry.value, used);
                }
            }
            TypedStmtKind::AsmBlock(_) | TypedStmtKind::TypeBlock(_) => {}
        }
    }

    fn collect_used_bindings_in_expr(&self, expr: &TypedExpr, used: &mut HashSet<String>) {
        match &expr.kind {
            TypedExprKind::Ident(name) => {
                used.insert(name.clone());
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.collect_used_bindings_in_expr(item, used);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.collect_used_bindings_in_expr(key, used);
                    self.collect_used_bindings_in_expr(value, used);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.collect_used_bindings_in_expr(lhs, used);
                self.collect_used_bindings_in_expr(rhs, used);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.collect_used_bindings_in_expr(inner, used),
            TypedExprKind::Call(callee, args) => {
                self.collect_used_bindings_in_expr(callee, used);
                for arg in args {
                    self.collect_used_bindings_in_expr(arg, used);
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.collect_used_bindings_in_expr(target, used);
                for arg in args {
                    self.collect_used_bindings_in_expr(arg, used);
                }
            }
            TypedExprKind::Index(target, index) => {
                self.collect_used_bindings_in_expr(target, used);
                self.collect_used_bindings_in_expr(index, used);
            }
            TypedExprKind::Lambda(_, body) => self.collect_used_bindings_in_expr(body, used),
            TypedExprKind::Match(subject, arms) => {
                self.collect_used_bindings_in_expr(subject, used);
                for arm in arms {
                    self.collect_used_bindings_in_expr(&arm.pattern, used);
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.collect_used_bindings_in_expr(expr, used);
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.collect_used_bindings_in_expr(expr, used);
                    }
                }
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn stmt_defined_bindings(&self, stmt: &TypedStmt) -> HashSet<String> {
        let mut defined = HashSet::new();
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                defined.insert(let_stmt.name.clone());
            }
            TypedStmtKind::LetDestructure(stmt) => {
                for binding in &stmt.bindings {
                    if let crate::typed_ast::TypedDestructureBinding::Name { name, .. } = binding {
                        defined.insert(name.clone());
                    }
                }
            }
            TypedStmtKind::For(for_stmt) => {
                defined.insert(for_stmt.name.clone());
            }
            _ => {}
        }
        defined
    }

    fn analyze_block(
        &mut self,
        block: &mut TypedBlock,
        env: &mut OwnershipEnv,
        fn_key: &str,
        closures: &HashMap<String, ClosureMeta>,
        live_after: &[HashSet<String>],
        errors: &mut Vec<OwnershipError>,
    ) {
        for (index, stmt) in block.stmts.iter_mut().enumerate() {
            let live = live_after.get(index).cloned().unwrap_or_default();
            self.analyze_stmt(stmt, env, fn_key, closures, &live, errors);
        }
    }

    fn analyze_stmt(
        &mut self,
        stmt: &mut TypedStmt,
        env: &mut OwnershipEnv,
        fn_key: &str,
        closures: &HashMap<String, ClosureMeta>,
        live_after: &HashSet<String>,
        errors: &mut Vec<OwnershipError>,
    ) {
        match &mut stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                let origin = if let Some(value) = &mut let_stmt.value {
                    self.analyze_initializer(value, env, closures, errors)
                } else {
                    None
                };
                env.bindings.insert(
                    let_stmt.name.clone(),
                    BindingState {
                        name: let_stmt.name.clone(),
                        ty: let_stmt.ty.clone(),
                        state: OwnershipState::Owned,
                        state_span: let_stmt.span,
                        origin: if self.is_copy_type(&let_stmt.ty) {
                            None
                        } else {
                            origin.or_else(|| Some(self.new_origin()))
                        },
                        is_mut: let_stmt.is_mut,
                        is_param: false,
                        is_closure: matches!(
                            let_stmt.value.as_ref().map(|value| &value.kind),
                            Some(TypedExprKind::Lambda(_, _))
                        ),
                    },
                );
                if let Some(meta) = closures.get(&let_stmt.name) {
                    self.apply_non_escaping_closure_borrows(meta, env, errors);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => {
                let origin = self.analyze_initializer(&mut stmt.value, env, closures, errors);
                for binding in &stmt.bindings {
                    if let crate::typed_ast::TypedDestructureBinding::Name { name, ty } = binding {
                        env.bindings.insert(
                            name.clone(),
                            BindingState {
                                name: name.clone(),
                                ty: ty.clone(),
                                state: OwnershipState::Owned,
                                state_span: stmt.span,
                                origin: if self.is_copy_type(ty) {
                                    None
                                } else {
                                    origin.or_else(|| Some(self.new_origin()))
                                },
                                is_mut: stmt.is_mut,
                                is_param: false,
                                is_closure: false,
                            },
                        );
                    }
                }
            }
            TypedStmtKind::Assign(assign) => self.analyze_assign(assign, env, closures, errors),
            TypedStmtKind::Return(ret) => self.analyze_return(ret, env, errors),
            TypedStmtKind::Expr(expr) => {
                let _ = self.analyze_expr(expr, env, UseEffect::BorrowShared, closures, errors);
            }
            TypedStmtKind::If(if_stmt) => self.analyze_if(if_stmt, env, fn_key, closures, errors),
            TypedStmtKind::For(for_stmt) => {
                self.analyze_for(for_stmt, env, fn_key, closures, errors)
            }
            TypedStmtKind::While(while_stmt) => {
                self.analyze_while(while_stmt, env, fn_key, closures, errors)
            }
            TypedStmtKind::Spawn(spawn) => {
                if let TypedSpawnBody::Expr(expr) = &mut spawn.body {
                    let _ = self.analyze_expr(expr, env, UseEffect::BorrowShared, closures, errors);
                }
            }
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => {
                let live = self.compute_live_after_block(block);
                let mut inner = env.clone();
                self.analyze_block(block, &mut inner, fn_key, closures, &live, errors);
                *env = self.join_envs(env, &inner);
            }
            TypedStmtKind::PointerBlock(block) => {
                self.reject_pointer_block_crossings(block, env, errors);
            }
            TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::GcConfig(_)
            | TypedStmtKind::IfCompile(_)
            | TypedStmtKind::TypeBlock(_) => {}
        }

        let to_free = env
            .bindings
            .iter()
            .filter(|(name, binding)| {
                !live_after.contains(*name)
                    && matches!(binding.state, OwnershipState::Owned)
                    && !self.is_copy_type(&binding.ty)
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in to_free {
            self.record_free(fn_key, &name, stmt.span);
            if let Some(binding) = env.bindings.get_mut(&name) {
                binding.state = OwnershipState::Escaped;
                binding.state_span = stmt.span;
            }
        }
        self.clear_temporary_borrows(env);
    }

    fn analyze_initializer(
        &mut self,
        expr: &mut TypedExpr,
        env: &mut OwnershipEnv,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) -> Option<usize> {
        if matches!(expr.kind, TypedExprKind::Lambda(_, _)) {
            let _ = self.analyze_expr(expr, env, UseEffect::BorrowShared, closures, errors);
            return Some(self.new_origin());
        }
        if self.is_expr_copy(expr) {
            let _ = self.analyze_expr(expr, env, UseEffect::Copy, closures, errors);
            None
        } else {
            self.analyze_expr(expr, env, UseEffect::Move, closures, errors)
                .or_else(|| Some(self.new_origin()))
        }
    }

    fn reject_pointer_block_crossings(
        &self,
        block: &TypedBlock,
        env: &OwnershipEnv,
        errors: &mut Vec<OwnershipError>,
    ) {
        let mut crossings = Vec::new();
        self.collect_pointer_crossings_in_block(block, &mut crossings);
        for (name, span) in crossings {
            let Some(binding) = env.bindings.get(&name) else {
                continue;
            };
            if !self.is_copy_type(&binding.ty) && matches!(binding.state, OwnershipState::Owned) {
                errors.push(OwnershipError::SafeToRawAliasRejection { name, span });
            }
        }
    }

    fn collect_pointer_crossings_in_block(
        &self,
        block: &TypedBlock,
        out: &mut Vec<(String, Span)>,
    ) {
        for stmt in &block.stmts {
            self.collect_pointer_crossings_in_stmt(stmt, out);
        }
    }

    fn collect_pointer_crossings_in_stmt(&self, stmt: &TypedStmt, out: &mut Vec<(String, Span)>) {
        let _ = self;
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.collect_pointer_crossings_in_expr(value, out);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => {
                self.collect_pointer_crossings_in_expr(&stmt.value, out);
            }
            TypedStmtKind::Assign(assign) => {
                self.collect_pointer_crossings_in_expr(&assign.target, out);
                if let Some(value) = &assign.value {
                    self.collect_pointer_crossings_in_expr(value, out);
                }
            }
            TypedStmtKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_pointer_crossings_in_expr(value, out);
                }
            }
            TypedStmtKind::Expr(expr) => self.collect_pointer_crossings_in_expr(expr, out),
            TypedStmtKind::If(if_stmt) => {
                self.collect_pointer_crossings_in_expr(&if_stmt.condition, out);
                self.collect_pointer_crossings_in_block(&if_stmt.then_branch, out);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch {
                        TypedElseBranch::If(inner) => {
                            self.collect_pointer_crossings_in_expr(&inner.condition, out);
                            self.collect_pointer_crossings_in_block(&inner.then_branch, out);
                            if let Some(TypedElseBranch::Block(block)) = &inner.else_branch {
                                self.collect_pointer_crossings_in_block(block, out);
                            }
                        }
                        TypedElseBranch::Block(block) => {
                            self.collect_pointer_crossings_in_block(block, out)
                        }
                    }
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.collect_pointer_crossings_in_expr(&for_stmt.iter, out);
                self.collect_pointer_crossings_in_block(&for_stmt.body, out);
            }
            TypedStmtKind::While(while_stmt) => {
                self.collect_pointer_crossings_in_expr(&while_stmt.condition, out);
                self.collect_pointer_crossings_in_block(&while_stmt.body, out);
            }
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => self.collect_pointer_crossings_in_expr(expr, out),
                TypedSpawnBody::Block(block) => self.collect_pointer_crossings_in_block(block, out),
            },
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => {
                self.collect_pointer_crossings_in_block(block, out)
            }
            TypedStmtKind::IfCompile(if_compile) => {
                self.collect_pointer_crossings_in_expr(&if_compile.condition, out);
                self.collect_pointer_crossings_in_block(&if_compile.body, out);
            }
            TypedStmtKind::GcConfig(gc) => {
                for entry in &gc.entries {
                    self.collect_pointer_crossings_in_expr(&entry.value, out);
                }
            }
            TypedStmtKind::TypeBlock(_) | TypedStmtKind::AsmBlock(_) => {}
        }
    }

    fn collect_pointer_crossings_in_expr(&self, expr: &TypedExpr, out: &mut Vec<(String, Span)>) {
        let _ = self;
        match &expr.kind {
            TypedExprKind::Ident(name) => out.push((name.clone(), expr.span)),
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.collect_pointer_crossings_in_expr(item, out);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.collect_pointer_crossings_in_expr(key, out);
                    self.collect_pointer_crossings_in_expr(value, out);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.collect_pointer_crossings_in_expr(lhs, out);
                self.collect_pointer_crossings_in_expr(rhs, out);
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => self.collect_pointer_crossings_in_expr(inner, out),
            TypedExprKind::Index(target, index) => {
                self.collect_pointer_crossings_in_expr(target, out);
                self.collect_pointer_crossings_in_expr(index, out);
            }
            TypedExprKind::Call(callee, args) => {
                self.collect_pointer_crossings_in_expr(callee, out);
                for arg in args {
                    self.collect_pointer_crossings_in_expr(arg, out);
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.collect_pointer_crossings_in_expr(target, out);
                for arg in args {
                    self.collect_pointer_crossings_in_expr(arg, out);
                }
            }
            TypedExprKind::Lambda(_, body) => self.collect_pointer_crossings_in_expr(body, out),
            TypedExprKind::Match(subject, arms) => {
                self.collect_pointer_crossings_in_expr(subject, out);
                for arm in arms {
                    self.collect_pointer_crossings_in_expr(&arm.pattern, out);
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.collect_pointer_crossings_in_expr(expr, out);
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.collect_pointer_crossings_in_expr(expr, out);
                    }
                }
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn analyze_assign(
        &mut self,
        assign: &mut TypedAssignStmt,
        env: &mut OwnershipEnv,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) {
        match &mut assign.target.kind {
            TypedExprKind::Ident(name) => {
                if let Some(binding) = env.bindings.get(name).cloned() {
                    if matches!(binding.state, OwnershipState::Owned)
                        && !self.is_copy_type(&binding.ty)
                    {
                        self.record_free("assign", name, assign.span);
                    }
                }
                let next_origin = assign
                    .value
                    .as_mut()
                    .and_then(|value| self.analyze_initializer(value, env, closures, errors));
                if let Some(binding) = env.bindings.get_mut(name) {
                    binding.state = OwnershipState::Owned;
                    binding.state_span = assign.span;
                    binding.origin = if self.is_copy_type(&binding.ty) {
                        None
                    } else {
                        next_origin.or_else(|| Some(self.new_origin()))
                    };
                }
            }
            TypedExprKind::Field(target, field) => {
                let parent =
                    self.analyze_expr(target, env, UseEffect::BorrowExclusive, closures, errors);
                let child = assign
                    .value
                    .as_mut()
                    .and_then(|value| self.analyze_initializer(value, env, closures, errors));
                if let (Some(parent), Some(child)) = (parent, child) {
                    self.attach_child(env, parent, child, assign.span, errors);
                } else if matches!(target.kind, TypedExprKind::Ident(ref name) if name == "self")
                    && matches!(assign.value.as_ref().map(|value| &value.kind), Some(TypedExprKind::Ident(name)) if name == "self")
                {
                    let _ = field;
                    errors.push(OwnershipError::OwnershipCycle { span: assign.span });
                }
            }
            TypedExprKind::Index(target, index) => {
                let parent =
                    self.analyze_expr(target, env, UseEffect::BorrowExclusive, closures, errors);
                let _ = self.analyze_expr(index, env, UseEffect::Copy, closures, errors);
                let child = assign
                    .value
                    .as_mut()
                    .and_then(|value| self.analyze_initializer(value, env, closures, errors));
                if let (Some(parent), Some(child)) = (parent, child) {
                    self.attach_child(env, parent, child, assign.span, errors);
                }
            }
            _ => {
                let _ = self.analyze_expr(
                    &mut assign.target,
                    env,
                    UseEffect::BorrowExclusive,
                    closures,
                    errors,
                );
                if let Some(value) = &mut assign.value {
                    let _ =
                        self.analyze_expr(value, env, UseEffect::BorrowShared, closures, errors);
                }
            }
        }
    }

    fn analyze_return(
        &mut self,
        ret: &mut TypedReturnStmt,
        env: &mut OwnershipEnv,
        errors: &mut Vec<OwnershipError>,
    ) {
        let escaped_origin = ret.value.as_mut().and_then(|value| {
            self.analyze_expr(value, env, UseEffect::Move, &HashMap::new(), errors)
        });
        let to_free = env
            .bindings
            .iter()
            .filter(|(_, binding)| {
                matches!(binding.state, OwnershipState::Owned)
                    && !self.is_copy_type(&binding.ty)
                    && binding.origin != escaped_origin
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in to_free {
            if let Some(binding) = env.bindings.get_mut(&name) {
                binding.state = OwnershipState::Escaped;
                binding.state_span = ret.span;
            }
        }
    }

    fn analyze_if(
        &mut self,
        if_stmt: &mut TypedIfStmt,
        env: &mut OwnershipEnv,
        fn_key: &str,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) {
        let _ = self.analyze_expr(
            &mut if_stmt.condition,
            env,
            UseEffect::Copy,
            closures,
            errors,
        );
        let live_then = self.compute_live_after_block(&if_stmt.then_branch);
        let mut then_env = env.clone();
        self.analyze_block(
            &mut if_stmt.then_branch,
            &mut then_env,
            fn_key,
            closures,
            &live_then,
            errors,
        );
        let mut else_env = env.clone();
        if let Some(else_branch) = &mut if_stmt.else_branch {
            match else_branch {
                TypedElseBranch::If(child) => {
                    self.analyze_if(child, &mut else_env, fn_key, closures, errors)
                }
                TypedElseBranch::Block(block) => {
                    let live = self.compute_live_after_block(block);
                    self.analyze_block(block, &mut else_env, fn_key, closures, &live, errors);
                }
            }
        }
        *env = self.join_envs(&then_env, &else_env);
    }

    fn analyze_for(
        &mut self,
        for_stmt: &mut TypedForStmt,
        env: &mut OwnershipEnv,
        fn_key: &str,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) {
        let _ = self.analyze_expr(
            &mut for_stmt.iter,
            env,
            UseEffect::BorrowShared,
            closures,
            errors,
        );
        let mut loop_env = env.clone();
        loop_env.bindings.insert(
            for_stmt.name.clone(),
            BindingState {
                name: for_stmt.name.clone(),
                ty: for_stmt.item_type.clone(),
                state: OwnershipState::Owned,
                state_span: for_stmt.span,
                origin: (!self.is_copy_type(&for_stmt.item_type)).then(|| self.new_origin()),
                is_mut: false,
                is_param: false,
                is_closure: false,
            },
        );
        let live = self.compute_live_after_block(&for_stmt.body);
        self.analyze_block(
            &mut for_stmt.body,
            &mut loop_env,
            fn_key,
            closures,
            &live,
            errors,
        );
        self.check_loop_reinit(env, &loop_env, for_stmt.span, errors);
        *env = self.join_envs(env, &loop_env);
    }

    fn analyze_while(
        &mut self,
        while_stmt: &mut TypedWhileStmt,
        env: &mut OwnershipEnv,
        fn_key: &str,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) {
        let _ = self.analyze_expr(
            &mut while_stmt.condition,
            env,
            UseEffect::Copy,
            closures,
            errors,
        );
        let mut loop_env = env.clone();
        let live = self.compute_live_after_block(&while_stmt.body);
        self.analyze_block(
            &mut while_stmt.body,
            &mut loop_env,
            fn_key,
            closures,
            &live,
            errors,
        );
        self.check_loop_reinit(env, &loop_env, while_stmt.span, errors);
        *env = self.join_envs(env, &loop_env);
    }

    fn check_loop_reinit(
        &self,
        before: &OwnershipEnv,
        after: &OwnershipEnv,
        span: Span,
        errors: &mut Vec<OwnershipError>,
    ) {
        for (name, binding_before) in &before.bindings {
            let Some(binding_after) = after.bindings.get(name) else {
                continue;
            };
            if matches!(binding_before.state, OwnershipState::Owned)
                && matches!(binding_after.state, OwnershipState::Moved)
            {
                errors.push(OwnershipError::LoopMoveWithoutReinit {
                    name: name.clone(),
                    span,
                });
            }
        }
    }

    fn analyze_expr(
        &mut self,
        expr: &mut TypedExpr,
        env: &mut OwnershipEnv,
        desired: UseEffect,
        closures: &HashMap<String, ClosureMeta>,
        errors: &mut Vec<OwnershipError>,
    ) -> Option<usize> {
        match &mut expr.kind {
            TypedExprKind::Ident(name) => {
                let effect = if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    desired
                };
                expr.use_effect = Some(effect.clone());
                if let Some(binding) = env.bindings.get(name).cloned() {
                    self.check_binding_use(&binding, &effect, expr.span, env, errors);
                    match effect {
                        UseEffect::BorrowShared => {
                            self.start_borrow(env, name, BorrowKind::Shared, expr.span, false)
                        }
                        UseEffect::BorrowExclusive => {
                            self.start_borrow(env, name, BorrowKind::Exclusive, expr.span, false)
                        }
                        UseEffect::Move => {
                            if let Some(binding) = env.bindings.get_mut(name) {
                                binding.state = OwnershipState::Moved;
                                binding.state_span = expr.span;
                            }
                        }
                        UseEffect::Copy => {}
                    }
                    binding.origin
                } else {
                    None
                }
            }
            TypedExprKind::Field(target, field) => {
                let base_origin = self.analyze_expr(
                    target,
                    env,
                    if desired == UseEffect::BorrowExclusive {
                        UseEffect::BorrowExclusive
                    } else {
                        UseEffect::BorrowShared
                    },
                    closures,
                    errors,
                );
                if desired == UseEffect::Move
                    && !self.is_copy_type(&expr.ty)
                    && self.base_ident_name(target).as_deref() != Some("self")
                {
                    errors.push(OwnershipError::PartialMove {
                        field: field.clone(),
                        base: self
                            .base_ident_name(target)
                            .unwrap_or_else(|| "value".to_string()),
                        span: expr.span,
                    });
                }
                expr.use_effect = Some(if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    UseEffect::BorrowShared
                });
                base_origin
            }
            TypedExprKind::Index(target, index) => {
                let base_origin = self.analyze_expr(
                    target,
                    env,
                    if desired == UseEffect::BorrowExclusive {
                        UseEffect::BorrowExclusive
                    } else {
                        UseEffect::BorrowShared
                    },
                    closures,
                    errors,
                );
                let _ = self.analyze_expr(index, env, UseEffect::Copy, closures, errors);
                if desired == UseEffect::Move
                    && !self.is_copy_type(&expr.ty)
                    && self.base_ident_name(target).as_deref() != Some("self")
                {
                    errors.push(OwnershipError::PartialMove {
                        field: "index".to_string(),
                        base: self
                            .base_ident_name(target)
                            .unwrap_or_else(|| "value".to_string()),
                        span: expr.span,
                    });
                }
                expr.use_effect = Some(if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    UseEffect::BorrowShared
                });
                base_origin
            }
            TypedExprKind::Call(callee, args) => {
                if matches!(&callee.kind, TypedExprKind::Ident(name) if env.bindings.contains_key(name))
                    && self.lookup_call_summary(callee).is_none()
                    && !env
                        .bindings
                        .get(match &callee.kind {
                            TypedExprKind::Ident(name) => name,
                            _ => unreachable!(),
                        })
                        .and_then(|binding| self.effect_contract_from_type(&binding.ty))
                        .is_some()
                {
                    if let Some(arg) = args.iter().find(|arg| !self.is_expr_copy(arg)) {
                        if let Some(name) = self.base_ident_name(arg) {
                            errors.push(OwnershipError::AmbiguousCallOwnership {
                                name,
                                span: expr.span,
                            });
                        }
                    }
                }
                let effects = self.call_effects(callee, args, env);
                let _ = self.analyze_expr(callee, env, UseEffect::Copy, closures, errors);
                let mut moved = None;
                for (arg, effect) in args.iter_mut().zip(effects.iter()) {
                    let origin = self.analyze_expr(arg, env, effect.clone(), closures, errors);
                    if *effect == UseEffect::Move {
                        moved = origin;
                    }
                }
                expr.use_effect = Some(if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    desired.clone()
                });
                moved.or_else(|| (!self.is_expr_copy(expr)).then(|| self.new_origin()))
            }
            TypedExprKind::MethodCall(target, name, args) => {
                let receiver_effect = self.method_receiver_effect(&target.ty, name);
                let target_origin =
                    self.analyze_expr(target, env, receiver_effect, closures, errors);
                let arg_effects = self.method_arg_effects(&target.ty, name, args);
                for (arg, effect) in args.iter_mut().zip(arg_effects) {
                    let _ = self.analyze_expr(arg, env, effect, closures, errors);
                }
                expr.use_effect = Some(if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    desired.clone()
                });
                if matches!(name.as_str(), "push" | "pop") {
                    target_origin
                } else {
                    None
                }
            }
            TypedExprKind::Lambda(params, body) => {
                let scope = env.bindings.keys().cloned().collect::<HashSet<_>>();
                let captures = self.collect_captures(params, body, &scope);
                let exclusive_captures =
                    self.collect_exclusive_captures(params, body, &scope, &captures);
                let escaping = desired == UseEffect::Move;
                for capture in captures {
                    if let Some(binding) = env.bindings.get(&capture).cloned() {
                        let effect = if escaping {
                            UseEffect::Move
                        } else if exclusive_captures.contains(&capture) {
                            UseEffect::BorrowExclusive
                        } else {
                            UseEffect::BorrowShared
                        };
                        if effect == UseEffect::Move
                            && matches!(binding.state, OwnershipState::Moved)
                        {
                            errors.push(OwnershipError::MultipleOwners {
                                name: capture.clone(),
                                span: expr.span,
                            });
                        } else {
                            self.check_binding_use(&binding, &effect, expr.span, env, errors);
                        }
                        if effect == UseEffect::Move
                            && !matches!(binding.state, OwnershipState::Moved)
                        {
                            if let Some(current) = env.bindings.get_mut(&capture) {
                                current.state = OwnershipState::Moved;
                                current.state_span = expr.span;
                            }
                        } else if matches!(
                            effect,
                            UseEffect::BorrowShared | UseEffect::BorrowExclusive
                        ) {
                            self.start_borrow(
                                env,
                                &capture,
                                if effect == UseEffect::BorrowExclusive {
                                    BorrowKind::Exclusive
                                } else {
                                    BorrowKind::Shared
                                },
                                expr.span,
                                true,
                            );
                        }
                    }
                }
                expr.use_effect = Some(UseEffect::Move);
                Some(self.new_origin())
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    let _ = self.analyze_expr(
                        item,
                        env,
                        if self.is_expr_copy(item) {
                            UseEffect::Copy
                        } else {
                            UseEffect::Move
                        },
                        closures,
                        errors,
                    );
                }
                expr.use_effect = Some(if self.is_expr_copy(expr) {
                    UseEffect::Copy
                } else {
                    UseEffect::Move
                });
                (!self.is_expr_copy(expr)).then(|| self.new_origin())
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    let _ = self.analyze_expr(key, env, UseEffect::Copy, closures, errors);
                    let _ = self.analyze_expr(
                        value,
                        env,
                        if self.is_expr_copy(value) {
                            UseEffect::Copy
                        } else {
                            UseEffect::Move
                        },
                        closures,
                        errors,
                    );
                }
                expr.use_effect = Some(UseEffect::Move);
                Some(self.new_origin())
            }
            TypedExprKind::Match(subject, arms) => {
                let _ = self.analyze_expr(subject, env, UseEffect::BorrowShared, closures, errors);
                expr.use_effect = Some(if self.is_copy_type(&expr.ty) {
                    UseEffect::Copy
                } else {
                    desired.clone()
                });
                for arm in arms {
                    let _ = self.analyze_expr(
                        &mut arm.pattern,
                        env,
                        UseEffect::BorrowShared,
                        closures,
                        errors,
                    );
                    if let TypedMatchArmBody::Expr(expr) = &mut arm.body {
                        let _ =
                            self.analyze_expr(expr, env, UseEffect::BorrowShared, closures, errors);
                    }
                }
                None
            }
            TypedExprKind::BinOp(lhs, op, rhs) => {
                let _ = self.analyze_expr(
                    lhs,
                    env,
                    if self.binop_moves_lhs(*op, lhs, rhs) {
                        UseEffect::Move
                    } else {
                        UseEffect::BorrowShared
                    },
                    closures,
                    errors,
                );
                let _ = self.analyze_expr(
                    rhs,
                    env,
                    if self.is_expr_copy(rhs) {
                        UseEffect::Copy
                    } else {
                        UseEffect::BorrowShared
                    },
                    closures,
                    errors,
                );
                expr.use_effect = Some(if self.is_expr_copy(expr) {
                    UseEffect::Copy
                } else {
                    desired
                });
                None
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => {
                let origin = self.analyze_expr(
                    inner,
                    env,
                    if self.is_expr_copy(inner) {
                        UseEffect::Copy
                    } else {
                        desired.clone()
                    },
                    closures,
                    errors,
                );
                expr.use_effect = Some(if self.is_expr_copy(expr) {
                    UseEffect::Copy
                } else {
                    desired
                });
                origin
            }
            TypedExprKind::Nullish(lhs, rhs) => {
                let _ = self.analyze_expr(lhs, env, UseEffect::BorrowShared, closures, errors);
                let _ = self.analyze_expr(rhs, env, UseEffect::BorrowShared, closures, errors);
                expr.use_effect = Some(if self.is_expr_copy(expr) {
                    UseEffect::Copy
                } else {
                    desired
                });
                None
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        let _ = self.analyze_expr(
                            expr,
                            env,
                            if self.is_expr_copy(expr) {
                                UseEffect::Copy
                            } else {
                                UseEffect::BorrowShared
                            },
                            closures,
                            errors,
                        );
                    }
                }
                expr.use_effect = Some(UseEffect::Move);
                Some(self.new_origin())
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {
                expr.use_effect = Some(if self.is_expr_copy(expr) {
                    UseEffect::Copy
                } else {
                    desired
                });
                None
            }
        }
    }

    fn apply_non_escaping_closure_borrows(
        &self,
        meta: &ClosureMeta,
        env: &mut OwnershipEnv,
        errors: &mut Vec<OwnershipError>,
    ) {
        if meta.escaping {
            return;
        }
        for capture in &meta.captures {
            if let Some(binding) = env.bindings.get(capture).cloned() {
                let effect = if meta.exclusive_captures.contains(capture) {
                    UseEffect::BorrowExclusive
                } else {
                    UseEffect::BorrowShared
                };
                self.check_binding_use(&binding, &effect, binding.state_span, env, errors);
                self.start_borrow(
                    env,
                    capture,
                    if meta.exclusive_captures.contains(capture) {
                        BorrowKind::Exclusive
                    } else {
                        BorrowKind::Shared
                    },
                    meta.last_call.unwrap_or(binding.state_span),
                    true,
                );
            }
        }
    }

    fn check_binding_use(
        &self,
        binding: &BindingState,
        effect: &UseEffect,
        span: Span,
        env: &OwnershipEnv,
        errors: &mut Vec<OwnershipError>,
    ) {
        if matches!(binding.state, OwnershipState::Moved) {
            errors.push(OwnershipError::UseAfterMove {
                name: binding.name.clone(),
                move_span: binding.state_span,
                use_span: span,
            });
            return;
        }
        let borrows = env.borrows.get(&binding.name).cloned().unwrap_or_default();
        let shared = borrows
            .iter()
            .any(|borrow| borrow.kind == BorrowKind::Shared);
        let exclusive = borrows
            .iter()
            .any(|borrow| borrow.kind == BorrowKind::Exclusive);
        match effect {
            UseEffect::BorrowShared if exclusive => {
                errors.push(OwnershipError::ReadDuringExclusiveBorrow {
                    name: binding.name.clone(),
                    borrow_span: borrows
                        .first()
                        .map(|borrow| borrow.span)
                        .unwrap_or(binding.state_span),
                    read_span: span,
                })
            }
            UseEffect::BorrowExclusive if shared => {
                errors.push(OwnershipError::ExclusiveBorrowDuringRead {
                    name: binding.name.clone(),
                    read_span: borrows
                        .first()
                        .map(|borrow| borrow.span)
                        .unwrap_or(binding.state_span),
                    modify_span: span,
                })
            }
            UseEffect::Move if (shared || exclusive) => {
                errors.push(OwnershipError::MoveWhileBorrowed {
                    name: binding.name.clone(),
                    borrow_span: borrows
                        .first()
                        .map(|borrow| borrow.span)
                        .unwrap_or(binding.state_span),
                    move_span: span,
                })
            }
            _ => {}
        }
    }

    fn start_borrow(
        &self,
        env: &mut OwnershipEnv,
        name: &str,
        kind: BorrowKind,
        span: Span,
        persistent: bool,
    ) {
        env.borrows
            .entry(name.to_string())
            .or_default()
            .push(BorrowRecord {
                kind,
                span,
                persistent,
            });
        if let Some(binding) = env.bindings.get_mut(name) {
            binding.state = if kind == BorrowKind::Exclusive {
                OwnershipState::BorrowedExclusive
            } else {
                OwnershipState::BorrowedShared
            };
            binding.state_span = span;
        }
    }

    fn call_effects(
        &self,
        callee: &TypedExpr,
        args: &[TypedExpr],
        env: &OwnershipEnv,
    ) -> Vec<UseEffect> {
        if let Some(summary) = self.lookup_call_summary(callee) {
            return args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if self.is_expr_copy(arg) {
                        UseEffect::Copy
                    } else {
                        summary
                            .summary
                            .params
                            .get(index)
                            .map(|param| param.effect.clone())
                            .unwrap_or(UseEffect::BorrowShared)
                    }
                })
                .collect();
        }
        if let TypedExprKind::Ident(name) = &callee.kind {
            if let Some(effect) = env
                .bindings
                .get(name)
                .and_then(|binding| self.effect_contract_from_type(&binding.ty))
            {
                return args
                    .iter()
                    .map(|arg| {
                        if self.is_expr_copy(arg) {
                            UseEffect::Copy
                        } else {
                            effect.clone()
                        }
                    })
                    .collect();
            }
        }
        args.iter()
            .map(|arg| {
                if self.is_expr_copy(arg) {
                    UseEffect::Copy
                } else {
                    UseEffect::BorrowShared
                }
            })
            .collect()
    }

    fn lookup_call_summary(&self, callee: &TypedExpr) -> Option<&InternalFnSummary> {
        match &callee.kind {
            TypedExprKind::Ident(name) => self.summaries.get(name),
            _ => None,
        }
    }

    fn effect_contract_from_type(&self, ty: &Type) -> Option<UseEffect> {
        match ty {
            Type::Fn(_, ret) => match ret.as_ref() {
                Type::Named(name, _) if name == "borrow" => Some(UseEffect::BorrowShared),
                Type::Named(name, _) if name == "move" => Some(UseEffect::Move),
                _ => None,
            },
            _ => None,
        }
    }

    fn method_receiver_effect(&self, target_ty: &Type, name: &str) -> UseEffect {
        match target_ty {
            Type::Array(_) | Type::Set(_) | Type::Chan(_) => match name {
                "push" | "pop" => UseEffect::BorrowExclusive,
                _ => UseEffect::BorrowShared,
            },
            Type::String => UseEffect::BorrowShared,
            Type::Named(class_name, _) => self
                .index
                .methods
                .get(&(class_name.clone(), name.to_string()))
                .and_then(|key| self.summaries.get(key))
                .and_then(|summary| summary.receiver_effect.clone())
                .unwrap_or(UseEffect::BorrowShared),
            _ => {
                if self.is_copy_type(target_ty) {
                    UseEffect::Copy
                } else {
                    UseEffect::BorrowShared
                }
            }
        }
    }

    fn method_arg_effects(
        &self,
        target_ty: &Type,
        name: &str,
        args: &[TypedExpr],
    ) -> Vec<UseEffect> {
        match target_ty {
            Type::Array(_) | Type::Set(_) | Type::Chan(_) if name == "push" => args
                .iter()
                .map(|arg| {
                    if self.is_expr_copy(arg) {
                        UseEffect::Copy
                    } else {
                        UseEffect::Move
                    }
                })
                .collect(),
            Type::Named(class_name, _) => self
                .index
                .methods
                .get(&(class_name.clone(), name.to_string()))
                .and_then(|key| self.summaries.get(key))
                .map(|summary| {
                    args.iter()
                        .enumerate()
                        .map(|(index, arg)| {
                            if self.is_expr_copy(arg) {
                                UseEffect::Copy
                            } else {
                                summary
                                    .summary
                                    .params
                                    .get(index)
                                    .map(|param| param.effect.clone())
                                    .unwrap_or(UseEffect::BorrowShared)
                            }
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    args.iter()
                        .map(|arg| {
                            if self.is_expr_copy(arg) {
                                UseEffect::Copy
                            } else {
                                UseEffect::BorrowShared
                            }
                        })
                        .collect()
                }),
            _ => args
                .iter()
                .map(|arg| {
                    if self.is_expr_copy(arg) {
                        UseEffect::Copy
                    } else {
                        UseEffect::BorrowShared
                    }
                })
                .collect(),
        }
    }

    fn attach_child(
        &self,
        env: &mut OwnershipEnv,
        parent: usize,
        child: usize,
        span: Span,
        errors: &mut Vec<OwnershipError>,
    ) {
        if parent == child
            || self.has_ancestor(env, parent, child)
            || env.origin_parents.contains_key(&child)
        {
            errors.push(OwnershipError::OwnershipCycle { span });
            return;
        }
        env.origin_parents.insert(child, parent);
    }

    fn has_ancestor(&self, env: &OwnershipEnv, start: usize, target: usize) -> bool {
        let mut current = Some(start);
        while let Some(origin) = current {
            if origin == target {
                return true;
            }
            current = env.origin_parents.get(&origin).copied();
        }
        false
    }

    fn join_envs(&self, left: &OwnershipEnv, right: &OwnershipEnv) -> OwnershipEnv {
        let mut env = left.clone();
        for (name, left_binding) in &left.bindings {
            if let Some(right_binding) = right.bindings.get(name) {
                if let Some(binding) = env.bindings.get_mut(name) {
                    binding.state = match (&left_binding.state, &right_binding.state) {
                        (OwnershipState::Moved, _) | (_, OwnershipState::Moved) => {
                            OwnershipState::Moved
                        }
                        (OwnershipState::Escaped, OwnershipState::Escaped) => {
                            OwnershipState::Escaped
                        }
                        _ => OwnershipState::Owned,
                    };
                    if binding.origin.is_none() {
                        binding.origin = right_binding.origin;
                    }
                }
            }
        }
        env.borrows.clear();
        env
    }

    fn free_owned_bindings(&mut self, fn_key: &str, env: &OwnershipEnv, span: Span) {
        let to_free = env
            .bindings
            .iter()
            .filter(|(_, binding)| {
                matches!(binding.state, OwnershipState::Owned) && !self.is_copy_type(&binding.ty)
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in to_free {
            self.record_free(fn_key, &name, span);
        }
    }

    fn record_free(&mut self, fn_key: &str, name: &str, span: Span) {
        self.free_points
            .entry(format!("{fn_key}:{name}"))
            .or_default()
            .push(span);
    }

    fn clear_temporary_borrows(&self, env: &mut OwnershipEnv) {
        let names = env.borrows.keys().cloned().collect::<Vec<_>>();
        for name in names {
            if let Some(records) = env.borrows.get_mut(&name) {
                records.retain(|record| record.persistent);
            }
            if env
                .borrows
                .get(&name)
                .is_none_or(|records| records.is_empty())
            {
                env.borrows.remove(&name);
                if let Some(binding) = env.bindings.get_mut(&name) {
                    if matches!(
                        binding.state,
                        OwnershipState::BorrowedShared | OwnershipState::BorrowedExclusive
                    ) {
                        binding.state = OwnershipState::Owned;
                    }
                }
            }
        }
    }

    fn base_ident_name(&self, expr: &TypedExpr) -> Option<String> {
        let _ = self;
        match &expr.kind {
            TypedExprKind::Ident(name) => Some(name.clone()),
            TypedExprKind::Field(target, _) | TypedExprKind::Index(target, _) => {
                self.base_ident_name(target)
            }
            _ => None,
        }
    }

    fn binop_moves_lhs(&self, op: BinOp, lhs: &TypedExpr, rhs: &TypedExpr) -> bool {
        matches!(op, BinOp::Add) && matches!(lhs.ty, Type::String) && matches!(rhs.ty, Type::String)
    }

    fn new_origin(&mut self) -> usize {
        let next = self.next_origin;
        self.next_origin += 1;
        next
    }

    fn is_expr_copy(&self, expr: &TypedExpr) -> bool {
        !matches!(expr.kind, TypedExprKind::Lambda(_, _)) && self.is_copy_type(&expr.ty)
    }

    fn is_copy_type(&self, ty: &Type) -> bool {
        match ty {
            Type::Named(name, _) if self.index.enum_names.contains(name) => true,
            _ => is_copy(ty),
        }
    }
}

impl Default for OwnershipChecker {
    fn default() -> Self {
        Self::new()
    }
}

struct SummaryWalker<'a> {
    checker: &'a OwnershipChecker,
    record: &'a FunctionRecord,
    param_effects: HashMap<String, UseEffect>,
    receiver_effect: Option<UseEffect>,
    contracts: HashMap<String, UseEffect>,
}

impl<'a> SummaryWalker<'a> {
    fn new(checker: &'a OwnershipChecker, record: &'a FunctionRecord) -> Self {
        Self {
            checker,
            record,
            param_effects: record
                .params
                .iter()
                .map(|param| (param.name.clone(), UseEffect::Copy))
                .collect(),
            receiver_effect: None,
            contracts: HashMap::new(),
        }
    }

    fn visit_block(&mut self, block: &TypedBlock) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_stmt(&mut self, stmt: &TypedStmt) {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.visit_expr(value, UseEffect::Move);
                }
            }
            TypedStmtKind::LetDestructure(stmt) => self.visit_expr(&stmt.value, UseEffect::Move),
            TypedStmtKind::Assign(assign) => {
                self.visit_expr(&assign.target, UseEffect::BorrowExclusive);
                if let Some(value) = &assign.value {
                    self.visit_expr(value, UseEffect::Move);
                }
            }
            TypedStmtKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.visit_expr(value, UseEffect::Move);
                }
            }
            TypedStmtKind::Expr(expr) => self.visit_expr(expr, UseEffect::BorrowShared),
            TypedStmtKind::If(if_stmt) => {
                self.visit_expr(&if_stmt.condition, UseEffect::Copy);
                self.visit_block(&if_stmt.then_branch);
                if let Some(else_branch) = &if_stmt.else_branch {
                    match else_branch {
                        TypedElseBranch::If(if_stmt) => self.visit_block(&if_stmt.then_branch),
                        TypedElseBranch::Block(block) => self.visit_block(block),
                    }
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.visit_expr(&for_stmt.iter, UseEffect::BorrowShared);
                self.visit_block(&for_stmt.body);
            }
            TypedStmtKind::While(while_stmt) => {
                self.visit_expr(&while_stmt.condition, UseEffect::Copy);
                self.visit_block(&while_stmt.body);
            }
            TypedStmtKind::Spawn(spawn) => {
                if let TypedSpawnBody::Expr(expr) = &spawn.body {
                    self.visit_expr(expr, UseEffect::BorrowShared);
                }
            }
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.visit_block(block),
            TypedStmtKind::IfCompile(if_compile) => {
                self.visit_expr(&if_compile.condition, UseEffect::Copy);
                self.visit_block(&if_compile.body);
            }
            TypedStmtKind::GcConfig(gc) => {
                for entry in &gc.entries {
                    self.visit_expr(&entry.value, UseEffect::BorrowShared);
                }
            }
            TypedStmtKind::TypeBlock(type_block) => {
                for member in &type_block.members {
                    if let TypedTypeMember::Binding { name, ty, .. } = member {
                        if let Some(effect) = self.checker.effect_contract_from_type(ty) {
                            self.contracts.insert(name.clone(), effect);
                        }
                    }
                }
            }
            TypedStmtKind::AsmBlock(_) => {}
        }
    }

    fn visit_expr(&mut self, expr: &TypedExpr, desired: UseEffect) {
        match &expr.kind {
            TypedExprKind::Ident(name) => self.raise(name, desired),
            TypedExprKind::Call(callee, args) => {
                let effects = self.summary_call_effects(callee, args);
                self.visit_expr(callee, UseEffect::Copy);
                for (arg, effect) in args.iter().zip(effects.iter()) {
                    self.visit_expr(arg, effect.clone());
                }
            }
            TypedExprKind::MethodCall(target, name, args) => {
                self.visit_expr(
                    target,
                    self.checker.method_receiver_effect(&target.ty, name),
                );
                for (arg, effect) in args
                    .iter()
                    .zip(self.checker.method_arg_effects(&target.ty, name, args))
                {
                    self.visit_expr(arg, effect);
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.visit_expr(
                        item,
                        if self.checker.is_expr_copy(item) {
                            UseEffect::Copy
                        } else {
                            UseEffect::Move
                        },
                    );
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.visit_expr(key, UseEffect::Copy);
                    self.visit_expr(
                        value,
                        if self.checker.is_expr_copy(value) {
                            UseEffect::Copy
                        } else {
                            UseEffect::Move
                        },
                    );
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs) | TypedExprKind::Nullish(lhs, rhs) => {
                self.visit_expr(
                    lhs,
                    if self.checker.is_expr_copy(lhs) {
                        UseEffect::Copy
                    } else {
                        UseEffect::BorrowShared
                    },
                );
                self.visit_expr(
                    rhs,
                    if self.checker.is_expr_copy(rhs) {
                        UseEffect::Copy
                    } else {
                        UseEffect::BorrowShared
                    },
                );
            }
            TypedExprKind::UnOp(_, inner)
            | TypedExprKind::Cast(inner, _)
            | TypedExprKind::Field(inner, _)
            | TypedExprKind::Index(inner, _)
            | TypedExprKind::Ok(inner)
            | TypedExprKind::Err(inner) => {
                self.visit_expr(
                    inner,
                    if self.checker.is_expr_copy(inner) {
                        UseEffect::Copy
                    } else {
                        desired
                    },
                );
            }
            TypedExprKind::Lambda(_, body) => self.visit_expr(body, UseEffect::BorrowShared),
            TypedExprKind::Match(subject, arms) => {
                self.visit_expr(subject, UseEffect::BorrowShared);
                for arm in arms {
                    self.visit_expr(&arm.pattern, UseEffect::BorrowShared);
                    if let TypedMatchArmBody::Expr(expr) = &arm.body {
                        self.visit_expr(expr, desired.clone());
                    }
                }
            }
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let crate::typed_ast::TypedFStrPart::Interp(expr) = part {
                        self.visit_expr(
                            expr,
                            if self.checker.is_expr_copy(expr) {
                                UseEffect::Copy
                            } else {
                                UseEffect::BorrowShared
                            },
                        );
                    }
                }
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Chan(_) => {}
        }
    }

    fn summary_call_effects(&self, callee: &TypedExpr, args: &[TypedExpr]) -> Vec<UseEffect> {
        if let Some(summary) = self.checker.lookup_call_summary(callee) {
            return args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if self.checker.is_expr_copy(arg) {
                        UseEffect::Copy
                    } else {
                        summary
                            .summary
                            .params
                            .get(index)
                            .map(|param| param.effect.clone())
                            .unwrap_or(UseEffect::BorrowShared)
                    }
                })
                .collect();
        }
        if let TypedExprKind::Ident(name) = &callee.kind {
            if let Some(effect) = self.contracts.get(name) {
                return args
                    .iter()
                    .map(|arg| {
                        if self.checker.is_expr_copy(arg) {
                            UseEffect::Copy
                        } else {
                            effect.clone()
                        }
                    })
                    .collect();
            }
        }
        args.iter()
            .map(|arg| {
                if self.checker.is_expr_copy(arg) {
                    UseEffect::Copy
                } else {
                    UseEffect::BorrowShared
                }
            })
            .collect()
    }

    fn raise(&mut self, name: &str, effect: UseEffect) {
        if let Some(param) = self.record.params.iter().find(|param| param.name == name) {
            if !self.checker.is_copy_type(&param.ty) {
                let current = self
                    .param_effects
                    .get(name)
                    .cloned()
                    .unwrap_or(UseEffect::Copy);
                self.param_effects
                    .insert(name.to_string(), join_effect(current, effect));
            }
        } else if name == "self" {
            if let Some(receiver_ty) = &self.record.receiver_ty {
                if !self.checker.is_copy_type(receiver_ty) {
                    self.receiver_effect = Some(join_effect(
                        self.receiver_effect.clone().unwrap_or(UseEffect::Copy),
                        effect,
                    ));
                }
            }
        }
    }
}

fn join_effect(left: UseEffect, right: UseEffect) -> UseEffect {
    if effect_rank(&left) >= effect_rank(&right) {
        left
    } else {
        right
    }
}

fn effect_rank(effect: &UseEffect) -> u8 {
    match effect {
        UseEffect::Copy => 0,
        UseEffect::BorrowShared => 1,
        UseEffect::BorrowExclusive => 2,
        UseEffect::Move => 3,
    }
}
