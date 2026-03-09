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
fn emits_branches_for_if_and_while() {
    let ir = compile_ir(
        r#"
fn main() {
    let mut x = 0
    if (x == 0) { x = 1 } else { x = 2 }
    while (x < 4) { x++ }
    x
}
"#,
    );
    assert!(ir.contains("if.then"), "{ir}");
    assert!(ir.contains("while.cond"), "{ir}");
    assert!(ir.contains("br i1"), "{ir}");
}

#[test]
fn emits_switch_for_integer_match() {
    let ir = compile_ir(
        r#"
fn main() {
    match 2 {
        1 => 10
        2 => 20
    }
}
"#,
    );
    assert!(ir.contains("switch i64 2"), "{ir}");
}
