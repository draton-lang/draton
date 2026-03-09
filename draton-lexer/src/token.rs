/// A byte span pointing at a lexeme in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

impl Token {
    pub(crate) fn new(kind: TokenKind, lexeme: String, span: Span) -> Self {
        Self { kind, lexeme, span }
    }
}

/// All token kinds in the Draton language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Literals
    IntLit,
    FloatLit,
    HexLit,
    BinLit,
    StrLit,
    FStrLit,
    BoolLit,
    NoneLit,

    // Keywords
    Let,
    Mut,
    Fn,
    Return,
    If,
    Else,
    For,
    While,
    In,
    Match,
    Class,
    Extends,
    Implements,
    Interface,
    Enum,
    Error,
    Pub,
    Import,
    As,
    Spawn,
    Chan,
    Const,
    Lambda,

    // Attribute keywords
    AtType,
    AtUnsafe,
    AtPointer,
    AtAsm,
    AtComptime,
    AtIf,
    AtGcConfig,
    AtPanicHandler,
    AtOomHandler,
    AtExtern,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    PlusPlus,
    MinusMinus,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AmpAmp,
    PipePipe,
    Bang,
    Amp,
    Pipe,
    Caret,
    Tilde,
    LtLt,
    GtGt,
    Question,
    QuestionQuestion,
    FatArrow,
    Arrow,
    DotDot,
    At,

    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Colon,
    Dot,

    // Special
    DocComment,
    Ident,
    Eof,
}
