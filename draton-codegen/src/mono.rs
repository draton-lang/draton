use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use draton_typeck::{
    typed_ast::{
        TypedClassDef, TypedDestructureBinding, TypedElseBranch, TypedForStmt, TypedGcConfigEntry,
        TypedGcConfigStmt, TypedIfCompileStmt, TypedIfStmt, TypedLetDestructureStmt, TypedLetStmt,
        TypedMatchArm, TypedReturnStmt, TypedSpawnBody, TypedSpawnStmt, TypedWhileStmt,
    },
    Type, TypedBlock, TypedExpr, TypedExprKind, TypedFStrPart, TypedFnDef, TypedItem,
    TypedMatchArmBody, TypedParam, TypedProgram, TypedStmt, TypedStmtKind,
};

use crate::mangle::{mangle_class, mangle_fn};

/// A generic class definition indexed for monomorphization.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericClassDef {
    pub def: TypedClassDef,
    pub type_vars: Vec<u32>,
}

/// A generic function definition indexed for monomorphization.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericFnDef {
    pub def: TypedFnDef,
    pub type_vars: Vec<u32>,
}

/// A concrete instantiation of a generic class.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInstantiation {
    pub class_name: String,
    pub type_args: Vec<Type>,
    pub mangled: String,
}

/// A concrete instantiation of a generic function.
#[derive(Debug, Clone, PartialEq)]
pub struct FnInstantiation {
    pub fn_name: String,
    pub class_name: Option<String>,
    pub type_args: Vec<Type>,
    pub mangled: String,
}

/// Collects all concrete generic instantiations reachable from a typed program.
#[derive(Debug, Clone, Default)]
pub struct MonoCollector {
    pub class_insts: Vec<ClassInstantiation>,
    pub fn_insts: Vec<FnInstantiation>,
    generic_classes: HashMap<String, GenericClassDef>,
    generic_functions: HashMap<String, GenericFnDef>,
    seen_classes: HashSet<String>,
    seen_fns: HashSet<String>,
    class_queue: VecDeque<ClassInstantiation>,
    fn_queue: VecDeque<FnInstantiation>,
}

impl MonoCollector {
    /// Creates an empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Walks a typed program and records every concrete generic instantiation.
    pub fn collect(mut self, program: &TypedProgram) -> Self {
        self.index_generics(program);
        let subst = HashMap::new();
        for item in &program.items {
            self.visit_item(item, &subst);
        }
        while !self.class_queue.is_empty() || !self.fn_queue.is_empty() {
            if let Some(inst) = self.class_queue.pop_front() {
                self.process_class_inst(&inst);
            }
            if let Some(inst) = self.fn_queue.pop_front() {
                self.process_fn_inst(&inst);
            }
        }
        self
    }

    fn index_generics(&mut self, program: &TypedProgram) {
        for item in &program.items {
            match item {
                TypedItem::Class(class_def) => {
                    if let Some(info) = generic_class_def(class_def) {
                        self.generic_classes.insert(class_def.name.clone(), info);
                    }
                }
                TypedItem::Fn(function)
                | TypedItem::PanicHandler(function)
                | TypedItem::OomHandler(function) => {
                    if let Some(info) = generic_fn_def(function) {
                        self.generic_functions.insert(function.name.clone(), info);
                    }
                }
                TypedItem::Extern(extern_block) => {
                    for function in &extern_block.functions {
                        if let Some(info) = generic_fn_def(function) {
                            self.generic_functions.insert(function.name.clone(), info);
                        }
                    }
                }
                TypedItem::Interface(_)
                | TypedItem::Enum(_)
                | TypedItem::Error(_)
                | TypedItem::Const(_)
                | TypedItem::Import(_)
                | TypedItem::TypeBlock(_) => {}
            }
        }
    }

    fn process_class_inst(&mut self, inst: &ClassInstantiation) {
        let Some(info) = self.generic_classes.get(&inst.class_name).cloned() else {
            return;
        };
        let subst = build_var_subst(&info.type_vars, &inst.type_args);
        for field in &info.def.fields {
            self.visit_type(&substitute_type(&field.ty, &subst));
        }
        for method in &info.def.methods {
            self.visit_fn(method, &subst);
        }
    }

    fn process_fn_inst(&mut self, inst: &FnInstantiation) {
        let Some(info) = self.generic_functions.get(&inst.fn_name).cloned() else {
            return;
        };
        let subst = build_var_subst(&info.type_vars, &inst.type_args);
        self.visit_fn(&info.def, &subst);
    }

    fn visit_item(&mut self, item: &TypedItem, subst: &HashMap<u32, Type>) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => self.visit_fn(function, subst),
            TypedItem::Class(class_def) => {
                for field in &class_def.fields {
                    self.visit_type(&substitute_type(&field.ty, subst));
                }
                for method in &class_def.methods {
                    self.visit_fn(method, subst);
                }
            }
            TypedItem::Extern(extern_block) => {
                for function in &extern_block.functions {
                    self.visit_fn(function, subst);
                }
            }
            TypedItem::Const(const_def) => {
                self.visit_type(&substitute_type(&const_def.ty, subst));
                self.visit_expr(&const_def.value, subst);
            }
            TypedItem::Interface(_)
            | TypedItem::Enum(_)
            | TypedItem::Error(_)
            | TypedItem::Import(_)
            | TypedItem::TypeBlock(_) => {}
        }
    }

    fn visit_fn(&mut self, function: &TypedFnDef, subst: &HashMap<u32, Type>) {
        self.visit_type(&substitute_type(&function.ty, subst));
        self.visit_type(&substitute_type(&function.ret_type, subst));
        for param in &function.params {
            self.visit_type(&substitute_type(&param.ty, subst));
        }
        if let Some(body) = &function.body {
            self.visit_block(body, subst);
        }
    }

    fn visit_block(&mut self, block: &TypedBlock, subst: &HashMap<u32, Type>) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt, subst);
        }
    }

    fn visit_stmt(&mut self, stmt: &TypedStmt, subst: &HashMap<u32, Type>) {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                self.visit_type(&substitute_type(&let_stmt.ty, subst));
                if let Some(value) = &let_stmt.value {
                    self.visit_expr(value, subst);
                }
            }
            TypedStmtKind::LetDestructure(let_stmt) => {
                self.visit_type(&substitute_type(&let_stmt.tuple_ty, subst));
                for binding in &let_stmt.bindings {
                    if let TypedDestructureBinding::Name { ty, .. } = binding {
                        self.visit_type(&substitute_type(ty, subst));
                    }
                }
                self.visit_expr(&let_stmt.value, subst);
            }
            TypedStmtKind::Assign(assign) => {
                self.visit_expr(&assign.target, subst);
                if let Some(value) = &assign.value {
                    self.visit_expr(value, subst);
                }
            }
            TypedStmtKind::Return(ret) => {
                self.visit_type(&substitute_type(&ret.ty, subst));
                if let Some(value) = &ret.value {
                    self.visit_expr(value, subst);
                }
            }
            TypedStmtKind::Expr(expr) => self.visit_expr(expr, subst),
            TypedStmtKind::If(if_stmt) => {
                self.visit_expr(&if_stmt.condition, subst);
                self.visit_block(&if_stmt.then_branch, subst);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.visit_else_branch(else_branch, subst);
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.visit_type(&substitute_type(&for_stmt.item_type, subst));
                self.visit_expr(&for_stmt.iter, subst);
                self.visit_block(&for_stmt.body, subst);
            }
            TypedStmtKind::While(while_stmt) => {
                self.visit_expr(&while_stmt.condition, subst);
                self.visit_block(&while_stmt.body, subst);
            }
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => self.visit_expr(expr, subst),
                TypedSpawnBody::Block(block) => self.visit_block(block, subst),
            },
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.visit_block(block, subst),
            TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::IfCompile(_)
            | TypedStmtKind::GcConfig(_) => {}
        }
    }

    fn visit_else_branch(&mut self, branch: &TypedElseBranch, subst: &HashMap<u32, Type>) {
        match branch {
            TypedElseBranch::If(if_stmt) => {
                self.visit_expr(&if_stmt.condition, subst);
                self.visit_block(&if_stmt.then_branch, subst);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.visit_else_branch(else_branch, subst);
                }
            }
            TypedElseBranch::Block(block) => self.visit_block(block, subst),
        }
    }

    fn visit_expr(&mut self, expr: &TypedExpr, subst: &HashMap<u32, Type>) {
        self.visit_type(&substitute_type(&expr.ty, subst));
        match &expr.kind {
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let TypedFStrPart::Interp(expr) = part {
                        self.visit_expr(expr, subst);
                    }
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.visit_expr(item, subst);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.visit_expr(key, subst);
                    self.visit_expr(value, subst);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs)
            | TypedExprKind::Index(lhs, rhs)
            | TypedExprKind::Nullish(lhs, rhs) => {
                self.visit_expr(lhs, subst);
                self.visit_expr(rhs, subst);
            }
            TypedExprKind::UnOp(_, value)
            | TypedExprKind::Cast(value, _)
            | TypedExprKind::Ok(value)
            | TypedExprKind::Err(value) => self.visit_expr(value, subst),
            TypedExprKind::Call(callee, args) => {
                self.visit_expr(callee, subst);
                for arg in args {
                    self.visit_expr(arg, subst);
                }
                if let TypedExprKind::Ident(name) = &callee.kind {
                    if let Some(info) = self.generic_functions.get(name).cloned() {
                        let actual_types = args
                            .iter()
                            .map(|arg| substitute_type(&arg.ty, subst))
                            .collect::<Vec<_>>();
                        if let Some(type_args) =
                            resolve_function_type_args(&info.def, &info.type_vars, &actual_types)
                        {
                            self.add_fn_inst(name, None, &type_args);
                        }
                    }
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.visit_expr(target, subst);
                for arg in args {
                    self.visit_expr(arg, subst);
                }
            }
            TypedExprKind::Field(target, _) => self.visit_expr(target, subst),
            TypedExprKind::Lambda(params, body) => {
                for param in params {
                    self.visit_type(&substitute_type(&param.ty, subst));
                }
                self.visit_expr(body, subst);
            }
            TypedExprKind::Match(subject, arms) => {
                self.visit_expr(subject, subst);
                for arm in arms {
                    self.visit_expr(&arm.pattern, subst);
                    match &arm.body {
                        TypedMatchArmBody::Expr(expr) => self.visit_expr(expr, subst),
                        TypedMatchArmBody::Block(block) => self.visit_block(block, subst),
                    }
                }
            }
            TypedExprKind::Chan(ty) => self.visit_type(&substitute_type(ty, subst)),
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Ident(_) => {}
        }
    }

    fn visit_type(&mut self, ty: &Type) {
        match ty {
            Type::Named(name, args) if !args.is_empty() => {
                let concrete_args = args.to_vec();
                if free_type_vars_in_types(&concrete_args).is_empty() {
                    self.add_class_inst(name, &concrete_args);
                }
                for arg in args {
                    self.visit_type(arg);
                }
            }
            Type::Array(inner)
            | Type::Set(inner)
            | Type::Option(inner)
            | Type::Chan(inner)
            | Type::Pointer(inner) => self.visit_type(inner),
            Type::Map(key, value) | Type::Result(key, value) => {
                self.visit_type(key);
                self.visit_type(value);
            }
            Type::Tuple(items) => {
                for item in items {
                    self.visit_type(item);
                }
            }
            Type::Fn(params, ret) => {
                for param in params {
                    self.visit_type(param);
                }
                self.visit_type(ret);
            }
            Type::Row { fields, rest } => {
                for field in fields.values() {
                    self.visit_type(field);
                }
                if let Some(rest) = rest {
                    self.visit_type(rest);
                }
            }
            Type::Named(_, _)
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
            | Type::Bool
            | Type::String
            | Type::Char
            | Type::Unit
            | Type::Never
            | Type::Var(_) => {}
        }
    }

    fn add_class_inst(&mut self, name: &str, args: &[Type]) {
        let mangled = mangle_class(name, args);
        if self.seen_classes.insert(mangled.clone()) {
            let inst = ClassInstantiation {
                class_name: name.to_string(),
                type_args: args.to_vec(),
                mangled,
            };
            self.class_queue.push_back(inst.clone());
            self.class_insts.push(inst);
        }
    }

    fn add_fn_inst(&mut self, fn_name: &str, class_name: Option<&str>, args: &[Type]) {
        let mangled = mangle_fn(fn_name, class_name, args);
        if self.seen_fns.insert(mangled.clone()) {
            let inst = FnInstantiation {
                fn_name: fn_name.to_string(),
                class_name: class_name.map(ToOwned::to_owned),
                type_args: args.to_vec(),
                mangled,
            };
            self.fn_queue.push_back(inst.clone());
            self.fn_insts.push(inst);
        }
    }
}

/// Returns the generic type variables referenced by a class, ordered by id.
pub fn generic_class_def(class_def: &TypedClassDef) -> Option<GenericClassDef> {
    let mut vars = BTreeSet::new();
    for field in &class_def.fields {
        vars.extend(free_type_vars(&field.ty));
    }
    for method in &class_def.methods {
        vars.extend(free_type_vars(&method.ty));
    }
    let type_vars = vars.into_iter().collect::<Vec<_>>();
    (!type_vars.is_empty()).then(|| GenericClassDef {
        def: class_def.clone(),
        type_vars,
    })
}

/// Returns the generic type variables referenced by a function, ordered by id.
pub fn generic_fn_def(function: &TypedFnDef) -> Option<GenericFnDef> {
    let type_vars = free_type_vars(&function.ty).into_iter().collect::<Vec<_>>();
    (!type_vars.is_empty()).then(|| GenericFnDef {
        def: function.clone(),
        type_vars,
    })
}

/// Builds a substitution map from ordered type variables to concrete arguments.
pub fn build_var_subst(type_vars: &[u32], type_args: &[Type]) -> HashMap<u32, Type> {
    type_vars
        .iter()
        .copied()
        .zip(type_args.iter().cloned())
        .collect()
}

/// Applies a type-variable substitution without materializing runtime mangled names.
pub fn substitute_type(ty: &Type, subst: &HashMap<u32, Type>) -> Type {
    match ty {
        Type::Var(id) => subst.get(id).cloned().unwrap_or(Type::Var(*id)),
        Type::Array(inner) => Type::Array(Box::new(substitute_type(inner, subst))),
        Type::Map(key, value) => Type::Map(
            Box::new(substitute_type(key, subst)),
            Box::new(substitute_type(value, subst)),
        ),
        Type::Set(inner) => Type::Set(Box::new(substitute_type(inner, subst))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| substitute_type(item, subst))
                .collect(),
        ),
        Type::Option(inner) => Type::Option(Box::new(substitute_type(inner, subst))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_type(ok, subst)),
            Box::new(substitute_type(err, subst)),
        ),
        Type::Chan(inner) => Type::Chan(Box::new(substitute_type(inner, subst))),
        Type::Fn(params, ret) => Type::Fn(
            params
                .iter()
                .map(|param| substitute_type(param, subst))
                .collect(),
            Box::new(substitute_type(ret, subst)),
        ),
        Type::Named(name, args) => Type::Named(
            name.clone(),
            args.iter().map(|arg| substitute_type(arg, subst)).collect(),
        ),
        Type::Row { fields, rest } => Type::Row {
            fields: fields
                .iter()
                .map(|(name, ty)| (name.clone(), substitute_type(ty, subst)))
                .collect::<BTreeMap<_, _>>(),
            rest: rest
                .as_ref()
                .map(|rest| Box::new(substitute_type(rest, subst))),
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(substitute_type(inner, subst))),
        other => other.clone(),
    }
}

/// Resolves the concrete type arguments for a generic function call.
pub fn resolve_function_type_args(
    function: &TypedFnDef,
    type_vars: &[u32],
    actual_args: &[Type],
) -> Option<Vec<Type>> {
    if function.params.len() != actual_args.len() {
        return None;
    }
    let allowed = type_vars.iter().copied().collect::<BTreeSet<_>>();
    let mut bindings = HashMap::new();
    for (param, actual) in function.params.iter().zip(actual_args.iter()) {
        if !match_type_pattern(&param.ty, actual, &allowed, &mut bindings) {
            return None;
        }
    }
    let mut resolved = Vec::with_capacity(type_vars.len());
    for var in type_vars {
        let ty = bindings.get(var).cloned()?;
        if !free_type_vars(&ty).is_empty() {
            return None;
        }
        resolved.push(ty);
    }
    Some(resolved)
}

/// Produces a monomorphic class definition ready for LLVM emission.
pub fn specialize_class(
    class_def: &TypedClassDef,
    type_vars: &[u32],
    inst: &ClassInstantiation,
) -> TypedClassDef {
    let subst = build_var_subst(type_vars, &inst.type_args);
    let self_class = (class_def.name.as_str(), inst.mangled.as_str());
    TypedClassDef {
        name: inst.mangled.clone(),
        extends: class_def.extends.clone(),
        implements: class_def.implements.clone(),
        fields: class_def
            .fields
            .iter()
            .map(|field| draton_typeck::typed_ast::TypedFieldDef {
                is_mut: field.is_mut,
                name: field.name.clone(),
                ty: materialize_type(&field.ty, &subst, Some(self_class)),
                span: field.span,
            })
            .collect(),
        methods: class_def
            .methods
            .iter()
            .map(|method| specialize_function(method, &subst, Some(self_class), None))
            .collect(),
        type_blocks: class_def.type_blocks.clone(),
        span: class_def.span,
    }
}

/// Produces a monomorphic function definition ready for LLVM emission.
pub fn specialize_function(
    function: &TypedFnDef,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
    mangled_name: Option<&str>,
) -> TypedFnDef {
    TypedFnDef {
        is_pub: function.is_pub,
        name: mangled_name.unwrap_or(&function.name).to_string(),
        params: function
            .params
            .iter()
            .map(|param| TypedParam {
                name: param.name.clone(),
                ty: materialize_type(&param.ty, subst, self_class),
                span: param.span,
            })
            .collect(),
        ret_type: materialize_type(&function.ret_type, subst, self_class),
        body: function
            .body
            .as_ref()
            .map(|body| substitute_block(body, subst, self_class)),
        ty: materialize_type(&function.ty, subst, self_class),
        span: function.span,
    }
}

fn substitute_block(
    block: &TypedBlock,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
) -> TypedBlock {
    TypedBlock {
        stmts: block
            .stmts
            .iter()
            .map(|stmt| substitute_stmt(stmt, subst, self_class))
            .collect(),
        span: block.span,
    }
}

fn substitute_stmt(
    stmt: &TypedStmt,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
) -> TypedStmt {
    let kind = match &stmt.kind {
        TypedStmtKind::Let(let_stmt) => TypedStmtKind::Let(TypedLetStmt {
            is_mut: let_stmt.is_mut,
            name: let_stmt.name.clone(),
            value: let_stmt
                .value
                .as_ref()
                .map(|value| substitute_expr(value, subst, self_class)),
            ty: materialize_type(&let_stmt.ty, subst, self_class),
            span: let_stmt.span,
        }),
        TypedStmtKind::LetDestructure(let_stmt) => {
            TypedStmtKind::LetDestructure(TypedLetDestructureStmt {
                is_mut: let_stmt.is_mut,
                bindings: let_stmt
                    .bindings
                    .iter()
                    .map(|binding| match binding {
                        TypedDestructureBinding::Name { name, ty } => {
                            TypedDestructureBinding::Name {
                                name: name.clone(),
                                ty: materialize_type(ty, subst, self_class),
                            }
                        }
                        TypedDestructureBinding::Wildcard => TypedDestructureBinding::Wildcard,
                    })
                    .collect(),
                value: substitute_expr(&let_stmt.value, subst, self_class),
                tuple_ty: materialize_type(&let_stmt.tuple_ty, subst, self_class),
                span: let_stmt.span,
            })
        }
        TypedStmtKind::Assign(assign) => {
            TypedStmtKind::Assign(draton_typeck::typed_ast::TypedAssignStmt {
                target: substitute_expr(&assign.target, subst, self_class),
                op: assign.op,
                value: assign
                    .value
                    .as_ref()
                    .map(|value| substitute_expr(value, subst, self_class)),
                span: assign.span,
            })
        }
        TypedStmtKind::Return(ret) => TypedStmtKind::Return(TypedReturnStmt {
            value: ret
                .value
                .as_ref()
                .map(|value| substitute_expr(value, subst, self_class)),
            ty: materialize_type(&ret.ty, subst, self_class),
            span: ret.span,
        }),
        TypedStmtKind::Expr(expr) => TypedStmtKind::Expr(substitute_expr(expr, subst, self_class)),
        TypedStmtKind::If(if_stmt) => TypedStmtKind::If(TypedIfStmt {
            condition: substitute_expr(&if_stmt.condition, subst, self_class),
            then_branch: substitute_block(&if_stmt.then_branch, subst, self_class),
            else_branch: if_stmt
                .else_branch
                .as_ref()
                .map(|branch| substitute_else_branch(branch, subst, self_class)),
            span: if_stmt.span,
        }),
        TypedStmtKind::For(for_stmt) => TypedStmtKind::For(TypedForStmt {
            name: for_stmt.name.clone(),
            iter: substitute_expr(&for_stmt.iter, subst, self_class),
            item_type: materialize_type(&for_stmt.item_type, subst, self_class),
            body: substitute_block(&for_stmt.body, subst, self_class),
            span: for_stmt.span,
        }),
        TypedStmtKind::While(while_stmt) => TypedStmtKind::While(TypedWhileStmt {
            condition: substitute_expr(&while_stmt.condition, subst, self_class),
            body: substitute_block(&while_stmt.body, subst, self_class),
            span: while_stmt.span,
        }),
        TypedStmtKind::Spawn(spawn) => TypedStmtKind::Spawn(TypedSpawnStmt {
            body: match &spawn.body {
                TypedSpawnBody::Expr(expr) => {
                    TypedSpawnBody::Expr(substitute_expr(expr, subst, self_class))
                }
                TypedSpawnBody::Block(block) => {
                    TypedSpawnBody::Block(substitute_block(block, subst, self_class))
                }
            },
            span: spawn.span,
        }),
        TypedStmtKind::Block(block) => {
            TypedStmtKind::Block(substitute_block(block, subst, self_class))
        }
        TypedStmtKind::UnsafeBlock(block) => {
            TypedStmtKind::UnsafeBlock(substitute_block(block, subst, self_class))
        }
        TypedStmtKind::PointerBlock(block) => {
            TypedStmtKind::PointerBlock(substitute_block(block, subst, self_class))
        }
        TypedStmtKind::AsmBlock(body) => TypedStmtKind::AsmBlock(body.clone()),
        TypedStmtKind::ComptimeBlock(block) => {
            TypedStmtKind::ComptimeBlock(substitute_block(block, subst, self_class))
        }
        TypedStmtKind::IfCompile(if_compile) => TypedStmtKind::IfCompile(TypedIfCompileStmt {
            condition: substitute_expr(&if_compile.condition, subst, self_class),
            body: substitute_block(&if_compile.body, subst, self_class),
            span: if_compile.span,
        }),
        TypedStmtKind::GcConfig(config) => TypedStmtKind::GcConfig(TypedGcConfigStmt {
            entries: config
                .entries
                .iter()
                .map(|entry| TypedGcConfigEntry {
                    key: entry.key.clone(),
                    value: substitute_expr(&entry.value, subst, self_class),
                    span: entry.span,
                })
                .collect(),
            span: config.span,
        }),
    };
    TypedStmt {
        kind,
        span: stmt.span,
    }
}

fn substitute_else_branch(
    branch: &TypedElseBranch,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
) -> TypedElseBranch {
    match branch {
        TypedElseBranch::If(if_stmt) => TypedElseBranch::If(Box::new(TypedIfStmt {
            condition: substitute_expr(&if_stmt.condition, subst, self_class),
            then_branch: substitute_block(&if_stmt.then_branch, subst, self_class),
            else_branch: if_stmt
                .else_branch
                .as_ref()
                .map(|branch| substitute_else_branch(branch, subst, self_class)),
            span: if_stmt.span,
        })),
        TypedElseBranch::Block(block) => {
            TypedElseBranch::Block(substitute_block(block, subst, self_class))
        }
    }
}

fn substitute_expr(
    expr: &TypedExpr,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
) -> TypedExpr {
    let kind = match &expr.kind {
        TypedExprKind::IntLit(value) => TypedExprKind::IntLit(*value),
        TypedExprKind::FloatLit(value) => TypedExprKind::FloatLit(*value),
        TypedExprKind::StrLit(value) => TypedExprKind::StrLit(value.clone()),
        TypedExprKind::FStrLit(parts) => TypedExprKind::FStrLit(
            parts
                .iter()
                .map(|part| match part {
                    TypedFStrPart::Literal(text) => TypedFStrPart::Literal(text.clone()),
                    TypedFStrPart::Interp(expr) => {
                        TypedFStrPart::Interp(substitute_expr(expr, subst, self_class))
                    }
                })
                .collect(),
        ),
        TypedExprKind::BoolLit(value) => TypedExprKind::BoolLit(*value),
        TypedExprKind::NoneLit => TypedExprKind::NoneLit,
        TypedExprKind::Ident(name) => TypedExprKind::Ident(name.clone()),
        TypedExprKind::Array(items) => TypedExprKind::Array(
            items
                .iter()
                .map(|item| substitute_expr(item, subst, self_class))
                .collect(),
        ),
        TypedExprKind::Map(entries) => TypedExprKind::Map(
            entries
                .iter()
                .map(|(key, value)| {
                    (
                        substitute_expr(key, subst, self_class),
                        substitute_expr(value, subst, self_class),
                    )
                })
                .collect(),
        ),
        TypedExprKind::Set(items) => TypedExprKind::Set(
            items
                .iter()
                .map(|item| substitute_expr(item, subst, self_class))
                .collect(),
        ),
        TypedExprKind::Tuple(items) => TypedExprKind::Tuple(
            items
                .iter()
                .map(|item| substitute_expr(item, subst, self_class))
                .collect(),
        ),
        TypedExprKind::BinOp(lhs, op, rhs) => TypedExprKind::BinOp(
            Box::new(substitute_expr(lhs, subst, self_class)),
            *op,
            Box::new(substitute_expr(rhs, subst, self_class)),
        ),
        TypedExprKind::UnOp(op, value) => {
            TypedExprKind::UnOp(*op, Box::new(substitute_expr(value, subst, self_class)))
        }
        TypedExprKind::Call(callee, args) => TypedExprKind::Call(
            Box::new(substitute_expr(callee, subst, self_class)),
            args.iter()
                .map(|arg| substitute_expr(arg, subst, self_class))
                .collect(),
        ),
        TypedExprKind::MethodCall(target, method, args) => TypedExprKind::MethodCall(
            Box::new(substitute_expr(target, subst, self_class)),
            method.clone(),
            args.iter()
                .map(|arg| substitute_expr(arg, subst, self_class))
                .collect(),
        ),
        TypedExprKind::Field(target, field) => TypedExprKind::Field(
            Box::new(substitute_expr(target, subst, self_class)),
            field.clone(),
        ),
        TypedExprKind::Index(target, index) => TypedExprKind::Index(
            Box::new(substitute_expr(target, subst, self_class)),
            Box::new(substitute_expr(index, subst, self_class)),
        ),
        TypedExprKind::Lambda(params, body) => TypedExprKind::Lambda(
            params
                .iter()
                .map(|param| TypedParam {
                    name: param.name.clone(),
                    ty: materialize_type(&param.ty, subst, self_class),
                    span: param.span,
                })
                .collect(),
            Box::new(substitute_expr(body, subst, self_class)),
        ),
        TypedExprKind::Cast(value, ty) => TypedExprKind::Cast(
            Box::new(substitute_expr(value, subst, self_class)),
            materialize_type(ty, subst, self_class),
        ),
        TypedExprKind::Match(subject, arms) => TypedExprKind::Match(
            Box::new(substitute_expr(subject, subst, self_class)),
            arms.iter()
                .map(|arm| TypedMatchArm {
                    pattern: substitute_expr(&arm.pattern, subst, self_class),
                    body: match &arm.body {
                        TypedMatchArmBody::Expr(expr) => {
                            TypedMatchArmBody::Expr(substitute_expr(expr, subst, self_class))
                        }
                        TypedMatchArmBody::Block(block) => {
                            TypedMatchArmBody::Block(substitute_block(block, subst, self_class))
                        }
                    },
                    span: arm.span,
                })
                .collect(),
        ),
        TypedExprKind::Ok(value) => {
            TypedExprKind::Ok(Box::new(substitute_expr(value, subst, self_class)))
        }
        TypedExprKind::Err(value) => {
            TypedExprKind::Err(Box::new(substitute_expr(value, subst, self_class)))
        }
        TypedExprKind::Nullish(lhs, rhs) => TypedExprKind::Nullish(
            Box::new(substitute_expr(lhs, subst, self_class)),
            Box::new(substitute_expr(rhs, subst, self_class)),
        ),
        TypedExprKind::Chan(ty) => TypedExprKind::Chan(materialize_type(ty, subst, self_class)),
    };
    TypedExpr {
        kind,
        ty: materialize_type(&expr.ty, subst, self_class),
        span: expr.span,
    }
}

fn materialize_type(
    ty: &Type,
    subst: &HashMap<u32, Type>,
    self_class: Option<(&str, &str)>,
) -> Type {
    let substituted = substitute_type(ty, subst);
    match substituted {
        Type::Array(inner) => Type::Array(Box::new(materialize_type(&inner, subst, self_class))),
        Type::Map(key, value) => Type::Map(
            Box::new(materialize_type(&key, subst, self_class)),
            Box::new(materialize_type(&value, subst, self_class)),
        ),
        Type::Set(inner) => Type::Set(Box::new(materialize_type(&inner, subst, self_class))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| materialize_type(item, subst, self_class))
                .collect(),
        ),
        Type::Option(inner) => Type::Option(Box::new(materialize_type(&inner, subst, self_class))),
        Type::Result(ok, err) => Type::Result(
            Box::new(materialize_type(&ok, subst, self_class)),
            Box::new(materialize_type(&err, subst, self_class)),
        ),
        Type::Chan(inner) => Type::Chan(Box::new(materialize_type(&inner, subst, self_class))),
        Type::Fn(params, ret) => Type::Fn(
            params
                .iter()
                .map(|param| materialize_type(param, subst, self_class))
                .collect(),
            Box::new(materialize_type(&ret, subst, self_class)),
        ),
        Type::Named(name, args) => {
            let args = args
                .iter()
                .map(|arg| materialize_type(arg, subst, self_class))
                .collect::<Vec<_>>();
            if args.is_empty() {
                if let Some((base, mangled)) = self_class {
                    if name == base {
                        return Type::Named(mangled.to_string(), Vec::new());
                    }
                }
                Type::Named(name, Vec::new())
            } else {
                Type::Named(mangle_class(&name, &args), Vec::new())
            }
        }
        Type::Row { fields, rest } => Type::Row {
            fields: fields
                .iter()
                .map(|(name, ty)| (name.clone(), materialize_type(ty, subst, self_class)))
                .collect(),
            rest: rest
                .as_ref()
                .map(|rest| Box::new(materialize_type(rest, subst, self_class))),
        },
        Type::Pointer(inner) => {
            Type::Pointer(Box::new(materialize_type(&inner, subst, self_class)))
        }
        other => other,
    }
}

fn match_type_pattern(
    pattern: &Type,
    actual: &Type,
    allowed_vars: &BTreeSet<u32>,
    bindings: &mut HashMap<u32, Type>,
) -> bool {
    match pattern {
        Type::Var(id) if allowed_vars.contains(id) => match bindings.get(id) {
            Some(bound) => bound == actual,
            None => {
                bindings.insert(*id, actual.clone());
                true
            }
        },
        Type::Array(inner) => {
            matches!(actual, Type::Array(actual_inner) if match_type_pattern(inner, actual_inner, allowed_vars, bindings))
        }
        Type::Map(key, value) => matches!(actual, Type::Map(actual_key, actual_value)
            if match_type_pattern(key, actual_key, allowed_vars, bindings)
                && match_type_pattern(value, actual_value, allowed_vars, bindings)),
        Type::Set(inner) => {
            matches!(actual, Type::Set(actual_inner) if match_type_pattern(inner, actual_inner, allowed_vars, bindings))
        }
        Type::Tuple(items) => matches!(actual, Type::Tuple(actual_items)
            if items.len() == actual_items.len()
                && items.iter().zip(actual_items.iter()).all(|(lhs, rhs)| match_type_pattern(lhs, rhs, allowed_vars, bindings))),
        Type::Option(inner) => {
            matches!(actual, Type::Option(actual_inner) if match_type_pattern(inner, actual_inner, allowed_vars, bindings))
        }
        Type::Result(ok, err) => matches!(actual, Type::Result(actual_ok, actual_err)
            if match_type_pattern(ok, actual_ok, allowed_vars, bindings)
                && match_type_pattern(err, actual_err, allowed_vars, bindings)),
        Type::Chan(inner) => {
            matches!(actual, Type::Chan(actual_inner) if match_type_pattern(inner, actual_inner, allowed_vars, bindings))
        }
        Type::Fn(params, ret) => matches!(actual, Type::Fn(actual_params, actual_ret)
            if params.len() == actual_params.len()
                && params.iter().zip(actual_params.iter()).all(|(lhs, rhs)| match_type_pattern(lhs, rhs, allowed_vars, bindings))
                && match_type_pattern(ret, actual_ret, allowed_vars, bindings)),
        Type::Named(name, args) => matches!(actual, Type::Named(actual_name, actual_args)
            if name == actual_name
                && args.len() == actual_args.len()
                && args.iter().zip(actual_args.iter()).all(|(lhs, rhs)| match_type_pattern(lhs, rhs, allowed_vars, bindings))),
        Type::Pointer(inner) => matches!(actual, Type::Pointer(actual_inner)
            if match_type_pattern(inner, actual_inner, allowed_vars, bindings)),
        Type::Var(id) => matches!(actual, Type::Var(actual_id) if id == actual_id),
        _ => pattern == actual,
    }
}

fn free_type_vars(ty: &Type) -> BTreeSet<u32> {
    match ty {
        Type::Var(id) => BTreeSet::from([*id]),
        Type::Array(inner)
        | Type::Set(inner)
        | Type::Option(inner)
        | Type::Chan(inner)
        | Type::Pointer(inner) => free_type_vars(inner),
        Type::Map(key, value) | Type::Result(key, value) => {
            let mut vars = free_type_vars(key);
            vars.extend(free_type_vars(value));
            vars
        }
        Type::Tuple(items) => free_type_vars_in_types(items),
        Type::Fn(params, ret) => {
            let mut vars = free_type_vars_in_types(params);
            vars.extend(free_type_vars(ret));
            vars
        }
        Type::Named(_, args) => free_type_vars_in_types(args),
        Type::Row { fields, rest } => {
            let mut vars = fields.values().fold(BTreeSet::new(), |mut acc, ty| {
                acc.extend(free_type_vars(ty));
                acc
            });
            if let Some(rest) = rest {
                vars.extend(free_type_vars(rest));
            }
            vars
        }
        Type::Int
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
        | Type::Bool
        | Type::String
        | Type::Char
        | Type::Unit
        | Type::Never => BTreeSet::new(),
    }
}

fn free_type_vars_in_types(types: &[Type]) -> BTreeSet<u32> {
    types.iter().fold(BTreeSet::new(), |mut acc, ty| {
        acc.extend(free_type_vars(ty));
        acc
    })
}
