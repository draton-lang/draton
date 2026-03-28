use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;

use draton_typeck::{OwnershipChecker, Type, TypedProgram};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{BasicTypeEnum, StructType};
use inkwell::values::{AsValueRef, BasicValueEnum, FunctionValue, GlobalValue, IntValue, PointerValue};
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
    /// Maps a span start offset to the LLVM pointer value that should be freed at that point.
    pub(crate) free_points: HashMap<usize, Vec<PointerValue<'ctx>>>,
    pub(crate) ownership_free_spans: HashMap<String, Vec<usize>>,
    pub(crate) current_function_free_bindings: HashMap<usize, Vec<String>>,
    pub(crate) pointer_pointee_types: RefCell<HashMap<usize, BasicTypeEnum<'ctx>>>,
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
            free_points: HashMap::new(),
            ownership_free_spans: HashMap::new(),
            current_function_free_bindings: HashMap::new(),
            pointer_pointee_types: RefCell::new(HashMap::new()),
        }
    }

    /// Emits LLVM IR for a typed Draton program.
    pub fn emit(mut self, program: &TypedProgram) -> Result<Module<'ctx>, CodeGenError> {
        let mut program = program.clone();
        let mut ownership_checker = OwnershipChecker::new();
        let ownership_errors = ownership_checker.check_program(&mut program);
        if let Some(error) = ownership_errors.first() {
            return Err(CodeGenError::UnsupportedExpr(format!(
                "ownership analysis failed during codegen: {error}"
            )));
        }
        self.ownership_free_spans = ownership_checker
            .recorded_free_points()
            .iter()
            .map(|(key, spans)| (key.clone(), spans.iter().map(|span| span.start).collect()))
            .collect();

        self.index_generic_items(&program);
        self.mono = MonoCollector::new().collect(&program);
        self.iface_registry = InterfaceRegistry::build(&program);
        self.emit_interface_runtime_types()?;
        self.declare_runtime()?;
        self.ensure_closure_runtime_metadata()?;
        self.predeclare_program_items(&program)?;
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

    pub(crate) fn register_gc_root(
        &mut self,
        storage: PointerValue<'ctx>,
        ty: &Type,
    ) -> Result<(), CodeGenError> {
        let _ = (storage, ty);
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
        let ptr = builder
            .build_alloca(ty, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.remember_pointer_pointee(ptr, ty);
        Ok(ptr)
    }

    pub(crate) fn current_block_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .and_then(BasicBlock::get_terminator)
            .is_some()
    }

    fn pointer_key(&self, ptr: PointerValue<'ctx>) -> usize {
        ptr.as_value_ref() as usize
    }

    pub(crate) fn remember_pointer_pointee(
        &self,
        ptr: PointerValue<'ctx>,
        pointee_ty: BasicTypeEnum<'ctx>,
    ) {
        self.pointer_pointee_types
            .borrow_mut()
            .insert(self.pointer_key(ptr), pointee_ty);
    }

    pub(crate) fn pointer_pointee(
        &self,
        ptr: PointerValue<'ctx>,
    ) -> Result<BasicTypeEnum<'ctx>, CodeGenError> {
        self.pointer_pointee_types
            .borrow()
            .get(&self.pointer_key(ptr))
            .copied()
            .ok_or_else(|| CodeGenError::Llvm("missing pointee type metadata".to_string()))
    }

    pub(crate) fn build_load(
        &self,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let pointee_ty = self.pointer_pointee(ptr)?;
        self.builder
            .build_load(pointee_ty, ptr, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    pub(crate) fn build_typed_load(
        &self,
        ptr: PointerValue<'ctx>,
        pointee_ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        self.builder
            .build_load(pointee_ty, ptr, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))
    }

    pub(crate) unsafe fn build_gep(
        &self,
        pointee_ty: BasicTypeEnum<'ctx>,
        ptr: PointerValue<'ctx>,
        indices: &[IntValue<'ctx>],
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let gep = self
            .builder
            .build_gep(pointee_ty, ptr, indices, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.remember_pointer_pointee(gep, pointee_ty);
        Ok(gep)
    }

    pub(crate) fn build_struct_gep(
        &self,
        struct_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        index: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let gep = self
            .builder
            .build_struct_gep(struct_ty, ptr, index, name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let field_ty = struct_ty
            .get_field_types()
            .get(index as usize)
            .copied()
            .ok_or_else(|| CodeGenError::Llvm(format!("field index {index} out of bounds")))?;
        self.remember_pointer_pointee(gep, field_ty);
        Ok(gep)
    }

    pub(crate) fn build_pointer_cast_to(
        &self,
        ptr: PointerValue<'ctx>,
        pointee_ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let casted = self
            .builder
            .build_pointer_cast(ptr, self.context.ptr_type(AddressSpace::default()), name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.remember_pointer_pointee(casted, pointee_ty);
        Ok(casted)
    }

    pub(crate) fn build_bit_cast_to(
        &self,
        ptr: PointerValue<'ctx>,
        pointee_ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodeGenError> {
        let casted = self
            .builder
            .build_bit_cast(ptr, self.context.ptr_type(AddressSpace::default()), name)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        self.remember_pointer_pointee(casted, pointee_ty);
        Ok(casted)
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

    pub(crate) fn register_free_point(&mut self, span_start: usize, ptr: PointerValue<'ctx>) {
        self.free_points.entry(span_start).or_default().push(ptr);
    }

    pub(crate) fn emit_pending_frees(&mut self, span_start: usize) -> Result<(), CodeGenError> {
        let Some(ptrs) = self.free_points.remove(&span_start) else {
            return Ok(());
        };
        let free = self
            .module
            .get_function("free")
            .ok_or_else(|| CodeGenError::MissingSymbol("free".to_string()))?;
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        for ptr in ptrs {
            let raw = self
                .builder
                .build_pointer_cast(ptr, i8_ptr, "owned.free.raw")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            let _ = self
                .builder
                .build_call(free, &[raw.into()], "")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        Ok(())
    }

    pub(crate) fn emit_all_pending_frees(&mut self) -> Result<(), CodeGenError> {
        let mut spans = self.free_points.keys().copied().collect::<Vec<_>>();
        spans.sort_unstable();
        for span in spans {
            self.emit_pending_frees(span)?;
        }
        Ok(())
    }

    pub(crate) fn begin_function_ownership_scope(&mut self, fn_key: &str) {
        let prefix = format!("{fn_key}:");
        self.current_function_free_bindings.clear();
        self.free_points.clear();
        for (key, spans) in &self.ownership_free_spans {
            let Some(name) = key.strip_prefix(&prefix) else {
                continue;
            };
            for &span_start in spans {
                self.current_function_free_bindings
                    .entry(span_start)
                    .or_default()
                    .push(name.to_string());
            }
        }
    }

    pub(crate) fn finish_function_ownership_scope(&mut self) {
        self.current_function_free_bindings.clear();
        self.free_points.clear();
    }

    pub(crate) fn schedule_binding_frees_at(
        &mut self,
        span_start: usize,
        excluded: &HashSet<String>,
    ) -> Result<(), CodeGenError> {
        let Some(names) = self.current_function_free_bindings.get(&span_start).cloned() else {
            return Ok(());
        };
        for name in names {
            if excluded.contains(&name) {
                continue;
            }
            let Some(storage) = self.lookup_local(&name) else {
                continue;
            };
            let loaded = self.build_load(storage, &format!("{name}.free.load"))?;
            if let Some(ptr) = self.freeable_pointer_from_value(loaded, &name)? {
                self.register_free_point(span_start, ptr);
            }
        }
        Ok(())
    }

    pub(crate) fn freeable_pointer_from_value(
        &mut self,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<Option<PointerValue<'ctx>>, CodeGenError> {
        match value {
            BasicValueEnum::PointerValue(ptr) => Ok(Some(ptr)),
            BasicValueEnum::StructValue(value) => {
                let field0 = self
                    .builder
                    .build_extract_value(value, 0, &format!("{name}.free.field0"))
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                Ok(match field0 {
                    BasicValueEnum::PointerValue(ptr) => Some(ptr),
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn normalize_global_ctors(&self) -> Result<(), CodeGenError> {
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
