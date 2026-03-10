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

#[test]
fn emits_write_barrier_for_field_assignment() {
    let ir = compile_ir(
        r#"
class Node {
    let next: Node
    fn link(next: Node) {
        self.next = next
    }
}
"#,
    );
    let method_start = ir.find("define void @Node.link").expect("Node.link");
    let method_ir = &ir[method_start..];
    assert!(
        method_ir.contains("call void @draton_gc_write_barrier"),
        "{method_ir}"
    );
}

#[test]
fn emits_gc_roots_for_pointer_backed_locals() {
    let ir = compile_ir(
        r#"
class User { }
@type { fn main(user: User) -> User }
fn main(user) {
    let other = user
    other
}
"#,
    );
    assert!(ir.contains("gc \"shadow-stack\""), "{ir}");
    assert!(ir.contains("call void @llvm.gcroot"), "{ir}");
}

#[test]
fn emits_type_descriptors_and_non_zero_type_ids_for_classes() {
    let ir = compile_ir(
        r#"
class User {
    let next: User
}
"#,
    );
    assert!(ir.contains("@TypeDesc_User = constant"), "{ir}");
    assert!(ir.contains("@draton_gc_alloc(i64"), "{ir}");
    assert!(ir.contains("i16 1"), "{ir}");
}
