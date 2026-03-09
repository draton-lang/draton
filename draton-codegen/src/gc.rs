use draton_typeck::{Type, TypedExpr};
use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, PointerValue};
use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;

/// Conservative function-level escape information.
#[derive(Debug, Clone, Default)]
pub struct FnScope;

/// GC metadata for a lowered heap type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDescriptor {
    pub size: u32,
    pub pointer_offsets: Vec<u32>,
}

/// Returns whether the expression conservatively escapes the current function.
pub fn escapes(_expr: &TypedExpr, _fn_scope: &FnScope) -> bool {
    true
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
        let size = match ty {
            Type::Named(class, _) => self
                .class_layouts
                .get(class)
                .and_then(|layout| layout.struct_type.size_of())
                .ok_or_else(|| CodeGenError::UnsupportedType(format!("size_of({class})")))?,
            _ => self
                .llvm_basic_type(ty)?
                .size_of()
                .ok_or_else(|| CodeGenError::UnsupportedType(format!("size_of({ty})")))?,
        };
        let raw = self
            .builder
            .build_call(
                alloc,
                &[size.into(), self.context.i16_type().const_zero().into()],
                name,
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| CodeGenError::Llvm("draton_gc_alloc returned void".to_string()))?
            .into_pointer_value();
        Ok(raw)
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
