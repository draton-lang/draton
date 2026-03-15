use draton_ast::{BinOp, Expr, FStrPart, MatchArm, MatchArmBody, UnOp};
use draton_lexer::{Lexer, TokenKind};

use crate::Parser;

impl Parser {
    pub(crate) fn parse_expression(&mut self) -> Option<Expr> {
        self.parse_nullish()
    }

    fn parse_nullish(&mut self) -> Option<Expr> {
        let mut expr = self.parse_or()?;
        while self.match_kind(TokenKind::QuestionQuestion) {
            let rhs = self.parse_or()?;
            let span = self.merge_spans(expr.span(), rhs.span());
            expr = Expr::Nullish(Box::new(expr), Box::new(rhs), span);
        }
        Some(expr)
    }

    fn parse_or(&mut self) -> Option<Expr> {
        self.parse_left_assoc(Self::parse_and, &[TokenKind::PipePipe], &[BinOp::Or])
    }

    fn parse_and(&mut self) -> Option<Expr> {
        self.parse_left_assoc(Self::parse_equality, &[TokenKind::AmpAmp], &[BinOp::And])
    }

    fn parse_equality(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_comparison,
            &[TokenKind::EqEq, TokenKind::BangEq],
            &[BinOp::Eq, BinOp::Ne],
        )
    }

    fn parse_comparison(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_range,
            &[
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
            ],
            &[BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge],
        )
    }

    fn parse_range(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_bit_or_xor,
            &[TokenKind::DotDot],
            &[BinOp::Range],
        )
    }

    fn parse_bit_or_xor(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_bit_and,
            &[TokenKind::Pipe, TokenKind::Caret],
            &[BinOp::BitOr, BinOp::BitXor],
        )
    }

    fn parse_bit_and(&mut self) -> Option<Expr> {
        self.parse_left_assoc(Self::parse_shift, &[TokenKind::Amp], &[BinOp::BitAnd])
    }

    fn parse_shift(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_additive,
            &[TokenKind::LtLt, TokenKind::GtGt],
            &[BinOp::Shl, BinOp::Shr],
        )
    }

    fn parse_additive(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_multiplicative,
            &[TokenKind::Plus, TokenKind::Minus],
            &[BinOp::Add, BinOp::Sub],
        )
    }

    fn parse_multiplicative(&mut self) -> Option<Expr> {
        self.parse_left_assoc(
            Self::parse_cast,
            &[TokenKind::Star, TokenKind::Slash, TokenKind::Percent],
            &[BinOp::Mul, BinOp::Div, BinOp::Mod],
        )
    }

    fn parse_cast(&mut self) -> Option<Expr> {
        let mut expr = self.parse_unary()?;
        while self.match_kind(TokenKind::As) {
            let ty = self.parse_type_expr()?;
            let span = self.merge_spans(expr.span(), ty.span());
            expr = Expr::Cast(Box::new(expr), ty, span);
        }
        Some(expr)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                let span = self.merge_spans(self.convert_span(token.span), expr.span());
                Some(Expr::UnOp(UnOp::Neg, Box::new(expr), span))
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                let span = self.merge_spans(self.convert_span(token.span), expr.span());
                Some(Expr::UnOp(UnOp::Not, Box::new(expr), span))
            }
            TokenKind::Tilde => {
                self.advance();
                let expr = self.parse_unary()?;
                let span = self.merge_spans(self.convert_span(token.span), expr.span());
                Some(Expr::UnOp(UnOp::BitNot, Box::new(expr), span))
            }
            TokenKind::Amp => {
                self.advance();
                let expr = self.parse_unary()?;
                let span = self.merge_spans(self.convert_span(token.span), expr.span());
                Some(Expr::UnOp(UnOp::Ref, Box::new(expr), span))
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                let span = self.merge_spans(self.convert_span(token.span), expr.span());
                Some(Expr::UnOp(UnOp::Deref, Box::new(expr), span))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if matches!(&expr, Expr::Ident(name, _) if name.chars().next().map(|ch| ch.is_ascii_uppercase()).unwrap_or(false))
                && self.check(TokenKind::LBracket)
                && self.looks_like_type_arg_list()
            {
                expr = self.attach_type_arg_suffix(expr)?;
                continue;
            }

            if matches!(expr, Expr::Ident(_, _)) && self.looks_like_class_literal() {
                expr = self.parse_class_literal(expr)?;
                continue;
            }

            if self.match_kind(TokenKind::LParen) {
                let args = self.parse_argument_list()?;
                let end = self
                    .previous_token()
                    .map(|token| self.convert_span(token.span))
                    .unwrap_or_else(|| expr.span());
                let span = self.merge_spans(expr.span(), end);
                expr = match expr {
                    Expr::Field(target, name, _) => Expr::MethodCall(target, name, args, span),
                    Expr::Ident(name, _) if name == "Ok" && args.len() == 1 => {
                        Expr::Ok(Box::new(args.into_iter().next()?), span)
                    }
                    Expr::Ident(name, _) if name == "Err" && args.len() == 1 => {
                        Expr::Err(Box::new(args.into_iter().next()?), span)
                    }
                    other => Expr::Call(Box::new(other), args, span),
                };
                continue;
            }

            if self.match_kind(TokenKind::Dot) {
                let (name, end) = self.consume_ident("field or method name")?;
                let span = self.merge_spans(expr.span(), end);
                expr = Expr::Field(Box::new(expr), name, span);
                continue;
            }

            if self.match_kind(TokenKind::LBracket) {
                let index = self.parse_expression()?;
                let end = self.token_span();
                let _ = self.expect(TokenKind::RBracket, "]");
                let span = self.merge_spans(expr.span(), end);
                expr = Expr::Index(Box::new(expr), Box::new(index), span);
                continue;
            }

            break;
        }

        Some(expr)
    }

    fn looks_like_class_literal(&self) -> bool {
        if !self.check(TokenKind::LBrace) {
            return false;
        }
        let Some(next) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        if matches!(next.kind, TokenKind::RBrace) {
            return true;
        }
        matches!(
            (next.kind.clone(), self.tokens.get(self.pos + 2).map(|token| token.kind.clone())),
            (TokenKind::Ident, Some(TokenKind::Colon))
        )
    }

    fn looks_like_type_arg_list(&self) -> bool {
        if !self.check(TokenKind::LBracket) {
            return false;
        }
        let mut index = self.pos + 1;
        let mut depth = 1usize;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::LBracket => depth += 1,
                TokenKind::RBracket => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(index + 1)
                            .map(|next| matches!(next.kind, TokenKind::LBrace))
                            .unwrap_or(false);
                    }
                }
                _ => {}
            }
            index += 1;
        }
        false
    }

    fn attach_type_arg_suffix(&mut self, ident: Expr) -> Option<Expr> {
        let Expr::Ident(name, span) = ident else {
            return Some(ident);
        };
        let _ = self.expect(TokenKind::LBracket, "[");
        let mut suffix = String::from("[");
        let mut depth = 1usize;
        while !self.is_eof() && depth > 0 {
            match self.current_kind() {
                TokenKind::LBracket => {
                    depth += 1;
                    suffix.push('[');
                    self.advance();
                }
                TokenKind::RBracket => {
                    depth -= 1;
                    suffix.push(']');
                    self.advance();
                }
                _ => {
                    suffix.push_str(self.current_token().lexeme.as_str());
                    self.advance();
                }
            }
        }
        Some(Expr::Ident(format!("{name}{suffix}"), span))
    }

    fn parse_class_literal(&mut self, ident: Expr) -> Option<Expr> {
        let start = ident.span();
        let _ = self.expect(TokenKind::LBrace, "{");
        let mut entries = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            let (name, key_span) = self.consume_ident("field name")?;
            let _ = self.expect(TokenKind::Colon, ":");
            let value = self.parse_expression()?;
            entries.push((Expr::StrLit(name, key_span), value));
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        let map = Expr::Map(entries, self.merge_spans(start, end));
        Some(Expr::Call(Box::new(ident), vec![map], self.merge_spans(start, end)))
    }

    fn parse_argument_list(&mut self) -> Option<Vec<Expr>> {
        let mut args = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RParen) {
            args.push(self.parse_expression()?);
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        let _ = self.expect(TokenKind::RParen, ")");
        Some(args)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        self.skip_doc_comments();
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::IntLit => self.parse_int_literal(token),
            TokenKind::HexLit => self.parse_radix_int_literal(token, 16, 2),
            TokenKind::BinLit => self.parse_radix_int_literal(token, 2, 2),
            TokenKind::FloatLit => self.parse_float_literal(token),
            TokenKind::StrLit => self.parse_string_literal(token),
            TokenKind::FStrLit => self.parse_fstring_literal(token),
            TokenKind::BoolLit => self.parse_bool_literal(token),
            TokenKind::NoneLit => {
                self.advance();
                Some(Expr::NoneLit(self.convert_span(token.span)))
            }
            TokenKind::Ident => {
                self.advance();
                Some(Expr::Ident(token.lexeme, self.convert_span(token.span)))
            }
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::LBrace => self.parse_brace_literal(),
            TokenKind::LParen => self.parse_paren_expr_or_tuple(),
            TokenKind::Lambda => self.parse_lambda_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Chan => self.parse_chan_expr(),
            _ => {
                self.error_unexpected(&token, "expression");
                None
            }
        }
    }

    fn parse_int_literal(&mut self, token: draton_lexer::Token) -> Option<Expr> {
        self.advance();
        match token.lexeme.parse::<i64>() {
            Ok(value) => Some(Expr::IntLit(value, self.convert_span(token.span))),
            Err(_) => {
                self.errors.push(crate::ParseError::InvalidExpr {
                    line: token.span.line,
                    col: token.span.col,
                });
                None
            }
        }
    }

    fn parse_radix_int_literal(
        &mut self,
        token: draton_lexer::Token,
        radix: u32,
        prefix_len: usize,
    ) -> Option<Expr> {
        self.advance();
        match i64::from_str_radix(&token.lexeme[prefix_len..], radix) {
            Ok(value) => Some(Expr::IntLit(value, self.convert_span(token.span))),
            Err(_) => {
                self.errors.push(crate::ParseError::InvalidExpr {
                    line: token.span.line,
                    col: token.span.col,
                });
                None
            }
        }
    }

    fn parse_float_literal(&mut self, token: draton_lexer::Token) -> Option<Expr> {
        self.advance();
        match token.lexeme.parse::<f64>() {
            Ok(value) => Some(Expr::FloatLit(value, self.convert_span(token.span))),
            Err(_) => {
                self.errors.push(crate::ParseError::InvalidExpr {
                    line: token.span.line,
                    col: token.span.col,
                });
                None
            }
        }
    }

    fn parse_string_literal(&mut self, token: draton_lexer::Token) -> Option<Expr> {
        self.advance();
        let raw = token.lexeme;
        let text = raw
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_default()
            .to_string();
        Some(Expr::StrLit(text, self.convert_span(token.span)))
    }

    fn parse_fstring_literal(&mut self, token: draton_lexer::Token) -> Option<Expr> {
        self.advance();
        let span = self.convert_span(token.span);
        let raw = token.lexeme;
        let inner = raw
            .strip_prefix("f\"")
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_default();

        let mut parts = Vec::new();
        let mut buffer = String::new();
        let chars: Vec<char> = inner.chars().collect();
        let mut index = 0usize;

        while index < chars.len() {
            if chars[index] == '{' {
                if !buffer.is_empty() {
                    parts.push(FStrPart::Literal(std::mem::take(&mut buffer)));
                }

                index += 1;
                let mut depth = 1usize;
                let start = index;
                while index < chars.len() && depth > 0 {
                    match chars[index] {
                        '{' => depth += 1,
                        '}' => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    index += 1;
                }

                if depth != 0 {
                    self.errors.push(crate::ParseError::InvalidExpr {
                        line: span.line,
                        col: span.col,
                    });
                    return Some(Expr::FStrLit(parts, span));
                }

                let expr_src: String = chars[start..index - 1].iter().collect();
                let lexed = Lexer::new(&expr_src).tokenize();
                let (parsed, errors) = Parser::new(lexed.tokens).parse_expression_only();
                self.errors.extend(errors);
                if let Some(expr) = parsed {
                    parts.push(FStrPart::Interp(expr));
                }
                continue;
            }

            buffer.push(chars[index]);
            index += 1;
        }

        if !buffer.is_empty() {
            parts.push(FStrPart::Literal(buffer));
        }

        Some(Expr::FStrLit(parts, span))
    }

    fn parse_bool_literal(&mut self, token: draton_lexer::Token) -> Option<Expr> {
        self.advance();
        Some(Expr::BoolLit(
            token.lexeme == "true",
            self.convert_span(token.span),
        ))
    }

    fn parse_array_literal(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::LBracket, "[");
        let mut items = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBracket) {
            items.push(self.parse_expression()?);
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        let end = self.token_span();
        let _ = self.expect(TokenKind::RBracket, "]");
        Some(Expr::Array(items, self.merge_spans(start, end)))
    }

    fn parse_brace_literal(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::LBrace, "{");
        if self.match_kind(TokenKind::RBrace) {
            return Some(Expr::Map(Vec::new(), start));
        }

        let first = self.parse_expression()?;
        if self.match_kind(TokenKind::Colon) {
            let mut items = Vec::new();
            let value = self.parse_expression()?;
            items.push((first, value));
            while self.match_kind(TokenKind::Comma) {
                if self.check(TokenKind::RBrace) {
                    break;
                }
                let key = self.parse_expression()?;
                let _ = self.expect(TokenKind::Colon, ":");
                let value = self.parse_expression()?;
                items.push((key, value));
            }
            let end = self.token_span();
            let _ = self.expect(TokenKind::RBrace, "}");
            Some(Expr::Map(items, self.merge_spans(start, end)))
        } else {
            let mut items = vec![first];
            while self.match_kind(TokenKind::Comma) {
                if self.check(TokenKind::RBrace) {
                    break;
                }
                items.push(self.parse_expression()?);
            }
            let end = self.token_span();
            let _ = self.expect(TokenKind::RBrace, "}");
            Some(Expr::Set(items, self.merge_spans(start, end)))
        }
    }

    fn parse_paren_expr_or_tuple(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::LParen, "(");
        if self.match_kind(TokenKind::RParen) {
            return Some(Expr::Tuple(Vec::new(), start));
        }

        let first = self.parse_expression()?;
        if self.match_kind(TokenKind::Comma) {
            let mut items = vec![first];
            while !self.is_eof() && !self.check(TokenKind::RParen) {
                items.push(self.parse_expression()?);
                if !self.match_kind(TokenKind::Comma) {
                    break;
                }
            }
            let end = self.token_span();
            let _ = self.expect(TokenKind::RParen, ")");
            Some(Expr::Tuple(items, self.merge_spans(start, end)))
        } else {
            let _ = self.expect(TokenKind::RParen, ")");
            Some(first)
        }
    }

    fn parse_lambda_expr(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Lambda, "lambda");
        let mut params = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::FatArrow) {
            let (name, _) = self.consume_ident("lambda parameter")?;
            params.push(name);
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        let _ = self.expect(TokenKind::FatArrow, "=>");
        let body = self.parse_expression()?;
        let span = self.merge_spans(start, body.span());
        Some(Expr::Lambda(params, Box::new(body), span))
    }

    fn parse_match_expr(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Match, "match");
        let subject = self.parse_expression()?;
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut arms = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            let arm_start = self.token_span();
            let pattern = self.parse_expression()?;
            let _ = self.expect(TokenKind::FatArrow, "=>");
            let body = if self.check(TokenKind::LBrace) {
                MatchArmBody::Block(self.parse_block()?)
            } else {
                MatchArmBody::Expr(self.parse_expression()?)
            };
            let end = match &body {
                MatchArmBody::Expr(expr) => expr.span(),
                MatchArmBody::Block(block) => block.span,
            };
            arms.push(MatchArm {
                pattern,
                body,
                span: self.merge_spans(arm_start, end),
            });
            let _ = self.match_kind(TokenKind::Comma);
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(Expr::Match(
            Box::new(subject),
            arms,
            self.merge_spans(start, end),
        ))
    }

    fn parse_chan_expr(&mut self) -> Option<Expr> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Chan, "chan");
        let _ = self.expect(TokenKind::LBracket, "[");
        let ty = self.parse_type_expr()?;
        let end = self.token_span();
        let _ = self.expect(TokenKind::RBracket, "]");
        Some(Expr::Chan(ty, self.merge_spans(start, end)))
    }

    fn parse_left_assoc(
        &mut self,
        next: fn(&mut Parser) -> Option<Expr>,
        tokens: &[TokenKind],
        ops: &[BinOp],
    ) -> Option<Expr> {
        let mut expr = next(self)?;
        loop {
            let kind = self.current_kind();
            let Some(index) = tokens.iter().position(|candidate| *candidate == kind) else {
                break;
            };
            self.advance();
            let rhs = next(self)?;
            let span = self.merge_spans(expr.span(), rhs.span());
            expr = Expr::BinOp(Box::new(expr), ops[index], Box::new(rhs), span);
        }
        Some(expr)
    }
}
