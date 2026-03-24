use crate::error::{LexError, LexResult};
use crate::token::{Span, Token, TokenKind};

/// A lexer that tokenizes Draton source code.
pub struct Lexer<'a> {
    source: &'a str,
    position: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer for the provided source text.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenizes the source and returns both tokens and collected errors.
    pub fn tokenize(mut self) -> LexResult {
        let mut result = LexResult::default();

        while self.peek_char().is_some() {
            if self.skip_whitespace() {
                continue;
            }

            let Some(ch) = self.peek_char() else {
                break;
            };

            let start = self.position;
            let line = self.line;
            let col = self.col;

            match ch {
                '/' => {
                    if self.skip_comment_or_emit_doc(&mut result) {
                        continue;
                    }

                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::SlashEq
                    } else {
                        TokenKind::Slash
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '"' => {
                    self.lex_string(&mut result, false, start, line, col);
                }
                'f' if self.peek_next_char() == Some('"') => {
                    self.lex_string(&mut result, true, start, line, col);
                }
                '0'..='9' => {
                    self.lex_number(&mut result, start, line, col);
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    self.lex_identifier(&mut result.tokens, start, line, col);
                }
                '@' => {
                    self.lex_at_token(&mut result.tokens, start, line, col);
                }
                '+' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::PlusEq
                    } else if self.consume_char('+') {
                        TokenKind::PlusPlus
                    } else {
                        TokenKind::Plus
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '-' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::MinusEq
                    } else if self.consume_char('-') {
                        TokenKind::MinusMinus
                    } else if self.consume_char('>') {
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '*' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::StarEq
                    } else {
                        TokenKind::Star
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '%' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::PercentEq
                    } else {
                        TokenKind::Percent
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '=' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::EqEq
                    } else if self.consume_char('>') {
                        TokenKind::FatArrow
                    } else {
                        TokenKind::Eq
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '!' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::BangEq
                    } else {
                        TokenKind::Bang
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '<' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::LtEq
                    } else if self.consume_char('<') {
                        TokenKind::LtLt
                    } else {
                        TokenKind::Lt
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '>' => {
                    self.advance_char();
                    let kind = if self.consume_char('=') {
                        TokenKind::GtEq
                    } else if self.consume_char('>') {
                        TokenKind::GtGt
                    } else {
                        TokenKind::Gt
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '&' => {
                    self.advance_char();
                    let kind = if self.consume_char('&') {
                        TokenKind::AmpAmp
                    } else {
                        TokenKind::Amp
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '|' => {
                    self.advance_char();
                    let kind = if self.consume_char('|') {
                        TokenKind::PipePipe
                    } else {
                        TokenKind::Pipe
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '^' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::Caret, start, line, col);
                }
                '~' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::Tilde, start, line, col);
                }
                '?' => {
                    self.advance_char();
                    let kind = if self.consume_char('?') {
                        TokenKind::QuestionQuestion
                    } else {
                        TokenKind::Question
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '.' => {
                    self.advance_char();
                    let kind = if self.consume_char('.') {
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                }
                '(' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::LParen, start, line, col);
                }
                ')' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::RParen, start, line, col);
                }
                '[' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::LBracket, start, line, col);
                }
                ']' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::RBracket, start, line, col);
                }
                '{' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::LBrace, start, line, col);
                }
                '}' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::RBrace, start, line, col);
                }
                ',' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::Comma, start, line, col);
                }
                ';' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::Semicolon, start, line, col);
                }
                ':' => {
                    self.advance_char();
                    self.push_token(&mut result.tokens, TokenKind::Colon, start, line, col);
                }
                _ => {
                    result.errors.push(LexError::UnexpectedChar {
                        found: ch,
                        line,
                        col,
                    });
                    self.advance_char();
                }
            }
        }

        result.tokens.push(Token::new(
            TokenKind::Eof,
            String::new(),
            Span {
                start: self.position,
                end: self.position,
                line: self.line,
                col: self.col,
            },
        ));

        result
    }

    fn skip_whitespace(&mut self) -> bool {
        let Some(ch) = self.peek_char() else {
            return false;
        };

        match ch {
            ' ' | '\t' => {
                self.advance_char();
                true
            }
            '\n' => {
                self.advance_newline(1);
                true
            }
            '\r' => {
                if self.peek_next_char() == Some('\n') {
                    self.advance_newline(2);
                } else {
                    self.advance_newline(1);
                }
                true
            }
            _ => false,
        }
    }

    fn skip_comment_or_emit_doc(&mut self, result: &mut LexResult) -> bool {
        if self.peek_char() != Some('/') {
            return false;
        }

        if self.peek_next_char() == Some('/') {
            let start = self.position;
            let line = self.line;
            let col = self.col;

            self.advance_char();
            self.advance_char();

            if self.consume_char('/') {
                let content_start = self.position;
                while let Some(ch) = self.peek_char() {
                    if ch == '\n' || ch == '\r' {
                        break;
                    }
                    self.advance_char();
                }

                let lexeme = self.source[content_start..self.position].to_string();
                result.tokens.push(Token::new(
                    TokenKind::DocComment,
                    lexeme,
                    Span {
                        start,
                        end: self.position,
                        line,
                        col,
                    },
                ));
            } else {
                while let Some(ch) = self.peek_char() {
                    if ch == '\n' || ch == '\r' {
                        break;
                    }
                    self.advance_char();
                }
            }

            return true;
        }

        if self.peek_next_char() == Some('*') {
            let start_line = self.line;
            let start_col = self.col;

            self.advance_char();
            self.advance_char();

            while let Some(ch) = self.peek_char() {
                if ch == '*' && self.peek_next_char() == Some('/') {
                    self.advance_char();
                    self.advance_char();
                    return true;
                }

                if ch == '\n' {
                    self.advance_newline(1);
                } else if ch == '\r' {
                    if self.peek_next_char() == Some('\n') {
                        self.advance_newline(2);
                    } else {
                        self.advance_newline(1);
                    }
                } else {
                    self.advance_char();
                }
            }

            result.errors.push(LexError::UnterminatedBlockComment {
                line: start_line,
                col: start_col,
            });
            return true;
        }

        false
    }

    fn lex_string(
        &mut self,
        result: &mut LexResult,
        is_fstring: bool,
        start: usize,
        line: usize,
        col: usize,
    ) {
        if is_fstring {
            self.advance_char();
        }

        self.advance_char();

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.advance_char();
                    let kind = if is_fstring {
                        TokenKind::FStrLit
                    } else {
                        TokenKind::StrLit
                    };
                    self.push_token(&mut result.tokens, kind, start, line, col);
                    return;
                }
                '\n' | '\r' => {
                    result
                        .errors
                        .push(LexError::UnterminatedString { line, col });
                    return;
                }
                '\\' => {
                    self.advance_char();
                    if self.peek_char().is_some() {
                        self.advance_char();
                    }
                }
                _ => {
                    self.advance_char();
                }
            }
        }

        result
            .errors
            .push(LexError::UnterminatedString { line, col });
    }

    fn lex_number(&mut self, result: &mut LexResult, start: usize, line: usize, col: usize) {
        let mut kind = TokenKind::IntLit;
        let mut invalid = false;

        if self.peek_char() == Some('0') {
            match self.peek_next_char() {
                Some('x') | Some('X') => {
                    kind = TokenKind::HexLit;
                    self.advance_char();
                    self.advance_char();
                    let digits_start = self.position;
                    while let Some(ch) = self.peek_char() {
                        if ch.is_ascii_alphanumeric() {
                            if !ch.is_ascii_hexdigit() {
                                invalid = true;
                            }
                            self.advance_char();
                        } else {
                            break;
                        }
                    }

                    if self.position == digits_start {
                        invalid = true;
                    }

                    if self.peek_char().is_some_and(is_ident_continue) {
                        invalid = true;
                        self.consume_ident_tail();
                    }

                    return self.finish_number(result, kind, invalid, start, line, col);
                }
                Some('b') | Some('B') => {
                    kind = TokenKind::BinLit;
                    self.advance_char();
                    self.advance_char();
                    let digits_start = self.position;
                    while let Some(ch) = self.peek_char() {
                        if ch.is_ascii_alphanumeric() {
                            if ch != '0' && ch != '1' {
                                invalid = true;
                            }
                            self.advance_char();
                        } else {
                            break;
                        }
                    }

                    if self.position == digits_start {
                        invalid = true;
                    }

                    if self.peek_char().is_some_and(is_ident_continue) {
                        invalid = true;
                        self.consume_ident_tail();
                    }

                    return self.finish_number(result, kind, invalid, start, line, col);
                }
                _ => {}
            }
        }

        self.consume_ascii_digits();

        if self.peek_char() == Some('.')
            && self.peek_next_char().is_some_and(|ch| ch.is_ascii_digit())
        {
            kind = TokenKind::FloatLit;
            self.advance_char();
            self.consume_ascii_digits();
        }

        if self.peek_char().is_some_and(is_ident_continue) {
            invalid = true;
            self.consume_ident_tail();
        }

        self.finish_number(result, kind, invalid, start, line, col);
    }

    fn finish_number(
        &mut self,
        result: &mut LexResult,
        kind: TokenKind,
        invalid: bool,
        start: usize,
        line: usize,
        col: usize,
    ) {
        let lexeme = self.source[start..self.position].to_string();

        if invalid {
            result
                .errors
                .push(LexError::InvalidNumericLiteral { lexeme, line, col });
            return;
        }

        result.tokens.push(Token::new(
            kind,
            lexeme,
            Span {
                start,
                end: self.position,
                line,
                col,
            },
        ));
    }

    fn lex_identifier(&mut self, tokens: &mut Vec<Token>, start: usize, line: usize, col: usize) {
        self.advance_char();
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.advance_char();
            } else {
                break;
            }
        }

        let lexeme = &self.source[start..self.position];
        let kind = match lexeme {
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "in" => TokenKind::In,
            "match" => TokenKind::Match,
            "class" => TokenKind::Class,
            "layer" => TokenKind::Layer,
            "extends" => TokenKind::Extends,
            "implements" => TokenKind::Implements,
            "interface" => TokenKind::Interface,
            "enum" => TokenKind::Enum,
            "error" => TokenKind::Error,
            "pub" => TokenKind::Pub,
            "import" => TokenKind::Import,
            "from" => TokenKind::From,
            "as" => TokenKind::As,
            "spawn" => TokenKind::Spawn,
            "chan" => TokenKind::Chan,
            "const" => TokenKind::Const,
            "lambda" => TokenKind::Lambda,
            "true" | "false" => TokenKind::BoolLit,
            "None" => TokenKind::NoneLit,
            _ => TokenKind::Ident,
        };

        tokens.push(Token::new(
            kind,
            lexeme.to_string(),
            Span {
                start,
                end: self.position,
                line,
                col,
            },
        ));
    }

    fn lex_at_token(&mut self, tokens: &mut Vec<Token>, start: usize, line: usize, col: usize) {
        if self.peek_next_char().is_some_and(is_ident_start) {
            let mut end = start + 1;
            if let Some(next) = self.peek_next_char() {
                end += next.len_utf8();
            }
            while let Some(ch) = self.char_at(end) {
                if is_ident_continue(ch) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }

            let lexeme = &self.source[start..end];
            let kind = match lexeme {
                "@type" => Some(TokenKind::AtType),
                "@unsafe" => Some(TokenKind::AtUnsafe),
                "@pointer" => Some(TokenKind::AtPointer),
                "@asm" => Some(TokenKind::AtAsm),
                "@comptime" => Some(TokenKind::AtComptime),
                "@if" => Some(TokenKind::AtIf),
                "@acyclic" => Some(TokenKind::AtAcyclic),
                "@gc_config" => Some(TokenKind::AtGcConfig),
                "@panic_handler" => Some(TokenKind::AtPanicHandler),
                "@oom_handler" => Some(TokenKind::AtOomHandler),
                "@extern" => Some(TokenKind::AtExtern),
                _ => None,
            };

            if let Some(kind) = kind {
                while self.position < end {
                    self.advance_char();
                }
                tokens.push(Token::new(
                    kind,
                    lexeme.to_string(),
                    Span {
                        start,
                        end: self.position,
                        line,
                        col,
                    },
                ));
                return;
            }
        }

        self.advance_char();
        self.push_token(tokens, TokenKind::At, start, line, col);
    }

    fn push_token(
        &self,
        tokens: &mut Vec<Token>,
        kind: TokenKind,
        start: usize,
        line: usize,
        col: usize,
    ) {
        tokens.push(Token::new(
            kind,
            self.source[start..self.position].to_string(),
            Span {
                start,
                end: self.position,
                line,
                col,
            },
        ));
    }

    fn consume_ascii_digits(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn consume_ident_tail(&mut self) {
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let ch = self.peek_char()?;
        self.source[self.position + ch.len_utf8()..].chars().next()
    }

    fn char_at(&self, index: usize) -> Option<char> {
        self.source.get(index..)?.chars().next()
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance_char();
            true
        } else {
            false
        }
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.position += ch.len_utf8();
        self.col += 1;
        Some(ch)
    }

    fn advance_newline(&mut self, bytes: usize) {
        self.position += bytes;
        self.line += 1;
        self.col = 1;
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
