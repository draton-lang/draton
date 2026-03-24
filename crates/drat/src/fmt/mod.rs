use draton_ast::Program;
use draton_lexer::Lexer;
use draton_parser::Parser;

mod exprs;
mod items;
mod printer;
mod stmts;

pub(crate) use printer::Printer;

/// Formats a Draton source string through the parser and pretty printer.
///
/// When the source cannot be lexed or parsed, the original input is returned unchanged.
pub(crate) fn format_source(source: &str) -> String {
    let lexed = Lexer::new(source).tokenize();
    if !lexed.errors.is_empty() {
        return source.to_string();
    }
    let parsed = Parser::new(lexed.tokens).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }
    format_program(&parsed.program)
}

fn format_program(program: &Program) -> String {
    let mut printer = Printer::new();
    printer.fmt_program(program);
    printer.result()
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn formats_simple_function() {
        let input = "fn add(a:Int,b:Int)->Int{a+b}";
        let output = format_source(input);
        assert_eq!(output, "fn add(a: Int, b: Int) -> Int {\n    a + b\n}\n");
    }

    #[test]
    fn preserves_class_with_layer() {
        let input = r#"class Foo { layer Bar { fn baz() { } } }"#;
        let output = format_source(input);
        assert!(output.contains("layer Bar {"));
        assert!(output.contains("fn baz() {"));
    }

    #[test]
    fn formatter_is_idempotent() {
        let source = r#"
fn add(a: Int, b: Int) -> Int {
    a + b
}

class Counter {
    let mut count: Int

    fn new() {
        self.count = 0
    }

    fn inc() {
        self.count++
    }
}
"#;
        let once = format_source(source);
        let twice = format_source(&once);
        assert_eq!(once, twice, "formatter must be idempotent");
    }

    #[test]
    fn parse_error_returns_source_unchanged() {
        let bad = "fn broken( { }";
        assert_eq!(format_source(bad), bad);
    }

    #[test]
    fn normalizes_operator_spacing() {
        let input = "fn f() { let x=1+2*3 }";
        let out = format_source(input);
        assert!(out.contains("1 + 2 * 3"), "operators need spaces");
    }

    #[test]
    fn inserts_blank_line_between_top_level_items() {
        let input = "fn a() { }\nfn b() { }";
        let out = format_source(input);
        assert!(out.contains("}\n\nfn"), "need blank line between fns");
    }

    #[test]
    fn formats_tuple_destructure_let() {
        let input = "fn main(){let(x,_)= (1,2)}";
        let out = format_source(input);
        assert!(out.contains("let (x, _) = (1, 2)"), "{out}");
    }
}
