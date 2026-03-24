use draton_codegen::{BuildMode, CodeGen};
use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::TypeChecker;
use inkwell::context::Context;

fn compile_ir(source: &str) -> String {
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
    let context = Context::create();
    let module = CodeGen::new(&context, BuildMode::Debug)
        .emit(&typed.typed_program)
        .expect("codegen");
    module.print_to_string().to_string()
}

#[test]
fn emits_integer_float_and_bool_literals() {
    let ir = compile_ir(
        r#"
fn int_value() { 42 }
fn float_value() { 3.14 }
fn bool_value() { true }
"#,
    );
    assert!(ir.contains("ret i64 42"), "{ir}");
    assert!(ir.contains("ret double 3.140000e+00"), "{ir}");
    assert!(
        ir.contains("ret i1 true") || ir.contains("ret i1 1"),
        "{ir}"
    );
}

#[test]
fn emits_string_struct_layout_and_global_literal() {
    let ir = compile_ir(
        r#"
fn greet() { "hello" }
"#,
    );
    assert!(ir.contains("{ i64, i8* }"), "{ir}");
    assert!(ir.contains("hello\\00"), "{ir}");
}
