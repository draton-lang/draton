use std::collections::BTreeSet;

use draton_typeck::typed_ast::{TypedClassDef, TypedExprKind};
use draton_typeck::{Type, TypedExpr};
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, PointerValue};
use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;
use crate::mangle::mangle_class;

/// Conservative function-level escape information.
#[derive(Debug, Clone, Default)]
pub struct FnScope {
    local_expr_starts: BTreeSet<usize>,
}

impl FnScope {
    /// Marks an expression as provably local to the current function scope.
    pub fn mark_local_expr(&mut self, expr: &TypedExpr) {
        self.local_expr_starts.insert(expr.span.start);
    }

    /// Returns whether the expression is known to stay local to the current scope.
    pub fn is_purely_local(&self, expr: &TypedExpr) -> bool {
        self.local_expr_starts.contains(&expr.span.start)
    }
}

/// GC metadata for a lowered heap type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    pub size: u32,
    pub pointer_offsets: Vec<u32>,
}

/// Returns whether the expression conservatively escapes the current function.
pub fn escapes(expr: &TypedExpr, fn_scope: &FnScope) -> bool {
    match &expr.kind {
        TypedExprKind::Array(_)
        | TypedExprKind::Tuple(_)
        | TypedExprKind::Call(_, _)
        | TypedExprKind::MethodCall(_, _, _) => !fn_scope.is_purely_local(expr),
        _ => true,
    }
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn emit_gc_alloc_for_type(
        &self,
        ty: &Type,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let alloc = self
            .module
            .get_function("draton_gc_alloc")
            .ok_or_else(|| CodeGenError::MissingSymbol("draton_gc_alloc".to_string()))?;
        let size = self.type_size_value(ty)?;
        let type_id = match ty {
            Type::Named(class_name, args) => self
                .type_descriptor_table
                .get(&if args.is_empty() {
                    class_name.clone()
                } else {
                    mangle_class(class_name, args)
                })
                .copied()
                .unwrap_or(0),
            _ => 0,
        };
        let raw = self
            .builder
            .build_call(
                alloc,
                &[
                    size.into(),
                    self.context
                        .i16_type()
                        .const_int(type_id as u64, false)
                        .into(),
                ],
                name,
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| CodeGenError::Llvm("draton_gc_alloc returned void".to_string()))?
            .into_pointer_value();
        Ok(raw)
    }

    pub(crate) fn emit_class_type_descriptor(
        &mut self,
        class_def: &TypedClassDef,
    ) -> Result<u16, CodeGenError> {
        if let Some(type_id) = self.type_descriptor_table.get(&class_def.name).copied() {
            return Ok(type_id);
        }
        let layout = self
            .class_layouts
            .get(&class_def.name)
            .ok_or_else(|| CodeGenError::MissingSymbol(class_def.name.clone()))?;
        let size = self.type_size_bytes(&Type::Named(class_def.name.clone(), Vec::new()))?;
        let pointer_offsets = class_def
            .fields
            .iter()
            .filter(|field| Self::is_gc_pointer_type(&field.ty))
            .map(|field| {
                layout
                    .field_indices
                    .get(&field.name)
                    .copied()
                    .unwrap_or(0)
                    .saturating_mul(8)
            })
            .collect::<Vec<_>>();
        let i32_type = self.context.i32_type();
        let offsets_ty = i32_type.array_type(pointer_offsets.len() as u32);
        let offsets = i32_type.const_array(
            &pointer_offsets
                .iter()
                .map(|offset| i32_type.const_int(u64::from(*offset), false))
                .collect::<Vec<_>>(),
        );
        let descriptor_ty = self.context.struct_type(
            &[i32_type.into(), i32_type.into(), offsets_ty.into()],
            false,
        );
        let descriptor = descriptor_ty.const_named_struct(&[
            i32_type.const_int(size, false).into(),
            i32_type
                .const_int(pointer_offsets.len() as u64, false)
                .into(),
            offsets.into(),
        ]);
        let global_name = format!("TypeDesc_{}", class_def.name);
        if self.module.get_global(&global_name).is_none() {
            let global = self.module.add_global(descriptor_ty, None, &global_name);
            global.set_constant(true);
            global.set_initializer(&descriptor);
        }
        let type_id = self.next_type_descriptor_id;
        self.next_type_descriptor_id = self.next_type_descriptor_id.saturating_add(1);
        self.type_descriptor_table
            .insert(class_def.name.clone(), type_id);
        Ok(type_id)
    }

    fn type_size_value(&self, ty: &Type) -> Result<inkwell::values::IntValue<'ctx>, CodeGenError> {
        Ok(self
            .context
            .i64_type()
            .const_int(self.type_size_bytes(ty)?, false))
    }

    fn type_size_bytes(&self, ty: &Type) -> Result<u64, CodeGenError> {
        match ty {
            Type::Named(class_name, args) => {
                let runtime_name = if args.is_empty() {
                    class_name.clone()
                } else {
                    mangle_class(class_name, args)
                };
                let layout = self
                    .class_layouts
                    .get(&runtime_name)
                    .ok_or_else(|| CodeGenError::MissingSymbol(runtime_name.clone()))?;
                let size = layout
                    .struct_type
                    .size_of()
                    .and_then(|value| value.get_zero_extended_constant())
                    .unwrap_or_else(|| {
                        layout
                            .struct_type
                            .get_field_types()
                            .into_iter()
                            .map(|field| self.basic_type_size_bytes(field))
                            .sum::<u64>()
                    });
                Ok(size.max(1))
            }
            _ => {
                let basic = self.llvm_basic_type(ty)?;
                Ok(self.basic_type_size_bytes(basic).max(1))
            }
        }
    }

    fn basic_type_size_bytes(&self, ty: BasicTypeEnum<'ctx>) -> u64 {
        ty.size_of()
            .and_then(|value| value.get_zero_extended_constant())
            .unwrap_or_else(|| match ty {
                BasicTypeEnum::StructType(struct_ty) => struct_ty
                    .get_field_types()
                    .into_iter()
                    .map(|field| self.basic_type_size_bytes(field))
                    .sum::<u64>(),
                BasicTypeEnum::ArrayType(array_ty) => {
                    let len = u64::from(array_ty.len());
                    len.saturating_mul(self.basic_type_size_bytes(array_ty.get_element_type()))
                }
                _ => 8,
            })
    }

    pub(crate) fn emit_gc_write_barrier(
        &self,
        object_ptr: PointerValue<'ctx>,
        field_ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) -> Result<(), CodeGenError> {
        let value_ptr = if let BasicValueEnum::PointerValue(ptr) = value {
            ptr
        } else {
            return Ok(());
        };
        let barrier = self
            .module
            .get_function("draton_gc_write_barrier")
            .ok_or_else(|| CodeGenError::MissingSymbol("draton_gc_write_barrier".to_string()))?;
        let cast_obj = self
            .builder
            .build_bitcast(
                object_ptr,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "gc.obj",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let cast_field = self
            .builder
            .build_bitcast(
                field_ptr,
                self.context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .ptr_type(AddressSpace::default()),
                "gc.field",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let cast_val = self
            .builder
            .build_bitcast(
                value_ptr,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "gc.val",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let _ = self
            .builder
            .build_call(
                barrier,
                &[cast_obj.into(), cast_field.into(), cast_val.into()],
                "",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }

    pub(crate) fn emit_safepoint_poll(&self) -> Result<(), CodeGenError> {
        let Some(flag) = self.module.get_global("draton_safepoint_flag") else {
            return Ok(());
        };
        let current_fn = self.current_function()?;
        let continue_block = self
            .context
            .append_basic_block(current_fn, "safepoint.cont");
        let slow_block = self
            .context
            .append_basic_block(current_fn, "safepoint.slow");
        let flag_value = self
            .builder
            .build_load(flag.as_pointer_value(), "safepoint.flag")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let needs_stop = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::NE,
                flag_value,
                self.context.i32_type().const_zero(),
                "safepoint.need_stop",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_conditional_branch(needs_stop, slow_block, continue_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder.position_at_end(slow_block);
        let slow = self
            .module
            .get_function("draton_safepoint_slow")
            .ok_or_else(|| CodeGenError::MissingSymbol("draton_safepoint_slow".to_string()))?;
        let _ = self
            .builder
            .build_call(slow, &[], "")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_unconditional_branch(continue_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder.position_at_end(continue_block);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use draton_ast::Span;
    use draton_typeck::{Type, TypedExpr, TypedExprKind};

    use super::{escapes, FnScope};

    #[test]
    fn marks_known_local_allocations_as_non_escaping() {
        let expr = TypedExpr {
            kind: TypedExprKind::Array(Vec::new()),
            ty: Type::Array(Box::new(Type::Int)),
            span: Span {
                start: 1,
                end: 2,
                line: 1,
                col: 1,
            },
        };
        let mut scope = FnScope::default();
        scope.mark_local_expr(&expr);
        assert!(!escapes(&expr, &scope));
    }

    #[test]
    fn treats_unknown_allocations_as_escaping() {
        let expr = TypedExpr {
            kind: TypedExprKind::Array(Vec::new()),
            ty: Type::Array(Box::new(Type::Int)),
            span: Span {
                start: 10,
                end: 11,
                line: 1,
                col: 10,
            },
        };
        assert!(escapes(&expr, &FnScope::default()));
    }
}
