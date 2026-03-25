use crate::Span;
use serde::Serialize;

/// A type expression used in annotations.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TypeExpr {
    /// A named type such as `Int` or `String`.
    Named(String, Span),
    /// A generic type such as `List[Int]`.
    Generic(String, Vec<TypeExpr>, Span),
    /// A function type such as `fn(Int) -> Int`.
    Fn(Vec<TypeExpr>, Box<TypeExpr>, Span),
    /// The raw `@pointer` type marker.
    Pointer(Span),
    /// An inferred type marker `_`.
    Infer(Span),
}

impl TypeExpr {
    /// Returns the span of this type expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Named(_, span)
            | Self::Generic(_, _, span)
            | Self::Fn(_, _, span)
            | Self::Pointer(span)
            | Self::Infer(span) => *span,
        }
    }
}
