use crate::stmt::Block;
use crate::types::TypeExpr;
use crate::Span;

/// An expression node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// An integer literal.
    IntLit(i64, Span),
    /// A floating-point literal.
    FloatLit(f64, Span),
    /// A string literal.
    StrLit(String, Span),
    /// An interpolated string literal.
    FStrLit(Vec<FStrPart>, Span),
    /// A boolean literal.
    BoolLit(bool, Span),
    /// The `None` literal.
    NoneLit(Span),
    /// An identifier expression.
    Ident(String, Span),
    /// An array literal.
    Array(Vec<Expr>, Span),
    /// A map literal.
    Map(Vec<(Expr, Expr)>, Span),
    /// A set literal.
    Set(Vec<Expr>, Span),
    /// A tuple literal.
    Tuple(Vec<Expr>, Span),
    /// A binary operator expression.
    BinOp(Box<Expr>, BinOp, Box<Expr>, Span),
    /// A unary operator expression.
    UnOp(UnOp, Box<Expr>, Span),
    /// A call expression.
    Call(Box<Expr>, Vec<Expr>, Span),
    /// A method call expression.
    MethodCall(Box<Expr>, String, Vec<Expr>, Span),
    /// A field access expression.
    Field(Box<Expr>, String, Span),
    /// An index expression.
    Index(Box<Expr>, Box<Expr>, Span),
    /// A lambda expression.
    Lambda(Vec<String>, Box<Expr>, Span),
    /// A cast expression.
    Cast(Box<Expr>, TypeExpr, Span),
    /// A match expression.
    Match(Box<Expr>, Vec<MatchArm>, Span),
    /// A success result value.
    Ok(Box<Expr>, Span),
    /// An error result value.
    Err(Box<Expr>, Span),
    /// A null-coalescing expression.
    Nullish(Box<Expr>, Box<Expr>, Span),
    /// A channel type constructor expression such as `chan[Int]`.
    Chan(TypeExpr, Span),
}

impl Expr {
    /// Returns the span of this expression.
    pub fn span(&self) -> Span {
        match self {
            Self::IntLit(_, span)
            | Self::FloatLit(_, span)
            | Self::StrLit(_, span)
            | Self::FStrLit(_, span)
            | Self::BoolLit(_, span)
            | Self::NoneLit(span)
            | Self::Ident(_, span)
            | Self::Array(_, span)
            | Self::Map(_, span)
            | Self::Set(_, span)
            | Self::Tuple(_, span)
            | Self::BinOp(_, _, _, span)
            | Self::UnOp(_, _, span)
            | Self::Call(_, _, span)
            | Self::MethodCall(_, _, _, span)
            | Self::Field(_, _, span)
            | Self::Index(_, _, span)
            | Self::Lambda(_, _, span)
            | Self::Cast(_, _, span)
            | Self::Match(_, _, span)
            | Self::Ok(_, span)
            | Self::Err(_, span)
            | Self::Nullish(_, _, span)
            | Self::Chan(_, span) => *span,
        }
    }
}

/// A part inside an interpolated string.
#[derive(Debug, Clone, PartialEq)]
pub enum FStrPart {
    /// A plain literal fragment.
    Literal(String),
    /// An interpolated expression fragment.
    Interp(Expr),
}

/// A match arm inside a match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Expr,
    pub body: MatchArmBody,
    pub span: Span,
}

/// The body of a match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchArmBody {
    /// An expression arm body.
    Expr(Expr),
    /// A block arm body.
    Block(Block),
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `..`
    Range,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// Unary minus.
    Neg,
    /// Logical not.
    Not,
    /// Bitwise not.
    BitNot,
    /// Address-of.
    Ref,
    /// Dereference.
    Deref,
}
