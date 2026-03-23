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
fn compiles_hello_world_and_calls_print() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
fn main() {
    print("hello")
    0
}
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("@draton_print"), "{ir}");
}

#[test]
fn compiles_print_and_println_to_distinct_runtime_symbols() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
fn main() {
    print("a")
    println("b")
    0
}
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("@draton_print("), "{ir}");
    assert!(ir.contains("@draton_println("), "{ir}");
}

#[test]
fn compiles_input_to_runtime_symbol() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
fn main() {
    let name = input("Name: ")
    println(name)
    0
}
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("@draton_input("), "{ir}");
}

#[test]
fn compiles_fibonacci_program() {
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
        6 => recur(5) + 1
        7 => recur(6) + 1
        8 => recur(7) + 1
        9 => recur(8) + 1
        10 => recur(9) + 1
        _ => 0
    }
}
fn main() { recur(10) }
"#,
    );
    assert_eq!(unsafe { run_i64_main(module) }, 10);
}

#[test]
fn compiles_method_dispatch_on_typed_parameter() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
class User {
    let name: String
    fn getName() { self.name }
}
@type { fn nameOf(u: User) -> String }
fn nameOf(u) { u.getName() }
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("call { i64, i8* } @\"User.getName\"")
            || ir.contains("call { i64, i8* } @User.getName"),
        "{ir}"
    );
}

#[test]
fn no_gc_alloc_in_emitted_ir() {
    let context = Context::create();
    let module = compile_module(
        &context,
        r#"
class User { }
fn main() {
    let user = User()
    0
}
"#,
    );
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("call i8* @malloc"), "{ir}");
    assert!(!ir.contains("draton_gc_alloc"), "{ir}");
}
