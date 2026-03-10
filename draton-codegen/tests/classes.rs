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
    assert!(
        ir.contains("i16 1") || ir.contains("i16 2") || ir.contains("i16 3"),
        "{ir}"
    );
}

#[test]
fn emits_generic_stack_int_as_monomorphized_struct() {
    let ir = compile_ir(
        r#"
class Stack[T] {
    let items: Array[T]
    fn len() { 0 }
}
@type { fn size_int(stack: Stack[Int]) -> Int }
fn size_int(stack) { stack.len() }
"#,
    );
    assert!(ir.contains("%Stack__Int = type"), "{ir}");
    assert!(
        ir.contains("define i64 @\"Stack__Int.len\"(%Stack__Int* %0)")
            || ir.contains("define i64 @Stack__Int.len(%Stack__Int* %0)"),
        "{ir}"
    );
    assert!(
        !ir.contains("%Stack = type") && !ir.contains("define i64 @\"Stack.len\""),
        "{ir}"
    );
}

#[test]
fn emits_two_generic_instantiations_as_distinct_structs() {
    let ir = compile_ir(
        r#"
class Stack[T] {
    let items: Array[T]
    fn len() { 0 }
}
@type { fn size_int(stack: Stack[Int]) -> Int }
fn size_int(stack) { stack.len() }
@type { fn size_string(stack: Stack[String]) -> Int }
fn size_string(stack) { stack.len() }
"#,
    );
    assert!(ir.contains("%Stack__Int = type"), "{ir}");
    assert!(ir.contains("%Stack__String = type"), "{ir}");
    assert!(
        ir.contains("define i64 @\"Stack__Int.len\"") || ir.contains("define i64 @Stack__Int.len"),
        "{ir}"
    );
    assert!(
        ir.contains("define i64 @\"Stack__String.len\"")
            || ir.contains("define i64 @Stack__String.len"),
        "{ir}"
    );
}

#[test]
fn emits_nested_generic_instantiation_names() {
    let ir = compile_ir(
        r#"
class Stack[T] {
    let items: Array[T]
    fn len() { 0 }
}
@type { fn size_nested(stack: Stack[Array[Int]]) -> Int }
fn size_nested(stack) { stack.len() }
"#,
    );
    assert!(ir.contains("%Stack__Array_Int_ = type"), "{ir}");
    assert!(
        ir.contains("define i64 @\"Stack__Array_Int_.len\"")
            || ir.contains("define i64 @Stack__Array_Int_.len"),
        "{ir}"
    );
}

#[test]
fn emits_interface_vtable_type_and_global() {
    let ir = compile_ir(
        r#"
interface Drawable {
    fn draw()
}
class Circle implements Drawable {
    fn draw() { print("circle") }
}
"#,
    );
    assert!(ir.contains("%Drawable_vtable = type"), "{ir}");
    assert!(
        ir.contains("%Drawable = type { i8*, %Drawable_vtable* }"),
        "{ir}"
    );
    assert!(
        ir.contains("@Circle_Drawable_vtable = constant %Drawable_vtable"),
        "{ir}"
    );
    assert!(ir.contains("@Circle.Drawable.draw_thunk"), "{ir}");
}

#[test]
fn emits_interface_dispatch_as_indirect_vtable_call() {
    let ir = compile_ir(
        r#"
interface Drawable {
    fn draw()
}
class Circle implements Drawable {
    fn draw() { print("circle") }
}
@type { fn render(shape: Drawable) -> Unit }
fn render(shape) {
    shape.draw()
}
"#,
    );
    assert!(ir.contains("load void (i8*)*"), "{ir}");
    assert!(ir.contains("call void %"), "{ir}");
}

#[test]
fn emits_upcast_to_interface_fat_pointer() {
    let ir = compile_ir(
        r#"
interface Drawable {
    fn draw()
}
class Circle implements Drawable {
    fn draw() { print("circle") }
}
@type { fn render(shape: Drawable) -> Unit }
fn render(shape) {
    shape.draw()
}
@type { fn use_circle(shape: Circle) -> Unit }
fn use_circle(shape) {
    render(shape)
}
"#,
    );
    assert!(ir.contains("insertvalue %Drawable"), "{ir}");
    assert!(ir.contains("@Circle_Drawable_vtable"), "{ir}");
}

#[test]
fn child_calls_parent_method_with_parent_upcast() {
    let ir = compile_ir(
        r#"
class Animal {
    fn speak() { print("...") }
}
class Dog extends Animal {
    fn fetch() { print("fetch!") }
}
@type { fn call_speak(d: Dog) -> Unit }
fn call_speak(d) {
    d.speak()
}
"#,
    );
    let function_start = ir.find("define void @call_speak").expect("call_speak");
    let function_ir = &ir[function_start..];
    assert!(
        function_ir.contains("call void @Animal.speak(%Animal*"),
        "{function_ir}"
    );
    assert!(
        function_ir.contains("%upcast.ptr") || function_ir.contains("%parent.ptr"),
        "{function_ir}"
    );
}

#[test]
fn child_layout_keeps_parent_as_first_field() {
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
fn override_prefers_child_method_symbol() {
    let ir = compile_ir(
        r#"
class Animal {
    fn speak() { print("...") }
}
class Dog extends Animal {
    fn speak() { print("woof") }
}
@type { fn call_speak(d: Dog) -> Unit }
fn call_speak(d) {
    d.speak()
}
"#,
    );
    let function_start = ir.find("define void @call_speak").expect("call_speak");
    let function_ir = &ir[function_start..];
    assert!(
        function_ir.contains("call void @Dog.speak(%Dog*"),
        "{function_ir}"
    );
    assert!(
        !function_ir.contains("call void @Animal.speak"),
        "{function_ir}"
    );
}
