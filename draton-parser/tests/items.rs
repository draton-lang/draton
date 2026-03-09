use draton_ast::{ClassMember, Item, TypeMember};
use draton_lexer::Lexer;
use draton_parser::{ParseError, Parser};
use pretty_assertions::assert_eq;

fn parse_program(source: &str) -> draton_parser::ParseResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    Parser::new(lexed.tokens).parse()
}

#[test]
fn parses_functions_with_type_blocks_and_pub() {
    let source = "@type { fn add(a: Int, b: Int) -> Int }\npub fn add(a, b) { a + b }";
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert_eq!(result.program.items.len(), 2);
    assert!(
        matches!(&result.program.items[0], Item::TypeBlock(block) if matches!(&block.members[0], TypeMember::Function(_)))
    );
    assert!(
        matches!(&result.program.items[1], Item::Fn(function) if function.is_pub && function.name == "add")
    );
}

#[test]
fn parses_class_interface_enum_error_and_const() {
    let source = r#"
class Dog extends Animal implements Drawable {
    let name: String
    fn speak() { print("Woof!") }
}
interface Drawable {
    fn draw()
}
enum Color { Red, Green, Blue }
error NotFound(msg: String)
const MAX = 100
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert!(
        matches!(&result.program.items[0], Item::Class(class_def) if class_def.extends.as_deref() == Some("Animal") && class_def.implements == vec!["Drawable".to_string()] && matches!(class_def.members[1], ClassMember::Method(_)))
    );
    assert!(matches!(&result.program.items[1], Item::Interface(_)));
    assert!(
        matches!(&result.program.items[2], Item::Enum(enum_def) if enum_def.variants.len() == 3)
    );
    assert!(
        matches!(&result.program.items[3], Item::Error(error_def) if error_def.fields.len() == 1)
    );
    assert!(matches!(&result.program.items[4], Item::Const(const_def) if const_def.name == "MAX"));
}

#[test]
fn parses_imports_and_extern_blocks() {
    let source = r#"
import {
    fs as f
    net as n
}
@extern "C" {
    fn malloc(size: UInt64) -> @pointer
    fn free(ptr: @pointer)
}
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert!(
        matches!(&result.program.items[0], Item::Import(import) if import.items.len() == 2 && import.items[0].alias.as_deref() == Some("f"))
    );
    assert!(
        matches!(&result.program.items[1], Item::Extern(ext) if ext.abi == "C" && ext.functions.len() == 2)
    );
}

#[test]
fn parses_class_with_layers() {
    let source = r#"
class UserService {
    layer Validation {
        fn validateName(name) { Ok(()) }
    }
    layer Persistence {
        pub fn save(user) { Ok(()) }
    }
}
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );

    let Item::Class(class_def) = &result.program.items[0] else {
        panic!("expected class item");
    };
    let layers = class_def
        .members
        .iter()
        .filter_map(|member| match member {
            ClassMember::Layer(layer) => Some(layer),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].name, "Validation");
    assert_eq!(layers[0].methods.len(), 1);
    assert_eq!(layers[1].name, "Persistence");
    assert!(layers[1].methods[0].is_pub);
}

#[test]
fn reports_nested_layer_error() {
    let source = r#"
class Foo {
    layer Outer {
        layer Inner { }
    }
}
"#;
    let result = parse_program(source);
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, ParseError::NestedLayerNotAllowed { .. })));
}

#[test]
fn reports_layer_outside_class_error() {
    let source = r#"
layer Validation {
    fn validate() { }
}
"#;
    let result = parse_program(source);
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, ParseError::LayerOutsideClass { .. })));
}

#[test]
fn parses_calls_to_layer_methods_on_self() {
    let source = r#"
class Foo {
    layer A {
        fn bar() { 42 }
    }

    fn baz() {
        self.bar()
    }
}
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
}
