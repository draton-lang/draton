use draton_lexer::{LexError, Lexer};
use draton_parser::{ParseError, Parser};
use draton_typeck::{TypeCheckResult, TypeChecker, TypedProgram};

#[derive(Debug)]
pub struct AnalysisResult {
    pub lex_errors: Vec<LexError>,
    pub parse_errors: Vec<ParseError>,
    pub type_errors: Vec<draton_typeck::TypeError>,
    pub typed_program: Option<TypedProgram>,
    pub span_type_map: Vec<SpanType>,
    pub def_map: Vec<DefEntry>,
}

#[derive(Debug, Clone)]
pub struct SpanType {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub type_str: String,
}

#[derive(Debug, Clone)]
pub struct DefEntry {
    pub ref_line: usize,
    pub ref_col: usize,
    pub def_line: usize,
    pub def_col: usize,
    pub def_uri: String,
}

pub fn analyze(text: &str) -> AnalysisResult {
    let lex_result = Lexer::new(text).tokenize();
    if !lex_result.errors.is_empty() {
        return AnalysisResult {
            lex_errors: lex_result.errors,
            parse_errors: Vec::new(),
            type_errors: Vec::new(),
            typed_program: None,
            span_type_map: Vec::new(),
            def_map: Vec::new(),
        };
    }

    let parse_result = Parser::new(lex_result.tokens).parse();
    if !parse_result.errors.is_empty() {
        return AnalysisResult {
            lex_errors: Vec::new(),
            parse_errors: parse_result.errors,
            type_errors: Vec::new(),
            typed_program: None,
            span_type_map: Vec::new(),
            def_map: Vec::new(),
        };
    }

    let TypeCheckResult {
        typed_program,
        errors,
        ..
    } = TypeChecker::new().check(parse_result.program);

    AnalysisResult {
        lex_errors: Vec::new(),
        parse_errors: Vec::new(),
        type_errors: errors,
        typed_program: Some(typed_program),
        span_type_map: Vec::new(),
        def_map: Vec::new(),
    }
}
