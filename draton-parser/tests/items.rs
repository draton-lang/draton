use draton_ast::{ClassMember, Item, Stmt, TypeMember};
use draton_lexer::Lexer;
use draton_parser::{ParseError, Parser};
use pretty_assertions::assert_eq;

fn parse_program(source: &str) -> draton_parser::ParseResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    Parser::new(lexed.tokens).parse()
}

#[test]
fn parses_functions_with_binding_style_type_blocks_and_pub() {
    let source = "@type { add: (Int, Int) -> Int }\npub fn add(a, b) { return a + b }";
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert_eq!(result.program.items.len(), 2);
    assert!(matches!(
        &result.program.items[0],
        Item::TypeBlock(block)
            if matches!(
                &block.members[0],
                TypeMember::Binding { name, .. } if name == "add"
            )
    ));
    assert!(
        matches!(&result.program.items[1], Item::Fn(function) if function.is_pub && function.name == "add")
    );
}

#[test]
fn parses_class_interface_enum_error_and_const() {
    let source = r#"
class Dog extends Animal implements Drawable {
    let name

    layer Voice {
        fn speak() { return "Woof!" }
    }

    @type {
        name: String
        speak: () -> String
    }
}
interface Drawable {
    fn draw()
}
enum Color { Red, Green, Blue }
error NotFound(msg)
const MAX = 100
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert!(matches!(
        &result.program.items[0],
        Item::Class(class_def)
            if class_def.extends.as_deref() == Some("Animal")
                && class_def.implements == vec!["Drawable".to_string()]
                && class_def.type_blocks.len() == 1
    ));
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
} from std.io
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
    assert!(matches!(
        &result.program.items[0],
        Item::Import(import)
            if import.items.len() == 2
                && import.items[0].alias.as_deref() == Some("f")
                && import.module == vec!["std".to_string(), "io".to_string()]
    ));
    assert!(
        matches!(&result.program.items[1], Item::Extern(ext) if ext.abi == "C" && ext.functions.len() == 2)
    );
}

#[test]
fn parses_class_with_layers_and_type_blocks() {
    let source = r#"
class UserService {
    layer Validation {
        fn validateName(name) { return Ok(()) }
        @type {
            validateName: (String) -> Result[Unit, Unit]
        }
    }
    layer Persistence {
        pub fn save(user) { return Ok(()) }
    }
    @type {
        save: (User) -> Result[Unit, Unit]
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
    assert_eq!(layers[0].type_blocks.len(), 1);
    assert_eq!(layers[1].name, "Persistence");
    assert!(layers[1].methods[0].is_pub);
    assert_eq!(class_def.type_blocks.len(), 1);
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
        fn bar() { return 42 }
    }

    fn baz() {
        return self.bar()
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

#[test]
fn parses_generic_class_type_params() {
    let source = r#"
class Stack[T] {
    let items

    layer Ops {
        fn push(item) { return None }
        fn pop() { return None }
    }

    @type {
        items: Array[T]
        push: (T) -> Option[T]
        pop: () -> Option[T]
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
        panic!("expected class");
    };
    assert_eq!(class_def.type_params, vec!["T".to_string()]);
    assert_eq!(class_def.type_blocks.len(), 1);
}

#[test]
fn parses_top_level_import_with_module_path_and_aliases() {
    let source = r#"
import { connect, listen as serve } from net.http
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    let Item::Import(import) = &result.program.items[0] else {
        panic!("expected import");
    };
    assert_eq!(import.module, vec!["net".to_string(), "http".to_string()]);
    assert_eq!(import.items.len(), 2);
    assert_eq!(import.items[1].alias.as_deref(), Some("serve"));
}

#[test]
fn parses_class_and_layer_type_blocks() {
    let source = r#"
class User {
    let name

    layer Info {
        fn greet() { return "hello " + self.name }
        @type {
            greet: () -> String
        }
    }

    @type {
        name: String
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
        panic!("expected class");
    };
    assert_eq!(class_def.type_blocks.len(), 1);
    let layer = class_def
        .members
        .iter()
        .find_map(|member| match member {
            ClassMember::Layer(layer) => Some(layer),
            _ => None,
        })
        .expect("expected layer");
    assert_eq!(layer.type_blocks.len(), 1);
}

#[test]
fn parses_interface_type_blocks() {
    let source = r#"
interface Drawable {
    fn draw()
    @type {
        draw: () -> Int
    }
}
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    let Item::Interface(interface_def) = &result.program.items[0] else {
        panic!("expected interface");
    };
    assert_eq!(interface_def.methods.len(), 1);
    assert_eq!(interface_def.type_blocks.len(), 1);
}

#[test]
fn parses_function_scope_type_blocks() {
    let source = r#"
fn main() {
    @type {
        head: Node??
    }
    let head = None
    return head
}
"#;
    let result = parse_program(source);
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    let Item::Fn(function) = &result.program.items[0] else {
        panic!("expected function");
    };
    let body = function.body.as_ref().expect("expected body");
    assert!(matches!(body.stmts.first(), Some(Stmt::TypeBlock(_))));
}
