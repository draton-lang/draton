use std::collections::BTreeSet;

use draton_typeck::typed_ast::{TypedClassDef, TypedExprKind};
use draton_typeck::{Type, TypedExpr};
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, PointerValue};

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
        let _ = (ty, name);
        panic!("GC alloc removed in Phase 4");
    }

    pub(crate) fn emit_owned_alloc(
        &mut self,
        ty: &Type,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let alloc = self
            .module
            .get_function("malloc")
            .ok_or_else(|| CodeGenError::MissingSymbol("malloc".to_string()))?;
        let size = self.type_size_value(ty)?;
        let raw = self
            .builder
            .build_call(alloc, &[size.into()], name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodeGenError::Llvm("malloc returned void".to_string()))?
            .into_pointer_value();
        let typed_ptr = match self.llvm_basic_type(ty)? {
            BasicTypeEnum::PointerType(ptr_ty) => {
                self.build_pointer_cast_to(raw, ptr_ty.into(), &format!("{name}.typed"))?
            }
            other => {
                return Err(CodeGenError::UnsupportedType(format!(
                    "owned allocation requires pointer-backed type, got {other:?}"
                )));
            }
        };
        Ok(typed_ptr)
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
        // Compute the actual byte offset of each GC-traceable pointer within the struct.
        // We must account for the real LLVM layout: each field may be more than 8 bytes
        // (e.g. String = { i64, i8* } is 16 bytes, Array = { i64, T** } is 16 bytes).
        let struct_fields = layout.struct_type.get_field_types();
        let pointer_offsets = {
            let mut offsets: Vec<u32> = Vec::new();
            for field in &class_def.fields {
                let field_idx = match layout.field_indices.get(&field.name).copied() {
                    Some(idx) => idx,
                    None => continue,
                };
                // Compute byte offset of this field by summing sizes of all preceding fields.
                let field_byte_offset: u32 = struct_fields
                    .iter()
                    .take(field_idx as usize)
                    .map(|ft| self.basic_type_size_bytes(*ft) as u32)
                    .sum();
                // Now determine what GC pointers are inside this field.
                Self::collect_gc_pointer_offsets_for_field(
                    &field.ty,
                    field_byte_offset,
                    &mut offsets,
                );
            }
            offsets
        };
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

    /// Emit a module constructor that registers the type descriptor with the
    /// runtime GC at program startup.
    fn emit_type_registration_ctor(
        &mut self,
        class_name: &str,
        type_id: u16,
        size: u64,
        pointer_offsets: &[u32],
    ) -> Result<(), CodeGenError> {
        let _ = (class_name, type_id, size, pointer_offsets);
        Ok(())
    }

    /// Queue `function` to be added to the module's `@llvm.global_ctors` array.
    /// The actual global is emitted as a single entry by `flush_global_ctors`
    /// at the end of codegen, avoiding the "unknown special variable" LLVM crash
    /// that occurs when multiple @llvm.global_ctors.N globals exist in one module.
    fn add_global_ctor(
        &mut self,
        function: inkwell::values::FunctionValue<'ctx>,
    ) -> Result<(), CodeGenError> {
        let _ = function;
        Ok(())
    }

    /// Push the byte offsets of all GC-managed heap pointers contained within a
    /// struct field of Draton type `ty` that starts at `field_byte_offset`.
    ///
    /// - `Named`/`Chan`/`Fn`/`Pointer` → single 8-byte pointer at the field start
    /// - `Array(inner)` / `Set(inner)` → the data pointer (`inner**`) at offset +8
    ///   if `inner` is itself a GC pointer type
    /// - `Map(k, v)` → key data pointer at +8 and value data pointer at +16 if GC types
    /// - `String` → NOT GC-managed (Rust allocator); no offsets
    /// - `Option(inner)` / `Result` / `Tuple` → recurse into the contained GC pointer
    ///   fields (treated as embedded values with adjusted offsets)
    fn collect_gc_pointer_offsets_for_field(ty: &Type, field_byte_offset: u32, out: &mut Vec<u32>) {
        match ty {
            // Simple 8-byte GC heap pointer.
            Type::Named(_, _) | Type::Chan(_) | Type::Pointer(_) | Type::Fn(_, _) => {
                out.push(field_byte_offset);
            }
            // Array[T] = { i64 len, T** data } — data pointer at +8 if T is GC type.
            Type::Array(inner) | Type::Set(inner) => {
                if Self::is_gc_pointer_type(inner) {
                    out.push(field_byte_offset + 8);
                }
            }
            // Map[K,V] = { i64 len, K** keys, V** vals } — pointers at +8 and +16.
            Type::Map(key, val) => {
                if Self::is_gc_pointer_type(key) {
                    out.push(field_byte_offset + 8);
                }
                if Self::is_gc_pointer_type(val) {
                    out.push(field_byte_offset + 16);
                }
            }
            // String = { i64, i8* } — string data is Rust-heap, not GC-managed.
            Type::String => {}
            // Option[T] = { bool, T } — inner value at offset +8 (aligned after bool+padding).
            Type::Option(inner) => {
                // LLVM aligns the inner value to its natural alignment.  For an 8-byte
                // pointer the layout is { i1, (7 bytes padding), ptr } so the inner value
                // starts at offset 8.
                Self::collect_gc_pointer_offsets_for_field(inner, field_byte_offset + 8, out);
            }
            // Result[Ok,Err] = { bool, Ok, Err } — both payloads follow the tag.
            Type::Result(ok, err) => {
                // Conservative: assume Ok starts at +8 and Err follows.
                Self::collect_gc_pointer_offsets_for_field(ok, field_byte_offset + 8, out);
                // Err starts after the Ok payload; size of Ok is 8 bytes for a pointer.
                let ok_size: u32 = if Self::is_gc_pointer_type(ok) { 8 } else { 0 };
                Self::collect_gc_pointer_offsets_for_field(
                    err,
                    field_byte_offset + 8 + ok_size,
                    out,
                );
            }
            _ => {}
        }
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

    pub(crate) fn basic_type_size_bytes(&self, ty: BasicTypeEnum<'ctx>) -> u64 {
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
        let _ = (object_ptr, field_ptr, value);
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
            use_effect: None,
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
            use_effect: None,
        };
        assert!(escapes(&expr, &FnScope::default()));
    }
}
