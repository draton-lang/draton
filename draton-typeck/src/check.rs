use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use draton_ast::{
    AssignOp, BinOp, Block, ClassDef, ClassMember, DestructureBinding, ElseBranch, Expr, FStrPart,
    FieldDef, FnDef, GcConfigStmt, IfStmt, Item, LetDestructureStmt, MatchArm, MatchArmBody,
    Program, SpawnBody, Stmt, TypeExpr, TypeMember, UnOp,
};

use crate::env::{Scheme, TypeEnv};
use crate::error::TypeError;
use crate::exhaust::{classify_subject, extract_pattern, ExhaustivenessChecker};
use crate::infer::{free_type_vars, free_type_vars_in_env, Substitution};
use crate::typed_ast::{
    Type, TypedAssignStmt, TypedBlock, TypedClassDef, TypedConstDef, TypedDestructureBinding,
    TypedElseBranch, TypedEnumDef, TypedErrorDef, TypedExpr, TypedExprKind, TypedExternBlock,
    TypedFStrPart, TypedFieldDef, TypedFnDef, TypedForStmt, TypedGcConfigEntry, TypedGcConfigStmt,
    TypedIfCompileStmt, TypedIfStmt, TypedImportDef, TypedImportItem, TypedInterfaceDef, TypedItem,
    TypedLetDestructureStmt, TypedLetStmt, TypedMatchArm, TypedMatchArmBody, TypedParam,
    TypedProgram, TypedReturnStmt, TypedSpawnBody, TypedSpawnStmt, TypedStmt, TypedStmtKind,
    TypedTypeBlock, TypedTypeMember, TypedWhileStmt,
};
use crate::unify::occurs;

/// The result of type checking a program.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeCheckResult {
    pub typed_program: TypedProgram,
    pub errors: Vec<TypeError>,
    pub warnings: Vec<TypeError>,
}

/// The Draton type checker.
pub struct TypeChecker {
    env: TypeEnv,
    errors: Vec<TypeError>,
    warnings: Vec<TypeError>,
    next_var: u32,
    fresh_counter: Rc<Cell<u32>>,
    subst: Substitution,
    class_fields: HashMap<String, HashMap<String, Type>>,
    class_methods: HashMap<String, HashMap<String, Scheme>>,
    class_parents: HashMap<String, String>,
    interface_methods: HashMap<String, HashMap<String, Scheme>>,
    class_interfaces: HashMap<String, Vec<String>>,
    declared_classes: BTreeSet<String>,
    enum_defs: HashMap<String, Vec<String>>,
    exhaust_checker: ExhaustivenessChecker,
    function_hints: HashMap<String, FnDef>,
    binding_hints: HashMap<String, TypeExpr>,
    current_return: Vec<Type>,
    current_effect: Vec<Option<Type>>,
    current_class: Vec<String>,
    type_param_scopes: Vec<HashMap<String, Type>>,
}

impl TypeChecker {
    /// Creates a new type checker with the default built-in environment.
    pub fn new() -> Self {
        let fresh_counter = Rc::new(Cell::new(0));
        let mut checker = Self {
            env: TypeEnv::with_counter(Rc::clone(&fresh_counter)),
            errors: Vec::new(),
            warnings: Vec::new(),
            next_var: 0,
            fresh_counter,
            subst: Substitution::empty(),
            class_fields: HashMap::new(),
            class_methods: HashMap::new(),
            class_parents: HashMap::new(),
            interface_methods: HashMap::new(),
            class_interfaces: HashMap::new(),
            declared_classes: BTreeSet::new(),
            enum_defs: HashMap::new(),
            exhaust_checker: ExhaustivenessChecker::default(),
            function_hints: HashMap::new(),
            binding_hints: HashMap::new(),
            current_return: Vec::new(),
            current_effect: Vec::new(),
            current_class: Vec::new(),
            type_param_scopes: Vec::new(),
        };
        checker.install_builtins();
        checker
    }

    /// Type-checks a full program.
    pub fn check(mut self, program: Program) -> TypeCheckResult {
        self.collect_type_hints(&program);
        self.predeclare_items(&program);

        let typed_program = TypedProgram {
            items: program
                .items
                .iter()
                .map(|item| self.infer_item(item))
                .collect(),
        };
        self.check_unresolved_vars(&typed_program);

        TypeCheckResult {
            typed_program,
            errors: self.errors,
            warnings: self.warnings,
        }
    }

    fn infer_item(&mut self, item: &Item) -> TypedItem {
        match item {
            Item::Fn(function) => TypedItem::Fn(self.infer_fn_item(function, None)),
            Item::PanicHandler(function) => {
                TypedItem::PanicHandler(self.infer_fn_item(function, None))
            }
            Item::OomHandler(function) => TypedItem::OomHandler(self.infer_fn_item(function, None)),
            Item::Const(const_def) => TypedItem::Const(self.infer_const_item(const_def)),
            Item::Class(class_def) => TypedItem::Class(self.infer_class_item(class_def)),
            Item::Interface(interface_def) => {
                TypedItem::Interface(self.infer_interface_item(interface_def))
            }
            Item::Enum(enum_def) => TypedItem::Enum(TypedEnumDef {
                name: enum_def.name.clone(),
                variants: enum_def.variants.clone(),
                span: enum_def.span,
            }),
            Item::Error(error_def) => TypedItem::Error(self.infer_error_item(error_def)),
            Item::Import(import_def) => TypedItem::Import(TypedImportDef {
                items: import_def
                    .items
                    .iter()
                    .map(|item| TypedImportItem {
                        name: item.name.clone(),
                        alias: item.alias.clone(),
                        span: item.span,
                    })
                    .collect(),
                span: import_def.span,
            }),
            Item::Extern(extern_block) => TypedItem::Extern(TypedExternBlock {
                abi: extern_block.abi.clone(),
                functions: extern_block
                    .functions
                    .iter()
                    .map(|function| self.infer_fn_item(function, None))
                    .collect(),
                span: extern_block.span,
            }),
            Item::TypeBlock(type_block) => TypedItem::TypeBlock(self.infer_type_block(type_block)),
        }
    }

    fn push_type_params(&mut self, params: &[String]) {
        let scope = params
            .iter()
            .map(|param| (param.clone(), self.fresh_var()))
            .collect::<HashMap<_, _>>();
        self.type_param_scopes.push(scope);
    }

    fn pop_type_params(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<Type> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn infer_const_item(&mut self, const_def: &draton_ast::ConstDef) -> TypedConstDef {
        let (typed_value, value_ty) = self.infer_expr(&const_def.value);
        let mut ty = value_ty;
        if let Some(hint) = self.binding_hints.get(&const_def.name).cloned() {
            let hinted = self.type_from_annotation(&hint);
            ty = self.unify(ty, hinted, const_def.span);
        }
        ty = self.apply_subst(ty);
        let _ = self.env.remove(&const_def.name);
        let scheme = self.generalize(ty.clone());
        self.env.define(&const_def.name, scheme);
        TypedConstDef {
            name: const_def.name.clone(),
            value: typed_value,
            ty,
            span: const_def.span,
        }
    }

    fn infer_class_item(&mut self, class_def: &ClassDef) -> TypedClassDef {
        self.push_type_params(&class_def.type_params);
        self.current_class.push(class_def.name.clone());
        self.env.push_scope();
        self.env.define(
            "self",
            Scheme {
                quantified: Vec::new(),
                ty: Type::Named(class_def.name.clone(), Vec::new()),
            },
        );

        let fields = class_def
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) => Some(self.infer_class_field(class_def, field)),
                ClassMember::Method(_) | ClassMember::Layer(_) => None,
            })
            .collect::<Vec<_>>();

        let mut methods = Vec::new();
        for member in &class_def.members {
            match member {
                ClassMember::Field(_) => {}
                ClassMember::Method(method) => {
                    methods.push(self.infer_fn_item(method, Some(&class_def.name)));
                }
                ClassMember::Layer(layer) => {
                    for method in &layer.methods {
                        methods.push(self.infer_fn_item(method, Some(&class_def.name)));
                    }
                }
            }
        }

        self.env.pop_scope();
        let _ = self.current_class.pop();
        self.pop_type_params();
        self.check_interface_impl(class_def);
        TypedClassDef {
            name: class_def.name.clone(),
            extends: class_def.extends.clone(),
            implements: class_def.implements.clone(),
            fields,
            methods,
            span: class_def.span,
        }
    }

    fn check_interface_impl(&mut self, class_def: &ClassDef) {
        for iface_name in &class_def.implements {
            let Some(required_methods) = self.interface_methods.get(iface_name).cloned() else {
                self.errors.push(TypeError::UndefinedVar {
                    name: iface_name.clone(),
                    line: class_def.span.line,
                    col: class_def.span.col,
                });
                continue;
            };
            for (method_name, iface_scheme) in required_methods {
                let Some(class_scheme) =
                    self.lookup_method_in_class_hierarchy(&class_def.name, &method_name)
                else {
                    self.errors.push(TypeError::MissingInterfaceMethod {
                        class: class_def.name.clone(),
                        interface: iface_name.clone(),
                        method: method_name,
                        line: class_def.span.line,
                        col: class_def.span.col,
                    });
                    continue;
                };
                let iface_ty = self.apply_subst(iface_scheme.ty);
                let class_ty = self.apply_subst(class_scheme.ty);
                if iface_ty != class_ty {
                    self.errors.push(TypeError::Mismatch {
                        expected: iface_ty.to_string(),
                        found: class_ty.to_string(),
                        hint: format!(
                            "make method '{}' match interface '{}'",
                            method_name, iface_name
                        ),
                        line: class_def.span.line,
                        col: class_def.span.col,
                    });
                }
            }
        }
    }

    fn infer_class_field(&mut self, class_def: &ClassDef, field: &FieldDef) -> TypedFieldDef {
        let ty = self
            .lookup_field_ty(&class_def.name, &field.name)
            .unwrap_or_else(|| {
                field
                    .type_hint
                    .as_ref()
                    .map(|hint| self.type_from_annotation(hint))
                    .unwrap_or_else(|| self.fresh_var())
            });
        TypedFieldDef {
            is_mut: field.is_mut,
            name: field.name.clone(),
            ty: self.apply_subst(ty),
            span: field.span,
        }
    }

    fn infer_interface_item(
        &mut self,
        interface_def: &draton_ast::InterfaceDef,
    ) -> TypedInterfaceDef {
        TypedInterfaceDef {
            name: interface_def.name.clone(),
            methods: interface_def
                .methods
                .iter()
                .map(|method| self.infer_interface_method(method))
                .collect(),
            span: interface_def.span,
        }
    }

    fn infer_interface_method(&mut self, function: &FnDef) -> TypedFnDef {
        let hint = self.function_hints.get(&function.name).cloned();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                param
                    .type_hint
                    .as_ref()
                    .or_else(|| {
                        hint.as_ref().and_then(|fn_hint| {
                            fn_hint
                                .params
                                .get(index)
                                .and_then(|value| value.type_hint.as_ref())
                        })
                    })
                    .map(|type_expr| self.type_from_annotation(type_expr))
                    .unwrap_or_else(|| self.fresh_var())
            })
            .collect::<Vec<_>>();
        let ret_type = function
            .ret_type
            .as_ref()
            .or_else(|| hint.as_ref().and_then(|fn_hint| fn_hint.ret_type.as_ref()))
            .map(|type_expr| self.type_from_annotation(type_expr))
            .unwrap_or(Type::Unit);
        let full_ty = Type::Fn(params.clone(), Box::new(ret_type.clone()));
        TypedFnDef {
            is_pub: function.is_pub,
            name: function.name.clone(),
            params: function
                .params
                .iter()
                .zip(params.iter())
                .map(|(param, ty)| TypedParam {
                    name: param.name.clone(),
                    ty: ty.clone(),
                    span: param.span,
                })
                .collect(),
            ret_type: ret_type.clone(),
            body: None,
            ty: full_ty,
            span: function.span,
        }
    }

    fn infer_error_item(&mut self, error_def: &draton_ast::ErrorDef) -> TypedErrorDef {
        TypedErrorDef {
            name: error_def.name.clone(),
            fields: error_def
                .fields
                .iter()
                .map(|field| TypedParam {
                    name: field.name.clone(),
                    ty: field
                        .type_hint
                        .as_ref()
                        .map(|hint| self.type_from_annotation(hint))
                        .unwrap_or_else(|| self.fresh_var()),
                    span: field.span,
                })
                .collect(),
            span: error_def.span,
        }
    }

    fn infer_type_block(&mut self, type_block: &draton_ast::TypeBlock) -> TypedTypeBlock {
        TypedTypeBlock {
            members: type_block
                .members
                .iter()
                .map(|member| match member {
                    TypeMember::Binding {
                        name,
                        type_expr,
                        span,
                    } => TypedTypeMember::Binding {
                        name: name.clone(),
                        ty: self.type_from_annotation(type_expr),
                        span: *span,
                    },
                    TypeMember::Function(function) => {
                        TypedTypeMember::Function(self.infer_fn_item(function, None))
                    }
                })
                .collect(),
            span: type_block.span,
        }
    }

    fn infer_fn_item(&mut self, function: &FnDef, current_class: Option<&str>) -> TypedFnDef {
        let hint = self.function_hints.get(&function.name).cloned();
        let placeholder = self
            .env
            .lookup(&function.name)
            .cloned()
            .map(|scheme| self.instantiate_scheme(&scheme));

        let mut param_types = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                param
                    .type_hint
                    .as_ref()
                    .or_else(|| {
                        hint.as_ref().and_then(|fn_hint| {
                            fn_hint
                                .params
                                .get(index)
                                .and_then(|item| item.type_hint.as_ref())
                        })
                    })
                    .map(|type_expr| self.type_from_annotation(type_expr))
                    .unwrap_or_else(|| self.fresh_var())
            })
            .collect::<Vec<_>>();

        let mut ret_type = function
            .ret_type
            .as_ref()
            .or_else(|| hint.as_ref().and_then(|fn_hint| fn_hint.ret_type.as_ref()))
            .map(|type_expr| self.type_from_annotation(type_expr))
            .unwrap_or_else(|| self.fresh_var());

        if let Some(existing) = placeholder {
            let expected = Type::Fn(param_types.clone(), Box::new(ret_type.clone()));
            let resolved = self.unify(expected, existing, function.span);
            if let Type::Fn(params, ret) = self.apply_subst(resolved) {
                param_types = params;
                ret_type = *ret;
            }
        }

        self.env.push_scope();
        if let Some(class_name) = current_class {
            self.env.define(
                "self",
                Scheme {
                    quantified: Vec::new(),
                    ty: Type::Named(class_name.to_string(), Vec::new()),
                },
            );
        }

        for (param, ty) in function.params.iter().zip(param_types.iter()) {
            self.env.define(
                &param.name,
                Scheme {
                    quantified: Vec::new(),
                    ty: ty.clone(),
                },
            );
        }

        self.current_return.push(ret_type.clone());
        self.current_effect.push(None);
        let typed_body = function.body.as_ref().map(|body| {
            let (typed_block, block_ty) = self.infer_block(body);
            let current_ret = self.current_return.last().cloned().unwrap_or(Type::Unit);
            let _ = self.unify(current_ret, block_ty, body.span);
            typed_block
        });
        let effect = self.current_effect.pop().unwrap_or(None);
        let _ = self.current_return.pop();
        self.env.pop_scope();

        let resolved_params = param_types
            .iter()
            .map(|ty| self.apply_subst(ty.clone()))
            .collect::<Vec<_>>();
        let mut resolved_ret = self.apply_subst(ret_type);
        if let Some(effect_ty) = effect {
            let resolved_effect = self.apply_subst(effect_ty);
            resolved_ret = match resolved_ret {
                Type::Result(ok, err) => {
                    let _ = self.unify(*err, resolved_effect.clone(), function.span);
                    Type::Result(ok, Box::new(resolved_effect))
                }
                other => Type::Result(Box::new(other), Box::new(resolved_effect)),
            };
        }

        let full_ty = Type::Fn(resolved_params.clone(), Box::new(resolved_ret.clone()));
        let _ = self.env.remove(&function.name);
        let scheme = self.generalize(full_ty.clone());
        if let Some(class_name) = current_class {
            self.class_methods
                .entry(class_name.to_string())
                .or_default()
                .insert(function.name.clone(), scheme.clone());
        } else {
            self.env.define(&function.name, scheme.clone());
        }

        TypedFnDef {
            is_pub: function.is_pub,
            name: function.name.clone(),
            params: function
                .params
                .iter()
                .zip(resolved_params.iter())
                .map(|(param, ty)| TypedParam {
                    name: param.name.clone(),
                    ty: ty.clone(),
                    span: param.span,
                })
                .collect(),
            ret_type: resolved_ret,
            body: typed_body,
            ty: full_ty,
            span: function.span,
        }
    }

    fn infer_block(&mut self, block: &Block) -> (TypedBlock, Type) {
        self.env.push_scope();
        let mut typed_stmts = Vec::new();
        let mut last_ty = Type::Unit;
        let last_index = block.stmts.len().saturating_sub(1);
        for (index, stmt) in block.stmts.iter().enumerate() {
            let typed_stmt = if index == last_index {
                if let Stmt::Expr(expr) = stmt {
                    let (typed_expr, _) =
                        self.infer_expr_with_expected(expr, self.current_return.last().cloned());
                    TypedStmt {
                        kind: TypedStmtKind::Expr(typed_expr),
                        span: expr.span(),
                    }
                } else {
                    self.infer_stmt(stmt)
                }
            } else {
                self.infer_stmt(stmt)
            };
            if let TypedStmtKind::Expr(expr) = &typed_stmt.kind {
                last_ty = expr.ty.clone();
            } else {
                last_ty = Type::Unit;
            }
            typed_stmts.push(typed_stmt);
        }
        self.env.pop_scope();
        (
            TypedBlock {
                stmts: typed_stmts,
                span: block.span,
            },
            self.apply_subst(last_ty),
        )
    }

    fn infer_stmt(&mut self, stmt: &Stmt) -> TypedStmt {
        match stmt {
            Stmt::Let(let_stmt) => {
                let hinted = let_stmt
                    .type_hint
                    .as_ref()
                    .map(|type_hint| self.type_from_annotation(type_hint));
                let (typed_value, mut ty) = if let Some(expr) = &let_stmt.value {
                    let (typed, value_ty) = self.infer_expr_with_expected(expr, hinted.clone());
                    (Some(typed), value_ty)
                } else {
                    (None, self.fresh_var())
                };
                if let Some(hinted) = hinted {
                    ty = self.unify(ty, hinted, let_stmt.span);
                }
                ty = self.apply_subst(ty);
                let scheme = self.generalize(ty.clone());
                self.env.define(&let_stmt.name, scheme);
                TypedStmt {
                    kind: TypedStmtKind::Let(TypedLetStmt {
                        is_mut: let_stmt.is_mut,
                        name: let_stmt.name.clone(),
                        value: typed_value,
                        ty,
                        span: let_stmt.span,
                    }),
                    span: let_stmt.span,
                }
            }
            Stmt::LetDestructure(let_stmt) => self.infer_let_destructure(let_stmt),
            Stmt::Assign(assign) => {
                let (typed_target, target_ty) = self.infer_expr(&assign.target);
                let typed_value = assign.value.as_ref().map(|expr| self.infer_expr(expr));
                if let Some((_, value_ty)) = &typed_value {
                    match assign.op {
                        AssignOp::Assign => {
                            let _ = self.unify(target_ty.clone(), value_ty.clone(), assign.span);
                        }
                        AssignOp::AddAssign
                        | AssignOp::SubAssign
                        | AssignOp::MulAssign
                        | AssignOp::DivAssign
                        | AssignOp::ModAssign => {
                            let _ = self.expect_numeric(
                                target_ty.clone(),
                                assign.span,
                                "assignment target",
                            );
                            let _ = self.expect_numeric(
                                value_ty.clone(),
                                assign.span,
                                "assignment value",
                            );
                            let _ = self.unify(target_ty.clone(), value_ty.clone(), assign.span);
                        }
                        AssignOp::Inc | AssignOp::Dec => {}
                    }
                } else if matches!(assign.op, AssignOp::Inc | AssignOp::Dec) {
                    let _ = self.expect_numeric(target_ty.clone(), assign.span, "increment target");
                }

                TypedStmt {
                    kind: TypedStmtKind::Assign(TypedAssignStmt {
                        target: typed_target,
                        op: assign.op,
                        value: typed_value.map(|(typed, _)| typed),
                        span: assign.span,
                    }),
                    span: assign.span,
                }
            }
            Stmt::Return(return_stmt) => {
                let expected = self.current_return.last().cloned();
                let (typed_value, ty) = if let Some(expr) = &return_stmt.value {
                    let (typed, ty) = self.infer_expr_with_expected(expr, expected.clone());
                    (Some(typed), ty)
                } else {
                    (None, Type::Unit)
                };
                if let Some(expected) = self.current_return.last().cloned() {
                    let _ = self.unify(expected, ty.clone(), return_stmt.span);
                }
                TypedStmt {
                    kind: TypedStmtKind::Return(TypedReturnStmt {
                        value: typed_value,
                        ty: self.apply_subst(ty),
                        span: return_stmt.span,
                    }),
                    span: return_stmt.span,
                }
            }
            Stmt::Expr(expr) => {
                let (typed, _) = self.infer_expr(expr);
                TypedStmt {
                    kind: TypedStmtKind::Expr(typed),
                    span: expr.span(),
                }
            }
            Stmt::If(if_stmt) => {
                let typed = self.infer_if_stmt(if_stmt);
                TypedStmt {
                    span: if_stmt.span,
                    kind: TypedStmtKind::If(typed),
                }
            }
            Stmt::For(for_stmt) => {
                let (typed_iter, iter_ty) = self.infer_expr(&for_stmt.iter);
                let item_ty = match self.apply_subst(iter_ty.clone()) {
                    Type::Array(inner) | Type::Set(inner) | Type::Chan(inner) => *inner,
                    Type::Var(_) => {
                        let fresh = self.fresh_var();
                        let array_ty = Type::Array(Box::new(fresh.clone()));
                        let _ = self.unify(iter_ty, array_ty, for_stmt.span);
                        fresh
                    }
                    other => {
                        self.errors.push(TypeError::Mismatch {
                            expected: "Array[T], Set[T], or Chan[T]".to_string(),
                            found: other.to_string(),
                            hint: "iterate over a collection value".to_string(),
                            line: for_stmt.span.line,
                            col: for_stmt.span.col,
                        });
                        self.fresh_var()
                    }
                };
                self.env.push_scope();
                self.env.define(
                    &for_stmt.name,
                    Scheme {
                        quantified: Vec::new(),
                        ty: item_ty.clone(),
                    },
                );
                let (typed_body, _) = self.infer_block(&for_stmt.body);
                self.env.pop_scope();
                TypedStmt {
                    span: for_stmt.span,
                    kind: TypedStmtKind::For(TypedForStmt {
                        name: for_stmt.name.clone(),
                        iter: typed_iter,
                        item_type: self.apply_subst(item_ty),
                        body: typed_body,
                        span: for_stmt.span,
                    }),
                }
            }
            Stmt::While(while_stmt) => {
                let (typed_cond, cond_ty) = self.infer_expr(&while_stmt.condition);
                let _ = self.unify(cond_ty, Type::Bool, while_stmt.span);
                let (typed_body, _) = self.infer_block(&while_stmt.body);
                TypedStmt {
                    span: while_stmt.span,
                    kind: TypedStmtKind::While(TypedWhileStmt {
                        condition: typed_cond,
                        body: typed_body,
                        span: while_stmt.span,
                    }),
                }
            }
            Stmt::Spawn(spawn_stmt) => {
                let body = match &spawn_stmt.body {
                    SpawnBody::Expr(expr) => TypedSpawnBody::Expr(self.infer_expr(expr).0),
                    SpawnBody::Block(block) => TypedSpawnBody::Block(self.infer_block(block).0),
                };
                TypedStmt {
                    span: spawn_stmt.span,
                    kind: TypedStmtKind::Spawn(TypedSpawnStmt {
                        body,
                        span: spawn_stmt.span,
                    }),
                }
            }
            Stmt::Block(block) => {
                let typed = self.infer_block(block).0;
                TypedStmt {
                    span: block.span,
                    kind: TypedStmtKind::Block(typed),
                }
            }
            Stmt::UnsafeBlock(block) => {
                let typed = self.infer_block(block).0;
                TypedStmt {
                    span: block.span,
                    kind: TypedStmtKind::UnsafeBlock(typed),
                }
            }
            Stmt::PointerBlock(block) => {
                let typed = self.infer_block(block).0;
                TypedStmt {
                    span: block.span,
                    kind: TypedStmtKind::PointerBlock(typed),
                }
            }
            Stmt::AsmBlock(code, span) => TypedStmt {
                span: *span,
                kind: TypedStmtKind::AsmBlock(code.clone()),
            },
            Stmt::ComptimeBlock(block) => {
                let typed = self.infer_block(block).0;
                TypedStmt {
                    span: block.span,
                    kind: TypedStmtKind::ComptimeBlock(typed),
                }
            }
            Stmt::IfCompile(if_compile) => {
                let (typed_cond, cond_ty) = self.infer_expr(&if_compile.condition);
                let _ = self.unify(cond_ty, Type::Bool, if_compile.span);
                let (typed_body, _) = self.infer_block(&if_compile.body);
                TypedStmt {
                    span: if_compile.span,
                    kind: TypedStmtKind::IfCompile(TypedIfCompileStmt {
                        condition: typed_cond,
                        body: typed_body,
                        span: if_compile.span,
                    }),
                }
            }
            Stmt::GcConfig(gc_stmt) => {
                let typed = self.infer_gc_config(gc_stmt);
                TypedStmt {
                    span: gc_stmt.span,
                    kind: TypedStmtKind::GcConfig(typed),
                }
            }
        }
    }

    fn infer_let_destructure(&mut self, stmt: &LetDestructureStmt) -> TypedStmt {
        let (typed_value, value_ty) = self.infer_expr(&stmt.value);
        let resolved_value_ty = self.apply_subst(value_ty.clone());
        let slot_tys = stmt
            .names
            .iter()
            .map(|_| self.fresh_var())
            .collect::<Vec<_>>();
        let expected_tuple = Type::Tuple(slot_tys.clone());
        let tuple_ty = match &resolved_value_ty {
            Type::Tuple(items) => {
                if items.len() != stmt.names.len() {
                    self.errors.push(TypeError::DestructureArity {
                        pattern_len: stmt.names.len(),
                        tuple_len: items.len(),
                        line: stmt.span.line,
                        col: stmt.span.col,
                    });
                }
                self.unify(resolved_value_ty.clone(), expected_tuple.clone(), stmt.span)
            }
            _ => self.unify(value_ty, expected_tuple.clone(), stmt.span),
        };

        let mut bindings = Vec::with_capacity(stmt.names.len());
        for (binding, slot_ty) in stmt.names.iter().zip(slot_tys.iter()) {
            let concrete = self.apply_subst(slot_ty.clone());
            match binding {
                DestructureBinding::Name(name) => {
                    self.env.define(
                        name,
                        Scheme {
                            quantified: Vec::new(),
                            ty: concrete.clone(),
                        },
                    );
                    bindings.push(TypedDestructureBinding::Name {
                        name: name.clone(),
                        ty: concrete,
                    });
                }
                DestructureBinding::Wildcard => bindings.push(TypedDestructureBinding::Wildcard),
            }
        }

        TypedStmt {
            kind: TypedStmtKind::LetDestructure(TypedLetDestructureStmt {
                is_mut: stmt.is_mut,
                bindings,
                value: typed_value,
                tuple_ty: self.apply_subst(tuple_ty),
                span: stmt.span,
            }),
            span: stmt.span,
        }
    }

    fn infer_if_stmt(&mut self, if_stmt: &IfStmt) -> TypedIfStmt {
        let (typed_cond, cond_ty) = self.infer_expr(&if_stmt.condition);
        let _ = self.unify(cond_ty, Type::Bool, if_stmt.span);
        let (typed_then, _) = self.infer_block(&if_stmt.then_branch);
        let typed_else = if_stmt.else_branch.as_ref().map(|branch| match branch {
            ElseBranch::If(child) => TypedElseBranch::If(Box::new(self.infer_if_stmt(child))),
            ElseBranch::Block(block) => TypedElseBranch::Block(self.infer_block(block).0),
        });
        TypedIfStmt {
            condition: typed_cond,
            then_branch: typed_then,
            else_branch: typed_else,
            span: if_stmt.span,
        }
    }

    fn infer_gc_config(&mut self, gc_stmt: &GcConfigStmt) -> TypedGcConfigStmt {
        TypedGcConfigStmt {
            entries: gc_stmt
                .entries
                .iter()
                .map(|entry| TypedGcConfigEntry {
                    key: entry.key.clone(),
                    value: self.infer_expr(&entry.value).0,
                    span: entry.span,
                })
                .collect(),
            span: gc_stmt.span,
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> (TypedExpr, Type) {
        match expr {
            Expr::IntLit(value, span) => {
                self.typed_expr(TypedExprKind::IntLit(*value), Type::Int, *span)
            }
            Expr::FloatLit(value, span) => {
                self.typed_expr(TypedExprKind::FloatLit(*value), Type::Float, *span)
            }
            Expr::StrLit(value, span) => {
                self.typed_expr(TypedExprKind::StrLit(value.clone()), Type::String, *span)
            }
            Expr::FStrLit(parts, span) => {
                let typed_parts = parts
                    .iter()
                    .map(|part| match part {
                        FStrPart::Literal(value) => TypedFStrPart::Literal(value.clone()),
                        FStrPart::Interp(expr) => TypedFStrPart::Interp(self.infer_expr(expr).0),
                    })
                    .collect();
                self.typed_expr(TypedExprKind::FStrLit(typed_parts), Type::String, *span)
            }
            Expr::BoolLit(value, span) => {
                self.typed_expr(TypedExprKind::BoolLit(*value), Type::Bool, *span)
            }
            Expr::NoneLit(span) => {
                let ty = Type::Option(Box::new(self.fresh_var()));
                self.typed_expr(TypedExprKind::NoneLit, ty.clone(), *span)
            }
            Expr::Ident(name, span) => {
                let ty = if let Some(scheme) = self.env.lookup(name).cloned() {
                    self.instantiate_scheme(&scheme)
                } else {
                    self.errors.push(TypeError::UndefinedVar {
                        name: name.clone(),
                        line: span.line,
                        col: span.col,
                    });
                    self.fresh_var()
                };
                self.typed_expr(TypedExprKind::Ident(name.clone()), ty.clone(), *span)
            }
            Expr::Array(items, span) => {
                let element_ty = self.fresh_var();
                let typed_items = items
                    .iter()
                    .map(|item| {
                        let (typed, ty) = self.infer_expr(item);
                        let _ = self.unify(element_ty.clone(), ty, item.span());
                        typed
                    })
                    .collect::<Vec<_>>();
                let ty = Type::Array(Box::new(self.apply_subst(element_ty.clone())));
                self.typed_expr(TypedExprKind::Array(typed_items), ty.clone(), *span)
            }
            Expr::Map(entries, span) => {
                let key_ty = self.fresh_var();
                let value_ty = self.fresh_var();
                let typed_entries = entries
                    .iter()
                    .map(|(key, value)| {
                        let (typed_key, inferred_key_ty) = self.infer_map_key_expr(key);
                        let (typed_value, inferred_value_ty) = self.infer_expr(value);
                        let _ = self.unify(key_ty.clone(), inferred_key_ty, key.span());
                        let _ = self.unify(value_ty.clone(), inferred_value_ty, value.span());
                        (typed_key, typed_value)
                    })
                    .collect::<Vec<_>>();
                let ty = Type::Map(
                    Box::new(self.apply_subst(key_ty.clone())),
                    Box::new(self.apply_subst(value_ty.clone())),
                );
                self.typed_expr(TypedExprKind::Map(typed_entries), ty.clone(), *span)
            }
            Expr::Set(items, span) => {
                let element_ty = self.fresh_var();
                let typed_items = items
                    .iter()
                    .map(|item| {
                        let (typed, ty) = self.infer_expr(item);
                        let _ = self.unify(element_ty.clone(), ty, item.span());
                        typed
                    })
                    .collect::<Vec<_>>();
                let ty = Type::Set(Box::new(self.apply_subst(element_ty.clone())));
                self.typed_expr(TypedExprKind::Set(typed_items), ty.clone(), *span)
            }
            Expr::Tuple(items, span) => {
                let typed_items = items
                    .iter()
                    .map(|item| self.infer_expr(item))
                    .collect::<Vec<_>>();
                let ty = Type::Tuple(
                    typed_items
                        .iter()
                        .map(|(_, ty)| self.apply_subst(ty.clone()))
                        .collect(),
                );
                self.typed_expr(
                    TypedExprKind::Tuple(typed_items.into_iter().map(|(expr, _)| expr).collect()),
                    ty.clone(),
                    *span,
                )
            }
            Expr::BinOp(lhs, op, rhs, span) => self.infer_binop(lhs, *op, rhs, *span),
            Expr::UnOp(op, expr, span) => self.infer_unop(*op, expr, *span),
            Expr::Call(callee, args, span) => self.infer_call(callee, args, *span),
            Expr::MethodCall(target, name, args, span) => {
                self.infer_method_call(target, name, args, *span)
            }
            Expr::Field(target, field, span) => self.infer_field(target, field, *span),
            Expr::Index(target, index, span) => self.infer_index(target, index, *span),
            Expr::Lambda(params, body, span) => self.infer_lambda(params, body, *span, None),
            Expr::Cast(expr, ty, span) => self.infer_cast(expr, ty, *span),
            Expr::Match(subject, arms, span) => self.infer_match(subject, arms, *span),
            Expr::Ok(expr, span) => {
                let (typed, ok_ty) = self.infer_expr(expr);
                let err_ty = self.fresh_var();
                let ty = Type::Result(Box::new(ok_ty), Box::new(err_ty));
                self.typed_expr(TypedExprKind::Ok(Box::new(typed)), ty.clone(), *span)
            }
            Expr::Err(expr, span) => {
                let (typed, err_ty) = self.infer_expr(expr);
                let ok_ty = self.fresh_var();
                let ty = Type::Result(Box::new(ok_ty), Box::new(err_ty));
                self.typed_expr(TypedExprKind::Err(Box::new(typed)), ty.clone(), *span)
            }
            Expr::Nullish(lhs, rhs, span) => self.infer_nullish(lhs, rhs, *span),
            Expr::Chan(type_expr, span) => {
                let ty = Type::Chan(Box::new(self.type_from_annotation(type_expr)));
                self.typed_expr(TypedExprKind::Chan(ty.clone()), ty.clone(), *span)
            }
        }
    }

    fn infer_expr_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<Type>,
    ) -> (TypedExpr, Type) {
        match expr {
            Expr::Lambda(params, body, span) => self.infer_lambda(params, body, *span, expected),
            _ => {
                let (mut typed, ty) = self.infer_expr(expr);
                if let Some(expected) = expected {
                    let original_ty = self.apply_subst(ty.clone());
                    let expected_ty = self.apply_subst(expected.clone());
                    let resolved = self.unify(ty, expected, expr.span());
                    let resolved = self.apply_subst(resolved);
                    if self.can_upcast_to_interface(&original_ty, &expected_ty) {
                        let cast_span = expr.span();
                        let cast_kind = TypedExprKind::Cast(Box::new(typed), expected_ty.clone());
                        (
                            TypedExpr {
                                kind: cast_kind,
                                ty: expected_ty.clone(),
                                span: cast_span,
                            },
                            expected_ty,
                        )
                    } else {
                        typed.ty = resolved.clone();
                        (typed, resolved)
                    }
                } else {
                    (typed, ty)
                }
            }
        }
    }

    fn infer_map_key_expr(&mut self, expr: &Expr) -> (TypedExpr, Type) {
        match expr {
            Expr::Ident(name, span) => {
                self.typed_expr(TypedExprKind::StrLit(name.clone()), Type::String, *span)
            }
            _ => self.infer_expr(expr),
        }
    }

    fn infer_binop(
        &mut self,
        lhs: &Expr,
        op: BinOp,
        rhs: &Expr,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_lhs, lhs_ty) = self.infer_expr(lhs);
        let (typed_rhs, rhs_ty) = self.infer_expr(rhs);
        let result_ty = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let lhs_resolved = self.expect_numeric(lhs_ty.clone(), span, "left-hand operand");
                let rhs_resolved = self.expect_numeric(rhs_ty.clone(), span, "right-hand operand");
                self.unify(lhs_resolved, rhs_resolved, span)
            }
            BinOp::Eq | BinOp::Ne => {
                let _ = self.unify(lhs_ty, rhs_ty, span);
                Type::Bool
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let lhs_resolved = self.expect_numeric(lhs_ty.clone(), span, "left-hand operand");
                let rhs_resolved = self.expect_numeric(rhs_ty.clone(), span, "right-hand operand");
                let _ = self.unify(lhs_resolved, rhs_resolved, span);
                Type::Bool
            }
            BinOp::And | BinOp::Or => {
                let _ = self.unify(lhs_ty, Type::Bool, span);
                let _ = self.unify(rhs_ty, Type::Bool, span);
                Type::Bool
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                let _ = self.unify(lhs_ty, Type::Int, span);
                let _ = self.unify(rhs_ty, Type::Int, span);
                Type::Int
            }
            BinOp::Range => {
                let _ = self.unify(lhs_ty, Type::Int, span);
                let _ = self.unify(rhs_ty, Type::Int, span);
                Type::Array(Box::new(Type::Int))
            }
        };
        self.typed_expr(
            TypedExprKind::BinOp(Box::new(typed_lhs), op, Box::new(typed_rhs)),
            self.apply_subst(result_ty.clone()),
            span,
        )
    }

    fn infer_unop(&mut self, op: UnOp, expr: &Expr, span: draton_ast::Span) -> (TypedExpr, Type) {
        let (typed_expr, expr_ty) = self.infer_expr(expr);
        let ty = match op {
            UnOp::Neg => self.expect_numeric(expr_ty, span, "operand"),
            UnOp::Not => {
                let _ = self.unify(expr_ty, Type::Bool, span);
                Type::Bool
            }
            UnOp::BitNot => {
                let _ = self.unify(expr_ty, Type::Int, span);
                Type::Int
            }
            UnOp::Ref => Type::Pointer(Box::new(expr_ty)),
            UnOp::Deref => {
                let inner = self.fresh_var();
                let _ = self.unify(expr_ty, Type::Pointer(Box::new(inner.clone())), span);
                inner
            }
        };
        self.typed_expr(
            TypedExprKind::UnOp(op, Box::new(typed_expr)),
            self.apply_subst(ty.clone()),
            span,
        )
    }

    fn infer_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_callee, callee_ty) = self.infer_expr(callee);
        let expected_params = match self.apply_subst(callee_ty.clone()) {
            Type::Fn(params, _) => Some(params),
            _ => None,
        };
        let typed_args = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                self.infer_expr_with_expected(
                    arg,
                    expected_params
                        .as_ref()
                        .and_then(|params| params.get(index).cloned()),
                )
            })
            .collect::<Vec<_>>();
        let arg_types = typed_args
            .iter()
            .map(|(_, ty)| ty.clone())
            .collect::<Vec<_>>();
        let ret_ty = self.fresh_var();
        let expected_fn = Type::Fn(arg_types.clone(), Box::new(ret_ty.clone()));
        let resolved_callee = self.apply_subst(callee_ty.clone());
        match resolved_callee {
            Type::Fn(params, _) if params.len() != arg_types.len() => {
                self.errors.push(TypeError::ArgCount {
                    expected: params.len(),
                    got: arg_types.len(),
                    line: span.line,
                    col: span.col,
                });
            }
            _ => {}
        }
        let _ = self.unify(callee_ty, expected_fn, span);
        self.typed_expr(
            TypedExprKind::Call(
                Box::new(typed_callee),
                typed_args.into_iter().map(|(expr, _)| expr).collect(),
            ),
            self.apply_subst(ret_ty.clone()),
            span,
        )
    }

    fn infer_method_call(
        &mut self,
        target: &Expr,
        name: &str,
        args: &[Expr],
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_target, target_ty) = self.infer_expr(target);
        let scheme = self.lookup_method_scheme(&target_ty, name, span);
        let instantiated = scheme
            .as_ref()
            .map(|scheme| self.instantiate_scheme(scheme))
            .unwrap_or_else(|| {
                Type::Fn(
                    args.iter().map(|_| self.fresh_var()).collect(),
                    Box::new(self.fresh_var()),
                )
            });
        let expected_params = match self.apply_subst(instantiated.clone()) {
            Type::Fn(params, _) => Some(params),
            _ => None,
        };
        let typed_args = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                self.infer_expr_with_expected(
                    arg,
                    expected_params
                        .as_ref()
                        .and_then(|params| params.get(index).cloned()),
                )
            })
            .collect::<Vec<_>>();
        let arg_types = typed_args
            .iter()
            .map(|(_, ty)| ty.clone())
            .collect::<Vec<_>>();
        let ret_ty = self.fresh_var();
        let expected = Type::Fn(arg_types, Box::new(ret_ty.clone()));
        let resolved = self.apply_subst(instantiated.clone());
        if let Type::Fn(params, _) = &resolved {
            if params.len() != args.len() {
                self.errors.push(TypeError::ArgCount {
                    expected: params.len(),
                    got: args.len(),
                    line: span.line,
                    col: span.col,
                });
            }
        }
        let _ = self.unify(instantiated, expected, span);
        self.typed_expr(
            TypedExprKind::MethodCall(
                Box::new(typed_target),
                name.to_string(),
                typed_args.into_iter().map(|(expr, _)| expr).collect(),
            ),
            self.apply_subst(ret_ty.clone()),
            span,
        )
    }

    fn infer_field(
        &mut self,
        target: &Expr,
        field: &str,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        if let Expr::Ident(enum_name, target_span) = target {
            if self
                .enum_defs
                .get(enum_name)
                .map(|variants| variants.iter().any(|variant| variant == field))
                .unwrap_or(false)
            {
                let enum_ty = Type::Named(enum_name.clone(), Vec::new());
                let typed_target = TypedExpr {
                    kind: TypedExprKind::Ident(enum_name.clone()),
                    ty: enum_ty.clone(),
                    span: *target_span,
                };
                return self.typed_expr(
                    TypedExprKind::Field(Box::new(typed_target), field.to_string()),
                    enum_ty,
                    span,
                );
            }
        }
        let (typed_target, target_ty) = self.infer_expr(target);
        let resolved_target_ty = self.apply_subst(target_ty.clone());
        let field_ty = match &resolved_target_ty {
            Type::Named(class_name, _) => {
                self.lookup_field_ty(class_name, field).unwrap_or_else(|| {
                    self.errors.push(TypeError::NoField {
                        field: field.to_string(),
                        ty: class_name.clone(),
                        line: span.line,
                        col: span.col,
                    });
                    self.fresh_var()
                })
            }
            _ => self.require_field(target_ty, field, span),
        };
        self.typed_expr(
            TypedExprKind::Field(Box::new(typed_target), field.to_string()),
            self.apply_subst(field_ty.clone()),
            span,
        )
    }

    fn infer_index(
        &mut self,
        target: &Expr,
        index: &Expr,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_target, target_ty) = self.infer_expr(target);
        let (typed_index, index_ty) = self.infer_expr(index);
        let result_ty = match self.apply_subst(target_ty.clone()) {
            Type::Array(inner) => {
                let _ = self.unify(index_ty, Type::Int, span);
                *inner
            }
            Type::Map(key, value) => {
                let _ = self.unify(index_ty, *key, span);
                *value
            }
            Type::String => {
                let _ = self.unify(index_ty, Type::Int, span);
                Type::Char
            }
            Type::Var(_) => {
                let elem = self.fresh_var();
                let _ = self.unify(target_ty, Type::Array(Box::new(elem.clone())), span);
                let _ = self.unify(index_ty, Type::Int, span);
                elem
            }
            other => {
                self.errors.push(TypeError::Mismatch {
                    expected: "indexable type".to_string(),
                    found: other.to_string(),
                    hint: "use indexing on arrays, maps, or strings".to_string(),
                    line: span.line,
                    col: span.col,
                });
                self.fresh_var()
            }
        };
        self.typed_expr(
            TypedExprKind::Index(Box::new(typed_target), Box::new(typed_index)),
            self.apply_subst(result_ty.clone()),
            span,
        )
    }

    fn infer_lambda(
        &mut self,
        params: &[String],
        body: &Expr,
        span: draton_ast::Span,
        expected: Option<Type>,
    ) -> (TypedExpr, Type) {
        let expected_fn = match expected.map(|ty| self.apply_subst(ty)) {
            Some(Type::Fn(params, ret)) => Some((params, *ret)),
            _ => None,
        };
        self.env.push_scope();
        let typed_params = params
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let ty = match &expected_fn {
                    Some((param_types, _)) if index < param_types.len() => {
                        let fresh = self.fresh_var();
                        self.unify(fresh, param_types[index].clone(), span)
                    }
                    _ => self.fresh_var(),
                };
                self.env.define(
                    name,
                    Scheme {
                        quantified: Vec::new(),
                        ty: ty.clone(),
                    },
                );
                TypedParam {
                    name: name.clone(),
                    ty,
                    span,
                }
            })
            .collect::<Vec<_>>();
        let expected_ret = expected_fn.as_ref().map(|(_, ret)| ret.clone());
        let (typed_body, mut body_ty) = self.infer_expr_with_expected(body, expected_ret.clone());
        if let Some(expected_ret) = expected_ret {
            body_ty = self.unify(body_ty, expected_ret, span);
        }
        self.env.pop_scope();
        let resolved_params = typed_params
            .into_iter()
            .map(|mut param| {
                param.ty = self.apply_subst(param.ty);
                param
            })
            .collect::<Vec<_>>();
        let fn_ty = Type::Fn(
            resolved_params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            Box::new(self.apply_subst(body_ty.clone())),
        );
        self.typed_expr(
            TypedExprKind::Lambda(resolved_params, Box::new(typed_body)),
            self.apply_subst(fn_ty.clone()),
            span,
        )
    }

    fn infer_cast(
        &mut self,
        expr: &Expr,
        type_expr: &TypeExpr,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_expr, from_ty) = self.infer_expr(expr);
        let to_ty = self.type_from_annotation(type_expr);
        if !self.is_castable(
            &self.apply_subst(from_ty.clone()),
            &self.apply_subst(to_ty.clone()),
        ) {
            self.errors.push(TypeError::BadCast {
                from: self.apply_subst(from_ty).to_string(),
                to: self.apply_subst(to_ty.clone()).to_string(),
                line: span.line,
                col: span.col,
            });
        }
        self.typed_expr(
            TypedExprKind::Cast(Box::new(typed_expr), to_ty.clone()),
            to_ty.clone(),
            span,
        )
    }

    fn infer_match(
        &mut self,
        subject: &Expr,
        arms: &[MatchArm],
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_subject, subject_ty) = self.infer_expr(subject);
        let result_ty = self.fresh_var();
        let expected_subject = self.apply_subst(subject_ty.clone());
        let typed_arms = arms
            .iter()
            .map(|arm| {
                self.env.push_scope();
                let (typed_pattern, pattern_ty) =
                    self.infer_pattern(&arm.pattern, Some(expected_subject.clone()));
                let _ = self.unify(subject_ty.clone(), pattern_ty, arm.pattern.span());
                let (typed_body, body_ty) = match &arm.body {
                    MatchArmBody::Expr(expr) => {
                        let (typed, ty) = self.infer_expr(expr);
                        (TypedMatchArmBody::Expr(typed), ty)
                    }
                    MatchArmBody::Block(block) => {
                        let (typed, ty) = self.infer_block(block);
                        (TypedMatchArmBody::Block(typed), ty)
                    }
                };
                let _ = self.unify(result_ty.clone(), body_ty, arm.span);
                self.env.pop_scope();
                TypedMatchArm {
                    pattern: typed_pattern,
                    body: typed_body,
                    span: arm.span,
                }
            })
            .collect::<Vec<_>>();
        let resolved_subject_ty = self.apply_subst(expected_subject);
        let subject_kind = classify_subject(&resolved_subject_ty, &self.enum_defs);
        let patterns = typed_arms
            .iter()
            .map(|arm| extract_pattern(&arm.pattern))
            .collect::<Vec<_>>();
        let missing = self.exhaust_checker.check(&patterns, &subject_kind);
        if !missing.is_empty() {
            self.errors.push(TypeError::NonExhaustiveMatch {
                missing: missing.join(", "),
                line: span.line,
                col: span.col,
            });
        }
        for (index, pattern) in self
            .exhaust_checker
            .check_redundancy(&patterns, &subject_kind)
        {
            let arm = &typed_arms[index];
            self.warnings.push(TypeError::RedundantPattern {
                pattern,
                line: arm.span.line,
                col: arm.span.col,
            });
        }
        self.typed_expr(
            TypedExprKind::Match(Box::new(typed_subject), typed_arms),
            self.apply_subst(result_ty.clone()),
            span,
        )
    }

    fn infer_pattern(&mut self, pattern: &Expr, expected: Option<Type>) -> (TypedExpr, Type) {
        match pattern {
            Expr::Ident(name, span) if name == "_" => {
                let ty = expected.unwrap_or_else(|| self.fresh_var());
                self.typed_expr(TypedExprKind::Ident(name.clone()), ty, *span)
            }
            Expr::Ident(name, span) => {
                if let Some(scheme) = self.env.lookup(name).cloned() {
                    let ty = self.instantiate_scheme(&scheme);
                    self.typed_expr(TypedExprKind::Ident(name.clone()), ty, *span)
                } else {
                    let ty = expected.unwrap_or_else(|| self.fresh_var());
                    self.env.define(
                        name,
                        Scheme {
                            quantified: Vec::new(),
                            ty: ty.clone(),
                        },
                    );
                    self.typed_expr(TypedExprKind::Ident(name.clone()), ty, *span)
                }
            }
            Expr::IntLit(value, span) => {
                self.typed_expr(TypedExprKind::IntLit(*value), Type::Int, *span)
            }
            Expr::FloatLit(value, span) => {
                self.typed_expr(TypedExprKind::FloatLit(*value), Type::Float, *span)
            }
            Expr::StrLit(value, span) => {
                self.typed_expr(TypedExprKind::StrLit(value.clone()), Type::String, *span)
            }
            Expr::BoolLit(value, span) => {
                self.typed_expr(TypedExprKind::BoolLit(*value), Type::Bool, *span)
            }
            Expr::NoneLit(span) => {
                let ty = match expected {
                    Some(expected_ty) => match self.apply_subst(expected_ty.clone()) {
                        Type::Option(_) => expected_ty,
                        _ => {
                            let inner = self.fresh_var();
                            self.unify(expected_ty, Type::Option(Box::new(inner)), *span)
                        }
                    },
                    None => Type::Option(Box::new(self.fresh_var())),
                };
                self.typed_expr(TypedExprKind::NoneLit, ty, *span)
            }
            Expr::Tuple(items, span) => {
                let expected_items = match expected {
                    Some(Type::Tuple(expected_items)) if expected_items.len() == items.len() => {
                        Some(expected_items)
                    }
                    _ => None,
                };
                let typed_items = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let expected_item = expected_items
                            .as_ref()
                            .and_then(|items| items.get(index))
                            .cloned();
                        self.infer_pattern(item, expected_item)
                    })
                    .collect::<Vec<_>>();
                let ty = Type::Tuple(
                    typed_items
                        .iter()
                        .map(|(_, ty)| self.apply_subst(ty.clone()))
                        .collect(),
                );
                self.typed_expr(
                    TypedExprKind::Tuple(typed_items.into_iter().map(|(expr, _)| expr).collect()),
                    ty,
                    *span,
                )
            }
            Expr::Ok(inner, span) => {
                let (ok_expected, err_expected) = match expected.clone() {
                    Some(Type::Result(ok, err)) => (Some(*ok), Some(*err)),
                    _ => (None, None),
                };
                let (typed_inner, ok_ty) = self.infer_pattern(inner, ok_expected);
                let err_ty = err_expected.unwrap_or_else(|| self.fresh_var());
                self.typed_expr(
                    TypedExprKind::Ok(Box::new(typed_inner)),
                    Type::Result(Box::new(self.apply_subst(ok_ty)), Box::new(err_ty)),
                    *span,
                )
            }
            Expr::Err(inner, span) => {
                let (ok_expected, err_expected) = match expected.clone() {
                    Some(Type::Result(ok, err)) => (Some(*ok), Some(*err)),
                    _ => (None, None),
                };
                let (typed_inner, err_ty) = self.infer_pattern(inner, err_expected);
                let ok_ty = ok_expected.unwrap_or_else(|| self.fresh_var());
                self.typed_expr(
                    TypedExprKind::Err(Box::new(typed_inner)),
                    Type::Result(Box::new(ok_ty), Box::new(self.apply_subst(err_ty))),
                    *span,
                )
            }
            Expr::Field(target, field, span) => {
                if let Expr::Ident(enum_name, target_span) = target.as_ref() {
                    if self
                        .enum_defs
                        .get(enum_name)
                        .map(|variants| variants.iter().any(|variant| variant == field))
                        .unwrap_or(false)
                    {
                        let enum_ty = Type::Named(enum_name.clone(), Vec::new());
                        let typed_target = TypedExpr {
                            kind: TypedExprKind::Ident(enum_name.clone()),
                            ty: enum_ty.clone(),
                            span: *target_span,
                        };
                        return self.typed_expr(
                            TypedExprKind::Field(Box::new(typed_target), field.clone()),
                            enum_ty,
                            *span,
                        );
                    }
                }
                self.infer_expr_with_expected(pattern, expected)
            }
            Expr::Call(callee, args, span) => {
                self.infer_pattern_call(callee, args, *span, expected)
            }
            _ => self.infer_expr_with_expected(pattern, expected),
        }
    }

    fn infer_pattern_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: draton_ast::Span,
        expected: Option<Type>,
    ) -> (TypedExpr, Type) {
        let (typed_callee, callee_ty) = self.infer_pattern(callee, None);
        let (param_types, ret_type) = match self.apply_subst(callee_ty.clone()) {
            Type::Fn(params, ret) => (params, *ret),
            Type::Var(_) => {
                let params = (0..args.len())
                    .map(|_| self.fresh_var())
                    .collect::<Vec<_>>();
                let ret = expected.unwrap_or_else(|| self.fresh_var());
                let resolved = self.unify(callee_ty, Type::Fn(params.clone(), Box::new(ret)), span);
                match self.apply_subst(resolved) {
                    Type::Fn(params, ret) => (params, *ret),
                    _ => (
                        (0..args.len()).map(|_| self.fresh_var()).collect(),
                        self.fresh_var(),
                    ),
                }
            }
            other => {
                self.errors.push(TypeError::Mismatch {
                    expected: "callable pattern".to_string(),
                    found: other.to_string(),
                    hint: "use constructors or tuple/literal patterns inside match arms"
                        .to_string(),
                    line: span.line,
                    col: span.col,
                });
                (
                    (0..args.len()).map(|_| self.fresh_var()).collect(),
                    self.fresh_var(),
                )
            }
        };
        if param_types.len() != args.len() {
            self.errors.push(TypeError::ArgCount {
                expected: param_types.len(),
                got: args.len(),
                line: span.line,
                col: span.col,
            });
        }
        let typed_args = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                let expected_arg = param_types.get(index).cloned();
                self.infer_pattern(arg, expected_arg)
            })
            .collect::<Vec<_>>();
        self.typed_expr(
            TypedExprKind::Call(
                Box::new(typed_callee),
                typed_args.into_iter().map(|(expr, _)| expr).collect(),
            ),
            self.apply_subst(ret_type),
            span,
        )
    }

    fn infer_nullish(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let (typed_lhs, lhs_ty) = self.infer_expr(lhs);
        let (typed_rhs, rhs_ty) = self.infer_expr(rhs);
        let result_ty = match self.apply_subst(lhs_ty.clone()) {
            Type::Result(ok, err) => {
                let _ = self.unify(*err.clone(), rhs_ty.clone(), span);
                self.record_effect(*err);
                *ok
            }
            Type::Option(inner) => *inner,
            Type::Var(_) => {
                let ok = self.fresh_var();
                let err = self.fresh_var();
                let _ = self.unify(
                    lhs_ty,
                    Type::Result(Box::new(ok.clone()), Box::new(err.clone())),
                    span,
                );
                let _ = self.unify(err.clone(), rhs_ty, span);
                self.record_effect(err);
                ok
            }
            other => {
                self.errors.push(TypeError::Mismatch {
                    expected: "Result[T, E] or Option[T]".to_string(),
                    found: other.to_string(),
                    hint: "use ?? with optional or fallible expressions".to_string(),
                    line: span.line,
                    col: span.col,
                });
                self.fresh_var()
            }
        };
        self.typed_expr(
            TypedExprKind::Nullish(Box::new(typed_lhs), Box::new(typed_rhs)),
            self.apply_subst(result_ty.clone()),
            span,
        )
    }

    fn record_effect(&mut self, err_ty: Type) {
        let current = self.current_effect.last().cloned().unwrap_or(None);
        let merged = match current {
            Some(existing) => {
                let before = self.errors.len();
                let unified = self.unify(
                    existing.clone(),
                    err_ty.clone(),
                    draton_ast::Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        col: 0,
                    },
                );
                if self.errors.len() > before {
                    self.errors.push(TypeError::IncompatibleErrors {
                        lhs: existing.to_string(),
                        rhs: err_ty.to_string(),
                        line: 0,
                        col: 0,
                    });
                }
                Some(unified)
            }
            None => Some(err_ty),
        };
        if let Some(slot) = self.current_effect.last_mut() {
            *slot = merged;
        }
    }

    fn require_field(&mut self, object_ty: Type, field: &str, span: draton_ast::Span) -> Type {
        let field_ty = self.fresh_var();
        let rest_ty = self.fresh_var();
        let mut fields = BTreeMap::new();
        fields.insert(field.to_string(), field_ty.clone());
        let expected = Type::Row {
            fields,
            rest: Some(Box::new(rest_ty)),
        };
        let _ = self.unify(object_ty, expected, span);
        self.apply_subst(field_ty)
    }

    fn lookup_field_ty(&self, class_name: &str, field: &str) -> Option<Type> {
        let mut current = Some(class_name.to_string());
        while let Some(class_name) = current {
            if let Some(field_ty) = self
                .class_fields
                .get(&class_name)
                .and_then(|fields| fields.get(field))
                .cloned()
            {
                return Some(field_ty);
            }
            current = self.class_parents.get(&class_name).cloned();
        }
        None
    }

    fn lookup_method_in_class_hierarchy(&self, class_name: &str, name: &str) -> Option<Scheme> {
        let mut current = Some(class_name.to_string());
        while let Some(class_name) = current {
            if let Some(method) = self
                .class_methods
                .get(&class_name)
                .and_then(|methods| methods.get(name))
                .cloned()
            {
                return Some(method);
            }
            current = self.class_parents.get(&class_name).cloned();
        }
        None
    }

    fn lookup_method_scheme(
        &mut self,
        target_ty: &Type,
        name: &str,
        span: draton_ast::Span,
    ) -> Option<Scheme> {
        match self.apply_subst(target_ty.clone()) {
            Type::Array(inner) => self.builtin_array_method(name, *inner),
            Type::String => self.builtin_string_method(name),
            Type::Int | Type::Float | Type::Bool => self.builtin_scalar_method(name),
            Type::Chan(inner) => self.builtin_chan_method(name, *inner),
            Type::Named(class_name, _) => self
                .lookup_method_in_class_hierarchy(&class_name, name)
                .or_else(|| {
                    self.interface_methods
                        .get(&class_name)
                        .and_then(|methods| methods.get(name).cloned())
                })
                .or_else(|| {
                    self.errors.push(TypeError::NoField {
                        field: name.to_string(),
                        ty: class_name,
                        line: span.line,
                        col: span.col,
                    });
                    None
                }),
            other => {
                self.errors.push(TypeError::NoField {
                    field: name.to_string(),
                    ty: other.to_string(),
                    line: span.line,
                    col: span.col,
                });
                None
            }
        }
    }

    fn is_interface_name(&self, name: &str) -> bool {
        self.interface_methods.contains_key(name)
    }

    fn class_implements_interface(&self, class_name: &str, interface_name: &str) -> bool {
        let mut current = Some(class_name.to_string());
        while let Some(class_name) = current {
            if self
                .class_interfaces
                .get(&class_name)
                .map(|interfaces| interfaces.iter().any(|name| name == interface_name))
                .unwrap_or(false)
            {
                return true;
            }
            current = self.class_parents.get(&class_name).cloned();
        }
        false
    }

    fn check_no_cycle(&self, start: &str) -> bool {
        let mut visited = BTreeSet::new();
        let mut current = Some(start.to_string());
        while let Some(class_name) = current {
            if !visited.insert(class_name.clone()) {
                return false;
            }
            current = self.class_parents.get(&class_name).cloned();
        }
        true
    }

    fn can_upcast_to_interface(&self, from: &Type, to: &Type) -> bool {
        matches!(
            (self.apply_subst(from.clone()), self.apply_subst(to.clone())),
            (Type::Named(class_name, from_args), Type::Named(interface_name, to_args))
                if from_args.is_empty()
                    && to_args.is_empty()
                    && self.is_interface_name(&interface_name)
                    && self.class_implements_interface(&class_name, &interface_name)
        )
    }

    fn builtin_array_method(&mut self, name: &str, item_ty: Type) -> Option<Scheme> {
        match name {
            "map" => {
                let out = self.fresh_var();
                Some(Scheme {
                    quantified: free_type_vars(&out).into_iter().collect(),
                    ty: Type::Fn(
                        vec![Type::Fn(vec![item_ty], Box::new(out.clone()))],
                        Box::new(Type::Array(Box::new(out))),
                    ),
                })
            }
            "filter" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(
                    vec![Type::Fn(vec![item_ty.clone()], Box::new(Type::Bool))],
                    Box::new(Type::Array(Box::new(item_ty))),
                ),
            }),
            "len" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(Vec::new(), Box::new(Type::Int)),
            }),
            _ => None,
        }
    }

    fn builtin_string_method(&self, name: &str) -> Option<Scheme> {
        match name {
            "trim" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(Vec::new(), Box::new(Type::String)),
            }),
            "len" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(Vec::new(), Box::new(Type::Int)),
            }),
            _ => None,
        }
    }

    fn builtin_scalar_method(&self, name: &str) -> Option<Scheme> {
        match name {
            "toString" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(Vec::new(), Box::new(Type::String)),
            }),
            _ => None,
        }
    }

    fn builtin_chan_method(&self, name: &str, inner: Type) -> Option<Scheme> {
        match name {
            "recv" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(Vec::new(), Box::new(inner)),
            }),
            "send" => Some(Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(vec![inner], Box::new(Type::Unit)),
            }),
            _ => None,
        }
    }

    fn collect_type_hints(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::TypeBlock(type_block) = item {
                for member in &type_block.members {
                    match member {
                        TypeMember::Binding {
                            name, type_expr, ..
                        } => {
                            self.binding_hints.insert(name.clone(), type_expr.clone());
                        }
                        TypeMember::Function(function) => {
                            self.function_hints
                                .insert(function.name.clone(), function.clone());
                        }
                    }
                }
            }
        }
    }

    fn predeclare_items(&mut self, program: &Program) {
        self.declared_classes = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Class(class_def) => Some(class_def.name.clone()),
                _ => None,
            })
            .collect();
        for item in &program.items {
            match item {
                Item::Class(class_def) => {
                    self.predeclare_class(class_def);
                }
                Item::Interface(interface_def) => {
                    self.predeclare_interface(interface_def);
                }
                Item::Fn(function) | Item::PanicHandler(function) | Item::OomHandler(function) => {
                    let ty = self.function_placeholder(function);
                    self.env.define(
                        &function.name,
                        Scheme {
                            quantified: Vec::new(),
                            ty,
                        },
                    );
                }
                Item::Extern(extern_block) => {
                    for function in &extern_block.functions {
                        let ty = self.function_placeholder(function);
                        self.env.define(
                            &function.name,
                            Scheme {
                                quantified: Vec::new(),
                                ty,
                            },
                        );
                    }
                }
                Item::Const(const_def) => {
                    let fresh = self.fresh_var();
                    self.env.define(
                        &const_def.name,
                        Scheme {
                            quantified: Vec::new(),
                            ty: fresh,
                        },
                    );
                }
                Item::Error(error_def) => {
                    let params = error_def
                        .fields
                        .iter()
                        .map(|field| {
                            field
                                .type_hint
                                .as_ref()
                                .map(|hint| self.type_from_annotation(hint))
                                .unwrap_or_else(|| self.fresh_var())
                        })
                        .collect::<Vec<_>>();
                    self.env.define(
                        &error_def.name,
                        Scheme {
                            quantified: Vec::new(),
                            ty: Type::Fn(
                                params,
                                Box::new(Type::Named(error_def.name.clone(), Vec::new())),
                            ),
                        },
                    );
                }
                Item::Enum(enum_def) => {
                    self.enum_defs
                        .insert(enum_def.name.clone(), enum_def.variants.clone());
                    self.exhaust_checker
                        .enum_defs
                        .insert(enum_def.name.clone(), enum_def.variants.clone());
                    for variant in &enum_def.variants {
                        self.env.define(
                            variant,
                            Scheme {
                                quantified: Vec::new(),
                                ty: Type::Named(enum_def.name.clone(), Vec::new()),
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn predeclare_class(&mut self, class_def: &ClassDef) {
        self.push_type_params(&class_def.type_params);
        self.class_interfaces
            .insert(class_def.name.clone(), class_def.implements.clone());
        if let Some(parent) = &class_def.extends {
            if !self.declared_classes.contains(parent) {
                self.errors.push(TypeError::UndefinedParent {
                    class: class_def.name.clone(),
                    parent: parent.clone(),
                    line: class_def.span.line,
                    col: class_def.span.col,
                });
            } else {
                self.class_parents
                    .insert(class_def.name.clone(), parent.clone());
                if !self.check_no_cycle(&class_def.name) {
                    self.errors.push(TypeError::CircularInheritance {
                        class: class_def.name.clone(),
                        line: class_def.span.line,
                        col: class_def.span.col,
                    });
                }
            }
        }
        let fields = class_def
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) => Some((
                    field.name.clone(),
                    field
                        .type_hint
                        .as_ref()
                        .map(|hint| self.type_from_annotation(hint))
                        .unwrap_or_else(|| self.fresh_var()),
                )),
                ClassMember::Method(_) | ClassMember::Layer(_) => None,
            })
            .collect::<HashMap<_, _>>();
        self.class_fields.insert(class_def.name.clone(), fields);

        let mut methods = HashMap::new();
        for member in &class_def.members {
            match member {
                ClassMember::Method(method) => {
                    let ty = self.function_placeholder(method);
                    methods.insert(
                        method.name.clone(),
                        Scheme {
                            quantified: Vec::new(),
                            ty,
                        },
                    );
                }
                ClassMember::Layer(layer) => {
                    for method in &layer.methods {
                        let ty = self.function_placeholder(method);
                        methods.insert(
                            method.name.clone(),
                            Scheme {
                                quantified: Vec::new(),
                                ty,
                            },
                        );
                    }
                }
                ClassMember::Field(_) => {}
            }
        }
        self.class_methods.insert(class_def.name.clone(), methods);
        self.pop_type_params();
    }

    fn predeclare_interface(&mut self, interface_def: &draton_ast::InterfaceDef) {
        let methods = interface_def
            .methods
            .iter()
            .map(|method| {
                let ty = self.interface_method_placeholder(method);
                (
                    method.name.clone(),
                    Scheme {
                        quantified: Vec::new(),
                        ty,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        self.interface_methods
            .insert(interface_def.name.clone(), methods);
    }

    fn interface_method_placeholder(&mut self, function: &FnDef) -> Type {
        let hint = self.function_hints.get(&function.name).cloned();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                param
                    .type_hint
                    .as_ref()
                    .or_else(|| {
                        hint.as_ref().and_then(|fn_hint| {
                            fn_hint
                                .params
                                .get(index)
                                .and_then(|value| value.type_hint.as_ref())
                        })
                    })
                    .map(|type_expr| self.type_from_annotation(type_expr))
                    .unwrap_or_else(|| self.fresh_var())
            })
            .collect::<Vec<_>>();
        let ret = function
            .ret_type
            .as_ref()
            .or_else(|| hint.as_ref().and_then(|fn_hint| fn_hint.ret_type.as_ref()))
            .map(|type_expr| self.type_from_annotation(type_expr))
            .unwrap_or(Type::Unit);
        Type::Fn(params, Box::new(ret))
    }

    fn function_placeholder(&mut self, function: &FnDef) -> Type {
        let hint = self.function_hints.get(&function.name).cloned();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                param
                    .type_hint
                    .as_ref()
                    .or_else(|| {
                        hint.as_ref().and_then(|fn_hint| {
                            fn_hint
                                .params
                                .get(index)
                                .and_then(|value| value.type_hint.as_ref())
                        })
                    })
                    .map(|type_expr| self.type_from_annotation(type_expr))
                    .unwrap_or_else(|| self.fresh_var())
            })
            .collect::<Vec<_>>();
        let ret = function
            .ret_type
            .as_ref()
            .or_else(|| hint.as_ref().and_then(|fn_hint| fn_hint.ret_type.as_ref()))
            .map(|type_expr| self.type_from_annotation(type_expr))
            .unwrap_or_else(|| self.fresh_var());
        Type::Fn(params, Box::new(ret))
    }

    fn install_builtins(&mut self) {
        let print_a = self.fresh_var();
        self.env.define(
            "print",
            Scheme {
                quantified: vec![extract_var(&print_a)],
                ty: Type::Fn(vec![print_a], Box::new(Type::Unit)),
            },
        );
        self.env.define(
            "range",
            Scheme {
                quantified: Vec::new(),
                ty: Type::Fn(
                    vec![Type::Int, Type::Int, Type::Int],
                    Box::new(Type::Array(Box::new(Type::Int))),
                ),
            },
        );
        let some_var = self.fresh_var();
        self.env.define(
            "Some",
            Scheme {
                quantified: vec![extract_var(&some_var)],
                ty: Type::Fn(
                    vec![some_var.clone()],
                    Box::new(Type::Option(Box::new(some_var))),
                ),
            },
        );
    }

    fn fresh_var(&mut self) -> Type {
        let id = self.next_var;
        self.next_var += 1;
        self.fresh_counter.set(self.next_var);
        Type::Var(id)
    }

    fn instantiate_scheme(&mut self, scheme: &Scheme) -> Type {
        let ty = self.env.instantiate(scheme);
        self.next_var = self.fresh_counter.get();
        ty
    }

    fn generalize(&self, ty: Type) -> Scheme {
        let ty = self.apply_subst(ty);
        let env_vars = free_type_vars_in_env(&self.env);
        let quantified = free_type_vars(&ty)
            .into_iter()
            .filter(|var| !env_vars.contains(var))
            .collect::<Vec<_>>();
        Scheme { quantified, ty }
    }

    fn apply_subst(&self, ty: Type) -> Type {
        self.subst.apply(ty)
    }

    fn unify(&mut self, lhs: Type, rhs: Type, span: draton_ast::Span) -> Type {
        let lhs = self.apply_subst(lhs);
        let rhs = self.apply_subst(rhs);
        match (lhs, rhs) {
            (Type::Var(id), ty) | (ty, Type::Var(id)) => self.bind_var(id, ty, span),
            (Type::Int, Type::Int) => Type::Int,
            (Type::Int8, Type::Int8) => Type::Int8,
            (Type::Int16, Type::Int16) => Type::Int16,
            (Type::Int32, Type::Int32) => Type::Int32,
            (Type::Int64, Type::Int64) => Type::Int64,
            (Type::UInt8, Type::UInt8) => Type::UInt8,
            (Type::UInt16, Type::UInt16) => Type::UInt16,
            (Type::UInt32, Type::UInt32) => Type::UInt32,
            (Type::UInt64, Type::UInt64) => Type::UInt64,
            (Type::Float, Type::Float) => Type::Float,
            (Type::Float32, Type::Float32) => Type::Float32,
            (Type::Float64, Type::Float64) => Type::Float64,
            (Type::Bool, Type::Bool) => Type::Bool,
            (Type::String, Type::String) => Type::String,
            (Type::Char, Type::Char) => Type::Char,
            (Type::Unit, Type::Unit) => Type::Unit,
            (Type::Never, Type::Never) => Type::Never,
            (Type::Array(lhs), Type::Array(rhs)) => {
                Type::Array(Box::new(self.unify(*lhs, *rhs, span)))
            }
            (Type::Set(lhs), Type::Set(rhs)) => Type::Set(Box::new(self.unify(*lhs, *rhs, span))),
            (Type::Option(lhs), Type::Option(rhs)) => {
                Type::Option(Box::new(self.unify(*lhs, *rhs, span)))
            }
            (Type::Chan(lhs), Type::Chan(rhs)) => {
                Type::Chan(Box::new(self.unify(*lhs, *rhs, span)))
            }
            (Type::Pointer(lhs), Type::Pointer(rhs)) => {
                Type::Pointer(Box::new(self.unify(*lhs, *rhs, span)))
            }
            (Type::Map(lhs_k, lhs_v), Type::Map(rhs_k, rhs_v)) => Type::Map(
                Box::new(self.unify(*lhs_k, *rhs_k, span)),
                Box::new(self.unify(*lhs_v, *rhs_v, span)),
            ),
            (Type::Tuple(lhs_items), Type::Tuple(rhs_items)) => {
                if lhs_items.len() != rhs_items.len() {
                    self.push_mismatch(
                        "tuple of equal arity",
                        &Type::Tuple(lhs_items),
                        &Type::Tuple(rhs_items),
                        span,
                    );
                    return self.fresh_var();
                }
                Type::Tuple(
                    lhs_items
                        .into_iter()
                        .zip(rhs_items)
                        .map(|(left, right)| self.unify(left, right, span))
                        .collect(),
                )
            }
            (Type::Result(lhs_ok, lhs_err), Type::Result(rhs_ok, rhs_err)) => Type::Result(
                Box::new(self.unify(*lhs_ok, *rhs_ok, span)),
                Box::new(self.unify(*lhs_err, *rhs_err, span)),
            ),
            (Type::Fn(lhs_params, lhs_ret), Type::Fn(rhs_params, rhs_ret)) => {
                if lhs_params.len() != rhs_params.len() {
                    self.errors.push(TypeError::ArgCount {
                        expected: lhs_params.len(),
                        got: rhs_params.len(),
                        line: span.line,
                        col: span.col,
                    });
                    return Type::Fn(lhs_params, lhs_ret);
                }
                Type::Fn(
                    lhs_params
                        .into_iter()
                        .zip(rhs_params)
                        .map(|(left, right)| self.unify(left, right, span))
                        .collect(),
                    Box::new(self.unify(*lhs_ret, *rhs_ret, span)),
                )
            }
            (Type::Named(lhs_name, lhs_args), Type::Named(rhs_name, rhs_args))
                if lhs_args.is_empty()
                    && rhs_args.is_empty()
                    && self.class_implements_interface(&lhs_name, &rhs_name) =>
            {
                Type::Named(rhs_name, Vec::new())
            }
            (Type::Named(lhs_name, lhs_args), Type::Named(rhs_name, rhs_args))
                if lhs_args.is_empty()
                    && rhs_args.is_empty()
                    && self.class_implements_interface(&rhs_name, &lhs_name) =>
            {
                Type::Named(lhs_name, Vec::new())
            }
            (Type::Named(lhs_name, lhs_args), Type::Named(rhs_name, rhs_args))
                if lhs_name == rhs_name && lhs_args.len() == rhs_args.len() =>
            {
                Type::Named(
                    lhs_name,
                    lhs_args
                        .into_iter()
                        .zip(rhs_args)
                        .map(|(left, right)| self.unify(left, right, span))
                        .collect(),
                )
            }
            (
                Type::Row {
                    fields: lhs_fields,
                    rest: lhs_rest,
                },
                Type::Row {
                    fields: rhs_fields,
                    rest: rhs_rest,
                },
            ) => self.unify_rows(lhs_fields, lhs_rest, rhs_fields, rhs_rest, span),
            (
                Type::Row {
                    fields: row_fields,
                    rest: row_rest,
                },
                Type::Named(name, args),
            )
            | (
                Type::Named(name, args),
                Type::Row {
                    fields: row_fields,
                    rest: row_rest,
                },
            ) => self.unify_row_with_named(row_fields, row_rest, name, args, span),
            (lhs, rhs) => {
                self.push_mismatch("matching types", &lhs, &rhs, span);
                lhs
            }
        }
    }

    fn bind_var(&mut self, var: u32, ty: Type, span: draton_ast::Span) -> Type {
        let ty = self.apply_subst(ty);
        if ty == Type::Var(var) {
            return ty;
        }
        if occurs(var, &ty) {
            self.errors.push(TypeError::InfiniteType {
                var: format!("t{var}"),
                line: span.line,
                col: span.col,
            });
            return Type::Var(var);
        }

        match self.subst.clone().bind(var, ty.clone()) {
            Ok(subst) => {
                self.subst = subst;
                ty
            }
            Err(TypeError::InfiniteType { var: var_name, .. }) => {
                self.errors.push(TypeError::InfiniteType {
                    var: var_name,
                    line: span.line,
                    col: span.col,
                });
                Type::Var(var)
            }
            Err(err) => {
                self.errors.push(err);
                Type::Var(var)
            }
        }
    }

    fn unify_rows(
        &mut self,
        mut lhs_fields: BTreeMap<String, Type>,
        lhs_rest: Option<Box<Type>>,
        mut rhs_fields: BTreeMap<String, Type>,
        rhs_rest: Option<Box<Type>>,
        span: draton_ast::Span,
    ) -> Type {
        let mut merged = BTreeMap::new();
        let shared = lhs_fields
            .keys()
            .filter(|key| rhs_fields.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in shared {
            if let (Some(left), Some(right)) = (lhs_fields.remove(&key), rhs_fields.remove(&key)) {
                merged.insert(key, self.unify(left, right, span));
            }
        }

        if rhs_rest.is_none() {
            for field in lhs_fields.keys() {
                self.push_no_field(
                    field,
                    &Type::Row {
                        fields: rhs_fields.clone(),
                        rest: None,
                    },
                    span,
                );
            }
        } else {
            merged.extend(lhs_fields.clone());
        }

        if lhs_rest.is_none() {
            for field in rhs_fields.keys() {
                self.push_no_field(
                    field,
                    &Type::Row {
                        fields: lhs_fields.clone(),
                        rest: None,
                    },
                    span,
                );
            }
        } else {
            merged.extend(rhs_fields.clone());
        }

        let rest = match (lhs_rest, rhs_rest) {
            (Some(lhs), Some(rhs)) => Some(Box::new(self.unify(*lhs, *rhs, span))),
            (Some(lhs), None) => Some(Box::new(self.apply_subst(*lhs))),
            (None, Some(rhs)) => Some(Box::new(self.apply_subst(*rhs))),
            (None, None) => None,
        };

        Type::Row {
            fields: merged
                .into_iter()
                .map(|(name, ty)| (name, self.apply_subst(ty)))
                .collect(),
            rest,
        }
    }

    fn unify_row_with_named(
        &mut self,
        row_fields: BTreeMap<String, Type>,
        row_rest: Option<Box<Type>>,
        name: String,
        args: Vec<Type>,
        span: draton_ast::Span,
    ) -> Type {
        let Some(class_fields) = self.class_fields.get(&name).cloned() else {
            self.push_mismatch(
                "type with accessible fields",
                &Type::Row {
                    fields: row_fields,
                    rest: row_rest,
                },
                &Type::Named(name.clone(), args.clone()),
                span,
            );
            return Type::Named(name, args);
        };

        let mut remaining = class_fields
            .into_iter()
            .map(|(field, ty)| (field, self.apply_subst(ty)))
            .collect::<BTreeMap<_, _>>();

        for (field, expected_ty) in row_fields {
            match remaining.remove(&field) {
                Some(actual_ty) => {
                    let _ = self.unify(expected_ty, actual_ty, span);
                }
                None => self.errors.push(TypeError::NoField {
                    field,
                    ty: name.clone(),
                    line: span.line,
                    col: span.col,
                }),
            }
        }

        if let Some(rest) = row_rest {
            let rest_row = Type::Row {
                fields: remaining,
                rest: None,
            };
            let _ = self.unify(*rest, rest_row, span);
        }

        Type::Named(name, args)
    }

    fn push_no_field(&mut self, field: &str, ty: &Type, span: draton_ast::Span) {
        self.errors.push(TypeError::NoField {
            field: field.to_string(),
            ty: ty.to_string(),
            line: span.line,
            col: span.col,
        });
    }

    fn type_from_annotation(&mut self, type_expr: &TypeExpr) -> Type {
        match type_expr {
            TypeExpr::Named(name, _) => self
                .lookup_type_param(name)
                .unwrap_or_else(|| self.named_type(name)),
            TypeExpr::Generic(name, args, _) => {
                let resolved = args
                    .iter()
                    .map(|arg| self.type_from_annotation(arg))
                    .collect::<Vec<_>>();
                match name.as_str() {
                    "Array" if resolved.len() == 1 => Type::Array(Box::new(resolved[0].clone())),
                    "Map" if resolved.len() == 2 => {
                        Type::Map(Box::new(resolved[0].clone()), Box::new(resolved[1].clone()))
                    }
                    "Set" if resolved.len() == 1 => Type::Set(Box::new(resolved[0].clone())),
                    "Option" if resolved.len() == 1 => Type::Option(Box::new(resolved[0].clone())),
                    "Result" if resolved.len() == 2 => {
                        Type::Result(Box::new(resolved[0].clone()), Box::new(resolved[1].clone()))
                    }
                    "Chan" if resolved.len() == 1 => Type::Chan(Box::new(resolved[0].clone())),
                    other => Type::Named(other.to_string(), resolved),
                }
            }
            TypeExpr::Pointer(_) => Type::Pointer(Box::new(self.fresh_var())),
            TypeExpr::Infer(_) => self.fresh_var(),
        }
    }

    fn named_type(&self, name: &str) -> Type {
        match name {
            "Int" => Type::Int,
            "Int8" => Type::Int8,
            "Int16" => Type::Int16,
            "Int32" => Type::Int32,
            "Int64" => Type::Int64,
            "UInt8" => Type::UInt8,
            "UInt16" => Type::UInt16,
            "UInt32" => Type::UInt32,
            "UInt64" => Type::UInt64,
            "Float" => Type::Float,
            "Float32" => Type::Float32,
            "Float64" => Type::Float64,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Char" => Type::Char,
            "Unit" => Type::Unit,
            "Never" => Type::Never,
            other => Type::Named(other.to_string(), Vec::new()),
        }
    }

    fn is_castable(&self, from: &Type, to: &Type) -> bool {
        if from == to {
            return true;
        }
        matches!(
            (from, to),
            (Type::Int, Type::Float)
                | (Type::Float, Type::Int)
                | (Type::Int, Type::Int8)
                | (Type::Int, Type::Int16)
                | (Type::Int, Type::Int32)
                | (Type::Int, Type::Int64)
                | (Type::Int, Type::UInt8)
                | (Type::Int, Type::UInt16)
                | (Type::Int, Type::UInt32)
                | (Type::Int, Type::UInt64)
                | (Type::Float, Type::Float32)
                | (Type::Float, Type::Float64)
                | (Type::Pointer(_), Type::Pointer(_))
        )
    }

    fn expect_numeric(&mut self, ty: Type, span: draton_ast::Span, hint: &str) -> Type {
        let resolved = self.apply_subst(ty);
        match resolved {
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
            | Type::Float64 => resolved,
            Type::Var(_) => self.unify(resolved, Type::Int, span),
            other => {
                self.errors.push(TypeError::Mismatch {
                    expected: "numeric type".to_string(),
                    found: other.to_string(),
                    hint: hint.to_string(),
                    line: span.line,
                    col: span.col,
                });
                other
            }
        }
    }

    fn check_unresolved_vars(&mut self, program: &TypedProgram) {
        let mut seen = BTreeSet::new();
        for item in &program.items {
            let allowed = self.allowed_item_vars(item);
            self.visit_typed_item(item, &allowed, &mut seen);
        }
    }

    fn allowed_item_vars(&self, item: &TypedItem) -> BTreeSet<u32> {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => free_type_vars(&function.ty),
            TypedItem::Class(class_def) => {
                let mut vars = BTreeSet::new();
                for field in &class_def.fields {
                    vars.extend(free_type_vars(&field.ty));
                }
                for method in &class_def.methods {
                    vars.extend(free_type_vars(&method.ty));
                }
                vars
            }
            TypedItem::Interface(interface_def) => {
                interface_def
                    .methods
                    .iter()
                    .fold(BTreeSet::new(), |mut vars, method| {
                        vars.extend(free_type_vars(&method.ty));
                        vars
                    })
            }
            TypedItem::Error(_) | TypedItem::Const(_) => BTreeSet::new(),
            TypedItem::Extern(extern_block) => {
                extern_block
                    .functions
                    .iter()
                    .fold(BTreeSet::new(), |mut vars, function| {
                        vars.extend(free_type_vars(&function.ty));
                        vars
                    })
            }
            TypedItem::TypeBlock(type_block) => {
                type_block
                    .members
                    .iter()
                    .fold(BTreeSet::new(), |mut vars, member| {
                        match member {
                            TypedTypeMember::Binding { .. } => {}
                            TypedTypeMember::Function(function) => {
                                vars.extend(free_type_vars(&function.ty));
                            }
                        }
                        vars
                    })
            }
            TypedItem::Import(_) | TypedItem::Enum(_) => BTreeSet::new(),
        }
    }

    fn visit_typed_item(
        &mut self,
        item: &TypedItem,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => self.visit_typed_fn(function, allowed, seen),
            TypedItem::Class(class_def) => {
                for field in &class_def.fields {
                    self.report_unresolved_type(&field.ty, &field.name, field.span, allowed, seen);
                }
                for method in &class_def.methods {
                    self.visit_typed_fn(method, allowed, seen);
                }
            }
            TypedItem::Const(const_def) => {
                self.report_unresolved_type(
                    &const_def.ty,
                    &const_def.name,
                    const_def.span,
                    allowed,
                    seen,
                );
                self.visit_typed_expr(&const_def.value, allowed, seen);
            }
            TypedItem::Extern(extern_block) => {
                for function in &extern_block.functions {
                    self.visit_typed_fn(function, allowed, seen);
                }
            }
            TypedItem::Interface(interface_def) => {
                for method in &interface_def.methods {
                    self.visit_typed_fn(method, allowed, seen);
                }
            }
            TypedItem::Error(error_def) => {
                for field in &error_def.fields {
                    self.report_unresolved_type(&field.ty, &field.name, field.span, allowed, seen);
                }
            }
            TypedItem::TypeBlock(type_block) => {
                for member in &type_block.members {
                    match member {
                        TypedTypeMember::Binding { name, ty, span } => {
                            self.report_unresolved_type(ty, name, *span, allowed, seen);
                        }
                        TypedTypeMember::Function(function) => {
                            self.visit_typed_fn(function, allowed, seen);
                        }
                    }
                }
            }
            TypedItem::Import(_) | TypedItem::Enum(_) => {}
        }
    }

    fn visit_typed_fn(
        &mut self,
        function: &TypedFnDef,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        self.report_unresolved_type(
            &function.ret_type,
            &function.name,
            function.span,
            allowed,
            seen,
        );
        self.report_unresolved_type(&function.ty, &function.name, function.span, allowed, seen);
        for param in &function.params {
            self.report_unresolved_type(&param.ty, &param.name, param.span, allowed, seen);
        }
        if let Some(body) = &function.body {
            self.visit_typed_block(body, allowed, seen);
        }
    }

    fn visit_typed_block(
        &mut self,
        block: &TypedBlock,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        for stmt in &block.stmts {
            self.visit_typed_stmt(stmt, allowed, seen);
        }
    }

    fn visit_typed_stmt(
        &mut self,
        stmt: &TypedStmt,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => {
                let stmt_allowed = let_stmt
                    .value
                    .as_ref()
                    .map(|value| self.extend_allowed_with_expr_vars(allowed, value))
                    .unwrap_or_else(|| allowed.clone());
                self.report_unresolved_type(
                    &let_stmt.ty,
                    &let_stmt.name,
                    let_stmt.span,
                    &stmt_allowed,
                    seen,
                );
                if let Some(value) = &let_stmt.value {
                    self.visit_typed_expr(value, &stmt_allowed, seen);
                }
            }
            TypedStmtKind::LetDestructure(let_stmt) => {
                let stmt_allowed = self.extend_allowed_with_expr_vars(allowed, &let_stmt.value);
                self.report_unresolved_type(
                    &let_stmt.tuple_ty,
                    "tuple destructure",
                    let_stmt.span,
                    &stmt_allowed,
                    seen,
                );
                for binding in &let_stmt.bindings {
                    if let TypedDestructureBinding::Name { name, ty } = binding {
                        self.report_unresolved_type(ty, name, let_stmt.span, &stmt_allowed, seen);
                    }
                }
                self.visit_typed_expr(&let_stmt.value, &stmt_allowed, seen);
            }
            TypedStmtKind::Assign(assign) => {
                self.visit_typed_expr(&assign.target, allowed, seen);
                if let Some(value) = &assign.value {
                    self.visit_typed_expr(value, allowed, seen);
                }
            }
            TypedStmtKind::Return(ret) => {
                self.report_unresolved_type(&ret.ty, "return", ret.span, allowed, seen);
                if let Some(value) = &ret.value {
                    self.visit_typed_expr(value, allowed, seen);
                }
            }
            TypedStmtKind::Expr(expr) => self.visit_typed_expr(expr, allowed, seen),
            TypedStmtKind::If(if_stmt) => {
                self.visit_typed_expr(&if_stmt.condition, allowed, seen);
                self.visit_typed_block(&if_stmt.then_branch, allowed, seen);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.visit_typed_else_branch(else_branch, allowed, seen);
                }
            }
            TypedStmtKind::For(for_stmt) => {
                self.report_unresolved_type(
                    &for_stmt.item_type,
                    &for_stmt.name,
                    for_stmt.span,
                    allowed,
                    seen,
                );
                self.visit_typed_expr(&for_stmt.iter, allowed, seen);
                self.visit_typed_block(&for_stmt.body, allowed, seen);
            }
            TypedStmtKind::While(while_stmt) => {
                self.visit_typed_expr(&while_stmt.condition, allowed, seen);
                self.visit_typed_block(&while_stmt.body, allowed, seen);
            }
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => self.visit_typed_expr(expr, allowed, seen),
                TypedSpawnBody::Block(block) => self.visit_typed_block(block, allowed, seen),
            },
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.visit_typed_block(block, allowed, seen),
            TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::IfCompile(_)
            | TypedStmtKind::GcConfig(_) => {}
        }
    }

    fn visit_typed_else_branch(
        &mut self,
        else_branch: &TypedElseBranch,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        match else_branch {
            TypedElseBranch::If(if_stmt) => {
                self.visit_typed_expr(&if_stmt.condition, allowed, seen);
                self.visit_typed_block(&if_stmt.then_branch, allowed, seen);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.visit_typed_else_branch(else_branch, allowed, seen);
                }
            }
            TypedElseBranch::Block(block) => self.visit_typed_block(block, allowed, seen),
        }
    }

    fn visit_typed_expr(
        &mut self,
        expr: &TypedExpr,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        let expr_allowed = self.extend_allowed_with_expr_vars(allowed, expr);
        self.report_unresolved_type(
            &expr.ty,
            &self.expr_label(expr),
            expr.span,
            &expr_allowed,
            seen,
        );
        match &expr.kind {
            TypedExprKind::FStrLit(parts) => {
                for part in parts {
                    if let TypedFStrPart::Interp(expr) = part {
                        self.visit_typed_expr(expr, &expr_allowed, seen);
                    }
                }
            }
            TypedExprKind::Array(items)
            | TypedExprKind::Set(items)
            | TypedExprKind::Tuple(items) => {
                for item in items {
                    self.visit_typed_expr(item, &expr_allowed, seen);
                }
            }
            TypedExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.visit_typed_expr(key, &expr_allowed, seen);
                    self.visit_typed_expr(value, &expr_allowed, seen);
                }
            }
            TypedExprKind::BinOp(lhs, _, rhs)
            | TypedExprKind::Index(lhs, rhs)
            | TypedExprKind::Nullish(lhs, rhs) => {
                self.visit_typed_expr(lhs, &expr_allowed, seen);
                self.visit_typed_expr(rhs, &expr_allowed, seen);
            }
            TypedExprKind::UnOp(_, value)
            | TypedExprKind::Cast(value, _)
            | TypedExprKind::Ok(value)
            | TypedExprKind::Err(value) => self.visit_typed_expr(value, &expr_allowed, seen),
            TypedExprKind::Call(callee, args) => {
                self.visit_typed_expr(callee, &expr_allowed, seen);
                for arg in args {
                    self.visit_typed_expr(arg, &expr_allowed, seen);
                }
            }
            TypedExprKind::MethodCall(target, _, args) => {
                self.visit_typed_expr(target, &expr_allowed, seen);
                for arg in args {
                    self.visit_typed_expr(arg, &expr_allowed, seen);
                }
            }
            TypedExprKind::Field(target, _) => self.visit_typed_expr(target, &expr_allowed, seen),
            TypedExprKind::Lambda(params, body) => {
                for param in params {
                    self.report_unresolved_type(
                        &param.ty,
                        &param.name,
                        param.span,
                        &expr_allowed,
                        seen,
                    );
                }
                self.visit_typed_expr(body, &expr_allowed, seen);
            }
            TypedExprKind::Match(subject, arms) => {
                self.visit_typed_expr(subject, &expr_allowed, seen);
                for arm in arms {
                    self.visit_typed_expr(&arm.pattern, &expr_allowed, seen);
                    match &arm.body {
                        TypedMatchArmBody::Expr(expr) => {
                            self.visit_typed_expr(expr, &expr_allowed, seen)
                        }
                        TypedMatchArmBody::Block(block) => {
                            self.visit_typed_block(block, &expr_allowed, seen)
                        }
                    }
                }
            }
            TypedExprKind::Chan(ty) => {
                self.report_unresolved_type(ty, "channel", expr.span, &expr_allowed, seen);
            }
            TypedExprKind::IntLit(_)
            | TypedExprKind::FloatLit(_)
            | TypedExprKind::StrLit(_)
            | TypedExprKind::BoolLit(_)
            | TypedExprKind::NoneLit
            | TypedExprKind::Ident(_) => {}
        }
    }

    fn report_unresolved_type(
        &mut self,
        ty: &Type,
        name: &str,
        span: draton_ast::Span,
        allowed: &BTreeSet<u32>,
        seen: &mut BTreeSet<u32>,
    ) {
        let resolved = self.apply_subst(ty.clone());
        for var in free_type_vars(&resolved) {
            if allowed.contains(&var) || !seen.insert(var) {
                continue;
            }
            self.errors.push(TypeError::CannotInfer {
                name: name.to_string(),
                line: span.line,
                col: span.col,
            });
        }
    }

    fn expr_label(&self, expr: &TypedExpr) -> String {
        match &expr.kind {
            TypedExprKind::Ident(name) => name.clone(),
            TypedExprKind::Field(_, field) => field.clone(),
            TypedExprKind::Lambda(_, _) => "lambda".to_string(),
            TypedExprKind::Array(_) => "array literal".to_string(),
            TypedExprKind::Map(_) => "map literal".to_string(),
            TypedExprKind::Set(_) => "set literal".to_string(),
            TypedExprKind::Tuple(_) => "tuple literal".to_string(),
            _ => "expression".to_string(),
        }
    }

    fn extend_allowed_with_expr_vars(
        &self,
        allowed: &BTreeSet<u32>,
        expr: &TypedExpr,
    ) -> BTreeSet<u32> {
        let mut vars = allowed.clone();
        vars.extend(self.allowed_expr_vars(expr));
        vars
    }

    fn allowed_expr_vars(&self, expr: &TypedExpr) -> BTreeSet<u32> {
        match &expr.kind {
            TypedExprKind::NoneLit
            | TypedExprKind::Ident(_)
            | TypedExprKind::Ok(_)
            | TypedExprKind::Err(_)
            | TypedExprKind::Lambda(_, _) => free_type_vars(&expr.ty),
            TypedExprKind::Tuple(items) if !items.is_empty() => free_type_vars(&expr.ty),
            _ => BTreeSet::new(),
        }
    }

    fn push_mismatch(
        &mut self,
        expected: &str,
        found_expected: &Type,
        found: &Type,
        span: draton_ast::Span,
    ) {
        self.errors.push(TypeError::Mismatch {
            expected: expected.to_string(),
            found: format!("{found_expected} vs {found}"),
            hint: "make the operand and annotation types agree".to_string(),
            line: span.line,
            col: span.col,
        });
    }

    fn typed_expr(
        &mut self,
        kind: TypedExprKind,
        ty: Type,
        span: draton_ast::Span,
    ) -> (TypedExpr, Type) {
        let ty = self.apply_subst(ty);
        (
            TypedExpr {
                kind,
                ty: ty.clone(),
                span,
            },
            ty,
        )
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_var(ty: &Type) -> u32 {
    match ty {
        Type::Var(id) => *id,
        _ => 0,
    }
}
