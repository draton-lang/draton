use std::io::{self, Write};

use anyhow::Result;
use draton_typeck::Type;
use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;

use crate::commands::build::{self, Profile};

pub(crate) fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "drat> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, ":quit" | ":exit") {
            break;
        }
        if input == ":help" {
            println!(":quit | :exit | :help");
            continue;
        }
        let wrapped = format!("fn main() {{ {input} }}\n");
        match build::compile_snippet(&wrapped) {
            Ok(compiled) => {
                let ty = build::main_return_type(&compiled).unwrap_or(Type::Unit);
                println!("type: {ty}");
                if matches!(ty, Type::Int | Type::Bool | Type::Unit) {
                    let context = Context::create();
                    let module = build::build_module(&context, &compiled, Profile::Debug)?;
                    unsafe {
                        run_jit(module, &ty)?;
                    }
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
    Ok(())
}

unsafe fn run_jit(module: inkwell::module::Module<'_>, ty: &Type) -> Result<()> {
    let ee = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    match ty {
        Type::Int | Type::Bool => {
            type Main = unsafe extern "C" fn() -> i64;
            let main: JitFunction<Main> = ee
                .get_function("main")
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            println!("value: {}", main.call());
        }
        Type::Unit => {
            type Main = unsafe extern "C" fn();
            let main: JitFunction<Main> = ee
                .get_function("main")
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            main.call();
        }
        _ => {}
    }
    Ok(())
}
