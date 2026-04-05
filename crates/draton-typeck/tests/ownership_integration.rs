use draton_ast::Span;
use draton_lexer::Lexer;
use draton_parser::{ParseWarning, Parser};
use draton_typeck::typed_ast::{TypedFnDef, TypedItem};
use draton_typeck::{OwnershipError, TypeCheckResult, TypeChecker, TypeError, UseEffect};

struct CheckedProgram {
    parser_warnings: Vec<ParseWarning>,
    typed: TypeCheckResult,
}

fn compile_program(source: &str) -> CheckedProgram {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(
        parsed.errors.is_empty(),
        "parser errors: {:?}",
        parsed.errors
    );
    let parser_warnings = parsed.warnings;
    let typed = TypeChecker::new().check(parsed.program);
    CheckedProgram {
        parser_warnings,
        typed,
    }
}

fn assert_compiles(source: &str) -> CheckedProgram {
    let checked = compile_program(source);
    assert!(
        checked.parser_warnings.is_empty(),
        "parser warnings: {:?}",
        checked.parser_warnings
    );
    assert!(
        checked.typed.errors.is_empty(),
        "errors: {:?}",
        checked.typed.errors
    );
    checked
}

fn first_ownership_error(result: &TypeCheckResult) -> &OwnershipError {
    result
        .errors
        .iter()
        .find_map(|error| match error {
            TypeError::Ownership(ownership) => Some(ownership),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected ownership error, found {:?}", result.errors))
}

fn assert_span(span: &Span, line: usize, col: usize) {
    assert_eq!((span.line, span.col), (line, col));
}

fn find_function<'a>(result: &'a TypeCheckResult, name: &str) -> &'a TypedFnDef {
    result
        .typed_program
        .items
        .iter()
        .find_map(|item| match item {
            TypedItem::Fn(function) if function.name == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function '{name}'"))
}

#[test]
fn straight_line_alloc_and_free() {
    assert_compiles(
        r#"class User {
    let name

    @type {
        name: String
    }
}

fn main() {
    let user = User { name: input("name: ") }
    print(user.name.len())
}
"#,
    );
}

#[test]
fn borrow_across_two_calls() {
    assert_compiles(
        r#"@type {
    show: (String) -> Unit
}

fn show(text) {
    print(text.len())
}

fn main() {
    let name = input("name: ")
    show(name)
    show(name)
}
"#,
    );
}

#[test]
fn move_then_reassign() {
    assert_compiles(
        r#"fn forward(text) {
    return text
}

fn main() {
    let mut name = input("name: ")
    let out = forward(name)
    print(out.len())
    name = input("name again: ")
    print(name.len())
}
"#,
    );
}

#[test]
fn branch_local_free() {
    assert_compiles(
        r#"fn forward(text) {
    return text
}

fn main(flag) {
    let name = input("name: ")
    if flag {
        print(name.len())
    } else {
        let out = forward(name)
        print(out.len())
    }
}
"#,
    );
}

#[test]
fn early_return_frees_locals() {
    assert_compiles(
        r#"fn main(flag) {
    let name = input("name: ")
    if flag {
        return 0
    }
    print(name.len())
    return 1
}
"#,
    );
}

#[test]
fn non_escaping_closure_borrows() {
    assert_compiles(
        r#"fn main() {
    let name = input("name: ")
    let show = lambda => name.len()
    print(show())
    print(name.len())
}
"#,
    );
}

#[test]
fn escaping_closure_moves_capture() {
    assert_compiles(
        r#"fn make_reader() {
    let name = input("name: ")
    return lambda => name.len()
}
"#,
    );
}

#[test]
fn higher_order_borrow_contract() {
    assert_compiles(
        r#"@type {
    run: (fn(String) -> borrow, String) -> Unit
}

fn run(op, text) {
    op(text)
    print(text.len())
}
"#,
    );
}

#[test]
fn higher_order_move_contract() {
    assert_compiles(
        r#"@type {
    run: (fn(String) -> move, String) -> move
}

fn run(op, text) {
    return op(text)
}
"#,
    );
}

#[test]
fn recursive_borrow_convergence() {
    let checked = assert_compiles(
        r#"@type {
    walk: (String, Int) -> Unit
}

fn walk(text, n) {
    walk(text, n)
}
"#,
    );
    let function = find_function(&checked.typed, "walk");
    assert_eq!(
        function
            .ownership_summary
            .as_ref()
            .expect("ownership summary")
            .params[0]
            .effect,
        UseEffect::BorrowShared
    );
}

#[test]
fn recursive_move_convergence() {
    let checked = compile_program(
        r#"fn pass_down(text, n) {
    if n == 0 {
        return text
    }
    return pass_down(text, n)
}
"#,
    );
    assert!(
        checked.parser_warnings.is_empty(),
        "parser warnings: {:?}",
        checked.parser_warnings
    );
    let function = find_function(&checked.typed, "pass_down");
    assert_eq!(
        function
            .ownership_summary
            .as_ref()
            .expect("ownership summary")
            .params[0]
            .effect,
        UseEffect::Move
    );
}

#[test]
fn acyclic_class_no_cycle_check() {
    assert_compiles(
        r#"@acyclic
class Artifact {
    let path

    @type {
        path: String
    }
}

@acyclic
class Package {
    let name
    let artifacts

    @type {
        name: String
        artifacts: Array[Artifact]
    }
}

fn main() {
    let mut artifacts = []
    artifacts.push(Artifact { path: "main.o" })
    let pkg = Package { name: "app", artifacts: artifacts }
    print(pkg)
}
"#,
    );
}

#[test]
fn copy_type_aliasing_allowed() {
    assert_compiles(
        r#"fn main() {
    let a = 1
    let b = a
    print(a)
    print(b)
}
"#,
    );
}

#[test]
fn use_after_move() {
    let checked = compile_program(
        r#"fn forward(text) {
    return text
}

fn main() {
    let name = input("name: ")
    let out = forward(name)
    print(name.len())
    print(out.len())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::UseAfterMove {
            name,
            move_span,
            use_span,
        } => {
            assert_eq!(name, "name");
            assert_span(move_span, 7, 23);
            assert_span(use_span, 8, 11);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn move_while_borrowed() {
    let checked = compile_program(
        r#"fn forward(text) {
    return text
}

fn main() {
    let name = input("name: ")
    let reader = lambda => name.len()
    let out = forward(name)
    print(reader())
    print(out.len())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::MoveWhileBorrowed {
            name,
            borrow_span,
            move_span,
        } => {
            assert_eq!(name, "name");
            assert_span(borrow_span, 7, 18);
            assert_span(move_span, 8, 23);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn read_during_exclusive_borrow() {
    let checked = compile_program(
        r#"@type {
    append_one: (Array[String]) -> Unit
}

fn append_one(items) {
    items.push("x")
}

fn main() {
    let mut items = []
    @type {
        items: Array[String]
    }
    let writer = lambda => append_one(items)
    print(items.len())
    writer()
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::ReadDuringExclusiveBorrow {
            name,
            borrow_span,
            read_span,
        } => {
            assert_eq!(name, "items");
            assert_span(borrow_span, 14, 18);
            assert_span(read_span, 15, 11);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn exclusive_borrow_during_read() {
    let checked = compile_program(
        r#"fn main() {
    let mut items = []
    @type {
        items: Array[String]
    }
    let reader = lambda => items.len()
    items.push("x")
    print(reader())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::ExclusiveBorrowDuringRead {
            name,
            read_span,
            modify_span,
        } => {
            assert_eq!(name, "items");
            assert_span(read_span, 6, 18);
            assert_span(modify_span, 7, 5);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn partial_move_from_class() {
    let checked = compile_program(
        r#"class User {
    let name

    @type {
        name: String
    }
}

fn main() {
    let user = User { name: input("name: ") }
    let name = user.name
    print(name.len())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::PartialMove { field, base, span } => {
            assert_eq!(field, "name");
            assert_eq!(base, "user");
            assert_span(span, 11, 16);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn ambiguous_open_callee() {
    let checked = compile_program(
        r#"fn run(op, text) {
    op(text)
    print(text.len())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::AmbiguousCallOwnership { name, span } => {
            assert_eq!(name, "text");
            assert_span(span, 2, 5);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn loop_move_without_reinit() {
    let checked = compile_program(
        r#"fn forward(text) {
    return text
}

fn main(items) {
    let mut name = input("name: ")
    while items.len() > 0 {
        let out = forward(name)
        print(out.len())
    }
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::LoopMoveWithoutReinit { name, span } => {
            assert_eq!(name, "name");
            assert_span(span, 7, 5);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn two_escaping_closures_same_value() {
    let checked = compile_program(
        r#"fn main() {
    let name = input("name: ")
    return [lambda => name.len(), lambda => name.len()]
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::MultipleOwners { name, span } => {
            assert_eq!(name, "name");
            assert_span(span, 3, 35);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn direct_ownership_cycle() {
    let checked = compile_program(
        r#"@acyclic
class Node {
    let next

    @type {
        next: Node??
    }
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::OwnershipCycle { span } => assert_span(span, 3, 5),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn safe_to_raw_alias_rejection() {
    let checked = compile_program(
        r#"@extern "C" {
    fn raw_keep(name: String)
}

fn main() {
    let name = input("name: ")
    @pointer {
        raw_keep(name)
    }
    print(name.len())
}
"#,
    );
    let error = first_ownership_error(&checked.typed);
    match error {
        OwnershipError::SafeToRawAliasRejection { name, span } => {
            assert_eq!(name, "name");
            assert_span(span, 8, 18);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
