use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::TypeChecker;
use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;

fn compile_module<'ctx>(context: &'ctx Context, source: &str) -> inkwell::module::Module<'ctx> {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(
        parsed.errors.is_empty(),
        "parser errors: {:?}",
        parsed.errors
    );
    let typed = TypeChecker::new().check(parsed.program);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
    CodeGen::new(context, BuildMode::Debug)
        .emit(&typed.typed_program)
        .expect("codegen")
}

unsafe fn run_i64_main(module: inkwell::module::Module<'_>) -> i64 {
    type Main = unsafe extern "C" fn() -> i64;
    let ee = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit engine");
    let main: JitFunction<Main> = ee.get_function("main").expect("main fn");
    main.call()
}

#[test]
fn emits_function_signatures_and_calls() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
@type { fn add(a: Int, b: Int) -> Int }
fn add(a, b) { a + b }
fn main() { add(2, 3) }
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("define i64 @add(i64 %0, i64 %1)"), "{ir}");
    assert!(ir.contains("call i64 @add(i64 2, i64 3)"), "{ir}");
}

#[test]
fn jit_runs_recursive_function() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
@type { fn recur(n: Int) -> Int }
fn recur(n) {
    match n {
        0 => 0
        1 => recur(0) + 1
        2 => recur(1) + 1
        3 => recur(2) + 1
        4 => recur(3) + 1
        5 => recur(4) + 1
    }
}
fn main() { recur(5) }
"#,
    );
    let value = unsafe { run_i64_main(module) };
    assert_eq!(value, 5);
}

#[test]
fn monomorphizes_generic_function_per_concrete_call_site() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
fn id(x) { x }
fn use_int() { id(42) }
fn use_string() { id("x") }
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("define i64 @id__Int(i64 %0)"), "{ir}");
    assert!(
        ir.contains("define { i64, i8* } @id__String({ i64, i8* } %0)"),
        "{ir}"
    );
    assert!(!ir.contains("define i64 @id("), "{ir}");
}
