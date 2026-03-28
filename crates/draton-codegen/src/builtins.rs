use std::env;

use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;

impl<'ctx> CodeGen<'ctx> {
    fn should_emit_runtime_fallbacks() -> bool {
        env::var_os("DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS").is_none()
    }

    pub(crate) fn declare_runtime(&mut self) -> Result<(), CodeGenError> {
        self.declare_libc()?;
        self.declare_print_runtime()?;
        self.declare_input_runtime()?;
        self.declare_string_runtime()?;
        self.declare_cli_runtime()?;
        self.declare_panic_runtime()?;
        if let Some(print_fn) = self.module.get_function("draton_print") {
            self.functions.insert("print".to_string(), print_fn);
        }
        if let Some(println_fn) = self.module.get_function("draton_println") {
            self.functions.insert("println".to_string(), println_fn);
        }
        if let Some(input_fn) = self.module.get_function("draton_input") {
            self.functions.insert("input".to_string(), input_fn);
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
        if self
            .module
            .get_function(Self::output_write_symbol())
            .is_none()
        {
            self.module.add_function(
                Self::output_write_symbol(),
                Self::output_write_return_type(self.context).fn_type(
                    &[
                        self.context.i32_type().into(),
                        i8_ptr.into(),
                        Self::output_write_len_type(self.context).into(),
                    ],
                    false,
                ),
                None,
            );
        }
        if self.module.get_function("abort").is_none() {
            self.module
                .add_function("abort", self.context.void_type().fn_type(&[], false), None);
        }
        Ok(())
    }

    fn declare_print_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_function("draton_print").is_some() {
            return Ok(());
        }
        let print_fn = self.module.add_function(
            "draton_print",
            self.context
                .void_type()
                .fn_type(&[self.string_type.into()], false),
            None,
        );
        let println_fn = self.module.add_function(
            "draton_println",
            self.context
                .void_type()
                .fn_type(&[self.string_type.into()], false),
            None,
        );
        let write_fn = self
            .module
            .get_function(Self::output_write_symbol())
            .ok_or_else(|| CodeGenError::MissingSymbol(Self::output_write_symbol().to_string()))?;
        self.build_print_fallback(print_fn, write_fn, false)?;
        self.build_print_fallback(println_fn, write_fn, true)?;
        Ok(())
    }

    fn declare_input_runtime(&mut self) -> Result<(), CodeGenError> {
        if self.module.get_function("draton_input").is_some() {
            return Ok(());
        }
        self.module.add_function(
            "draton_input",
            self.string_type.fn_type(&[self.string_type.into()], false),
            None,
        );
        Ok(())
    }

    fn build_print_fallback(
        &self,
        function: FunctionValue<'ctx>,
        write_fn: FunctionValue<'ctx>,
        append_newline: bool,
    ) -> Result<(), CodeGenError> {
        let builder = self.context.create_builder();
        let entry = self.context.append_basic_block(function, "entry");
        builder.position_at_end(entry);
        let value = function
            .get_first_param()
            .ok_or_else(|| CodeGenError::Llvm("missing draton_print string param".to_string()))?
            .into_struct_value();
        let len = builder
            .build_extract_value(value, 0, "str.len")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let ptr = builder
            .build_extract_value(value, 1, "str.ptr")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        let len = if len.get_type() == Self::output_write_len_type(self.context) {
            len
        } else {
            builder
                .build_int_cast(
                    len,
                    Self::output_write_len_type(self.context),
                    "str.len.cast",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
        };
        let _ = builder
            .build_call(
                write_fn,
                &[
                    self.context.i32_type().const_int(1, false).into(),
                    ptr.into(),
                    len.into(),
                ],
                "",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        if append_newline {
            let newline = builder
                .build_global_string_ptr("\n", "draton.println.nl")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            let _ = builder
                .build_call(
                    write_fn,
                    &[
                        self.context.i32_type().const_int(1, false).into(),
                        newline.as_pointer_value().into(),
                        Self::output_write_len_type(self.context)
                            .const_int(1, false)
                            .into(),
                    ],
                    "",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        builder
            .build_return(None)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }

    #[cfg(windows)]
    fn output_write_symbol() -> &'static str {
        "_write"
    }

    #[cfg(not(windows))]
    fn output_write_symbol() -> &'static str {
        "write"
    }

    #[cfg(windows)]
    fn output_write_len_type(
        context: &'ctx inkwell::context::Context,
    ) -> inkwell::types::IntType<'ctx> {
        context.i32_type()
    }

    #[cfg(not(windows))]
    fn output_write_len_type(
        context: &'ctx inkwell::context::Context,
    ) -> inkwell::types::IntType<'ctx> {
        context.i64_type()
    }

    #[cfg(windows)]
    fn output_write_return_type(
        context: &'ctx inkwell::context::Context,
    ) -> inkwell::types::IntType<'ctx> {
        context.i32_type()
    }

    #[cfg(not(windows))]
    fn output_write_return_type(
        context: &'ctx inkwell::context::Context,
    ) -> inkwell::types::IntType<'ctx> {
        context.i64_type()
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
        if self.module.get_function("draton_str_contains").is_none() {
            self.module.add_function(
                "draton_str_contains",
                self.context
                    .bool_type()
                    .fn_type(&[self.string_type.into(), self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_str_starts_with").is_none() {
            self.module.add_function(
                "draton_str_starts_with",
                self.context
                    .bool_type()
                    .fn_type(&[self.string_type.into(), self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_str_eq").is_none() {
            self.module.add_function(
                "draton_str_eq",
                self.context
                    .bool_type()
                    .fn_type(&[self.string_type.into(), self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_str_replace").is_none() {
            self.module.add_function(
                "draton_str_replace",
                self.string_type.fn_type(
                    &[
                        self.string_type.into(),
                        self.string_type.into(),
                        self.string_type.into(),
                    ],
                    false,
                ),
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
        if self.module.get_function("draton_read_file").is_none() {
            self.module.add_function(
                "draton_read_file",
                self.string_type.fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        if self
            .module
            .get_function("draton_string_parse_int")
            .is_none()
        {
            self.module.add_function(
                "draton_string_parse_int",
                self.context
                    .i64_type()
                    .fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        if self
            .module
            .get_function("draton_string_parse_int_radix")
            .is_none()
        {
            self.module.add_function(
                "draton_string_parse_int_radix",
                self.context.i64_type().fn_type(
                    &[self.string_type.into(), self.context.i64_type().into()],
                    false,
                ),
                None,
            );
        }
        if self
            .module
            .get_function("draton_string_parse_float")
            .is_none()
        {
            self.module.add_function(
                "draton_string_parse_float",
                self.context
                    .f64_type()
                    .fn_type(&[self.string_type.into()], false),
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
        if self.module.get_function("draton_host_type_dump").is_none() {
            self.module.add_function(
                "draton_host_type_dump",
                self.string_type.fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_host_lex_json").is_none() {
            self.module.add_function(
                "draton_host_lex_json",
                self.string_type.fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_host_parse_json").is_none() {
            self.module.add_function(
                "draton_host_parse_json",
                self.string_type.fn_type(&[self.string_type.into()], false),
                None,
            );
        }
        if self.module.get_function("draton_host_type_json").is_none() {
            self.module.add_function(
                "draton_host_type_json",
                self.string_type.fn_type(
                    &[self.string_type.into(), self.context.i64_type().into()],
                    false,
                ),
                None,
            );
        }
        if self.module.get_function("draton_host_build_json").is_none() {
            self.module.add_function(
                "draton_host_build_json",
                self.string_type.fn_type(
                    &[
                        self.string_type.into(),
                        self.string_type.into(),
                        self.string_type.into(),
                        self.context.i64_type().into(),
                        self.string_type.into(),
                    ],
                    false,
                ),
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
        if Self::should_emit_runtime_fallbacks() {
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
            let msg = function.get_nth_param(0).ok_or_else(|| {
                CodeGenError::Llvm("missing draton_panic message param".to_string())
            })?;
            let _ = builder
                .build_call(print, &[msg.into()], "")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            let _ = builder
                .build_call(abort, &[], "")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            builder
                .build_unreachable()
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }
        Ok(())
    }
}
