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
fn emits_named_struct_and_self_method_signature() {
    let ir = compile_ir(
        r#"
class User {
    let name: String
    fn getName() { self.name }
}
"#,
    );
    assert!(ir.contains("%User = type { { i64, i8* } }"), "{ir}");
    assert!(
        ir.contains("define { i64, i8* } @User.getName(%User* %0)"),
        "{ir}"
    );
    assert!(ir.contains("define %User* @User_new()"), "{ir}");
}

#[test]
fn emits_inheritance_as_parent_first_field() {
    let ir = compile_ir(
        r#"
class Animal {
    let name: String
}
class Dog extends Animal {
    let age: Int
}
"#,
    );
    assert!(ir.contains("%Dog = type { %Animal, i64 }"), "{ir}");
}
