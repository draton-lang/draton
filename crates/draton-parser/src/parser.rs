use draton_ast::{Program, Span};
use draton_lexer::{Token, TokenKind};
use serde::Serialize;

use crate::error::{ParseError, ParseWarning};

/// The result of parsing a token stream.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParseResult {
    pub program: Program,
    pub errors: Vec<ParseError>,
    pub warnings: Vec<ParseWarning>,
}

/// A recursive-descent parser for Draton tokens.
pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) errors: Vec<ParseError>,
    pub(crate) warnings: Vec<ParseWarning>,
}

impl Parser {
    /// Creates a parser over a token vector.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Parses a full program and returns both the AST and collected errors.
    pub fn parse(mut self) -> ParseResult {
        let mut program = Program::default();

        while !self.is_eof() {
            self.skip_doc_comments();

            if self.is_eof() {
                break;
            }

            let before = self.pos;
            if let Some(item) = self.parse_item() {
                program.items.push(item);
                continue;
            }

            if self.pos == before {
                self.synchronize_top_level();
            }
        }

        ParseResult {
            program,
            errors: self.errors,
            warnings: self.warnings,
        }
    }

    pub fn parse_expression_only(mut self) -> (Option<draton_ast::Expr>, Vec<ParseError>) {
        self.skip_doc_comments();
        let expr = self.parse_expression();
        self.skip_doc_comments();
        if !self.is_eof() {
            let token = self.current_token().clone();
            self.error_unexpected(&token, "end of expression");
        }
        (expr, self.errors)
    }

    pub(crate) fn current_token(&self) -> &Token {
        let idx = self.pos.min(self.tokens.len().saturating_sub(1));
        &self.tokens[idx]
    }

    pub(crate) fn current_kind(&self) -> TokenKind {
        self.current_token().kind.clone()
    }

    pub(crate) fn previous_token(&self) -> Option<&Token> {
        self.pos.checked_sub(1).and_then(|idx| self.tokens.get(idx))
    }

    pub(crate) fn is_eof(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    pub(crate) fn skip_doc_comments(&mut self) {
        while matches!(self.current_kind(), TokenKind::DocComment) {
            self.advance();
        }
    }

    pub(crate) fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if self.pos < self.tokens.len().saturating_sub(1) {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn match_kind(&mut self, kind: TokenKind) -> bool {
        self.skip_doc_comments();
        if self.current_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn check(&self, kind: TokenKind) -> bool {
        self.current_kind() == kind
    }

    pub(crate) fn expect(&mut self, kind: TokenKind, expected: &str) -> bool {
        self.skip_doc_comments();
        if self.current_kind() == kind {
            self.advance();
            true
        } else {
            let token = self.current_token().clone();
            self.error_unexpected(&token, expected);
            false
        }
    }

    pub(crate) fn consume_ident(&mut self, expected: &str) -> Option<(String, Span)> {
        self.skip_doc_comments();
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::Ident => {
                self.advance();
                Some((token.lexeme, self.convert_span(token.span)))
            }
            _ => {
                self.error_unexpected(&token, expected);
                None
            }
        }
    }

    pub(crate) fn error_unexpected(&mut self, token: &Token, expected: &str) {
        match token.kind {
            TokenKind::Eof => self.errors.push(ParseError::UnexpectedEof {
                expected: expected.to_string(),
                line: token.span.line,
                col: token.span.col,
            }),
            _ => self.errors.push(ParseError::UnexpectedToken {
                found: token.lexeme.clone(),
                expected: expected.to_string(),
                line: token.span.line,
                col: token.span.col,
            }),
        }
    }

    pub(crate) fn push_deprecated_warning(
        &mut self,
        syntax: impl Into<String>,
        replacement: impl Into<String>,
        span: Span,
    ) {
        self.warnings.push(ParseWarning::DeprecatedSyntax {
            syntax: syntax.into(),
            replacement: replacement.into(),
            line: span.line,
            col: span.col,
        });
    }

    pub(crate) fn convert_span(&self, span: draton_lexer::Span) -> Span {
        Span {
            start: span.start,
            end: span.end,
            line: span.line,
            col: span.col,
        }
    }

    pub(crate) fn merge_spans(&self, start: Span, end: Span) -> Span {
        Span {
            start: start.start,
            end: end.end,
            line: start.line,
            col: start.col,
        }
    }

    pub(crate) fn token_span(&self) -> Span {
        self.convert_span(self.current_token().span)
    }

    pub(crate) fn optional_semicolon(&mut self) {
        let _ = self.match_kind(TokenKind::Semicolon);
    }

    pub(crate) fn synchronize_top_level(&mut self) {
        while !self.is_eof() {
            match self.current_kind() {
                TokenKind::Fn
                | TokenKind::Class
                | TokenKind::Interface
                | TokenKind::Enum
                | TokenKind::Error
                | TokenKind::Const
                | TokenKind::Import
                | TokenKind::Pub
                | TokenKind::AtType
                | TokenKind::AtAcyclic
                | TokenKind::AtExtern
                | TokenKind::AtPanicHandler
                | TokenKind::AtOomHandler => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    pub(crate) fn synchronize_stmt(&mut self) {
        while !self.is_eof() {
            match self.current_kind() {
                TokenKind::Semicolon => {
                    self.advance();
                    break;
                }
                TokenKind::RBrace
                | TokenKind::Let
                | TokenKind::Return
                | TokenKind::If
                | TokenKind::Elif
                | TokenKind::For
                | TokenKind::While
                | TokenKind::Spawn
                | TokenKind::AtUnsafe
                | TokenKind::AtPointer
                | TokenKind::AtAsm
                | TokenKind::AtComptime
                | TokenKind::AtIf
                | TokenKind::AtGcConfig => break,
                _ => {
                    self.advance();
                }
            }
        }
    }
}
