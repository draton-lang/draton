use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn declare_runtime(&mut self) -> Result<(), CodeGenError> {
        self.declare_libc()?;
        self.declare_safepoint_runtime()?;
        self.declare_gc_runtime()?;
        self.declare_print_runtime()?;
        self.declare_string_runtime()?;
        self.declare_cli_runtime()?;
        self.declare_panic_runtime()?;
        if let Some(print_fn) = self.module.get_function("draton_print") {
            self.functions.insert("print".to_string(), print_fn);
        }
        Ok(())
    }

    fn declare_libc(&mut self) -> Result<(), CodeGenError> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        if self.module.get_function("malloc").is_none() {
            self.module
                .add_function("malloc", i8_ptr.fn_type(&[i64_type.into()], false), None);
        }
        if self.module.get_function("free").is_none() {
            self.module.add_function(
                "free",
                self.context.void_type().fn_type(&[i8_ptr.into()], false),
                None,
            );
        }
        if self.module.get_function("puts").is_none() {
            self.module.add_function(
                "puts",
                self.context.i32_type().fn_type(&[i8_ptr.into()], false),
                None,
            );
        }
        if self.module.get_function("abort").is_none() {
            self.module
                .add_function("abort", self.context.void_type().fn_type(&[], false), None);
        }
        Ok(())
    }

    fn declare_safepoint_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_global("draton_safepoint_flag").is_none() {
            let flag =
                self.module
                    .add_global(self.context.i32_type(), None, "draton_safepoint_flag");
            flag.set_initializer(&self.context.i32_type().const_zero());
        }
        if self.module.get_function("draton_safepoint_slow").is_none() {
            let function = self.module.add_function(
                "draton_safepoint_slow",
                self.context.void_type().fn_type(&[], false),
                None,
            );
            let builder = self.context.create_builder();
            let entry = self.context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            builder
                .build_return(None)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        Ok(())
    }

    fn declare_gc_runtime(&mut self) -> Result<(), CodeGenError> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        if self.module.get_function("draton_gc_alloc").is_none() {
            let function = self.module.add_function(
                "draton_gc_alloc",
                i8_ptr.fn_type(
                    &[
                        self.context.i64_type().into(),
                        self.context.i16_type().into(),
                    ],
                    false,
                ),
                None,
            );
            let builder = self.context.create_builder();
            let entry = self.context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            let size = function
                .get_nth_param(0)
                .ok_or_else(|| {
                    CodeGenError::Llvm("missing draton_gc_alloc size param".to_string())
                })?
                .into_int_value();
            let malloc = self
                .module
                .get_function("malloc")
                .ok_or_else(|| CodeGenError::MissingSymbol("malloc".to_string()))?;
            let ptr = builder
                .build_call(malloc, &[size.into()], "gc.raw")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .try_as_basic_value()
                .left()
                .ok_or_else(|| CodeGenError::Llvm("malloc returned void".to_string()))?;
            builder
                .build_return(Some(&ptr))
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        if self
            .module
            .get_function("draton_gc_write_barrier")
            .is_none()
        {
            let field_ptr_ty = i8_ptr.ptr_type(AddressSpace::default());
            let function = self.module.add_function(
                "draton_gc_write_barrier",
                self.context
                    .void_type()
                    .fn_type(&[i8_ptr.into(), field_ptr_ty.into(), i8_ptr.into()], false),
                None,
            );
            let builder = self.context.create_builder();
            let entry = self.context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            builder
                .build_return(None)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        if self.module.get_function("draton_alloc").is_none() {
            let function = self.module.add_function(
                "draton_alloc",
                i8_ptr.fn_type(&[self.context.i64_type().into()], false),
                None,
            );
            let builder = self.context.create_builder();
            let entry = self.context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            let alloc = self
                .module
                .get_function("draton_gc_alloc")
                .ok_or_else(|| CodeGenError::MissingSymbol("draton_gc_alloc".to_string()))?;
            let size = function
                .get_first_param()
                .ok_or_else(|| CodeGenError::Llvm("missing draton_alloc size param".to_string()))?;
            let ptr = builder
                .build_call(
                    alloc,
                    &[size.into(), self.context.i16_type().const_zero().into()],
                    "alloc.ptr",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                .try_as_basic_value()
                .left()
                .ok_or_else(|| CodeGenError::Llvm("draton_gc_alloc returned void".to_string()))?;
            builder
                .build_return(Some(&ptr))
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        if self.module.get_function("draton_dealloc").is_none() {
            let function = self.module.add_function(
                "draton_dealloc",
                self.context.void_type().fn_type(&[i8_ptr.into()], false),
                None,
            );
            let builder = self.context.create_builder();
            let entry = self.context.append_basic_block(function, "entry");
            builder.position_at_end(entry);
            let free = self
                .module
                .get_function("free")
                .ok_or_else(|| CodeGenError::MissingSymbol("free".to_string()))?;
            let ptr = function.get_first_param().ok_or_else(|| {
                CodeGenError::Llvm("missing draton_dealloc ptr param".to_string())
            })?;
            let _ = builder
                .build_call(free, &[ptr.into()], "")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            builder
                .build_return(None)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        Ok(())
    }

    fn declare_print_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_function("draton_print").is_some() {
            return Ok(());
        }
        let function = self.module.add_function(
            "draton_print",
            self.context
                .void_type()
                .fn_type(&[self.string_type.into()], false),
            None,
        );
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(function, "entry");
        builder.position_at_end(entry);
        let puts = self
            .module
            .get_function("puts")
            .ok_or_else(|| CodeGenError::MissingSymbol("puts".to_string()))?;
        let value = function
            .get_first_param()
            .ok_or_else(|| CodeGenError::Llvm("missing draton_print string param".to_string()))?
            .into_struct_value();
        let ptr = builder
            .build_extract_value(value, 1, "str.ptr")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let _ = builder
            .build_call(puts, &[ptr.into()], "")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        builder
            .build_return(None)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }

    fn declare_string_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_function("draton_str_slice").is_none() {
            self.module.add_function(
                "draton_str_slice",
                self.string_type.fn_type(
                    &[
                        self.string_type.into(),
                        self.context.i64_type().into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                ),
                None,
            );
        }
        if self.module.get_function("draton_str_concat").is_none() {
            self.module.add_function(
                "draton_str_concat",
                self.string_type
                    .fn_type(&[self.string_type.into(), self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_int_to_string").is_none() {
            self.module.add_function(
                "draton_int_to_string",
                self.string_type
                    .fn_type(&[self.context.i64_type().into()], false),
                None,
            );
        }
        if self.module.get_function("draton_ascii_char").is_none() {
            self.module.add_function(
                "draton_ascii_char",
                self.string_type
                    .fn_type(&[self.context.i64_type().into()], false),
                None,
            );
        }
        Ok(())
    }

    fn declare_cli_runtime(&mut self) -> Result<(), CodeGenError> {
        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let argv_ty = i8_ptr.ptr_type(AddressSpace::default());
        if self.module.get_function("draton_set_cli_args").is_none() {
            self.module.add_function(
                "draton_set_cli_args",
                self.context
                    .void_type()
                    .fn_type(&[self.context.i32_type().into(), argv_ty.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_cli_argc").is_none() {
            self.module.add_function(
                "draton_cli_argc",
                self.context.i64_type().fn_type(&[], false),
                None,
            );
        }
        if self.module.get_function("draton_cli_arg").is_none() {
            self.module.add_function(
                "draton_cli_arg",
                self.string_type
                    .fn_type(&[self.context.i64_type().into()], false),
                None,
            );
        }
        if self.module.get_function("draton_host_ast_dump").is_none() {
            self.module.add_function(
                "draton_host_ast_dump",
                self.string_type.fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        Ok(())
    }

    fn declare_panic_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_function("draton_panic").is_some() {
            return Ok(());
        }
        let function = self.module.add_function(
            "draton_panic",
            self.context.void_type().fn_type(
                &[
                    self.string_type.into(),
                    self.string_type.into(),
                    self.context.i64_type().into(),
                ],
                false,
            ),
            None,
        );
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(function, "entry");
        builder.position_at_end(entry);
        let print = self
            .module
            .get_function("draton_print")
            .ok_or_else(|| CodeGenError::MissingSymbol("draton_print".to_string()))?;
        let abort = self
            .module
            .get_function("abort")
            .ok_or_else(|| CodeGenError::MissingSymbol("abort".to_string()))?;
        let msg = function
            .get_nth_param(0)
            .ok_or_else(|| CodeGenError::Llvm("missing draton_panic message param".to_string()))?;
        let _ = builder
            .build_call(print, &[msg.into()], "")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let _ = builder
            .build_call(abort, &[], "")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        builder
            .build_unreachable()
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }
}
