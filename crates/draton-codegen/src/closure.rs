use std::collections::{HashMap, HashSet};

use draton_ast::Span;
use draton_typeck::{
    typed_ast::{TypedElseBranch, TypedGcConfigEntry, TypedSpawnBody},
    Type, TypedBlock, TypedDestructureBinding, TypedExpr, TypedExprKind, TypedFStrPart,
    TypedMatchArmBody, TypedParam, TypedStmt, TypedStmtKind,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;

/// A captured variable lowered into a closure environment.
#[derive(Debug, Clone)]
pub struct CapturedVar<'ctx> {
    pub name: String,
    pub storage: PointerValue<'ctx>,
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn ensure_closure_runtime_metadata(&mut self) -> Result<(), CodeGenError> {
        if self.closure_type_descriptor_id != 0 {
            return Ok(());
        }
        let descriptor_name = "TypeDesc_draton_closure";
        if self.module.get_global(descriptor_name).is_none() {
            let i32_type = self.context.i32_type();
            let offsets_ty = i32_type.array_type(1);
            let descriptor_ty = self.context.struct_type(
                &[i32_type.into(), i32_type.into(), offsets_ty.into()],
                false,
            );
            let descriptor = descriptor_ty.const_named_struct(&[
                i32_type.const_int(16, false).into(),
                i32_type.const_int(1, false).into(),
                i32_type.const_array(&[i32_type.const_int(8, false)]).into(),
            ]);
            let global = self.module.add_global(descriptor_ty, None, descriptor_name);
            global.set_initializer(&descriptor);
            global.set_constant(true);
        }
        let type_id = self.next_type_descriptor_id;
        self.next_type_descriptor_id = self.next_type_descriptor_id.saturating_add(1);
        self.closure_type_descriptor_id = type_id;
        Ok(())
    }

    pub(crate) fn next_closure_id(&mut self) -> usize {
        let id = self.closure_counter;
        self.closure_counter = self.closure_counter.saturating_add(1);
        id
    }

    pub(crate) fn emit_lambda(
        &mut self,
        params: &[TypedParam],
        body: &TypedExpr,
        span: Span,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        self.ensure_closure_runtime_metadata()?;
        let locals = self.all_locals();
        let captures = collect_captures(body, params, &locals);
        let closure_id = self.next_closure_id();

        let env_type = if captures.is_empty() {
            None
        } else {
            let env_name = format!("closure_env_{closure_id}");
            let env_type = self.context.opaque_struct_type(&env_name);
            let field_types = captures
                .iter()
                .map(|capture| {
                    self.pointer_pointee(capture.storage).map_err(|_| {
                        CodeGenError::UnsupportedExpr(format!(
                            "capture {} has unknown storage type",
                            capture.name
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            env_type.set_body(&field_types, false);
            Some(env_type)
        };

        let env_ptr = if let Some(env_type) = env_type {
            let env_ptr = self.emit_closure_env(env_type, &captures)?;
            Some(env_ptr)
        } else {
            None
        };

        let closure_fn = self.emit_closure_body(closure_id, params, body, &captures, env_type)?;
        self.emit_closure_record(closure_fn, env_ptr, span)
            .map(Into::into)
    }

    pub(crate) fn emit_closure_call(
        &mut self,
        closure_ptr: PointerValue<'ctx>,
        args: &[TypedExpr],
        params: &[Type],
        ret_ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let closure = self
            .build_load(closure_ptr, "closure.value")?
            .into_struct_value();
        let fn_ptr_raw = self
            .builder
            .build_extract_value(closure, 0, "closure.fn")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let env_ptr = self
            .builder
            .build_extract_value(closure, 1, "closure.env")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let fn_ty = self.llvm_closure_body_function_type(params, ret_ty)?;
        let fn_ptr = self
            .builder
            .build_bit_cast(
                fn_ptr_raw,
                self.context.ptr_type(AddressSpace::default()),
                "closure.fn.typed",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();

        let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![env_ptr.into()];
        for (index, arg) in args.iter().enumerate() {
            let expected_ty = params.get(index).unwrap_or(&arg.ty);
            let value = self.emit_expr_as_type(arg, expected_ty)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("closure arg missing value".to_string())
            })?;
            call_args.push(value.into());
        }

        let call = self
            .builder
            .build_indirect_call(fn_ty, fn_ptr, &call_args, "closure.call")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let ret_val = call.try_as_basic_value().basic();
        Ok(ret_val)
    }

    pub(crate) fn emit_named_function_closure(
        &mut self,
        symbol: &str,
        ty: &Type,
        span: Span,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let function = self
            .functions
            .get(symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.to_string()))?;
        let fn_ptr = self
            .builder
            .build_bit_cast(
                function.as_global_value().as_pointer_value(),
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "closure.fn.raw",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        self.emit_closure_record_with_raw_fn(fn_ptr, None, ty, span)
            .map(Into::into)
    }

    pub(crate) fn llvm_closure_body_function_type(
        &self,
        params: &[Type],
        ret: &Type,
    ) -> Result<inkwell::types::FunctionType<'ctx>, CodeGenError> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let mut all_params: Vec<BasicMetadataTypeEnum<'ctx>> = vec![i8_ptr.into()];
        all_params.extend(
            params
                .iter()
                .map(|param| self.llvm_payload_type(param))
                .collect::<Result<Vec<_>, _>>()?,
        );
        if Self::is_void_type(ret) {
            Ok(self.context.void_type().fn_type(&all_params, false))
        } else {
            Ok(self.llvm_basic_type(ret)?.fn_type(&all_params, false))
        }
    }

    fn emit_closure_env(
        &mut self,
        env_type: StructType<'ctx>,
        captures: &[CapturedVar<'ctx>],
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let alloc = self
            .module
            .get_function("malloc")
            .ok_or_else(|| CodeGenError::MissingSymbol("malloc".to_string()))?;
        let env_size = env_type
            .size_of()
            .and_then(|size| size.get_zero_extended_constant())
            .unwrap_or_else(|| {
                env_type
                    .get_field_types()
                    .into_iter()
                    .map(|field| {
                        field
                            .size_of()
                            .and_then(|size| size.get_zero_extended_constant())
                            .unwrap_or(8)
                    })
                    .sum::<u64>()
            })
            .max(1);
        let raw = self
            .builder
            .build_call(
                alloc,
                &[self.context.i64_type().const_int(env_size, false).into()],
                "closure.env.raw",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm("malloc returned void".to_string()))?
            .into_pointer_value();
        let env_ptr =
            self.build_bit_cast_to(raw, BasicTypeEnum::StructType(env_type), "closure.env")?;
        for (index, capture) in captures.iter().enumerate() {
            let field_ptr = self.build_struct_gep(
                env_type,
                env_ptr,
                index as u32,
                &format!("env.{}", capture.name),
            )?;
            let captured_value =
                self.build_load(capture.storage, &format!("capture.{}", capture.name))?;
            self.build_store(field_ptr, captured_value)?;
        }
        Ok(env_ptr)
    }

    fn emit_closure_body(
        &mut self,
        id: usize,
        params: &[TypedParam],
        body: &TypedExpr,
        captures: &[CapturedVar<'ctx>],
        env_type: Option<StructType<'ctx>>,
    ) -> Result<FunctionValue<'ctx>, CodeGenError> {
        let fn_name = format!("closure_body_{id}");
        let fn_ty = self.llvm_closure_body_function_type(
            &params
                .iter()
                .map(|param| param.ty.clone())
                .collect::<Vec<_>>(),
            &body.ty,
        )?;
        let function = self.module.add_function(&fn_name, fn_ty, None);

        let prev_function = self.current_function;
        let prev_return_type = self.current_return_type.clone();
        let prev_block = self.builder.get_insert_block();
        let prev_variables = self.variables.clone();

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.current_function = Some(function);
        self.current_return_type = Some(body.ty.clone());
        self.variables = vec![HashMap::new()];

        if let Some(env_type) = env_type {
            let env_raw = function
                .get_first_param()
                .ok_or_else(|| CodeGenError::MissingSymbol(format!("{fn_name}:env")))?;
            let env_ptr = self.build_bit_cast_to(
                env_raw.into_pointer_value(),
                BasicTypeEnum::StructType(env_type),
                "closure.env.typed",
            )?;
            for (index, capture) in captures.iter().enumerate() {
                let field_ptr = self.build_struct_gep(
                    env_type,
                    env_ptr,
                    index as u32,
                    &format!("env.load.{}", capture.name),
                )?;
                let captured_value =
                    self.build_load(field_ptr, &format!("capture.{}", capture.name))?;
                let storage = self.create_entry_alloca(
                    function,
                    captured_value.get_type(),
                    &format!("capture.{}", capture.name),
                )?;
                self.build_store(storage, captured_value)?;
                self.define_local(&capture.name, storage);
            }
        }

        for (param, value) in params.iter().zip(function.get_params().iter().skip(1)) {
            let storage =
                self.create_entry_alloca(function, self.llvm_basic_type(&param.ty)?, &param.name)?;
            self.build_store(storage, *value)?;
            self.register_gc_root(storage, &param.ty)?;
            self.define_local(&param.name, storage);
        }

        let result = self.emit_expr(body)?;
        if !self.current_block_terminated() {
            if let Some(value) = result {
                self.builder
                    .build_return(Some(&value))
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            } else {
                self.builder
                    .build_return(None)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            }
        }

        self.current_function = prev_function;
        self.current_return_type = prev_return_type;
        self.variables = prev_variables;
        if let Some(block) = prev_block {
            self.builder.position_at_end(block);
        }

        Ok(function)
    }

    fn emit_closure_record(
        &mut self,
        function: FunctionValue<'ctx>,
        env_ptr: Option<PointerValue<'ctx>>,
        span: Span,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let fn_ptr = self
            .builder
            .build_bit_cast(
                function.as_global_value().as_pointer_value(),
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "closure.fn.raw",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        self.emit_closure_record_with_raw_fn(fn_ptr, env_ptr, &Type::Unit, span)
    }

    fn emit_closure_record_with_raw_fn(
        &mut self,
        fn_ptr: PointerValue<'ctx>,
        env_ptr: Option<PointerValue<'ctx>>,
        _ty: &Type,
        _span: Span,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let alloc = self
            .module
            .get_function("malloc")
            .ok_or_else(|| CodeGenError::MissingSymbol("malloc".to_string()))?;
        let size = self
            .closure_record_type
            .size_of()
            .and_then(|value| value.get_zero_extended_constant())
            .unwrap_or(16);
        let raw = self
            .builder
            .build_call(
                alloc,
                &[self.context.i64_type().const_int(size, false).into()],
                "closure.raw",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm("malloc returned void".to_string()))?
            .into_pointer_value();
        let closure_ptr =
            self.build_bit_cast_to(raw, self.closure_record_type.into(), "closure.ptr")?;
        let fn_slot =
            self.build_struct_gep(self.closure_record_type, closure_ptr, 0, "closure.fn.slot")?;
        self.build_store(fn_slot, fn_ptr.into())?;
        let env_slot =
            self.build_struct_gep(self.closure_record_type, closure_ptr, 1, "closure.env.slot")?;
        let erased_env = env_ptr
            .map(|env_ptr| {
                self.builder
                    .build_bit_cast(
                        env_ptr,
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        "closure.env.raw",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))
                    .map(|value| value.into_pointer_value())
            })
            .transpose()?
            .unwrap_or_else(|| {
                self.context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .const_null()
            });
        self.build_store(env_slot, erased_env.into())?;
        Ok(closure_ptr)
    }
}

pub fn collect_captures<'ctx>(
    body: &TypedExpr,
    params: &[TypedParam],
    outer_locals: &HashMap<String, PointerValue<'ctx>>,
) -> Vec<CapturedVar<'ctx>> {
    let mut param_names = HashSet::new();
    for param in params {
        let _ = param_names.insert(param.name.clone());
    }
    let mut captures = Vec::new();
    let mut seen = HashSet::new();
    collect_captures_expr(body, &param_names, outer_locals, &mut captures, &mut seen);
    captures
}

fn collect_captures_expr<'ctx>(
    expr: &TypedExpr,
    params: &HashSet<String>,
    outer_locals: &HashMap<String, PointerValue<'ctx>>,
    captures: &mut Vec<CapturedVar<'ctx>>,
    seen: &mut HashSet<String>,
) {
    match &expr.kind {
        TypedExprKind::Ident(name) => {
            if !params.contains(name) {
                if let Some(storage) = outer_locals.get(name).copied() {
                    if seen.insert(name.clone()) {
                        captures.push(CapturedVar {
                            name: name.clone(),
                            storage,
                        });
                    }
                }
            }
        }
        TypedExprKind::Array(items) | TypedExprKind::Set(items) | TypedExprKind::Tuple(items) => {
            for item in items {
                collect_captures_expr(item, params, outer_locals, captures, seen);
            }
        }
        TypedExprKind::Map(entries) => {
            for (key, value) in entries {
                collect_captures_expr(key, params, outer_locals, captures, seen);
                collect_captures_expr(value, params, outer_locals, captures, seen);
            }
        }
        TypedExprKind::FStrLit(parts) => {
            for part in parts {
                if let TypedFStrPart::Interp(expr) = part {
                    collect_captures_expr(expr, params, outer_locals, captures, seen);
                }
            }
        }
        TypedExprKind::BinOp(lhs, _, rhs)
        | TypedExprKind::Nullish(lhs, rhs)
        | TypedExprKind::Index(lhs, rhs) => {
            collect_captures_expr(lhs, params, outer_locals, captures, seen);
            collect_captures_expr(rhs, params, outer_locals, captures, seen);
        }
        TypedExprKind::UnOp(_, expr)
        | TypedExprKind::Field(expr, _)
        | TypedExprKind::Cast(expr, _)
        | TypedExprKind::Ok(expr)
        | TypedExprKind::Err(expr) => {
            collect_captures_expr(expr, params, outer_locals, captures, seen);
        }
        TypedExprKind::Call(callee, args) => {
            collect_captures_expr(callee, params, outer_locals, captures, seen);
            for arg in args {
                collect_captures_expr(arg, params, outer_locals, captures, seen);
            }
        }
        TypedExprKind::MethodCall(target, _, args) => {
            collect_captures_expr(target, params, outer_locals, captures, seen);
            for arg in args {
                collect_captures_expr(arg, params, outer_locals, captures, seen);
            }
        }
        TypedExprKind::Lambda(inner_params, inner_body) => {
            let mut inner_param_names = params.clone();
            for param in inner_params {
                let _ = inner_param_names.insert(param.name.clone());
            }
            collect_captures_expr(inner_body, &inner_param_names, outer_locals, captures, seen);
        }
        TypedExprKind::Match(subject, arms) => {
            collect_captures_expr(subject, params, outer_locals, captures, seen);
            for arm in arms {
                collect_captures_expr(&arm.pattern, params, outer_locals, captures, seen);
                match &arm.body {
                    TypedMatchArmBody::Expr(expr) => {
                        collect_captures_expr(expr, params, outer_locals, captures, seen);
                    }
                    TypedMatchArmBody::Block(block) => {
                        collect_captures_block(block, params, outer_locals, captures, seen);
                    }
                }
            }
        }
        TypedExprKind::Chan(_)
        | TypedExprKind::IntLit(_)
        | TypedExprKind::FloatLit(_)
        | TypedExprKind::StrLit(_)
        | TypedExprKind::BoolLit(_)
        | TypedExprKind::NoneLit => {}
    }
}

fn collect_captures_block<'ctx>(
    block: &TypedBlock,
    params: &HashSet<String>,
    outer_locals: &HashMap<String, PointerValue<'ctx>>,
    captures: &mut Vec<CapturedVar<'ctx>>,
    seen: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        collect_captures_stmt(stmt, params, outer_locals, captures, seen);
    }
}

fn collect_captures_stmt<'ctx>(
    stmt: &TypedStmt,
    params: &HashSet<String>,
    outer_locals: &HashMap<String, PointerValue<'ctx>>,
    captures: &mut Vec<CapturedVar<'ctx>>,
    seen: &mut HashSet<String>,
) {
    match &stmt.kind {
        TypedStmtKind::Let(let_stmt) => {
            if let Some(value) = &let_stmt.value {
                collect_captures_expr(value, params, outer_locals, captures, seen);
            }
        }
        TypedStmtKind::LetDestructure(let_stmt) => {
            for binding in &let_stmt.bindings {
                if let TypedDestructureBinding::Name { .. } = binding {}
            }
            collect_captures_expr(&let_stmt.value, params, outer_locals, captures, seen);
        }
        TypedStmtKind::Assign(assign) => {
            collect_captures_expr(&assign.target, params, outer_locals, captures, seen);
            if let Some(value) = &assign.value {
                collect_captures_expr(value, params, outer_locals, captures, seen);
            }
        }
        TypedStmtKind::Return(ret) => {
            if let Some(value) = &ret.value {
                collect_captures_expr(value, params, outer_locals, captures, seen);
            }
        }
        TypedStmtKind::Expr(expr) => {
            collect_captures_expr(expr, params, outer_locals, captures, seen)
        }
        TypedStmtKind::If(if_stmt) => {
            collect_captures_expr(&if_stmt.condition, params, outer_locals, captures, seen);
            collect_captures_block(&if_stmt.then_branch, params, outer_locals, captures, seen);
            if let Some(else_branch) = &if_stmt.else_branch {
                match else_branch {
                    TypedElseBranch::If(child) => {
                        collect_captures_expr(
                            &child.condition,
                            params,
                            outer_locals,
                            captures,
                            seen,
                        );
                        collect_captures_block(
                            &child.then_branch,
                            params,
                            outer_locals,
                            captures,
                            seen,
                        );
                        if let Some(grand_else) = &child.else_branch {
                            match grand_else {
                                TypedElseBranch::If(grand_child) => {
                                    collect_captures_expr(
                                        &grand_child.condition,
                                        params,
                                        outer_locals,
                                        captures,
                                        seen,
                                    );
                                    collect_captures_block(
                                        &grand_child.then_branch,
                                        params,
                                        outer_locals,
                                        captures,
                                        seen,
                                    );
                                }
                                TypedElseBranch::Block(block) => {
                                    collect_captures_block(
                                        block,
                                        params,
                                        outer_locals,
                                        captures,
                                        seen,
                                    );
                                }
                            }
                        }
                    }
                    TypedElseBranch::Block(block) => {
                        collect_captures_block(block, params, outer_locals, captures, seen);
                    }
                }
            }
        }
        TypedStmtKind::For(for_stmt) => {
            collect_captures_expr(&for_stmt.iter, params, outer_locals, captures, seen);
            collect_captures_block(&for_stmt.body, params, outer_locals, captures, seen);
        }
        TypedStmtKind::While(while_stmt) => {
            collect_captures_expr(&while_stmt.condition, params, outer_locals, captures, seen);
            collect_captures_block(&while_stmt.body, params, outer_locals, captures, seen);
        }
        TypedStmtKind::Spawn(spawn) => match &spawn.body {
            TypedSpawnBody::Expr(expr) => {
                collect_captures_expr(expr, params, outer_locals, captures, seen);
            }
            TypedSpawnBody::Block(block) => {
                collect_captures_block(block, params, outer_locals, captures, seen);
            }
        },
        TypedStmtKind::Block(block)
        | TypedStmtKind::UnsafeBlock(block)
        | TypedStmtKind::PointerBlock(block)
        | TypedStmtKind::ComptimeBlock(block) => {
            collect_captures_block(block, params, outer_locals, captures, seen);
        }
        TypedStmtKind::IfCompile(if_compile) => {
            collect_captures_expr(&if_compile.condition, params, outer_locals, captures, seen);
            collect_captures_block(&if_compile.body, params, outer_locals, captures, seen);
        }
        TypedStmtKind::GcConfig(gc) => {
            for TypedGcConfigEntry { value, .. } in &gc.entries {
                collect_captures_expr(value, params, outer_locals, captures, seen);
            }
        }
        TypedStmtKind::AsmBlock(_) | TypedStmtKind::TypeBlock(_) => {}
    }
}
