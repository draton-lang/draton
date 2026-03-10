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
fn destructure_emits_extract_value_for_each_binding() {
    let ir = compile_ir(
        r#"
fn pair() {
    (1, 2)
}
fn main() {
    let (x, y) = pair()
    x + y
}
"#,
    );
    assert!(ir.contains("extractvalue { i64, i64 } %"), "{ir}");
    assert!(ir.matches("extractvalue").count() >= 2, "{ir}");
}

#[test]
fn wildcard_destructure_skips_alloca_for_discarded_slot() {
    let ir = compile_ir(
        r#"
fn pair() {
    (1, 2)
}
fn main() {
    let (_, y) = pair()
    y
}
"#,
    );
    let function_start = ir.find("define i64 @main").expect("main function");
    let function_ir = &ir[function_start..];
    assert!(
        function_ir.matches("extractvalue").count() == 1,
        "{function_ir}"
    );
    assert!(
        function_ir.matches("alloca i64").count() == 1,
        "{function_ir}"
    );
}
