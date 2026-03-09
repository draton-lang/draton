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
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, OptimizationLevel};

use crate::error::CodeGenError;

#[derive(Debug, Clone)]
pub(crate) struct ClassLayout<'ctx> {
    pub struct_type: StructType<'ctx>,
    pub field_indices: HashMap<String, u32>,
    pub method_names: HashMap<String, String>,
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
    pub(crate) string_counter: u64,
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
            string_counter: 0,
        }
    }

    /// Emits LLVM IR for a typed Draton program.
    pub fn emit(mut self, program: &TypedProgram) -> Result<Module<'ctx>, CodeGenError> {
        self.declare_runtime()?;
        self.predeclare_program_items(program)?;
        for item in &program.items {
            self.emit_item(item)?;
        }
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
