use crate::analysis::AnalysisResult;
use serde_json::{json, Value};

pub fn collect_diagnostics(result: &AnalysisResult) -> Value {
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
            | draton_typeck::TypeError::RedundantPattern { line, col, .. }
            | draton_typeck::TypeError::DeprecatedSyntax { line, col, .. } => (*line, *col),
            draton_typeck::TypeError::Ownership(error) => match error {
                draton_typeck::OwnershipError::UseAfterMove { use_span, .. } => {
                    (use_span.line, use_span.col)
                }
                draton_typeck::OwnershipError::MoveWhileBorrowed { move_span, .. } => {
                    (move_span.line, move_span.col)
                }
                draton_typeck::OwnershipError::ReadDuringExclusiveBorrow { read_span, .. } => {
                    (read_span.line, read_span.col)
                }
                draton_typeck::OwnershipError::ExclusiveBorrowDuringRead {
                    modify_span, ..
                } => (modify_span.line, modify_span.col),
                draton_typeck::OwnershipError::PartialMove { span, .. }
                | draton_typeck::OwnershipError::AmbiguousCallOwnership { span, .. }
                | draton_typeck::OwnershipError::BorrowedValueEscapes { span, .. }
                | draton_typeck::OwnershipError::MultipleOwners { span, .. }
                | draton_typeck::OwnershipError::OwnershipCycle { span }
                | draton_typeck::OwnershipError::LoopMoveWithoutReinit { span, .. }
                | draton_typeck::OwnershipError::ExternalBoundaryRejection { span, .. }
                | draton_typeck::OwnershipError::SafeToRawAliasRejection { span, .. } => {
                    (span.line, span.col)
                }
            },
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

pub fn empty_diagnostics() -> Value {
    json!([])
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
