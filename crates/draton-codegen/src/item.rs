use std::collections::HashMap;

use draton_typeck::{Type, TypedFnDef, TypedItem, TypedProgram};

use crate::codegen::{ClassLayout, CodeGen};
use crate::error::CodeGenError;
use crate::mono::{specialize_class, specialize_function};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn predeclare_program_items(
        &mut self,
        program: &TypedProgram,
    ) -> Result<(), CodeGenError> {
        for item in &program.items {
            if let TypedItem::Class(class_def) = item {
                self.trace(format!("predeclare:class-opaque {}", class_def.name));
                if self.generic_classes.contains_key(&class_def.name) {
                    continue;
                }
                let struct_type = self.context.opaque_struct_type(&class_def.name);
                self.class_layouts.insert(
                    class_def.name.clone(),
                    ClassLayout {
                        struct_type,
                        field_indices: HashMap::new(),
                        method_names: HashMap::new(),
                        parent_class: class_def.extends.clone(),
                    },
                );
            }
        }

        for item in &program.items {
            if let TypedItem::Class(class_def) = item {
                self.trace(format!("predeclare:class-body {}", class_def.name));
                if self.generic_classes.contains_key(&class_def.name) {
                    continue;
                }
                let mut field_indices = HashMap::new();
                let mut body_types = Vec::new();
                if let Some(parent) = &class_def.extends {
                    let parent_layout = self
                        .class_layouts
                        .get(parent)
                        .ok_or_else(|| CodeGenError::MissingSymbol(parent.clone()))?;
                    body_types.push(parent_layout.struct_type.into());
                }
                for field in &class_def.fields {
                    let index = body_types.len() as u32;
                    field_indices.insert(field.name.clone(), index);
                    body_types.push(self.llvm_basic_type(&field.ty)?);
                }
                let layout = self
                    .class_layouts
                    .get_mut(&class_def.name)
                    .ok_or_else(|| CodeGenError::MissingSymbol(class_def.name.clone()))?;
                layout.struct_type.set_body(&body_types, false);
                layout.field_indices = field_indices;
            }
        }

        self.predeclare_mono_classes()?;

        for item in &program.items {
            match item {
                TypedItem::Fn(function)
                | TypedItem::PanicHandler(function)
                | TypedItem::OomHandler(function) => {
                    if self.generic_functions.contains_key(&function.name) {
                        continue;
                    }
                    self.predeclare_function(function, None)?;
                }
                TypedItem::Extern(extern_block) => {
                    for function in &extern_block.functions {
                        if self.generic_functions.contains_key(&function.name) {
                            continue;
                        }
                        self.predeclare_function(function, None)?;
                    }
                }
                TypedItem::Class(class_def) => {
                    self.trace(format!("predeclare:class-runtime {}", class_def.name));
                    if self.generic_classes.contains_key(&class_def.name) {
                        continue;
                    }
                    let _ = self.emit_class_type_descriptor(class_def)?;
                    self.predeclare_constructor(&class_def.name)?;
                    // Layer methods are already flattened into the typed class method list.
                    for method in &class_def.methods {
                        self.predeclare_function(method, Some(&class_def.name))?;
                    }
                    self.predeclare_vtable_bindings_for_class(class_def)?;
                }
                _ => {}
            }
        }
        self.predeclare_mono_functions()?;
        Ok(())
    }

    fn predeclare_function(
        &mut self,
        function: &TypedFnDef,
        current_class: Option<&str>,
    ) -> Result<(), CodeGenError> {
        let (symbol, llvm_type) = if let Some(class_name) = current_class {
            (
                format!("{class_name}.{}", function.name),
                self.llvm_method_function_type(
                    class_name,
                    &function.ret_type,
                    &function
                        .params
                        .iter()
                        .filter(|param| param.name != "self")
                        .map(|param| param.ty.clone())
                        .collect::<Vec<_>>(),
                )?,
            )
        } else {
            (
                function.name.clone(),
                self.llvm_function_type(
                    &function.ret_type,
                    &function
                        .params
                        .iter()
                        .map(|param| param.ty.clone())
                        .collect::<Vec<_>>(),
                )?,
            )
        };
        if self.functions.contains_key(&symbol) {
            return Ok(());
        }
        let value = self.module.add_function(&symbol, llvm_type, None);
        self.functions.insert(symbol.clone(), value);
        if let Some(class_name) = current_class {
            if let Some(layout) = self.class_layouts.get_mut(class_name) {
                layout.method_names.insert(function.name.clone(), symbol);
            }
        }
        Ok(())
    }

    fn predeclare_constructor(&mut self, class_name: &str) -> Result<(), CodeGenError> {
        let symbol = format!("{class_name}_new");
        if self.functions.contains_key(&symbol) {
            return Ok(());
        }
        let layout = self
            .class_layouts
            .get(class_name)
            .ok_or_else(|| CodeGenError::MissingSymbol(class_name.to_string()))?;
        let fn_ty = layout
            .struct_type
            .ptr_type(inkwell::AddressSpace::default())
            .fn_type(&[], false);
        let value = self.module.add_function(&symbol, fn_ty, None);
        self.functions.insert(symbol, value);
        Ok(())
    }

    pub(crate) fn emit_item(&mut self, item: &TypedItem) -> Result<(), CodeGenError> {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => {
                self.trace(format!("emit:function {}", function.name));
                if self.generic_functions.contains_key(&function.name) {
                    Ok(())
                } else {
                    self.emit_function(function, None)
                }
            }
            TypedItem::Class(class_def) => {
                self.trace(format!("emit:class {}", class_def.name));
                if self.generic_classes.contains_key(&class_def.name) {
                    return Ok(());
                }
                self.emit_constructor(class_def)?;
                // Layer methods are already flattened into the typed class method list.
                for method in &class_def.methods {
                    self.emit_function(method, Some(&class_def.name))?;
                }
                self.emit_vtable_for_class(class_def)?;
                Ok(())
            }
            TypedItem::Extern(_)
            | TypedItem::Import(_)
            | TypedItem::TypeBlock(_)
            | TypedItem::Interface(_)
            | TypedItem::Enum(_)
            | TypedItem::Error(_) => Ok(()),
            TypedItem::Const(const_def) => {
                let value = self.emit_const_expr(&const_def.value)?;
                let global = self
                    .module
                    .add_global(value.get_type(), None, &const_def.name);
                global.set_initializer(&value);
                Ok(())
            }
        }
    }

    pub(crate) fn emit_mono_items(&mut self) -> Result<(), CodeGenError> {
        for inst in self.mono.class_insts.clone() {
            let Some(info) = self.generic_classes.get(&inst.class_name).cloned() else {
                continue;
            };
            let specialized = specialize_class(&info.def, &info.type_vars, &inst);
            self.emit_constructor(&specialized)?;
            for method in &specialized.methods {
                self.emit_function(method, Some(&specialized.name))?;
            }
            self.emit_vtable_for_class(&specialized)?;
        }
        for inst in self.mono.fn_insts.clone() {
            let Some(info) = self.generic_functions.get(&inst.fn_name).cloned() else {
                continue;
            };
            let specialized = specialize_function(
                &info.def,
                &crate::mono::build_var_subst(&info.type_vars, &inst.type_args),
                None,
                Some(&inst.mangled),
            );
            self.emit_function(&specialized, None)?;
        }
        Ok(())
    }

    fn predeclare_mono_classes(&mut self) -> Result<(), CodeGenError> {
        for inst in self.mono.class_insts.clone() {
            if self.class_layouts.contains_key(&inst.mangled) {
                continue;
            }
            let struct_type = self.context.opaque_struct_type(&inst.mangled);
            self.class_layouts.insert(
                inst.mangled.clone(),
                ClassLayout {
                    struct_type,
                    field_indices: HashMap::new(),
                    method_names: HashMap::new(),
                    parent_class: None,
                },
            );
        }

        for inst in self.mono.class_insts.clone() {
            let Some(info) = self.generic_classes.get(&inst.class_name).cloned() else {
                continue;
            };
            let specialized = specialize_class(&info.def, &info.type_vars, &inst);
            let mut field_indices = HashMap::new();
            let mut body_types = Vec::new();
            if let Some(parent) = &specialized.extends {
                let parent_layout = self
                    .class_layouts
                    .get(parent)
                    .ok_or_else(|| CodeGenError::MissingSymbol(parent.clone()))?;
                body_types.push(parent_layout.struct_type.into());
            }
            for field in &specialized.fields {
                let index = body_types.len() as u32;
                field_indices.insert(field.name.clone(), index);
                body_types.push(self.llvm_basic_type(&field.ty)?);
            }
            let layout = self
                .class_layouts
                .get_mut(&specialized.name)
                .ok_or_else(|| CodeGenError::MissingSymbol(specialized.name.clone()))?;
            layout.struct_type.set_body(&body_types, false);
            layout.field_indices = field_indices;
            layout.parent_class = specialized.extends.clone();

            let _ = self.emit_class_type_descriptor(&specialized)?;
            self.predeclare_constructor(&specialized.name)?;
            for method in &specialized.methods {
                self.predeclare_function(method, Some(&specialized.name))?;
            }
            self.predeclare_vtable_bindings_for_class(&specialized)?;
        }

        Ok(())
    }

    fn predeclare_mono_functions(&mut self) -> Result<(), CodeGenError> {
        for inst in self.mono.fn_insts.clone() {
            let Some(info) = self.generic_functions.get(&inst.fn_name).cloned() else {
                continue;
            };
            let specialized = specialize_function(
                &info.def,
                &crate::mono::build_var_subst(&info.type_vars, &inst.type_args),
                None,
                Some(&inst.mangled),
            );
            self.predeclare_function(&specialized, None)?;
        }
        Ok(())
    }

    pub(crate) fn emit_function(
        &mut self,
        function: &TypedFnDef,
        current_class: Option<&str>,
    ) -> Result<(), CodeGenError> {
        if function.body.is_none() {
            return Ok(());
        }
        let symbol = current_class
            .map(|class_name| format!("{class_name}.{}", function.name))
            .unwrap_or_else(|| function.name.clone());
        let llvm_fn = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.clone()))?;
        if llvm_fn.get_first_basic_block().is_some() {
            return Ok(());
        }

        self.current_function = Some(llvm_fn);
        self.current_return_type = Some(function.ret_type.clone());
        self.current_class = current_class.map(ToOwned::to_owned);
        let ownership_key = current_class
            .map(|class_name| format!("{class_name}::{}", function.name))
            .unwrap_or_else(|| function.name.clone());
        self.begin_function_ownership_scope(&ownership_key);
        self.push_scope();

        let entry = self.context.append_basic_block(llvm_fn, "entry");
        self.builder.position_at_end(entry);

        let mut param_index = 0;
        if let Some(class_name) = current_class {
            let self_param = llvm_fn
                .get_nth_param(param_index)
                .ok_or_else(|| CodeGenError::MissingSymbol(format!("{symbol}:self")))?;
            let self_ptr = self.create_entry_alloca(llvm_fn, self_param.get_type(), "self")?;
            self.build_store(self_ptr, self_param)?;
            self.register_gc_root(self_ptr, &Type::Named(class_name.to_string(), Vec::new()))?;
            self.define_local("self", self_ptr);
            self.current_class = Some(class_name.to_string());
            param_index += 1;
        }

        for param in &function.params {
            if current_class.is_some() && param.name == "self" {
                continue;
            }
            let value = llvm_fn
                .get_nth_param(param_index)
                .ok_or_else(|| CodeGenError::MissingSymbol(format!("{symbol}:{}", param.name)))?;
            let ptr = self.create_entry_alloca(llvm_fn, value.get_type(), &param.name)?;
            self.build_store(ptr, value)?;
            self.register_gc_root(ptr, &param.ty)?;
            self.define_local(&param.name, ptr);
            param_index += 1;
        }

        let tail_value = if let Some(body) = &function.body {
            self.emit_block(body)?
        } else {
            None
        };

        if !self.current_block_terminated() {
            if let Some(body) = &function.body {
                self.emit_tail_stmt_frees(body)?;
            }
            self.emit_all_pending_frees()?;
            if let Some(value) = tail_value {
                self.builder
                    .build_return(Some(&value))
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            } else if matches!(function.ret_type, Type::Unit | Type::Never) {
                self.builder
                    .build_return(None)
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            } else {
                let zero = self.zero_value(&function.ret_type)?;
                self.builder
                    .build_return(Some(&zero))
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            }
        }

        self.finish_function_ownership_scope();
        self.pop_scope();
        self.current_function = None;
        self.current_return_type = None;
        self.current_class = None;
        Ok(())
    }

    fn emit_constructor(
        &mut self,
        class_def: &draton_typeck::typed_ast::TypedClassDef,
    ) -> Result<(), CodeGenError> {
        let symbol = format!("{}_new", class_def.name);
        let function = self
            .functions
            .get(&symbol)
            .copied()
            .ok_or_else(|| CodeGenError::MissingSymbol(symbol.clone()))?;
        if function.get_first_basic_block().is_some() {
            return Ok(());
        }
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let raw =
            self.emit_owned_alloc(&Type::Named(class_def.name.clone(), Vec::new()), "ctor.raw")?;
        let object_ptr = raw;
        for field in &class_def.fields {
            let layout = self
                .class_layouts
                .get(&class_def.name)
                .ok_or_else(|| CodeGenError::MissingSymbol(class_def.name.clone()))?;
            let index = layout
                .field_indices
                .get(&field.name)
                .copied()
                .ok_or_else(|| {
                    CodeGenError::MissingSymbol(format!("{}.{}", class_def.name, field.name))
                })?;
            let field_ptr =
                self.build_struct_gep(layout.struct_type, object_ptr, index, &field.name)?;
            let zero = self.zero_value(&field.ty)?;
            self.build_store(field_ptr, zero)?;
            if Self::is_gc_pointer_type(&field.ty) {
                let _ = self.emit_gc_write_barrier(object_ptr, field_ptr, zero);
            }
        }
        self.builder
            .build_return(Some(&object_ptr))
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }
}
