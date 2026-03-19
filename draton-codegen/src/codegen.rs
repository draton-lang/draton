use std::collections::HashMap;
use std::env;
use std::path::Path;

use draton_typeck::{Type, TypedProgram};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::StructType;
use inkwell::values::{
    BasicValueEnum, FunctionValue, GlobalValue, InstructionOpcode, PointerValue,
};
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
    pub(crate) closure_record_type: StructType<'ctx>,
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
    pub(crate) closure_counter: usize,
    pub(crate) closure_type_descriptor_id: u16,
    pub(crate) pending_ctors: Vec<FunctionValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    /// Creates a new LLVM module builder for the requested build mode.
    pub fn new(context: &'ctx Context, mode: BuildMode) -> Self {
        let module = context.create_module("draton");
        let builder = context.create_builder();
        let i64_type = context.i64_type();
        let i8_ptr = context.i8_type().ptr_type(AddressSpace::default());
        let string_type = context.struct_type(&[i64_type.into(), i8_ptr.into()], false);
        let closure_record_type = context.opaque_struct_type("draton.closure");
        closure_record_type.set_body(&[i8_ptr.into(), i8_ptr.into()], false);
        Self {
            context,
            module,
            builder,
            mode,
            string_type,
            closure_record_type,
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
            closure_counter: 0,
            closure_type_descriptor_id: 0,
            pending_ctors: Vec::new(),
        }
    }

    /// Emits LLVM IR for a typed Draton program.
    pub fn emit(mut self, program: &TypedProgram) -> Result<Module<'ctx>, CodeGenError> {
        self.index_generic_items(program);
        self.mono = MonoCollector::new().collect(program);
        self.iface_registry = InterfaceRegistry::build(program);
        self.emit_interface_runtime_types()?;
        self.declare_runtime()?;
        self.ensure_closure_runtime_metadata()?;
        self.predeclare_program_items(program)?;
        for item in &program.items {
            self.emit_item(item)?;
        }
        self.emit_mono_items()?;
        self.normalize_global_ctors()?;
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
        let triple = preferred_target_triple();
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
        module.set_triple(&triple);
        let data_layout = machine.get_target_data().get_data_layout();
        module.set_data_layout(&data_layout);
        machine
            .write_to_file(module, FileType::Object, path)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    /// Emit a single @llvm.global_ctors global containing all pending ctor functions.
    pub(crate) fn flush_global_ctors(&mut self) -> Result<(), CodeGenError> {
        if self.pending_ctors.is_empty() {
            return Ok(());
        }
        let i32_type = self.context.i32_type();
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let entry_ty = self.context.struct_type(
            &[
                i32_type.into(),
                self.context
                    .void_type()
                    .fn_type(&[], false)
                    .ptr_type(AddressSpace::default())
                    .into(),
                i8_ptr.into(),
            ],
            false,
        );
        let priority = i32_type.const_int(65535, false);
        let null_data = i8_ptr.const_null();
        let mut entries = Vec::new();
        for &func in &self.pending_ctors {
            let fn_ptr = func.as_global_value().as_pointer_value();
            let entry =
                entry_ty.const_named_struct(&[priority.into(), fn_ptr.into(), null_data.into()]);
            entries.push(entry);
        }
        let arr_ty = entry_ty.array_type(entries.len() as u32);
        let arr = entry_ty.const_array(&entries);
        // Remove any existing @llvm.global_ctors.* globals created previously.
        const CTORS: &str = "llvm.global_ctors";
        // Delete stale numbered variants if present (shouldn't be, but clean up).
        for i in 0..self.string_counter {
            let name = format!("llvm.global_ctors.{i}");
            if let Some(g) = self.module.get_global(&name) {
                unsafe { g.delete() };
            }
        }
        if let Some(existing) = self.module.get_global(CTORS) {
            unsafe { existing.delete() };
        }
        let g = self.module.add_global(arr_ty, None, CTORS);
        g.set_initializer(&arr);
        g.set_linkage(inkwell::module::Linkage::Appending);
        self.pending_ctors.clear();
        Ok(())
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
        if self.class_layouts.contains_key(name) && arg_types.is_empty() {
            let symbol = format!("{name}_new");
            if self.functions.contains_key(&symbol) {
                return Ok(symbol);
            }
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
        if env::var_os("DRATON_DISABLE_GCROOT").is_some() {
            return Ok(());
        }
        if !Self::is_gc_rootable_type(ty) {
            return Ok(());
        }
        let function = match self.current_function() {
            Ok(function) => function,
            Err(_) => return Ok(()),
        };
        function.set_gc("shadow-stack");
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let gcroot = self.get_or_declare_gcroot_intrinsic();
        let entry_builder = self.context.create_builder();
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodeGenError::Llvm("register_gc_root: no entry block".to_string()))?;
        let mut cursor = entry.get_first_instruction();
        let mut first_non_alloca = None;
        while let Some(instr) = cursor {
            if instr.get_opcode() != InstructionOpcode::Alloca {
                first_non_alloca = Some(instr);
                break;
            }
            cursor = instr.get_next_instruction();
        }
        match first_non_alloca {
            Some(instr) => entry_builder.position_before(&instr),
            None => entry_builder.position_at_end(entry),
        }

        let root_location = entry_builder
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
        let _ = entry_builder
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

    pub(crate) fn all_locals(&self) -> HashMap<String, PointerValue<'ctx>> {
        let mut locals = HashMap::new();
        for scope in self.variables.iter().rev() {
            for (name, ptr) in scope {
                locals.entry(name.clone()).or_insert(*ptr);
            }
        }
        locals
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

    fn normalize_global_ctors(&self) -> Result<(), CodeGenError> {
        const CTORS: &str = "llvm.global_ctors";
        const CTORS_PREFIX: &str = "llvm.global_ctors.";

        let mut ctors = self
            .module
            .get_functions()
            .filter(|function| {
                function
                    .get_name()
                    .to_str()
                    .map(|name| name.starts_with("__gc_register_"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if ctors.is_empty() {
            return Ok(());
        }
        ctors.sort_by(|lhs, rhs| lhs.get_name().to_bytes().cmp(rhs.get_name().to_bytes()));

        let i32_type = self.context.i32_type();
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_ptr_ty = ctors[0].get_type().ptr_type(AddressSpace::default());
        let entry_ty = self
            .context
            .struct_type(&[i32_type.into(), fn_ptr_ty.into(), i8_ptr.into()], false);
        let priority = i32_type.const_int(65535, false);
        let data = i8_ptr.const_null();
        let entries = ctors
            .iter()
            .map(|function| {
                entry_ty.const_named_struct(&[
                    priority.into(),
                    function.as_global_value().as_pointer_value().into(),
                    data.into(),
                ])
            })
            .collect::<Vec<_>>();
        let array_ty = entry_ty.array_type(entries.len() as u32);
        let array = entry_ty.const_array(&entries);

        let stale_globals = self
            .module
            .get_globals()
            .filter(|global| {
                global
                    .get_name()
                    .to_str()
                    .map(|name| name == CTORS || name.starts_with(CTORS_PREFIX))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        for global in stale_globals {
            unsafe { global.delete() };
        }

        let global = self.module.add_global(array_ty, None, CTORS);
        global.set_initializer(&array);
        global.set_linkage(Linkage::Appending);
        Ok(())
    }

    fn apply_optimizations(&self) {
        let _ = self.mode;
    }
}

fn preferred_target_triple() -> TargetTriple {
    if let Ok(triple) = env::var("DRATON_LLVM_TARGET_TRIPLE") {
        let triple = triple.trim();
        if !triple.is_empty() {
            return TargetTriple::create(triple);
        }
    }
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) && packaged_windows_gnu_root() {
        // The packaged Windows release links generated objects with MinGW-w64.
        // Emitting an MSVC-flavored object on that path breaks aggregate FFI ABI.
        return TargetTriple::create("x86_64-pc-windows-gnu");
    }
    TargetMachine::get_default_triple()
}

fn packaged_windows_gnu_root() -> bool {
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("windows-gnu")))
        .is_some_and(|path| path.exists())
}
