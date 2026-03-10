use std::collections::HashMap;
use std::path::Path;

use draton_typeck::{Type, TypedProgram};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::StructType;
use inkwell::values::{BasicValueEnum, FunctionValue, GlobalValue, PointerValue};
use inkwell::{AddressSpace, OptimizationLevel};

use crate::error::CodeGenError;
use crate::mangle::mangle_fn;
use crate::mono::{
    generic_class_def, generic_fn_def, resolve_function_type_args, GenericClassDef, GenericFnDef,
    MonoCollector,
};
use crate::vtable::InterfaceRegistry;

#[derive(Debug, Clone)]
pub(crate) struct ClassLayout<'ctx> {
    pub struct_type: StructType<'ctx>,
    pub field_indices: HashMap<String, u32>,
    pub method_names: HashMap<String, String>,
    pub parent_class: Option<String>,
}

/// Build flavor for LLVM emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    /// `-O0`
    Debug,
    /// `-O3`
    Release,
    /// `-Os`
    Size,
    /// `-O1`
    Fast,
}

/// Stateful LLVM IR generator for a typed Draton program.
pub struct CodeGen<'ctx> {
    pub(crate) context: &'ctx Context,
    pub(crate) module: Module<'ctx>,
    pub(crate) builder: Builder<'ctx>,
    pub(crate) mode: BuildMode,
    pub(crate) string_type: StructType<'ctx>,
    pub(crate) functions: HashMap<String, FunctionValue<'ctx>>,
    pub(crate) class_layouts: HashMap<String, ClassLayout<'ctx>>,
    pub(crate) variables: Vec<HashMap<String, PointerValue<'ctx>>>,
    pub(crate) current_function: Option<FunctionValue<'ctx>>,
    pub(crate) current_return_type: Option<Type>,
    pub(crate) current_class: Option<String>,
    pub(crate) type_descriptor_table: HashMap<String, u16>,
    pub(crate) next_type_descriptor_id: u16,
    pub(crate) string_counter: u64,
    pub(crate) mono: MonoCollector,
    pub(crate) generic_classes: HashMap<String, GenericClassDef>,
    pub(crate) generic_functions: HashMap<String, GenericFnDef>,
    pub(crate) iface_registry: InterfaceRegistry,
    pub(crate) vtable_types: HashMap<String, StructType<'ctx>>,
    pub(crate) fat_pointer_types: HashMap<String, StructType<'ctx>>,
    pub(crate) vtable_globals: HashMap<(String, String), GlobalValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    /// Creates a new LLVM module builder for the requested build mode.
    pub fn new(context: &'ctx Context, mode: BuildMode) -> Self {
        let module = context.create_module("draton");
        let builder = context.create_builder();
        let i64_type = context.i64_type();
        let i8_ptr = context.i8_type().ptr_type(AddressSpace::default());
        let string_type = context.struct_type(&[i64_type.into(), i8_ptr.into()], false);
        Self {
            context,
            module,
            builder,
            mode,
            string_type,
            functions: HashMap::new(),
            class_layouts: HashMap::new(),
            variables: Vec::new(),
            current_function: None,
            current_return_type: None,
            current_class: None,
            type_descriptor_table: HashMap::new(),
            next_type_descriptor_id: 1,
            string_counter: 0,
            mono: MonoCollector::new(),
            generic_classes: HashMap::new(),
            generic_functions: HashMap::new(),
            iface_registry: InterfaceRegistry::default(),
            vtable_types: HashMap::new(),
            fat_pointer_types: HashMap::new(),
            vtable_globals: HashMap::new(),
        }
    }

    /// Emits LLVM IR for a typed Draton program.
    pub fn emit(mut self, program: &TypedProgram) -> Result<Module<'ctx>, CodeGenError> {
        self.index_generic_items(program);
        self.mono = MonoCollector::new().collect(program);
        self.iface_registry = InterfaceRegistry::build(program);
        self.emit_interface_runtime_types()?;
        self.declare_runtime()?;
        self.predeclare_program_items(program)?;
        for item in &program.items {
            self.emit_item(item)?;
        }
        self.emit_mono_items()?;
        self.apply_optimizations();
        self.module
            .verify()
            .map_err(|err| CodeGenError::Verify(err.to_string()))?;
        Ok(self.module)
    }

    /// Writes the module IR to a textual `.ll` file.
    pub fn write_ir(module: &Module<'_>, path: &Path) -> Result<(), CodeGenError> {
        module
            .print_to_file(path)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    /// Writes the module to an object file for the host target.
    pub fn write_object(module: &Module<'_>, path: &Path) -> Result<(), CodeGenError> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let triple = TargetMachine::get_default_triple();
        let target =
            Target::from_triple(&triple).map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::None,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| CodeGenError::Llvm("unable to create target machine".to_string()))?;
        machine
            .write_to_file(module, FileType::Object, path)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    pub(crate) fn current_function(&self) -> Result<FunctionValue<'ctx>, CodeGenError> {
        self.current_function
            .ok_or_else(|| CodeGenError::MissingSymbol("current function".to_string()))
    }

    pub(crate) fn index_generic_items(&mut self, program: &TypedProgram) {
        self.generic_classes.clear();
        self.generic_functions.clear();
        for item in &program.items {
            match item {
                draton_typeck::TypedItem::Class(class_def) => {
                    if let Some(info) = generic_class_def(class_def) {
                        self.generic_classes.insert(class_def.name.clone(), info);
                    }
                }
                draton_typeck::TypedItem::Fn(function)
                | draton_typeck::TypedItem::PanicHandler(function)
                | draton_typeck::TypedItem::OomHandler(function) => {
                    if let Some(info) = generic_fn_def(function) {
                        self.generic_functions.insert(function.name.clone(), info);
                    }
                }
                draton_typeck::TypedItem::Extern(extern_block) => {
                    for function in &extern_block.functions {
                        if let Some(info) = generic_fn_def(function) {
                            self.generic_functions.insert(function.name.clone(), info);
                        }
                    }
                }
                draton_typeck::TypedItem::Interface(_)
                | draton_typeck::TypedItem::Enum(_)
                | draton_typeck::TypedItem::Error(_)
                | draton_typeck::TypedItem::Const(_)
                | draton_typeck::TypedItem::Import(_)
                | draton_typeck::TypedItem::TypeBlock(_) => {}
            }
        }
    }

    pub(crate) fn resolve_function_symbol(
        &self,
        name: &str,
        arg_types: &[Type],
    ) -> Result<String, CodeGenError> {
        if self.functions.contains_key(name) {
            return Ok(name.to_string());
        }
        if let Some(info) = self.generic_functions.get(name) {
            if let Some(type_args) =
                resolve_function_type_args(&info.def, &info.type_vars, arg_types)
            {
                let symbol = mangle_fn(name, None, &type_args);
                if self.functions.contains_key(&symbol) {
                    return Ok(symbol);
                }
            }
        }
        Err(CodeGenError::MissingSymbol(name.to_string()))
    }

    pub(crate) fn get_or_declare_gcroot_intrinsic(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("llvm.gcroot") {
            return function;
        }
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let i8_ptr_ptr = i8_ptr.ptr_type(AddressSpace::default());
        let fn_type = self
            .context
            .void_type()
            .fn_type(&[i8_ptr_ptr.into(), i8_ptr.into()], false);
        self.module.add_function("llvm.gcroot", fn_type, None)
    }

    pub(crate) fn is_gc_rootable_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Named(_, _) | Type::Chan(_) | Type::Pointer(_) | Type::Fn(_, _)
        )
    }

    pub(crate) fn register_gc_root(
        &self,
        storage: PointerValue<'ctx>,
        ty: &Type,
    ) -> Result<(), CodeGenError> {
        if !Self::is_gc_rootable_type(ty) {
            return Ok(());
        }
        if let Ok(function) = self.current_function() {
            function.set_gc("shadow-stack");
        }
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let gcroot = self.get_or_declare_gcroot_intrinsic();
        let root_location = self
            .builder
            .build_bitcast(
                storage,
                i8_ptr.ptr_type(AddressSpace::default()),
                "gc.root.slot",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let metadata = self
            .context
            .i64_type()
            .const_int(1, false)
            .const_to_pointer(i8_ptr);
        let _ = self
            .builder
            .build_call(gcroot, &[root_location.into(), metadata.into()], "")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }

    pub(crate) fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        let _ = self.variables.pop();
    }

    pub(crate) fn define_local(&mut self, name: &str, ptr: PointerValue<'ctx>) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name.to_string(), ptr);
        }
    }

    pub(crate) fn lookup_local(&self, name: &str) -> Option<PointerValue<'ctx>> {
        self.variables
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    pub(crate) fn create_entry_alloca(
        &self,
        function: FunctionValue<'ctx>,
        ty: inkwell::types::BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let builder = self.context.create_builder();
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodeGenError::Llvm("function without entry block".to_string()))?;
        if let Some(instr) = entry.get_first_instruction() {
            builder.position_before(&instr);
        } else {
            builder.position_at_end(entry);
        }
        builder
            .build_alloca(ty, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    pub(crate) fn current_block_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .and_then(BasicBlock::get_terminator)
            .is_some()
    }

    pub(crate) fn build_load(
        &self,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        self.builder
            .build_load(ptr, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    pub(crate) fn build_store(
        &self,
        ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) -> Result<(), CodeGenError> {
        self.builder
            .build_store(ptr, value)
            .map(|_| ())
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    fn apply_optimizations(&self) {
        let _ = self.mode;
    }
}
