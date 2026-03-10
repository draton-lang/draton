use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::TypeChecker;
use inkwell::context::Context;

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

fn compile_ir(source: &str) -> String {
    let context = Context::create();
    let module = compile_module(&context, source);
    module.print_to_string().to_string()
}

#[test]
fn emits_simple_lambda_without_environment_allocation() {
    let ir = compile_ir(
        r#"
fn main() {
    let double = lambda x => x * 2
    double(5)
}
"#,
    );
    assert!(
        ir.contains("define i64 @closure_body_0(i8* %0, i64 %1)"),
        "{ir}"
    );
    assert!(ir.contains("%draton.closure = type { i8*, i8* }"), "{ir}");
    assert!(!ir.contains("%closure_env_0 = type"), "{ir}");
}

#[test]
fn emits_lambda_with_capture_environment() {
    let ir = compile_ir(
        r#"
fn main() {
    let y = 10
    let add_y = lambda x => x + y
    add_y(5)
}
"#,
    );
    assert!(ir.contains("%closure_env_0 = type { i64* }"), "{ir}");
    assert!(ir.contains("load i64*, i64**"), "{ir}");
    assert!(ir.contains("call i64"), "{ir}");
}

#[test]
fn emits_lambda_passed_through_function_parameter() {
    let ir = compile_ir(
        r#"
fn apply(f, x) { f(x) }
fn main() {
    apply(lambda x => x + 1, 41)
}
"#,
    );
    assert!(
        ir.contains("define i64 @apply__Int_Int(%draton.closure* %0, i64 %1)")
            || ir.contains("define i64 @apply(%draton.closure* %0, i64 %1)"),
        "{ir}"
    );
    assert!(
        ir.contains("%closure.call = call i64 %closure.fn.typed(i8* %closure.env, i64 %x2)")
            || ir.contains("%closure.call = call i64 %closure.fn.typed"),
        "{ir}"
    );
}

#[test]
fn emits_nested_lambda_capture_without_escape() {
    let ir = compile_ir(
        r#"
fn main() {
    let a = 1
    let outer = lambda x => (lambda y => x + y + a)(2)
    outer(10)
}
"#,
    );
    assert!(ir.contains("define i64 @closure_body_0"), "{ir}");
    assert!(ir.contains("define i64 @closure_body_1"), "{ir}");
    assert!(ir.contains("%closure_env_1 = type { i64*, i64* }"), "{ir}");
}
