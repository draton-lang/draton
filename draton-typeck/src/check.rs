use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use draton_ast::{
    AssignOp, BinOp, Block, ClassDef, ClassMember, ElseBranch, Expr, FStrPart, FieldDef, FnDef,
    GcConfigStmt, IfStmt, Item, MatchArm, MatchArmBody, Program, SpawnBody, Stmt, TypeExpr,
    TypeMember, UnOp,
};

use crate::env::{Scheme, TypeEnv};
use crate::error::TypeError;
use crate::infer::{free_type_vars, free_type_vars_in_env};
use crate::typed_ast::{
    Type, TypedAssignStmt, TypedBlock, TypedClassDef, TypedConstDef, TypedElseBranch, TypedEnumDef,
    TypedErrorDef, TypedExpr, TypedExprKind, TypedExternBlock, TypedFStrPart, TypedFieldDef,
    TypedFnDef, TypedForStmt, TypedGcConfigEntry, TypedGcConfigStmt, TypedIfCompileStmt,
    TypedIfStmt, TypedImportDef, TypedImportItem, TypedInterfaceDef, TypedItem, TypedLetStmt,
    TypedMatchArm, TypedMatchArmBody, TypedParam, TypedProgram, TypedReturnStmt, TypedSpawnBody,
    TypedSpawnStmt, TypedStmt, TypedStmtKind, TypedTypeBlock, TypedTypeMember, TypedWhileStmt,
};
use crate::unify::occurs;

/// The result of type checking a program.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeCheckResult {
    pub typed_program: TypedProgram,
    pub errors: Vec<TypeError>,
}

/// The Draton type checker.
pub struct TypeChecker {
    env: TypeEnv,
    errors: Vec<TypeError>,
    next_var: u32,
    fresh_counter: Rc<Cell<u32>>,
    subst: HashMap<u32, Type>,
    row_fields: HashMap<u32, HashMap<String, Type>>,
    class_fields: HashMap<String, HashMap<String, Type>>,
    class_methods: HashMap<String, HashMap<String, Scheme>>,
    function_hints: HashMap<String, FnDef>,
    binding_hints: HashMap<String, TypeExpr>,
    current_return: Vec<Type>,
    current_effect: Vec<Option<Type>>,
    current_class: Vec<String>,
}

impl TypeChecker {
    /// Creates a new type checker with the default built-in environment.
    pub fn new() -> Self {
        let fresh_counter = Rc::new(Cell::new(0));
        let mut checker = Self {
            env: TypeEnv::with_counter(Rc::clone(&fresh_counter)),
            errors: Vec::new(),
            next_var: 0,
            fresh_counter,
            subst: HashMap::new(),
            row_fields: HashMap::new(),
            class_fields: HashMap::new(),
            class_methods: HashMap::new(),
            function_hints: HashMap::new(),
            binding_hints: HashMap::new(),
            current_return: Vec::new(),
            current_effect: Vec::new(),
            current_class: Vec::new(),
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

        TypeCheckResult {
            typed_program,
            errors: self.errors,
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
                ClassMember::Method(_) => None,
            })
            .collect::<Vec<_>>();

        let methods = class_def
            .members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(_) => None,
                ClassMember::Method(method) => {
                    Some(self.infer_fn_item(method, Some(&class_def.name)))
                }
            })
            .collect::<Vec<_>>();

        self.env.pop_scope();
        let _ = self.current_class.pop();
        TypedClassDef {
            name: class_def.name.clone(),
            extends: class_def.extends.clone(),
            implements: class_def.implements.clone(),
            fields,
            methods,
            span: class_def.span,
        }
    }

    fn infer_class_field(&mut self, class_def: &ClassDef, field: &FieldDef) -> TypedFieldDef {
        let ty = self
            .class_fields
            .get(&class_def.name)
            .and_then(|fields| fields.get(&field.name))
            .cloned()
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
                .map(|method| self.infer_fn_item(method, None))
                .collect(),
            span: interface_def.span,
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

        if scheme.quantified.is_empty()
            && !free_type_vars(&self.apply_subst(full_ty.clone())).is_empty()
        {
            self.errors.push(TypeError::CannotInfer {
                name: function.name.clone(),
                line: function.span.line,
                col: function.span.col,
            });
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
        for stmt in &block.stmts {
            let typed_stmt = self.infer_stmt(stmt);
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
                let (typed_value, mut ty) = if let Some(expr) = &let_stmt.value {
                    let (typed, value_ty) = self.infer_expr(expr);
                    (Some(typed), value_ty)
                } else {
                    (None, self.fresh_var())
                };
                if let Some(type_hint) = &let_stmt.type_hint {
                    let hinted = self.type_from_annotation(type_hint);
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
                let (typed_value, ty) = if let Some(expr) = &return_stmt.value {
                    let (typed, ty) = self.infer_expr(expr);
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
            Expr::Lambda(params, body, span) => self.infer_lambda(params, body, *span),
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
        let typed_args = args
            .iter()
            .map(|arg| self.infer_expr(arg))
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
        let typed_args = args
            .iter()
            .map(|arg| self.infer_expr(arg))
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
        let (typed_target, target_ty) = self.infer_expr(target);
        let field_ty = self.require_field(target_ty, field, span);
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
    ) -> (TypedExpr, Type) {
        self.env.push_scope();
        let typed_params = params
            .iter()
            .map(|name| {
                let ty = self.fresh_var();
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
        let (typed_body, body_ty) = self.infer_expr(body);
        self.env.pop_scope();
        let fn_ty = Type::Fn(
            typed_params.iter().map(|param| param.ty.clone()).collect(),
            Box::new(body_ty.clone()),
        );
        self.typed_expr(
            TypedExprKind::Lambda(typed_params, Box::new(typed_body)),
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
        let typed_arms = arms
            .iter()
            .map(|arm| {
                self.env.push_scope();
                let (typed_pattern, pattern_ty) = self.infer_expr(&arm.pattern);
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
        self.typed_expr(
            TypedExprKind::Match(Box::new(typed_subject), typed_arms),
            self.apply_subst(result_ty.clone()),
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
            Some(existing) => Some(self.unify(
                existing,
                err_ty,
                draton_ast::Span {
                    start: 0,
                    end: 0,
                    line: 0,
                    col: 0,
                },
            )),
            None => Some(err_ty),
        };
        if let Some(slot) = self.current_effect.last_mut() {
            *slot = merged;
        }
    }

    fn require_field(&mut self, object_ty: Type, field: &str, span: draton_ast::Span) -> Type {
        match self.apply_subst(object_ty.clone()) {
            Type::Named(name, _) => {
                if let Some(fields) = self.class_fields.get(&name) {
                    if let Some(field_ty) = fields.get(field) {
                        return field_ty.clone();
                    }
                }
                self.errors.push(TypeError::NoField {
                    field: field.to_string(),
                    ty: name,
                    line: span.line,
                    col: span.col,
                });
                self.fresh_var()
            }
            Type::Var(var) => {
                let fresh = self.fresh_var();
                let field_ty = self
                    .row_fields
                    .entry(var)
                    .or_default()
                    .entry(field.to_string())
                    .or_insert(fresh)
                    .clone();
                field_ty
            }
            other => {
                self.errors.push(TypeError::NoField {
                    field: field.to_string(),
                    ty: other.to_string(),
                    line: span.line,
                    col: span.col,
                });
                self.fresh_var()
            }
        }
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
                .class_methods
                .get(&class_name)
                .and_then(|methods| methods.get(name).cloned())
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
        for item in &program.items {
            match item {
                Item::Class(class_def) => {
                    self.predeclare_class(class_def);
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
                ClassMember::Method(_) => None,
            })
            .collect::<HashMap<_, _>>();
        self.class_fields.insert(class_def.name.clone(), fields);

        let mut methods = HashMap::new();
        for member in &class_def.members {
            if let ClassMember::Method(method) = member {
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
        self.class_methods.insert(class_def.name.clone(), methods);
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
        let row_vars = self.row_fields.keys().copied().collect::<BTreeSet<_>>();
        let quantified = free_type_vars(&ty)
            .into_iter()
            .filter(|var| !env_vars.contains(var) && !row_vars.contains(var))
            .collect::<Vec<_>>();
        Scheme { quantified, ty }
    }

    fn apply_subst(&self, ty: Type) -> Type {
        match ty {
            Type::Var(id) => self
                .subst
                .get(&id)
                .cloned()
                .map(|inner| self.apply_subst(inner))
                .unwrap_or(Type::Var(id)),
            Type::Array(inner) => Type::Array(Box::new(self.apply_subst(*inner))),
            Type::Map(key, value) => Type::Map(
                Box::new(self.apply_subst(*key)),
                Box::new(self.apply_subst(*value)),
            ),
            Type::Set(inner) => Type::Set(Box::new(self.apply_subst(*inner))),
            Type::Tuple(items) => Type::Tuple(
                items
                    .into_iter()
                    .map(|item| self.apply_subst(item))
                    .collect(),
            ),
            Type::Option(inner) => Type::Option(Box::new(self.apply_subst(*inner))),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.apply_subst(*ok)),
                Box::new(self.apply_subst(*err)),
            ),
            Type::Chan(inner) => Type::Chan(Box::new(self.apply_subst(*inner))),
            Type::Fn(params, ret) => Type::Fn(
                params
                    .into_iter()
                    .map(|param| self.apply_subst(param))
                    .collect(),
                Box::new(self.apply_subst(*ret)),
            ),
            Type::Named(name, args) => Type::Named(
                name,
                args.into_iter().map(|arg| self.apply_subst(arg)).collect(),
            ),
            Type::Pointer(inner) => Type::Pointer(Box::new(self.apply_subst(*inner))),
            other => other,
        }
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

        if let Some(fields) = self.row_fields.get(&var).cloned() {
            match self.apply_subst(ty.clone()) {
                Type::Named(name, _) => {
                    if let Some(class_fields) = self.class_fields.get(&name).cloned() {
                        for (field_name, field_ty) in fields {
                            if let Some(actual_ty) = class_fields.get(&field_name) {
                                let _ = self.unify(field_ty, actual_ty.clone(), span);
                            } else {
                                self.errors.push(TypeError::NoField {
                                    field: field_name,
                                    ty: name.clone(),
                                    line: span.line,
                                    col: span.col,
                                });
                            }
                        }
                    }
                }
                Type::Var(other) => {
                    let entry = self.row_fields.entry(other).or_default();
                    for (field_name, field_ty) in fields {
                        entry.entry(field_name).or_insert(field_ty);
                    }
                }
                other => {
                    self.errors.push(TypeError::Mismatch {
                        expected: "type with required fields".to_string(),
                        found: other.to_string(),
                        hint: "add a concrete class type or field annotation".to_string(),
                        line: span.line,
                        col: span.col,
                    });
                }
            }
        }

        self.subst.insert(var, ty.clone());
        ty
    }

    fn type_from_annotation(&mut self, type_expr: &TypeExpr) -> Type {
        match type_expr {
            TypeExpr::Named(name, _) => self.named_type(name),
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
            | Type::Float64
            | Type::Var(_) => resolved,
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
