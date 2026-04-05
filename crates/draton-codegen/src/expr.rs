use draton_ast::{BinOp, UnOp};
use draton_typeck::{Type, TypedExpr, TypedExprKind, TypedFStrPart, TypedMatchArmBody};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, PointerValue};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

use crate::codegen::CodeGen;
use crate::error::CodeGenError;
use crate::mangle::mangle_class;

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn emit_const_expr(
        &mut self,
        expr: &TypedExpr,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        self.emit_expr(expr)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("constant produced no value".to_string()))
    }

    pub(crate) fn emit_expr(
        &mut self,
        expr: &TypedExpr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        match &expr.kind {
            TypedExprKind::IntLit(value) => Ok(Some(
                self.context
                    .i64_type()
                    .const_int(*value as u64, true)
                    .into(),
            )),
            TypedExprKind::FloatLit(value) => {
                Ok(Some(self.context.f64_type().const_float(*value).into()))
            }
            TypedExprKind::BoolLit(value) => Ok(Some(
                self.context
                    .bool_type()
                    .const_int(u64::from(*value), false)
                    .into(),
            )),
            TypedExprKind::StrLit(text) => self.emit_string_literal(text).map(Some),
            TypedExprKind::FStrLit(parts) => self.emit_fstring(parts).map(Some),
            TypedExprKind::NoneLit => {
                if let Type::Option(inner) = &expr.ty {
                    let option_ty = self.llvm_basic_type(&expr.ty)?.into_struct_type();
                    let tag = self.context.bool_type().const_zero().into();
                    let payload = self.zero_value(inner)?;
                    Ok(Some(
                        self.build_struct_value(option_ty, &[tag, payload], "none")?
                            .into(),
                    ))
                } else {
                    Ok(None)
                }
            }
            TypedExprKind::Ident(name) => {
                if let Some(ptr) = self.lookup_local(name) {
                    self.build_typed_load(ptr, self.llvm_basic_type(&expr.ty)?, name)
                        .map(Some)
                } else if matches!(expr.ty, Type::Fn(_, _)) {
                    let symbol = self.resolve_function_value_symbol(name, &expr.ty)?;
                    self.emit_named_function_closure(&symbol, &expr.ty, expr.span)
                        .map(Some)
                } else if let Some(global) = self.module.get_global(name) {
                    self.build_typed_load(
                        global.as_pointer_value(),
                        self.llvm_basic_type(&expr.ty)?,
                        name,
                    )
                    .map(Some)
                } else {
                    Err(CodeGenError::MissingSymbol(name.clone()))
                }
            }
            TypedExprKind::Array(items) => self.emit_array(items, &expr.ty),
            TypedExprKind::Tuple(items) => self.emit_tuple(items, &expr.ty),
            TypedExprKind::BinOp(lhs, op, rhs) => self.emit_binop(lhs, *op, rhs, &expr.ty),
            TypedExprKind::UnOp(op, value) => self.emit_unop(*op, value, &expr.ty),
            TypedExprKind::Call(callee, args) => self.emit_call(callee, args, &expr.ty),
            TypedExprKind::MethodCall(target, method, args) => {
                self.emit_method_call(target, method, args)
            }
            TypedExprKind::Field(target, field) => {
                self.emit_field_load(target, field, &expr.ty).map(Some)
            }
            TypedExprKind::Index(target, index) => {
                self.emit_index(target, index, &expr.ty).map(Some)
            }
            TypedExprKind::Cast(value, to_ty) => self.emit_cast(value, to_ty).map(Some),
            TypedExprKind::Match(subject, arms) => self.emit_match(subject, arms, &expr.ty),
            TypedExprKind::Ok(value) => self.emit_result(value, &expr.ty, true),
            TypedExprKind::Err(value) => self.emit_result(value, &expr.ty, false),
            TypedExprKind::Nullish(lhs, rhs) => self.emit_nullish(lhs, rhs, &expr.ty),
            TypedExprKind::Lambda(params, body) => {
                self.emit_lambda(params, body, expr.span).map(Some)
            }
            TypedExprKind::Map(_) | TypedExprKind::Set(_) | TypedExprKind::Chan(_) => {
                Err(CodeGenError::UnsupportedExpr(format!("{:?}", expr.kind)))
            }
        }
    }

    pub(crate) fn emit_lvalue_ptr(
        &mut self,
        expr: &TypedExpr,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        match &expr.kind {
            TypedExprKind::Ident(name) => self
                .lookup_local(name)
                .or_else(|| {
                    self.module
                        .get_global(name)
                        .map(|global| global.as_pointer_value())
                })
                .ok_or_else(|| CodeGenError::MissingSymbol(name.clone())),
            TypedExprKind::Field(target, field) => self.emit_field_ptr(target, field),
            TypedExprKind::UnOp(UnOp::Deref, inner) => self
                .emit_expr(inner)?
                .ok_or_else(|| CodeGenError::UnsupportedExpr("deref missing value".to_string()))
                .map(BasicValueEnum::into_pointer_value),
            TypedExprKind::Index(target, index) => {
                let array = self
                    .emit_expr(target)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("index target missing value".to_string())
                    })?
                    .into_struct_value();
                let ptr = self
                    .builder
                    .build_extract_value(array, 1, "index.ptr")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .into_pointer_value();
                let idx = self
                    .emit_expr(index)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("index operand missing value".to_string())
                    })?
                    .into_int_value();
                let Type::Array(inner) = &target.ty else {
                    return Err(CodeGenError::UnsupportedExpr(
                        "index lvalue on non-array value".to_string(),
                    ));
                };
                unsafe { self.build_gep(self.llvm_basic_type(inner)?, ptr, &[idx], "index.gep") }
            }
            _ => Err(CodeGenError::UnsupportedExpr(format!(
                "non-assignable lvalue {:?}",
                expr.kind
            ))),
        }
    }

    pub(crate) fn emit_string_literal(
        &mut self,
        text: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let name = format!("str.{}", self.string_counter);
        self.string_counter = self.string_counter.saturating_add(1);
        let global = self
            .builder
            .build_global_string_ptr(text, &name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(self
            .build_struct_value(
                self.string_type,
                &[
                    self.context
                        .i64_type()
                        .const_int(text.len() as u64, false)
                        .into(),
                    global.as_pointer_value().into(),
                ],
                "string",
            )?
            .into())
    }

    fn emit_fstring(
        &mut self,
        parts: &[TypedFStrPart],
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let concat = self
            .module
            .get_function("draton_str_concat")
            .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_concat".to_string()))?;
        let mut rendered = self.emit_string_literal("")?;
        for part in parts {
            let piece = match part {
                TypedFStrPart::Literal(text) => self.emit_string_literal(text)?,
                TypedFStrPart::Interp(expr) => self.emit_coerce_to_string(expr)?,
            };
            let call = self
                .builder
                .build_call(concat, &[rendered.into(), piece.into()], "fstr.concat")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            rendered = call.try_as_basic_value().basic().ok_or_else(|| {
                CodeGenError::UnsupportedExpr("str_concat returned no value".to_string())
            })?;
        }
        Ok(rendered)
    }

    fn emit_tuple(
        &mut self,
        items: &[TypedExpr],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let tuple_ty = self.llvm_basic_type(ty)?.into_struct_type();
        let values = items
            .iter()
            .map(|item| {
                self.emit_expr(item)?.ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("tuple item missing value".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            self.build_struct_value(tuple_ty, &values, "tuple")?.into(),
        ))
    }

    fn emit_array(
        &mut self,
        items: &[TypedExpr],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let Type::Array(inner) = ty else {
            return Err(CodeGenError::UnsupportedType(ty.to_string()));
        };
        let item_ty = self.llvm_basic_type(inner)?;
        let data_ptr = self
            .builder
            .build_array_alloca(
                item_ty,
                self.context.i64_type().const_int(items.len() as u64, false),
                "array.data",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        for (index, item) in items.iter().enumerate() {
            let value = self.emit_expr(item)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("array item missing value".to_string())
            })?;
            let slot = unsafe {
                self.build_gep(
                    item_ty,
                    data_ptr,
                    &[self.context.i64_type().const_int(index as u64, false)],
                    "array.slot",
                )?
            };
            self.build_store(slot, value)?;
        }
        let array_ty = self.llvm_basic_type(ty)?.into_struct_type();
        Ok(Some(
            self.build_struct_value(
                array_ty,
                &[
                    self.context
                        .i64_type()
                        .const_int(items.len() as u64, false)
                        .into(),
                    data_ptr.into(),
                ],
                "array",
            )?
            .into(),
        ))
    }

    fn emit_binop(
        &mut self,
        lhs: &TypedExpr,
        op: BinOp,
        rhs: &TypedExpr,
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if op == BinOp::Range {
            return Err(CodeGenError::UnsupportedExpr(
                "range expressions are lowered only inside for loops".to_string(),
            ));
        }
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            if let Some(value) = self.emit_option_none_compare(lhs, op, rhs)? {
                return Ok(Some(value));
            }
        }
        let lhs_value = self
            .emit_expr(lhs)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("binop lhs missing value".to_string()))?;
        let rhs_value = self
            .emit_expr(rhs)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("binop rhs missing value".to_string()))?;
        if matches!(op, BinOp::Eq | BinOp::Ne)
            && matches!((&lhs.ty, &rhs.ty), (Type::String, Type::String))
        {
            let function = self
                .module
                .get_function("draton_str_eq")
                .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_eq".to_string()))?;
            let equal = self
                .builder
                .build_call(
                    function,
                    &[lhs_value.into(), rhs_value.into()],
                    if op == BinOp::Eq {
                        "str.eq"
                    } else {
                        "str.ne.eq"
                    },
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .try_as_basic_value()
                .basic()
                .ok_or_else(|| CodeGenError::Llvm("draton_str_eq returned void".to_string()))?
                .into_int_value();
            let value = if op == BinOp::Eq {
                equal
            } else {
                self.builder
                    .build_not(equal, "str.ne")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            };
            return Ok(Some(value.into()));
        }
        let uses_float_ops = matches!(
            (&lhs.ty, &rhs.ty),
            (
                Type::Float | Type::Float32 | Type::Float64,
                Type::Float | Type::Float32 | Type::Float64
            )
        );
        let value = match ty {
            Type::Float | Type::Float32 | Type::Float64 if uses_float_ops => {
                let lhs = lhs_value.into_float_value();
                let rhs = rhs_value.into_float_value();
                match op {
                    BinOp::Add => self
                        .builder
                        .build_float_add(lhs, rhs, "fadd")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Sub => self
                        .builder
                        .build_float_sub(lhs, rhs, "fsub")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Mul => self
                        .builder
                        .build_float_mul(lhs, rhs, "fmul")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Div => self
                        .builder
                        .build_float_div(lhs, rhs, "fdiv")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Eq => self
                        .builder
                        .build_float_compare(FloatPredicate::OEQ, lhs, rhs, "feq")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ne => self
                        .builder
                        .build_float_compare(FloatPredicate::ONE, lhs, rhs, "fne")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Lt => self
                        .builder
                        .build_float_compare(FloatPredicate::OLT, lhs, rhs, "flt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Le => self
                        .builder
                        .build_float_compare(FloatPredicate::OLE, lhs, rhs, "fle")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Gt => self
                        .builder
                        .build_float_compare(FloatPredicate::OGT, lhs, rhs, "fgt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ge => self
                        .builder
                        .build_float_compare(FloatPredicate::OGE, lhs, rhs, "fge")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    _ => return Err(CodeGenError::UnsupportedExpr(format!("float op {op:?}"))),
                }
            }
            Type::Bool if uses_float_ops => {
                let lhs = lhs_value.into_float_value();
                let rhs = rhs_value.into_float_value();
                match op {
                    BinOp::Eq => self
                        .builder
                        .build_float_compare(FloatPredicate::OEQ, lhs, rhs, "feq")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ne => self
                        .builder
                        .build_float_compare(FloatPredicate::ONE, lhs, rhs, "fne")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Lt => self
                        .builder
                        .build_float_compare(FloatPredicate::OLT, lhs, rhs, "flt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Le => self
                        .builder
                        .build_float_compare(FloatPredicate::OLE, lhs, rhs, "fle")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Gt => self
                        .builder
                        .build_float_compare(FloatPredicate::OGT, lhs, rhs, "fgt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ge => self
                        .builder
                        .build_float_compare(FloatPredicate::OGE, lhs, rhs, "fge")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    _ => return Err(CodeGenError::UnsupportedExpr(format!("float op {op:?}"))),
                }
            }
            _ => {
                let lhs = lhs_value.into_int_value();
                let rhs = rhs_value.into_int_value();
                match op {
                    BinOp::Add => self
                        .builder
                        .build_int_add(lhs, rhs, "add")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Sub => self
                        .builder
                        .build_int_sub(lhs, rhs, "sub")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Mul => self
                        .builder
                        .build_int_mul(lhs, rhs, "mul")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Div => self
                        .builder
                        .build_int_signed_div(lhs, rhs, "div")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Mod => self
                        .builder
                        .build_int_signed_rem(lhs, rhs, "rem")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Eq => self
                        .builder
                        .build_int_compare(IntPredicate::EQ, lhs, rhs, "eq")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ne => self
                        .builder
                        .build_int_compare(IntPredicate::NE, lhs, rhs, "ne")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Lt => self
                        .builder
                        .build_int_compare(IntPredicate::SLT, lhs, rhs, "lt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Le => self
                        .builder
                        .build_int_compare(IntPredicate::SLE, lhs, rhs, "le")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Gt => self
                        .builder
                        .build_int_compare(IntPredicate::SGT, lhs, rhs, "gt")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Ge => self
                        .builder
                        .build_int_compare(IntPredicate::SGE, lhs, rhs, "ge")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::And => self
                        .builder
                        .build_and(lhs, rhs, "and")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Or => self
                        .builder
                        .build_or(lhs, rhs, "or")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::BitAnd => self
                        .builder
                        .build_and(lhs, rhs, "bitand")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::BitOr => self
                        .builder
                        .build_or(lhs, rhs, "bitor")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::BitXor => self
                        .builder
                        .build_xor(lhs, rhs, "bitxor")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Shl => self
                        .builder
                        .build_left_shift(lhs, rhs, "shl")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Shr => self
                        .builder
                        .build_right_shift(lhs, rhs, true, "shr")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                        .into(),
                    BinOp::Range => unreachable!(),
                }
            }
        };
        Ok(Some(value))
    }

    fn emit_option_none_compare(
        &mut self,
        lhs: &TypedExpr,
        op: BinOp,
        rhs: &TypedExpr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let option_expr = match (&lhs.kind, &rhs.kind) {
            (TypedExprKind::NoneLit, _) if matches!(rhs.ty, Type::Option(_)) => Some(rhs),
            (_, TypedExprKind::NoneLit) if matches!(lhs.ty, Type::Option(_)) => Some(lhs),
            _ => None,
        };
        let Some(option_expr) = option_expr else {
            return Ok(None);
        };
        let option_value = self
            .emit_expr(option_expr)?
            .ok_or_else(|| {
                CodeGenError::UnsupportedExpr("option compare missing value".to_string())
            })?
            .into_struct_value();
        let has_value = self
            .builder
            .build_extract_value(option_value, 0, "option.has_value")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let zero = self.context.bool_type().const_zero();
        let pred = match op {
            BinOp::Eq => IntPredicate::EQ,
            BinOp::Ne => IntPredicate::NE,
            _ => unreachable!(),
        };
        let cmp = self
            .builder
            .build_int_compare(pred, has_value, zero, "option.none.cmp")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(Some(cmp.into()))
    }

    fn emit_unop(
        &mut self,
        op: UnOp,
        value_expr: &TypedExpr,
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let value = self.emit_expr(value_expr)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("unary operand missing value".to_string())
        })?;
        let result = match op {
            UnOp::Neg => match ty {
                Type::Float | Type::Float32 | Type::Float64 => self
                    .builder
                    .build_float_neg(value.into_float_value(), "fneg")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .into(),
                _ => self
                    .builder
                    .build_int_neg(value.into_int_value(), "neg")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .into(),
            },
            UnOp::Not | UnOp::BitNot => self
                .builder
                .build_not(value.into_int_value(), "not")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .into(),
            UnOp::Ref => self.emit_lvalue_ptr(value_expr)?.into(),
            UnOp::Deref => self.build_typed_load(
                value.into_pointer_value(),
                self.llvm_basic_type(ty)?,
                "deref",
            )?,
        };
        Ok(Some(result))
    }

    fn emit_call(
        &mut self,
        callee: &TypedExpr,
        args: &[TypedExpr],
        ret_ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if let TypedExprKind::Ident(name) = &callee.kind {
            if self.lookup_local(name).is_none() {
                return self.emit_direct_call(name, callee, args);
            }
        }

        let (params, ret) = match &callee.ty {
            Type::Fn(params, ret) => (params.clone(), ret.as_ref().clone()),
            _ => (
                args.iter().map(|arg| arg.ty.clone()).collect::<Vec<_>>(),
                ret_ty.clone(),
            ),
        };
        let closure_ptr = self
            .emit_expr(callee)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("callee missing value".to_string()))?
            .into_pointer_value();
        self.emit_closure_call(closure_ptr, args, &params, &ret)
    }

    fn emit_direct_call(
        &mut self,
        name: &str,
        callee: &TypedExpr,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let class_literal_name = match &callee.ty {
            Type::Named(class_name, type_args) if class_name == name && !type_args.is_empty() => {
                mangle_class(class_name, type_args)
            }
            _ => name.to_string(),
        };
        if self.class_layouts.contains_key(&class_literal_name)
            && args.len() == 1
            && matches!(args[0].kind, TypedExprKind::Map(_))
        {
            return self.emit_class_literal_call(&class_literal_name, &args[0]);
        }
        if let Some(value) = self.emit_builtin_call(name, args)? {
            return Ok(value);
        }
        let expected_params = match &callee.ty {
            Type::Fn(params, _) => Some(params.clone()),
            _ => None,
        };
        let symbol = self.resolve_function_symbol(
            name,
            &args.iter().map(|arg| arg.ty.clone()).collect::<Vec<_>>(),
        )?;
        if symbol == "print" || symbol == "println" {
            let arg = args.first().ok_or_else(|| {
                CodeGenError::UnsupportedExpr(format!("{symbol} requires one argument"))
            })?;
            let value = self.emit_coerce_to_string(arg)?;
            let runtime_symbol = if symbol == "println" {
                "draton_println"
            } else {
                "draton_print"
            };
            let function = self
                .module
                .get_function(runtime_symbol)
                .ok_or_else(|| CodeGenError::MissingSymbol(runtime_symbol.to_string()))?;
            let _ = self
                .builder
                .build_call(function, &[value.into()], runtime_symbol)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            return Ok(None);
        }
        let function = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.clone()))?;
        let args = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                let value = if let Some(expected) = expected_params
                    .as_ref()
                    .and_then(|params| params.get(index))
                {
                    self.emit_expr_as_type(arg, expected)?
                } else {
                    self.emit_expr(arg)?
                };
                value.ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("call arg missing value".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let call = self
            .builder
            .build_call(
                function,
                &args
                    .iter()
                    .copied()
                    .map(BasicMetadataValueEnum::from)
                    .collect::<Vec<_>>(),
                "call",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let ret_val = call.try_as_basic_value().basic();
        Ok(ret_val)
    }

    fn emit_coerce_to_string(
        &mut self,
        arg: &TypedExpr,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        match &arg.ty {
            Type::String => self.emit_expr(arg)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("print string arg missing value".to_string())
            }),
            Type::Int
            | Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::UInt8
            | Type::UInt16
            | Type::UInt32
            | Type::UInt64 => {
                let fn_int_to_str = self
                    .module
                    .get_function("draton_int_to_string")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_int_to_string".to_string())
                    })?;
                let val = self
                    .emit_expr(arg)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("print int arg missing value".to_string())
                    })?
                    .into_int_value();
                let i64_val = if val.get_type().get_bit_width() < 64 {
                    self.builder
                        .build_int_s_extend(val, self.context.i64_type(), "sext")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                } else {
                    val
                };
                self.builder
                    .build_call(fn_int_to_str, &[i64_val.into()], "int.to_string")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("int_to_string returned void".to_string())
                    })
            }
            Type::Bool => {
                let val = self
                    .emit_expr(arg)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("print bool arg missing value".to_string())
                    })?
                    .into_int_value();
                let true_str = self.emit_string_literal("true")?;
                let false_str = self.emit_string_literal("false")?;
                self.builder
                    .build_select(val, true_str, false_str, "bool.to_string")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))
            }
            Type::Float | Type::Float64 => {
                if self
                    .module
                    .get_function("__draton_std_float_to_string")
                    .is_none()
                {
                    self.module.add_function(
                        "__draton_std_float_to_string",
                        self.string_type
                            .fn_type(&[self.context.f64_type().into()], false),
                        None,
                    );
                }
                let fn_float_to_str = self
                    .module
                    .get_function("__draton_std_float_to_string")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("__draton_std_float_to_string".to_string())
                    })?;
                let val = self.emit_expr(arg)?.ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("print float arg missing value".to_string())
                })?;
                self.builder
                    .build_call(fn_float_to_str, &[val.into()], "float.to_string")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("float_to_string returned void".to_string())
                    })
            }
            Type::Float32 => {
                if self
                    .module
                    .get_function("__draton_std_float_to_string")
                    .is_none()
                {
                    self.module.add_function(
                        "__draton_std_float_to_string",
                        self.string_type
                            .fn_type(&[self.context.f64_type().into()], false),
                        None,
                    );
                }
                let fn_float_to_str = self
                    .module
                    .get_function("__draton_std_float_to_string")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("__draton_std_float_to_string".to_string())
                    })?;
                let val = self
                    .emit_expr(arg)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("print float32 arg missing value".to_string())
                    })?
                    .into_float_value();
                let f64_val = self
                    .builder
                    .build_float_ext(val, self.context.f64_type(), "fpext")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                self.builder
                    .build_call(fn_float_to_str, &[f64_val.into()], "float32.to_string")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("float32_to_string returned void".to_string())
                    })
            }
            _ => self.emit_expr(arg)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("print arg missing value".to_string())
            }),
        }
    }

    fn emit_class_literal_call(
        &mut self,
        class_name: &str,
        field_map: &TypedExpr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let ctor_name = format!("{class_name}_new");
        let function = self
            .functions
            .get(&ctor_name)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(ctor_name.clone()))?;
        let call = self
            .builder
            .build_call(function, &[], "ctor.call")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let object_ptr = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::UnsupportedExpr("constructor returned void".to_string()))?
            .into_pointer_value();
        let TypedExprKind::Map(entries) = &field_map.kind else {
            return Ok(Some(object_ptr.into()));
        };
        for (key, value_expr) in entries {
            let TypedExprKind::StrLit(field_name) = &key.kind else {
                continue;
            };
            let field_ptr = self.emit_field_ptr_in_class(object_ptr, class_name, field_name)?;
            let value = self.emit_expr(value_expr)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("class field initializer missing value".to_string())
            })?;
            self.build_store(field_ptr, value)?;
            if Self::is_gc_pointer_type(&value_expr.ty) {
                let _ = self.emit_gc_write_barrier(object_ptr, field_ptr, value);
            }
        }
        Ok(Some(object_ptr.into()))
    }

    fn emit_builtin_call(
        &mut self,
        name: &str,
        args: &[TypedExpr],
    ) -> Result<Option<Option<BasicValueEnum<'ctx>>>, CodeGenError> {
        match name {
            "Some" => {
                let arg = args.first().ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("Some requires one argument".to_string())
                })?;
                let value = self.emit_expr(arg)?.ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("Some arg missing value".to_string())
                })?;
                let option_ty = self
                    .llvm_basic_type(&Type::Option(Box::new(arg.ty.clone())))?
                    .into_struct_type();
                let tag = self.context.bool_type().const_int(1, false).into();
                let option = self.build_struct_value(option_ty, &[tag, value], "some")?;
                Ok(Some(Some(option.into())))
            }
            "str_len" => {
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("str_len requires one argument".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("str_len arg missing value".to_string())
                    })?
                    .into_struct_value();
                let len = self
                    .builder
                    .build_extract_value(value, 0, "str.len")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(Some(len)))
            }
            "str_byte_at" => {
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "str_byte_at requires two arguments".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "str_byte_at string arg missing value".to_string(),
                        )
                    })?
                    .into_struct_value();
                let index = self
                    .emit_expr(args.get(1).ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "str_byte_at requires two arguments".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "str_byte_at index arg missing value".to_string(),
                        )
                    })?
                    .into_int_value();
                let ptr = self
                    .builder
                    .build_extract_value(value, 1, "str.ptr")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .into_pointer_value();
                let byte_ptr = unsafe {
                    self.build_gep(self.context.i8_type().into(), ptr, &[index], "str.byte.ptr")?
                };
                let byte = self
                    .build_typed_load(byte_ptr, self.context.i8_type().into(), "str.byte")?
                    .into_int_value();
                let widened = self
                    .builder
                    .build_int_z_extend(byte, self.context.i64_type(), "str.byte.int")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(Some(widened.into())))
            }
            "str_slice" => {
                let function = self
                    .module
                    .get_function("draton_str_slice")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_slice".to_string()))?;
                let values = args
                    .iter()
                    .map(|arg| {
                        self.emit_expr(arg)?.ok_or_else(|| {
                            CodeGenError::UnsupportedExpr("str_slice arg missing value".to_string())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &values
                            .iter()
                            .copied()
                            .map(BasicMetadataValueEnum::from)
                            .collect::<Vec<_>>(),
                        "str.slice",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "str_concat" => {
                let function = self
                    .module
                    .get_function("draton_str_concat")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_concat".to_string()))?;
                let values = args
                    .iter()
                    .map(|arg| {
                        self.emit_expr(arg)?.ok_or_else(|| {
                            CodeGenError::UnsupportedExpr(
                                "str_concat arg missing value".to_string(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &values
                            .iter()
                            .copied()
                            .map(BasicMetadataValueEnum::from)
                            .collect::<Vec<_>>(),
                        "str.concat",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "int_to_string" => {
                let function = self
                    .module
                    .get_function("draton_int_to_string")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_int_to_string".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "int_to_string requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("int_to_string arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "int.to_string")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "ascii_char" => {
                let function = self
                    .module
                    .get_function("draton_ascii_char")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_ascii_char".to_string()))?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "ascii_char requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("ascii_char arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "ascii.char")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "read_file" => {
                let function = self
                    .module
                    .get_function("draton_read_file")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_read_file".to_string()))?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("read_file requires one argument".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("read_file arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "read.file")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "string_parse_int" => {
                let function = self
                    .module
                    .get_function("draton_string_parse_int")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_string_parse_int".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "string_parse_int requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "string_parse_int arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "string.parse_int")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "string_parse_int_radix" => {
                let function = self
                    .module
                    .get_function("draton_string_parse_int_radix")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_string_parse_int_radix".to_string())
                    })?;
                let values = args
                    .iter()
                    .map(|arg| {
                        self.emit_expr(arg)?.ok_or_else(|| {
                            CodeGenError::UnsupportedExpr(
                                "string_parse_int_radix arg missing value".to_string(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &values
                            .iter()
                            .copied()
                            .map(BasicMetadataValueEnum::from)
                            .collect::<Vec<_>>(),
                        "string.parse_int_radix",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "string_parse_float" => {
                let function = self
                    .module
                    .get_function("draton_string_parse_float")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_string_parse_float".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "string_parse_float requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "string_parse_float arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "string.parse_float")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "cli_argc" => {
                let function = self
                    .module
                    .get_function("draton_cli_argc")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_cli_argc".to_string()))?;
                let call = self
                    .builder
                    .build_call(function, &[], "cli.argc")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "cli_arg" => {
                let function = self
                    .module
                    .get_function("draton_cli_arg")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_cli_arg".to_string()))?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("cli_arg requires one argument".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("cli_arg arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "cli.arg")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_ast_dump" => {
                let function = self
                    .module
                    .get_function("draton_host_ast_dump")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_ast_dump".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_ast_dump requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("host_ast_dump arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "host.ast_dump")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_type_dump" => {
                let function = self
                    .module
                    .get_function("draton_host_type_dump")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_type_dump".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_type_dump requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_type_dump arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "host.type_dump")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_lex_json" => {
                let function = self
                    .module
                    .get_function("draton_host_lex_json")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_lex_json".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_lex_json requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("host_lex_json arg missing value".to_string())
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "host.lex_json")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_parse_json" => {
                let function = self
                    .module
                    .get_function("draton_host_parse_json")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_parse_json".to_string())
                    })?;
                let value = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_parse_json requires one argument".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "host_parse_json arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(function, &[value.into()], "host.parse_json")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_type_json" => {
                let function = self
                    .module
                    .get_function("draton_host_type_json")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_type_json".to_string())
                    })?;
                let values = args
                    .iter()
                    .map(|arg| {
                        self.emit_expr(arg)?.ok_or_else(|| {
                            CodeGenError::UnsupportedExpr(
                                "host_type_json arg missing value".to_string(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &values
                            .iter()
                            .copied()
                            .map(BasicMetadataValueEnum::from)
                            .collect::<Vec<_>>(),
                        "host.type_json",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            "host_build_json" => {
                let function = self
                    .module
                    .get_function("draton_host_build_json")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_host_build_json".to_string())
                    })?;
                let values = args
                    .iter()
                    .map(|arg| {
                        self.emit_expr(arg)?.ok_or_else(|| {
                            CodeGenError::UnsupportedExpr(
                                "host_build_json arg missing value".to_string(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &values
                            .iter()
                            .copied()
                            .map(BasicMetadataValueEnum::from)
                            .collect::<Vec<_>>(),
                        "host.build_json",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(call.try_as_basic_value().basic()))
            }
            _ => Ok(None),
        }
    }

    fn resolve_function_value_symbol(&self, name: &str, ty: &Type) -> Result<String, CodeGenError> {
        if self.functions.contains_key(name) {
            return Ok(name.to_string());
        }
        if let Type::Fn(params, _) = ty {
            return self.resolve_function_symbol(name, params);
        }
        Err(CodeGenError::MissingSymbol(name.to_string()))
    }

    fn emit_method_call(
        &mut self,
        target: &TypedExpr,
        method: &str,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if let Type::Array(inner) = &target.ty {
            return self.emit_array_method_call(target, inner, method, args);
        }
        if matches!(target.ty, Type::String) {
            return self.emit_string_method_call(target, method, args);
        }
        let Type::Named(class_name, type_args) = &target.ty else {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "method call on non-class type {}",
                target.ty
            )));
        };
        if type_args.is_empty() && self.is_interface_type_name(class_name) {
            let fat_ptr = self
                .emit_expr(target)?
                .ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("interface target missing value".to_string())
                })?
                .into_struct_value();
            return self.emit_interface_method_call(fat_ptr, class_name, method, args);
        }
        let runtime_name = if type_args.is_empty() {
            class_name.clone()
        } else {
            mangle_class(class_name, type_args)
        };
        let (owner_class, function) = self
            .resolve_method_fn(&runtime_name, method)
            .map(|(owner, symbol)| {
                self.functions
                    .get(&symbol)
                    .copied()
                    .map(|function| (owner, function))
                    .ok_or(CodeGenError::MissingSymbol(symbol))
            })
            .transpose()?
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{runtime_name}.{method}")))?;
        let mut call_args = Vec::new();
        let target_ptr = self
            .emit_expr(target)?
            .ok_or_else(|| {
                CodeGenError::UnsupportedExpr("method target missing value".to_string())
            })?
            .into_pointer_value();
        let target_value = if owner_class == runtime_name {
            target_ptr.into()
        } else {
            self.emit_upcast_to_parent(target_ptr, &runtime_name, &owner_class)?
                .into()
        };
        call_args.push(target_value);
        call_args.extend(
            args.iter()
                .map(|arg| {
                    self.emit_expr(arg)?.ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("method arg missing value".to_string())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        let call = self
            .builder
            .build_call(
                function,
                &call_args
                    .iter()
                    .copied()
                    .map(BasicMetadataValueEnum::from)
                    .collect::<Vec<_>>(),
                "method.call",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(call.try_as_basic_value().basic())
    }

    fn emit_string_method_call(
        &mut self,
        target: &TypedExpr,
        method: &str,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let target_value = self
            .emit_expr(target)?
            .ok_or_else(|| {
                CodeGenError::UnsupportedExpr("string target missing value".to_string())
            })?
            .into_struct_value();
        match method {
            "len" => {
                let len = self
                    .builder
                    .build_extract_value(target_value, 0, "str.len")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(Some(len))
            }
            "slice" => {
                let function = self
                    .module
                    .get_function("draton_str_slice")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_slice".to_string()))?;
                let start = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("String.slice expects 2 args".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.slice start arg missing value".to_string(),
                        )
                    })?;
                let end = self
                    .emit_expr(args.get(1).ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("String.slice expects 2 args".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.slice end arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &[target_value.into(), start.into(), end.into()],
                        "str.slice",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(call.try_as_basic_value().basic())
            }
            "contains" => {
                let function =
                    self.module
                        .get_function("draton_str_contains")
                        .ok_or_else(|| {
                            CodeGenError::MissingSymbol("draton_str_contains".to_string())
                        })?;
                let needle = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("String.contains expects 1 arg".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.contains arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &[target_value.into(), needle.into()],
                        "str.contains",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(call.try_as_basic_value().basic())
            }
            "starts_with" => {
                let function = self
                    .module
                    .get_function("draton_str_starts_with")
                    .ok_or_else(|| {
                        CodeGenError::MissingSymbol("draton_str_starts_with".to_string())
                    })?;
                let prefix = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.starts_with expects 1 arg".to_string(),
                        )
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.starts_with arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &[target_value.into(), prefix.into()],
                        "str.starts",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(call.try_as_basic_value().basic())
            }
            "replace" => {
                let function = self
                    .module
                    .get_function("draton_str_replace")
                    .ok_or_else(|| CodeGenError::MissingSymbol("draton_str_replace".to_string()))?;
                let from = self
                    .emit_expr(args.first().ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("String.replace expects 2 args".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.replace from arg missing value".to_string(),
                        )
                    })?;
                let to = self
                    .emit_expr(args.get(1).ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("String.replace expects 2 args".to_string())
                    })?)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "String.replace to arg missing value".to_string(),
                        )
                    })?;
                let call = self
                    .builder
                    .build_call(
                        function,
                        &[target_value.into(), from.into(), to.into()],
                        "str.replace",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(call.try_as_basic_value().basic())
            }
            _ => Err(CodeGenError::UnsupportedExpr(format!(
                "unsupported string method {method}"
            ))),
        }
    }

    fn emit_array_method_call(
        &mut self,
        target: &TypedExpr,
        inner: &Type,
        method: &str,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if method == "len" {
            let array = self
                .emit_expr(target)?
                .ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("array target missing value".to_string())
                })?
                .into_struct_value();
            let len = self
                .builder
                .build_extract_value(array, 0, "array.len")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            return Ok(Some(len));
        }
        if method != "push" {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "unsupported array method {method}"
            )));
        }
        if args.len() != 1 {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "array.push expects 1 arg, got {}",
                args.len()
            )));
        }

        let array_ptr = self.emit_lvalue_ptr(target)?;
        let array_value = self
            .build_load(array_ptr, "array.push.load")?
            .into_struct_value();
        let old_len = self
            .builder
            .build_extract_value(array_value, 0, "array.old.len")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let old_data = self
            .builder
            .build_extract_value(array_value, 1, "array.old.ptr")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let elem_ty = self.llvm_basic_type(inner)?;
        let elem_size = self.basic_type_size_bytes(elem_ty);
        let new_len = self
            .builder
            .build_int_add(
                old_len,
                self.context.i64_type().const_int(1, false),
                "array.new.len",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let total_bytes = self
            .builder
            .build_int_mul(
                new_len,
                self.context.i64_type().const_int(elem_size, false),
                "array.alloc.bytes",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let raw_ptr = self
            .module
            .get_function("malloc")
            .ok_or_else(|| CodeGenError::MissingSymbol("malloc".to_string()))?;
        let raw_ptr = self
            .builder
            .build_call(raw_ptr, &[total_bytes.into()], "array.push.raw")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm("malloc returned void".to_string()))?
            .into_pointer_value();
        let new_data = self.build_pointer_cast_to(raw_ptr, elem_ty, "array.push.ptr")?;

        let old_bytes = self
            .builder
            .build_int_mul(
                old_len,
                self.context.i64_type().const_int(elem_size, false),
                "array.copy.bytes",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let old_i8 = self
            .builder
            .build_pointer_cast(
                old_data,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "array.old.i8",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let new_i8 = self
            .builder
            .build_pointer_cast(
                new_data,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "array.new.i8",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_memcpy(new_i8, 1, old_i8, 1, old_bytes)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        let value = self.emit_expr(&args[0])?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("array.push arg missing value".to_string())
        })?;
        let end_slot = unsafe { self.build_gep(elem_ty, new_data, &[old_len], "array.push.slot")? };
        self.build_store(end_slot, value)?;

        let array_ty = self.llvm_basic_type(&target.ty)?.into_struct_type();
        let len_ptr = self.build_struct_gep(array_ty, array_ptr, 0, "array.len.ptr")?;
        self.build_store(len_ptr, new_len.into())?;
        let data_ptr = self.build_struct_gep(array_ty, array_ptr, 1, "array.data.ptr")?;
        self.build_store(data_ptr, new_data.into())?;
        Ok(None)
    }

    fn emit_field_load(
        &mut self,
        target: &TypedExpr,
        field: &str,
        field_ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let ptr = self.emit_field_ptr(target, field)?;
        self.build_typed_load(ptr, self.llvm_basic_type(field_ty)?, field)
    }

    fn emit_index(
        &mut self,
        target: &TypedExpr,
        index: &TypedExpr,
        item_ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let ptr = self.emit_lvalue_ptr(&TypedExpr {
            kind: TypedExprKind::Index(Box::new(target.clone()), Box::new(index.clone())),
            ty: target.ty.clone(),
            span: target.span,
            use_effect: None,
        })?;
        self.build_typed_load(ptr, self.llvm_basic_type(item_ty)?, "index.load")
    }

    fn emit_field_ptr(
        &mut self,
        target: &TypedExpr,
        field: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let Type::Named(class_name, type_args) = &target.ty else {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "field access on non-class type {}",
                target.ty
            )));
        };
        let runtime_name = if type_args.is_empty() {
            class_name.clone()
        } else {
            mangle_class(class_name, type_args)
        };
        let base = self
            .emit_expr(target)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("field target missing value".to_string()))?
            .into_pointer_value();
        self.emit_field_ptr_in_class(base, &runtime_name, field)
    }

    fn emit_field_ptr_in_class(
        &mut self,
        ptr: PointerValue<'ctx>,
        class_name: &str,
        field: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let layout = self
            .class_layouts
            .get(class_name)
            .cloned()
            .ok_or_else(|| CodeGenError::MissingSymbol(class_name.to_string()))?;
        if let Some(index) = layout.field_indices.get(field).copied() {
            return self.build_struct_gep(layout.struct_type, ptr, index, field);
        }
        if let Some(parent_class) = layout.parent_class {
            let parent_ptr = self.build_struct_gep(layout.struct_type, ptr, 0, "parent.ptr")?;
            return self.emit_field_ptr_in_class(parent_ptr, &parent_class, field);
        }
        Err(CodeGenError::MissingSymbol(format!("{class_name}.{field}")))
    }

    fn resolve_method_fn(&self, class_name: &str, method: &str) -> Option<(String, String)> {
        let mut current = Some(class_name.to_string());
        while let Some(class_name) = current {
            let symbol = format!("{class_name}.{method}");
            if self.functions.contains_key(&symbol) {
                return Some((class_name, symbol));
            }
            current = self
                .class_layouts
                .get(&class_name)
                .and_then(|layout| layout.parent_class.clone());
        }
        None
    }

    fn emit_upcast_to_parent(
        &mut self,
        child_ptr: PointerValue<'ctx>,
        child_class: &str,
        target_class: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        if child_class == target_class {
            return Ok(child_ptr);
        }
        let layout = self
            .class_layouts
            .get(child_class)
            .cloned()
            .ok_or_else(|| CodeGenError::MissingSymbol(child_class.to_string()))?;
        let parent_class = layout.parent_class.ok_or_else(|| {
            CodeGenError::MissingSymbol(format!("{child_class} does not extend {target_class}"))
        })?;
        let parent_ptr = self.build_struct_gep(layout.struct_type, child_ptr, 0, "upcast.ptr")?;
        self.emit_upcast_to_parent(parent_ptr, &parent_class, target_class)
    }

    fn class_extends(&self, child_class: &str, target_class: &str) -> bool {
        if child_class == target_class {
            return true;
        }
        let mut current = Some(child_class.to_string());
        while let Some(class_name) = current {
            if class_name == target_class {
                return true;
            }
            current = self
                .class_layouts
                .get(&class_name)
                .and_then(|layout| layout.parent_class.clone());
        }
        false
    }

    fn class_implements_interface(&self, class_name: &str, iface_name: &str) -> bool {
        let mut current = Some(class_name.to_string());
        while let Some(class_name) = current {
            if self
                .iface_registry
                .implementors
                .get(iface_name)
                .map(|implementors| implementors.iter().any(|item| item == &class_name))
                .unwrap_or(false)
            {
                return true;
            }
            current = self
                .class_layouts
                .get(&class_name)
                .and_then(|layout| layout.parent_class.clone());
        }
        false
    }

    pub(crate) fn emit_expr_as_type(
        &mut self,
        expr: &TypedExpr,
        expected_ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        match (&expr.ty, expected_ty) {
            (Type::Named(class_name, class_args), Type::Named(iface_name, iface_args))
                if iface_args.is_empty()
                    && self.is_interface_type_name(iface_name)
                    && self.class_implements_interface(
                        &self.runtime_class_name(class_name, class_args),
                        iface_name,
                    ) =>
            {
                let runtime_name = self.runtime_class_name(class_name, class_args);
                let value = self
                    .emit_expr(expr)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "interface upcast source missing value".to_string(),
                        )
                    })?
                    .into_pointer_value();
                self.emit_upcast_to_interface(value, &runtime_name, iface_name)
                    .map(|value| Some(value.into()))
            }
            (Type::Named(class_name, class_args), Type::Named(parent_name, parent_args)) => {
                let runtime_name = self.runtime_class_name(class_name, class_args);
                let target_name = self.runtime_class_name(parent_name, parent_args);
                if runtime_name != target_name && self.class_extends(&runtime_name, &target_name) {
                    let value = self
                        .emit_expr(expr)?
                        .ok_or_else(|| {
                            CodeGenError::UnsupportedExpr(
                                "parent upcast source missing value".to_string(),
                            )
                        })?
                        .into_pointer_value();
                    return self
                        .emit_upcast_to_parent(value, &runtime_name, &target_name)
                        .map(|value| Some(value.into()));
                }
                self.emit_expr(expr)
            }
            _ => self.emit_expr(expr),
        }
    }

    fn emit_cast(
        &mut self,
        value: &TypedExpr,
        to_ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        if let (Type::Named(class_name, class_args), Type::Named(target_name, target_args)) =
            (&value.ty, to_ty)
        {
            let runtime_name = self.runtime_class_name(class_name, class_args);
            let target_runtime_name = self.runtime_class_name(target_name, target_args);
            if target_args.is_empty() && self.is_interface_type_name(target_name) {
                let runtime_name = self.runtime_class_name(class_name, class_args);
                let value = self
                    .emit_expr(value)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "interface cast source missing value".to_string(),
                        )
                    })?
                    .into_pointer_value();
                return self
                    .emit_upcast_to_interface(value, &runtime_name, target_name)
                    .map(Into::into);
            }
            if runtime_name != target_runtime_name
                && self.class_extends(&runtime_name, &target_runtime_name)
            {
                let value = self
                    .emit_expr(value)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedExpr(
                            "parent cast source missing value".to_string(),
                        )
                    })?
                    .into_pointer_value();
                return self
                    .emit_upcast_to_parent(value, &runtime_name, &target_runtime_name)
                    .map(Into::into);
            }
        }
        let value = self.emit_expr(value)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("cast source missing value".to_string())
        })?;
        Ok(match (&value.get_type(), to_ty) {
            (inkwell::types::BasicTypeEnum::IntType(_), Type::Float | Type::Float64) => self
                .builder
                .build_signed_int_to_float(
                    value.into_int_value(),
                    self.context.f64_type(),
                    "sitofp",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .into(),
            (inkwell::types::BasicTypeEnum::FloatType(_), Type::Int | Type::Int64) => self
                .builder
                .build_float_to_signed_int(
                    value.into_float_value(),
                    self.context.i64_type(),
                    "fptosi",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .into(),
            _ => value,
        })
    }

    fn emit_match(
        &mut self,
        subject: &TypedExpr,
        arms: &[draton_typeck::TypedMatchArm],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if matches!(subject.ty, Type::Int | Type::Int64 | Type::Bool) {
            self.emit_switch_match(subject, arms, ty)
        } else {
            self.emit_linear_match(subject, arms, ty)
        }
    }

    fn emit_switch_match(
        &mut self,
        subject: &TypedExpr,
        arms: &[draton_typeck::TypedMatchArm],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let function = self.current_function()?;
        let subject = self
            .emit_expr(subject)?
            .ok_or_else(|| {
                CodeGenError::UnsupportedExpr("match subject missing value".to_string())
            })?
            .into_int_value();
        let exit = self.context.append_basic_block(function, "match.exit");
        let default = self.context.append_basic_block(function, "match.default");
        let result_slot = if matches!(ty, Type::Unit | Type::Never) {
            None
        } else {
            Some(self.create_entry_alloca(function, self.llvm_basic_type(ty)?, "match.slot")?)
        };

        let subject_ty = subject.get_type();
        let mut cases = Vec::new();
        let mut arm_blocks = Vec::new();
        let mut default_arm = None;
        for arm in arms {
            match arm.pattern.kind {
                TypedExprKind::IntLit(value) => {
                    let block = self.context.append_basic_block(function, "match.case");
                    cases.push((subject_ty.const_int(value as u64, true), block));
                    arm_blocks.push((block, arm));
                }
                TypedExprKind::BoolLit(value) => {
                    let block = self.context.append_basic_block(function, "match.case");
                    cases.push((subject_ty.const_int(u64::from(value), false), block));
                    arm_blocks.push((block, arm));
                }
                TypedExprKind::Ident(ref name) if name == "_" => {
                    let block = self
                        .context
                        .append_basic_block(function, "match.default.arm");
                    default_arm = Some((block, arm));
                }
                _ => {}
            }
        }
        let default_target = default_arm.map(|(block, _)| block).unwrap_or(default);
        self.builder
            .build_switch(subject, default_target, &cases)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        for (block, arm) in arm_blocks {
            self.builder.position_at_end(block);
            let value = self.emit_match_arm_body(&arm.body)?;
            if let (Some(slot), Some(value)) = (result_slot, value) {
                self.build_store(slot, value)?;
            }
            if !self.current_block_terminated() {
                self.builder
                    .build_unconditional_branch(exit)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            }
        }

        if let Some((block, arm)) = default_arm {
            self.builder.position_at_end(block);
            let value = self.emit_match_arm_body(&arm.body)?;
            if let (Some(slot), Some(value)) = (result_slot, value) {
                self.build_store(slot, value)?;
            }
            if !self.current_block_terminated() {
                self.builder
                    .build_unconditional_branch(exit)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            }
        }

        self.builder.position_at_end(default);
        if !self.current_block_terminated() {
            if let Some(slot) = result_slot {
                let fallback = self.zero_value(ty)?;
                self.build_store(slot, fallback)?;
            }
            self.builder
                .build_unconditional_branch(exit)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(exit);
        if let Some(slot) = result_slot {
            self.build_load(slot, "match.result").map(Some)
        } else {
            Ok(None)
        }
    }

    fn emit_linear_match(
        &mut self,
        subject: &TypedExpr,
        arms: &[draton_typeck::TypedMatchArm],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if matches!(subject.ty, Type::Option(_)) {
            return self.emit_option_match(subject, arms, ty);
        }
        let subject = self.emit_expr(subject)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("match subject missing value".to_string())
        })?;
        for arm in arms {
            if matches!(&arm.pattern.kind, TypedExprKind::Ident(name) if name == "_") {
                return self.emit_match_arm_body(&arm.body);
            }
            if let TypedExprKind::Ok(inner) = &arm.pattern.kind {
                let result = subject.into_struct_value();
                let tag = self
                    .builder
                    .build_extract_value(result, 0, "match.tag")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                    .into_int_value();
                let function = self.current_function()?;
                let ok_block = self.context.append_basic_block(function, "match.ok");
                let next_block = self.context.append_basic_block(function, "match.next");
                let exit_block = self.context.append_basic_block(function, "match.exit");
                let phi = if matches!(ty, Type::Unit | Type::Never) {
                    None
                } else {
                    Some(
                        self.builder
                            .build_phi(self.llvm_basic_type(ty)?, "match.phi")
                            .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
                    )
                };
                self.builder
                    .build_conditional_branch(tag, ok_block, next_block)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                self.builder.position_at_end(ok_block);
                let payload = self
                    .builder
                    .build_extract_value(result, 1, "match.ok.value")
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                if let TypedExprKind::Ident(name) = &inner.kind {
                    let ptr = self.create_entry_alloca(function, payload.get_type(), name)?;
                    self.build_store(ptr, payload)?;
                    self.push_scope();
                    self.define_local(name, ptr);
                    let arm_value = self.emit_match_arm_body(&arm.body)?;
                    self.pop_scope();
                    if let (Some(phi), Some(value)) = (&phi, arm_value) {
                        phi.add_incoming(&[(&value, ok_block)]);
                    }
                }
                if !self.current_block_terminated() {
                    self.builder
                        .build_unconditional_branch(exit_block)
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                }
                self.builder.position_at_end(next_block);
                let fallback = self.zero_value(ty)?;
                if let Some(phi) = &phi {
                    phi.add_incoming(&[(&fallback, next_block)]);
                }
                self.builder
                    .build_unconditional_branch(exit_block)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                self.builder.position_at_end(exit_block);
                return Ok(phi.map(|phi| phi.as_basic_value()));
            }
        }
        if matches!(ty, Type::Unit | Type::Never) {
            Ok(None)
        } else {
            self.zero_value(ty).map(Some)
        }
    }

    fn emit_option_match(
        &mut self,
        subject: &TypedExpr,
        arms: &[draton_typeck::TypedMatchArm],
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let function = self.current_function()?;
        let subject = self
            .emit_expr(subject)?
            .ok_or_else(|| {
                CodeGenError::UnsupportedExpr("match subject missing value".to_string())
            })?
            .into_struct_value();
        let tag = self
            .builder
            .build_extract_value(subject, 0, "match.option.tag")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let payload = self
            .builder
            .build_extract_value(subject, 1, "match.option.payload")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let exit = self
            .context
            .append_basic_block(function, "match.option.exit");
        let result_slot = if matches!(ty, Type::Unit | Type::Never) {
            None
        } else {
            Some(self.create_entry_alloca(
                function,
                self.llvm_basic_type(ty)?,
                "match.option.slot",
            )?)
        };

        let mut next_block = self
            .context
            .append_basic_block(function, "match.option.check");
        self.builder
            .build_unconditional_branch(next_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        for arm in arms {
            self.builder.position_at_end(next_block);
            if matches!(&arm.pattern.kind, TypedExprKind::Ident(name) if name == "_") {
                let value = self.emit_match_arm_body(&arm.body)?;
                if let (Some(slot), Some(value)) = (result_slot, value) {
                    self.build_store(slot, value)?;
                }
                if !self.current_block_terminated() {
                    self.builder
                        .build_unconditional_branch(exit)
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                }
                self.builder.position_at_end(exit);
                return if let Some(slot) = result_slot {
                    self.build_load(slot, "match.option.result").map(Some)
                } else {
                    Ok(None)
                };
            }

            let arm_block = self
                .context
                .append_basic_block(function, "match.option.arm");
            let fallthrough = self
                .context
                .append_basic_block(function, "match.option.next");

            match &arm.pattern.kind {
                TypedExprKind::Call(callee, args)
                    if matches!(&callee.kind, TypedExprKind::Ident(name) if name == "Some")
                        && args.len() == 1 =>
                {
                    self.builder
                        .build_conditional_branch(tag, arm_block, fallthrough)
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                    self.builder.position_at_end(arm_block);
                    if let TypedExprKind::Ident(name) = &args[0].kind {
                        if name != "_" {
                            let ptr =
                                self.create_entry_alloca(function, payload.get_type(), name)?;
                            self.build_store(ptr, payload)?;
                            self.push_scope();
                            self.define_local(name, ptr);
                            let value = self.emit_match_arm_body(&arm.body)?;
                            self.pop_scope();
                            if let (Some(slot), Some(value)) = (result_slot, value) {
                                self.build_store(slot, value)?;
                            }
                        } else {
                            let value = self.emit_match_arm_body(&arm.body)?;
                            if let (Some(slot), Some(value)) = (result_slot, value) {
                                self.build_store(slot, value)?;
                            }
                        }
                    } else {
                        return Err(CodeGenError::UnsupportedExpr(
                            "unsupported Some pattern shape".to_string(),
                        ));
                    }
                }
                TypedExprKind::NoneLit => {
                    let is_none = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            tag,
                            self.context.bool_type().const_zero(),
                            "match.option.is_none",
                        )
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                    self.builder
                        .build_conditional_branch(is_none, arm_block, fallthrough)
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                    self.builder.position_at_end(arm_block);
                    let value = self.emit_match_arm_body(&arm.body)?;
                    if let (Some(slot), Some(value)) = (result_slot, value) {
                        self.build_store(slot, value)?;
                    }
                }
                _ => {
                    return Err(CodeGenError::UnsupportedExpr(
                        "unsupported option match pattern".to_string(),
                    ));
                }
            }

            if !self.current_block_terminated() {
                self.builder
                    .build_unconditional_branch(exit)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            }
            next_block = fallthrough;
        }

        self.builder.position_at_end(next_block);
        if !self.current_block_terminated() {
            if let Some(slot) = result_slot {
                let fallback = self.zero_value(ty)?;
                self.build_store(slot, fallback)?;
            }
            self.builder
                .build_unconditional_branch(exit)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(exit);
        if let Some(slot) = result_slot {
            self.build_load(slot, "match.option.result").map(Some)
        } else {
            Ok(None)
        }
    }

    fn emit_match_arm_body(
        &mut self,
        body: &TypedMatchArmBody,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        match body {
            TypedMatchArmBody::Expr(expr) => self.emit_expr(expr),
            TypedMatchArmBody::Block(block) => {
                let _ = self.emit_block(block)?;
                Ok(None)
            }
        }
    }

    fn emit_result(
        &mut self,
        value: &TypedExpr,
        ty: &Type,
        is_ok: bool,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let Type::Result(ok_ty, err_ty) = ty else {
            return Err(CodeGenError::UnsupportedType(ty.to_string()));
        };
        let payload = self.emit_expr(value)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("result payload missing value".to_string())
        })?;
        let result_ty = self.llvm_basic_type(ty)?.into_struct_type();
        let values = if is_ok {
            vec![
                self.context.bool_type().const_int(1, false).into(),
                payload,
                self.zero_value(err_ty)?,
            ]
        } else {
            vec![
                self.context.bool_type().const_zero().into(),
                self.zero_value(ok_ty)?,
                payload,
            ]
        };
        Ok(Some(
            self.build_struct_value(result_ty, &values, "result")?
                .into(),
        ))
    }

    fn emit_nullish(
        &mut self,
        lhs: &TypedExpr,
        rhs: &TypedExpr,
        ty: &Type,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let function = self.current_function()?;
        let result = self
            .emit_expr(lhs)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("nullish lhs missing value".to_string()))?
            .into_struct_value();
        let tag = self
            .builder
            .build_extract_value(result, 0, "nullish.tag")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let ok_block = self.context.append_basic_block(function, "nullish.ok");
        let err_block = self.context.append_basic_block(function, "nullish.err");
        let cont_block = self.context.append_basic_block(function, "nullish.cont");
        let phi = self
            .builder
            .build_phi(self.llvm_basic_type(ty)?, "nullish.phi")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_conditional_branch(tag, ok_block, err_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(ok_block);
        let ok_value = self
            .builder
            .build_extract_value(result, 1, "nullish.ok.value")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        phi.add_incoming(&[(&ok_value, ok_block)]);
        self.builder
            .build_unconditional_branch(cont_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(err_block);
        let err_value = self.emit_expr(rhs)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("nullish rhs missing value".to_string())
        })?;
        if let Some(Type::Result(ok_ty, err_ty)) = &self.current_return_type {
            let result_ty = self
                .llvm_basic_type(&Type::Result(ok_ty.clone(), err_ty.clone()))?
                .into_struct_type();
            let propagated = self.build_struct_value(
                result_ty,
                &[
                    self.context.bool_type().const_zero().into(),
                    self.zero_value(ok_ty)?,
                    err_value,
                ],
                "nullish.err",
            )?;
            self.builder
                .build_return(Some(&propagated))
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        } else {
            let fallback = self.zero_value(ty)?;
            phi.add_incoming(&[(&fallback, err_block)]);
            self.builder
                .build_unconditional_branch(cont_block)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(cont_block);
        Ok(Some(phi.as_basic_value()))
    }
}
