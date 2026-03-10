use draton_typeck::Type;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{BasicValueEnum, IntValue, StructValue};
use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;
use crate::mangle::mangle_class;

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn is_void_type(ty: &Type) -> bool {
        matches!(ty, Type::Unit | Type::Never)
    }

    pub(crate) fn llvm_basic_type(&self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, CodeGenError> {
        match ty {
            Type::Int | Type::Int64 | Type::UInt64 => Ok(self.context.i64_type().into()),
            Type::Int8 | Type::UInt8 => Ok(self.context.i8_type().into()),
            Type::Int16 | Type::UInt16 => Ok(self.context.i16_type().into()),
            Type::Int32 | Type::UInt32 => Ok(self.context.i32_type().into()),
            Type::Float | Type::Float64 => Ok(self.context.f64_type().into()),
            Type::Float32 => Ok(self.context.f32_type().into()),
            Type::Bool => Ok(self.context.bool_type().into()),
            Type::Char => Ok(self.context.i8_type().into()),
            Type::String => Ok(self.string_type.into()),
            Type::Array(inner) => Ok(self
                .context
                .struct_type(
                    &[
                        self.context.i64_type().into(),
                        self.llvm_basic_type(inner)?
                            .ptr_type(AddressSpace::default())
                            .into(),
                    ],
                    false,
                )
                .into()),
            Type::Map(key, value) => Ok(self
                .context
                .struct_type(
                    &[
                        self.context.i64_type().into(),
                        self.llvm_basic_type(key)?
                            .ptr_type(AddressSpace::default())
                            .into(),
                        self.llvm_basic_type(value)?
                            .ptr_type(AddressSpace::default())
                            .into(),
                    ],
                    false,
                )
                .into()),
            Type::Set(inner) => Ok(self
                .context
                .struct_type(
                    &[
                        self.context.i64_type().into(),
                        self.llvm_basic_type(inner)?
                            .ptr_type(AddressSpace::default())
                            .into(),
                    ],
                    false,
                )
                .into()),
            Type::Tuple(items) => Ok(self
                .context
                .struct_type(
                    &items
                        .iter()
                        .map(|item| self.llvm_basic_type(item))
                        .collect::<Result<Vec<_>, _>>()?,
                    false,
                )
                .into()),
            Type::Option(inner) => Ok(self
                .context
                .struct_type(
                    &[
                        self.context.bool_type().into(),
                        self.llvm_storage_type(inner)?,
                    ],
                    false,
                )
                .into()),
            Type::Result(ok, err) => Ok(self
                .context
                .struct_type(
                    &[
                        self.context.bool_type().into(),
                        self.llvm_storage_type(ok)?,
                        self.llvm_storage_type(err)?,
                    ],
                    false,
                )
                .into()),
            Type::Chan(_) => Ok(self
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .into()),
            Type::Fn(_, _) => Ok(self
                .closure_record_type
                .ptr_type(AddressSpace::default())
                .into()),
            Type::Named(name, args) if args.is_empty() && self.is_interface_type_name(name) => self
                .fat_pointer_types
                .get(name)
                .copied()
                .map(Into::into)
                .ok_or_else(|| {
                    CodeGenError::UnsupportedType(format!("unknown interface type {name}"))
                }),
            Type::Named(name, args) => {
                let runtime_name = if args.is_empty() {
                    name.clone()
                } else {
                    mangle_class(name, args)
                };
                self.class_layouts
                    .get(&runtime_name)
                    .map(|layout| layout.struct_type.ptr_type(AddressSpace::default()).into())
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedType(format!("unknown named type {runtime_name}"))
                    })
            }
            Type::Pointer(inner) => Ok(self
                .llvm_storage_type(inner)?
                .ptr_type(AddressSpace::default())
                .into()),
            Type::Var(id) => Err(CodeGenError::UnsupportedType(format!(
                "unresolved type variable t{id}"
            ))),
            Type::Row { .. } => Err(CodeGenError::UnsupportedType(
                "row types are not materialized at runtime".to_string(),
            )),
            Type::Unit | Type::Never => Err(CodeGenError::UnsupportedType(format!(
                "{ty} has no basic LLVM representation"
            ))),
        }
    }

    pub(crate) fn llvm_payload_type(
        &self,
        ty: &Type,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, CodeGenError> {
        Ok(self.llvm_storage_type(ty)?.into())
    }

    pub(crate) fn llvm_storage_type(&self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, CodeGenError> {
        if Self::is_void_type(ty) {
            Ok(self.context.i8_type().into())
        } else {
            self.llvm_basic_type(ty)
        }
    }

    pub(crate) fn llvm_function_type(
        &self,
        ret: &Type,
        params: &[Type],
    ) -> Result<FunctionType<'ctx>, CodeGenError> {
        let param_types = params
            .iter()
            .map(|param| self.llvm_payload_type(param))
            .collect::<Result<Vec<_>, _>>()?;
        if Self::is_void_type(ret) {
            Ok(self.context.void_type().fn_type(&param_types, false))
        } else {
            Ok(self.llvm_basic_type(ret)?.fn_type(&param_types, false))
        }
    }

    pub(crate) fn llvm_method_function_type(
        &self,
        class_name: &str,
        ret: &Type,
        params: &[Type],
    ) -> Result<FunctionType<'ctx>, CodeGenError> {
        let self_ty = Type::Named(class_name.to_string(), Vec::new());
        let mut all_params = vec![self.llvm_payload_type(&self_ty)?];
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

    pub(crate) fn zero_value(&self, ty: &Type) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        Ok(self.llvm_basic_type(ty)?.const_zero())
    }

    pub(crate) fn build_struct_value(
        &self,
        ty: inkwell::types::StructType<'ctx>,
        fields: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> Result<StructValue<'ctx>, CodeGenError> {
        let mut current = ty.get_undef();
        for (index, value) in fields.iter().enumerate() {
            current = self
                .builder
                .build_insert_value(current, *value, index as u32, &format!("{name}.{index}"))
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .into_struct_value();
        }
        Ok(current)
    }

    pub(crate) fn to_bool_value(
        &self,
        value: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, CodeGenError> {
        match value {
            BasicValueEnum::IntValue(int) => Ok(int),
            other => Err(CodeGenError::UnsupportedType(format!(
                "expected boolean/int value, found {:?}",
                other.get_type()
            ))),
        }
    }

    pub(crate) fn is_gc_pointer_type(ty: &Type) -> bool {
        match ty {
            Type::String
            | Type::Array(_)
            | Type::Map(_, _)
            | Type::Set(_)
            | Type::Chan(_)
            | Type::Named(_, _)
            | Type::Pointer(_)
            | Type::Fn(_, _) => true,
            Type::Option(inner) => Self::is_gc_pointer_type(inner),
            Type::Result(ok, err) => Self::is_gc_pointer_type(ok) || Self::is_gc_pointer_type(err),
            Type::Tuple(items) => items.iter().any(Self::is_gc_pointer_type),
            _ => false,
        }
    }
}
