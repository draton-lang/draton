use draton_typeck::{
    typed_ast::{TypedClassDef, TypedInterfaceDef},
    Type, TypedFnDef, TypedItem, TypedProgram,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;
use std::collections::HashMap;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;
use crate::mangle::mangle_class;

/// The method contract exposed by an interface vtable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    pub name: String,
    pub param_types: Vec<Type>,
    pub ret_type: Type,
}

impl MethodSig {
    /// Builds a method signature from a typed function declaration.
    pub fn from_fn_def(function: &TypedFnDef) -> Self {
        Self {
            name: function.name.clone(),
            param_types: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            ret_type: function.ret_type.clone(),
        }
    }
}

/// Tracks all interface definitions and their implementors for code generation.
#[derive(Debug, Clone, Default)]
pub struct InterfaceRegistry {
    pub interface_defs: HashMap<String, TypedInterfaceDef>,
    pub interface_methods: HashMap<String, Vec<MethodSig>>,
    pub implementors: HashMap<String, Vec<String>>,
}

impl InterfaceRegistry {
    /// Builds a registry from the typed program.
    pub fn build(program: &TypedProgram) -> Self {
        let mut registry = Self::default();
        for item in &program.items {
            match item {
                TypedItem::Interface(interface_def) => {
                    registry.interface_methods.insert(
                        interface_def.name.clone(),
                        interface_def
                            .methods
                            .iter()
                            .map(MethodSig::from_fn_def)
                            .collect(),
                    );
                    registry
                        .interface_defs
                        .insert(interface_def.name.clone(), interface_def.clone());
                }
                TypedItem::Class(class_def) => {
                    for interface in &class_def.implements {
                        registry
                            .implementors
                            .entry(interface.clone())
                            .or_default()
                            .push(class_def.name.clone());
                    }
                }
                TypedItem::Fn(_)
                | TypedItem::Enum(_)
                | TypedItem::Error(_)
                | TypedItem::Const(_)
                | TypedItem::Import(_)
                | TypedItem::Extern(_)
                | TypedItem::TypeBlock(_)
                | TypedItem::PanicHandler(_)
                | TypedItem::OomHandler(_) => {}
            }
        }
        registry
    }
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn emit_interface_runtime_types(&mut self) -> Result<(), CodeGenError> {
        self.emit_interface_vtable_types()?;
        self.emit_fat_pointer_types();
        Ok(())
    }

    pub(crate) fn emit_interface_vtable_types(&mut self) -> Result<(), CodeGenError> {
        let interfaces = self.iface_registry.interface_methods.clone();
        for (iface_name, methods) in interfaces {
            if self.vtable_types.contains_key(&iface_name) {
                continue;
            }
            let vtable_type = self
                .module
                .get_struct_type(&format!("{iface_name}_vtable"))
                .unwrap_or_else(|| {
                    self.context
                        .opaque_struct_type(&format!("{iface_name}_vtable"))
                });
            let fields = methods
                .iter()
                .map(|method| {
                    self.interface_dispatch_function_type(method)
                        .map(|ty| ty.ptr_type(AddressSpace::default()).into())
                })
                .collect::<Result<Vec<BasicTypeEnum<'ctx>>, _>>()?;
            vtable_type.set_body(&fields, false);
            self.vtable_types.insert(iface_name, vtable_type);
        }
        Ok(())
    }

    pub(crate) fn emit_fat_pointer_types(&mut self) {
        let interfaces = self.vtable_types.clone();
        for (iface_name, vtable_type) in interfaces {
            if self.fat_pointer_types.contains_key(&iface_name) {
                continue;
            }
            let fat_ptr_type = self
                .module
                .get_struct_type(&iface_name)
                .unwrap_or_else(|| self.context.opaque_struct_type(&iface_name));
            fat_ptr_type.set_body(
                &[
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::default())
                        .into(),
                    vtable_type.ptr_type(AddressSpace::default()).into(),
                ],
                false,
            );
            let anchor_name = format!("__draton_iface_{}_anchor", iface_name);
            if self.module.get_global(&anchor_name).is_none() {
                let anchor = self.module.add_global(fat_ptr_type, None, &anchor_name);
                anchor.set_initializer(&fat_ptr_type.const_zero());
                anchor.set_constant(true);
                anchor.set_linkage(inkwell::module::Linkage::Private);
            }
            self.fat_pointer_types.insert(iface_name, fat_ptr_type);
        }
    }

    pub(crate) fn predeclare_vtable_bindings_for_class(
        &mut self,
        class_def: &TypedClassDef,
    ) -> Result<(), CodeGenError> {
        if class_def.implements.is_empty() {
            return Ok(());
        }
        for iface_name in &class_def.implements {
            let Some(methods) = self
                .iface_registry
                .interface_methods
                .get(iface_name)
                .cloned()
            else {
                continue;
            };
            let vtable_type = *self
                .vtable_types
                .get(iface_name)
                .ok_or_else(|| CodeGenError::MissingSymbol(format!("{iface_name}_vtable")))?;
            let mut thunk_ptrs = Vec::with_capacity(methods.len());
            for method in &methods {
                let thunk_name = self.vtable_thunk_name(&class_def.name, iface_name, &method.name);
                let thunk = self.predeclare_vtable_thunk(&class_def.name, &thunk_name, method)?;
                thunk_ptrs.push(thunk.as_global_value().as_pointer_value().into());
            }
            let global_name = format!("{}_{}_vtable", class_def.name, iface_name);
            let global = self
                .module
                .get_global(&global_name)
                .unwrap_or_else(|| self.module.add_global(vtable_type, None, &global_name));
            let init = vtable_type.const_named_struct(&thunk_ptrs);
            global.set_initializer(&init);
            global.set_constant(true);
            self.vtable_globals
                .insert((class_def.name.clone(), iface_name.to_string()), global);
        }
        Ok(())
    }

    pub(crate) fn emit_vtable_for_class(
        &mut self,
        class_def: &TypedClassDef,
    ) -> Result<(), CodeGenError> {
        for iface_name in &class_def.implements {
            let Some(methods) = self
                .iface_registry
                .interface_methods
                .get(iface_name)
                .cloned()
            else {
                continue;
            };
            for method in methods {
                let thunk_name = self.vtable_thunk_name(&class_def.name, iface_name, &method.name);
                let _ = self.emit_vtable_thunk(&class_def.name, &thunk_name, &method)?;
            }
        }
        Ok(())
    }

    pub(crate) fn emit_upcast_to_interface(
        &mut self,
        value: PointerValue<'ctx>,
        class_name: &str,
        iface_name: &str,
    ) -> Result<inkwell::values::StructValue<'ctx>, CodeGenError> {
        let (value, implementor) =
            self.resolve_interface_receiver(value, class_name, iface_name)?;
        let fat_ptr_type = *self
            .fat_pointer_types
            .get(iface_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(iface_name.to_string()))?;
        let vtable_global = *self
            .vtable_globals
            .get(&(implementor.clone(), iface_name.to_string()))
            .ok_or_else(|| {
                CodeGenError::MissingSymbol(format!("{implementor}_{iface_name}_vtable"))
            })?;
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let data_ptr = self
            .builder
            .build_bit_cast(value, i8_ptr, "iface.data")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        self.build_struct_value(
            fat_ptr_type,
            &[data_ptr.into(), vtable_global.as_pointer_value().into()],
            "iface.upcast",
        )
    }

    fn resolve_interface_receiver(
        &mut self,
        value: PointerValue<'ctx>,
        class_name: &str,
        iface_name: &str,
    ) -> Result<(PointerValue<'ctx>, String), CodeGenError> {
        let mut current_ptr = value;
        let mut current_class = class_name.to_string();
        loop {
            if self
                .vtable_globals
                .contains_key(&(current_class.clone(), iface_name.to_string()))
            {
                return Ok((current_ptr, current_class));
            }
            let parent_class = self
                .class_layouts
                .get(&current_class)
                .and_then(|layout| layout.parent_class.clone())
                .ok_or_else(|| {
                    CodeGenError::MissingSymbol(format!("{class_name}_{iface_name}_vtable"))
                })?;
            let current_layout = self
                .class_layouts
                .get(&current_class)
                .ok_or_else(|| CodeGenError::MissingSymbol(current_class.clone()))?;
            current_ptr =
                self.build_struct_gep(current_layout.struct_type, current_ptr, 0, "iface.parent")?;
            current_class = parent_class;
        }
    }

    pub(crate) fn emit_interface_method_call(
        &mut self,
        fat_ptr: inkwell::values::StructValue<'ctx>,
        iface_name: &str,
        method_name: &str,
        args: &[draton_typeck::TypedExpr],
    ) -> Result<Option<inkwell::values::BasicValueEnum<'ctx>>, CodeGenError> {
        let methods = self
            .iface_registry
            .interface_methods
            .get(iface_name)
            .cloned()
            .ok_or_else(|| CodeGenError::MissingSymbol(iface_name.to_string()))?;
        let method_index = methods
            .iter()
            .position(|method| method.name == method_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{iface_name}.{method_name}")))?;
        let data_ptr = self
            .builder
            .build_extract_value(fat_ptr, 0, "iface.data")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let vtable_ptr = self
            .builder
            .build_extract_value(fat_ptr, 1, "iface.vtable")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let vtable_type = *self
            .vtable_types
            .get(iface_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{iface_name}_vtable")))?;
        let fn_slot = self.build_struct_gep(
            vtable_type,
            vtable_ptr,
            method_index as u32,
            "iface.fn.slot",
        )?;
        let fn_ty = self.interface_dispatch_function_type(&methods[method_index])?;
        let fn_ptr = self
            .builder
            .build_load(fn_ty.ptr_type(AddressSpace::default()), fn_slot, "iface.fn")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let mut call_args = vec![BasicMetadataValueEnum::from(data_ptr)];
        for arg in args {
            let value = self.emit_expr(arg)?.ok_or_else(|| {
                CodeGenError::UnsupportedExpr("interface arg missing value".to_string())
            })?;
            call_args.push(value.into());
        }
        let call = self
            .builder
            .build_indirect_call(fn_ty, fn_ptr, &call_args, "iface.call")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(call.try_as_basic_value().basic())
    }

    pub(crate) fn interface_dispatch_function_type(
        &self,
        method: &MethodSig,
    ) -> Result<inkwell::types::FunctionType<'ctx>, CodeGenError> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let mut params = vec![BasicMetadataTypeEnum::from(i8_ptr)];
        params.extend(
            method
                .param_types
                .iter()
                .skip(1)
                .map(|ty| self.llvm_payload_type(ty))
                .collect::<Result<Vec<_>, _>>()?,
        );
        if Self::is_void_type(&method.ret_type) {
            Ok(self.context.void_type().fn_type(&params, false))
        } else {
            Ok(self
                .llvm_basic_type(&method.ret_type)?
                .fn_type(&params, false))
        }
    }

    fn predeclare_vtable_thunk(
        &mut self,
        class_name: &str,
        thunk_name: &str,
        method: &MethodSig,
    ) -> Result<FunctionValue<'ctx>, CodeGenError> {
        if let Some(function) = self.module.get_function(thunk_name) {
            return Ok(function);
        }
        let fn_type = self.interface_dispatch_function_type(method)?;
        let function = self.module.add_function(thunk_name, fn_type, None);
        if let Some(layout) = self.class_layouts.get_mut(class_name) {
            layout
                .method_names
                .entry(format!("{}#thunk", method.name))
                .or_insert_with(|| thunk_name.to_string());
        }
        Ok(function)
    }

    fn emit_vtable_thunk(
        &mut self,
        class_name: &str,
        thunk_name: &str,
        method: &MethodSig,
    ) -> Result<FunctionValue<'ctx>, CodeGenError> {
        let function = self
            .module
            .get_function(thunk_name)
            .unwrap_or(self.predeclare_vtable_thunk(class_name, thunk_name, method)?);
        if function.get_first_basic_block().is_some() {
            return Ok(function);
        }
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let raw_self = function
            .get_first_param()
            .ok_or_else(|| CodeGenError::MissingSymbol(format!("{thunk_name}:self")))?
            .into_pointer_value();
        let class_layout = self
            .class_layouts
            .get(class_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(class_name.to_string()))?;
        let typed_self = self
            .builder
            .build_bit_cast(
                raw_self,
                class_layout.struct_type.ptr_type(AddressSpace::default()),
                "self.typed",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let concrete_method_name = format!("{class_name}.{}", method.name);
        let concrete_method = self
            .module
            .get_function(&concrete_method_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(concrete_method_name.clone()))?;
        let mut args = vec![BasicMetadataValueEnum::from(typed_self)];
        args.extend(
            function
                .get_params()
                .into_iter()
                .skip(1)
                .map(BasicMetadataValueEnum::from),
        );
        let call = self
            .builder
            .build_call(concrete_method, &args, "iface.thunk")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        if Self::is_void_type(&method.ret_type) {
            self.builder
                .build_return(None)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        } else {
            let value = call
                .try_as_basic_value()
                .basic()
                .ok_or_else(|| CodeGenError::Llvm("thunk expected return value".to_string()))?;
            self.builder
                .build_return(Some(&value))
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        Ok(function)
    }

    fn vtable_thunk_name(&self, class_name: &str, iface_name: &str, method_name: &str) -> String {
        format!("{class_name}.{iface_name}.{method_name}_thunk")
    }

    pub(crate) fn is_interface_type_name(&self, name: &str) -> bool {
        self.iface_registry.interface_methods.contains_key(name)
    }

    pub(crate) fn runtime_class_name(&self, class_name: &str, args: &[Type]) -> String {
        if args.is_empty() {
            class_name.to_string()
        } else {
            mangle_class(class_name, args)
        }
    }
}
