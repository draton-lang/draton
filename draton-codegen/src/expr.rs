use draton_ast::{BinOp, UnOp};
use draton_typeck::{Type, TypedExpr, TypedExprKind, TypedFStrPart, TypedMatchArmBody};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, PointerValue};
use inkwell::{FloatPredicate, IntPredicate};

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
                    self.build_load(ptr, name).map(Some)
                } else if let Some(function) = self.functions.get(name).copied() {
                    Ok(Some(function.as_global_value().as_pointer_value().into()))
                } else if let Some(global) = self.module.get_global(name) {
                    self.build_load(global.as_pointer_value(), name).map(Some)
                } else {
                    Err(CodeGenError::MissingSymbol(name.clone()))
                }
            }
            TypedExprKind::Array(items) => self.emit_array(items, &expr.ty),
            TypedExprKind::Tuple(items) => self.emit_tuple(items, &expr.ty),
            TypedExprKind::BinOp(lhs, op, rhs) => self.emit_binop(lhs, *op, rhs, &expr.ty),
            TypedExprKind::UnOp(op, value) => self.emit_unop(*op, value, &expr.ty),
            TypedExprKind::Call(callee, args) => self.emit_call(callee, args),
            TypedExprKind::MethodCall(target, method, args) => {
                self.emit_method_call(target, method, args)
            }
            TypedExprKind::Field(target, field) => self.emit_field_load(target, field).map(Some),
            TypedExprKind::Cast(value, to_ty) => self.emit_cast(value, to_ty).map(Some),
            TypedExprKind::Match(subject, arms) => self.emit_match(subject, arms, &expr.ty),
            TypedExprKind::Ok(value) => self.emit_result(value, &expr.ty, true),
            TypedExprKind::Err(value) => self.emit_result(value, &expr.ty, false),
            TypedExprKind::Nullish(lhs, rhs) => self.emit_nullish(lhs, rhs, &expr.ty),
            TypedExprKind::Map(_)
            | TypedExprKind::Set(_)
            | TypedExprKind::Index(_, _)
            | TypedExprKind::Lambda(_, _)
            | TypedExprKind::Chan(_) => {
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
                unsafe {
                    self.builder
                        .build_gep(ptr, &[idx], "index.gep")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))
                }
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
        let mut rendered = String::new();
        for part in parts {
            match part {
                TypedFStrPart::Literal(text) => rendered.push_str(text),
                TypedFStrPart::Interp(expr) => rendered.push_str(&format!("<{}>", expr.ty)),
            }
        }
        self.emit_string_literal(&rendered)
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
                self.builder
                    .build_gep(
                        data_ptr,
                        &[self.context.i64_type().const_int(index as u64, false)],
                        "array.slot",
                    )
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?
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
        let lhs = self
            .emit_expr(lhs)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("binop lhs missing value".to_string()))?;
        let rhs = self
            .emit_expr(rhs)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("binop rhs missing value".to_string()))?;
        let value = match ty {
            Type::Float | Type::Float32 | Type::Float64 => {
                let lhs = lhs.into_float_value();
                let rhs = rhs.into_float_value();
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
            _ => {
                let lhs = lhs.into_int_value();
                let rhs = rhs.into_int_value();
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
            UnOp::Deref => self
                .build_load(value.into_pointer_value(), "deref")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
        };
        Ok(Some(result))
    }

    fn emit_call(
        &mut self,
        callee: &TypedExpr,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let symbol = match &callee.kind {
            TypedExprKind::Ident(name) => self.resolve_function_symbol(
                name,
                &args.iter().map(|arg| arg.ty.clone()).collect::<Vec<_>>(),
            )?,
            _ => {
                return Err(CodeGenError::UnsupportedExpr(
                    "indirect calls are not lowered".to_string(),
                ))
            }
        };
        if symbol == "print" {
            let value = args
                .first()
                .ok_or_else(|| {
                    CodeGenError::UnsupportedExpr("print requires one argument".to_string())
                })
                .and_then(|arg| {
                    self.emit_expr(arg)?.ok_or_else(|| {
                        CodeGenError::UnsupportedExpr("print arg missing value".to_string())
                    })
                })?;
            let function = self
                .module
                .get_function("draton_print")
                .ok_or_else(|| CodeGenError::MissingSymbol("draton_print".to_string()))?;
            let _ = self
                .builder
                .build_call(function, &[value.into()], "print")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            self.emit_safepoint_poll()?;
            return Ok(None);
        }
        let function = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.clone()))?;
        let args = args
            .iter()
            .map(|arg| {
                self.emit_expr(arg)?.ok_or_else(|| {
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
        self.emit_safepoint_poll()?;
        Ok(call.try_as_basic_value().left())
    }

    fn emit_method_call(
        &mut self,
        target: &TypedExpr,
        method: &str,
        args: &[TypedExpr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let Type::Named(class_name, type_args) = &target.ty else {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "method call on non-class type {}",
                target.ty
            )));
        };
        let runtime_name = if type_args.is_empty() {
            class_name.clone()
        } else {
            mangle_class(class_name, type_args)
        };
        let symbol = self
            .class_layouts
            .get(&runtime_name)
            .and_then(|layout| layout.method_names.get(method))
            .cloned()
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{runtime_name}.{method}")))?;
        let function = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.clone()))?;
        let mut call_args = Vec::new();
        call_args.push(self.emit_expr(target)?.ok_or_else(|| {
            CodeGenError::UnsupportedExpr("method target missing value".to_string())
        })?);
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
        self.emit_safepoint_poll()?;
        Ok(call.try_as_basic_value().left())
    }

    fn emit_field_load(
        &mut self,
        target: &TypedExpr,
        field: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let ptr = self.emit_field_ptr(target, field)?;
        self.build_load(ptr, field)
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
        let layout = self
            .class_layouts
            .get(&runtime_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(runtime_name.clone()))?;
        let index = layout
            .field_indices
            .get(field)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{runtime_name}.{field}")))?;
        let base = self
            .emit_expr(target)?
            .ok_or_else(|| CodeGenError::UnsupportedExpr("field target missing value".to_string()))?
            .into_pointer_value();
        self.builder
            .build_struct_gep(base, index, field)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    fn emit_cast(
        &mut self,
        value: &TypedExpr,
        to_ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
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

        let mut cases = Vec::new();
        let mut arm_blocks = Vec::new();
        for arm in arms {
            if let TypedExprKind::IntLit(value) = arm.pattern.kind {
                let block = self.context.append_basic_block(function, "match.case");
                cases.push((self.context.i64_type().const_int(value as u64, true), block));
                arm_blocks.push((block, arm));
            }
        }
        self.builder
            .build_switch(subject, default, &cases)
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
