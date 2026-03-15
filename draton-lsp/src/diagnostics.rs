use crate::analysis::analyze;
use serde_json::{json, Value};

pub fn collect_diagnostics(text: &str) -> Value {
    let result = analyze(text);
    let mut diags = Vec::new();

    for error in &result.lex_errors {
        let (line, col) = match error {
            draton_lexer::LexError::UnexpectedChar { line, col, .. }
            | draton_lexer::LexError::UnterminatedString { line, col }
            | draton_lexer::LexError::UnterminatedBlockComment { line, col }
            | draton_lexer::LexError::InvalidNumericLiteral { line, col, .. } => (*line, *col),
        };
        diags.push(make_diag(
            line.saturating_sub(1),
            col.saturating_sub(1),
            line.saturating_sub(1),
            col.saturating_sub(1) + 1,
            1,
            &error.to_string(),
            "draton-lex",
        ));
    }

    for error in &result.parse_errors {
        let (line, col) = match error {
            draton_parser::ParseError::UnexpectedToken { line, col, .. }
            | draton_parser::ParseError::UnexpectedEof { line, col, .. }
            | draton_parser::ParseError::InvalidExpr { line, col }
            | draton_parser::ParseError::NestedLayerNotAllowed { line, col }
            | draton_parser::ParseError::LayerOutsideClass { line, col } => (*line, *col),
        };
        diags.push(make_diag(
            line.saturating_sub(1),
            col.saturating_sub(1),
            line.saturating_sub(1),
            col.saturating_sub(1) + 1,
            1,
            &error.to_string(),
            "draton-parse",
        ));
    }

    for error in &result.type_errors {
        let (line, col) = match error {
            draton_typeck::TypeError::Mismatch { line, col, .. }
            | draton_typeck::TypeError::UndefinedVar { line, col, .. }
            | draton_typeck::TypeError::UndefinedFn { line, col, .. }
            | draton_typeck::TypeError::NoField { line, col, .. }
            | draton_typeck::TypeError::BadBinOp { line, col, .. }
            | draton_typeck::TypeError::ArgCount { line, col, .. }
            | draton_typeck::TypeError::DestructureArity { line, col, .. }
            | draton_typeck::TypeError::CannotInfer { line, col, .. }
            | draton_typeck::TypeError::InfiniteType { line, col, .. }
            | draton_typeck::TypeError::BadCast { line, col, .. }
            | draton_typeck::TypeError::IncompatibleErrors { line, col, .. }
            | draton_typeck::TypeError::MissingInterfaceMethod { line, col, .. }
            | draton_typeck::TypeError::CircularInheritance { line, col, .. }
            | draton_typeck::TypeError::UndefinedParent { line, col, .. }
            | draton_typeck::TypeError::NonExhaustiveMatch { line, col, .. }
            | draton_typeck::TypeError::RedundantPattern { line, col, .. } => (*line, *col),
        };
        diags.push(make_diag(
            line.saturating_sub(1),
            col.saturating_sub(1),
            line.saturating_sub(1),
            col.saturating_sub(1) + 1,
            1,
            &error.to_string(),
            "draton-type",
        ));
    }

    json!(diags)
}

fn make_diag(
    sl: usize,
    sc: usize,
    el: usize,
    ec: usize,
    severity: u8,
    message: &str,
    source: &str,
) -> Value {
    json!({
        "range": {
            "start": { "line": sl, "character": sc },
            "end":   { "line": el, "character": ec },
        },
        "severity": severity,
        "source": source,
        "message": message,
    })
}
