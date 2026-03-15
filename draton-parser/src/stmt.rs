use draton_ast::{
    AssignOp, AssignStmt, Block, DestructureBinding, ElseBranch, Expr, ForStmt, GcConfigEntry,
    GcConfigStmt, IfCompileStmt, IfStmt, LetDestructureStmt, LetStmt, ReturnStmt, SpawnBody,
    SpawnStmt, Stmt, WhileStmt,
};
use draton_lexer::TokenKind;

use crate::Parser;

impl Parser {
    pub(crate) fn parse_block(&mut self) -> Option<Block> {
        let start = self.token_span();
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut stmts = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            self.skip_doc_comments();
            let before = self.pos;
            if let Some(stmt) = self.parse_stmt() {
                stmts.push(stmt);
            } else if self.pos == before {
                self.synchronize_stmt();
            }
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(Block {
            stmts,
            span: self.merge_spans(start, end),
        })
    }

    pub(crate) fn parse_stmt(&mut self) -> Option<Stmt> {
        self.skip_doc_comments();
        match self.current_kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt().map(Stmt::Return),
            TokenKind::If => self.parse_if_stmt().map(Stmt::If),
            TokenKind::For => self.parse_for_stmt().map(Stmt::For),
            TokenKind::While => self.parse_while_stmt().map(Stmt::While),
            TokenKind::Spawn => self.parse_spawn_stmt().map(Stmt::Spawn),
            TokenKind::AtUnsafe => {
                self.parse_special_block(TokenKind::AtUnsafe, SpecialBlockKind::Unsafe)
            }
            TokenKind::AtPointer => {
                self.parse_special_block(TokenKind::AtPointer, SpecialBlockKind::Pointer)
            }
            TokenKind::AtComptime => {
                self.parse_special_block(TokenKind::AtComptime, SpecialBlockKind::Comptime)
            }
            TokenKind::AtAsm => self.parse_asm_block(),
            TokenKind::AtIf => self.parse_if_compile_stmt().map(Stmt::IfCompile),
            TokenKind::AtGcConfig => self.parse_gc_config_stmt().map(Stmt::GcConfig),
            TokenKind::LBrace => self.parse_block().map(Stmt::Block),
            _ => self.parse_expr_stmt_or_assignment(),
        }
    }

    fn parse_special_block(&mut self, token: TokenKind, kind: SpecialBlockKind) -> Option<Stmt> {
        let _ = self.expect(token, "attribute block");
        let block = self.parse_block()?;
        Some(match kind {
            SpecialBlockKind::Unsafe => Stmt::UnsafeBlock(block),
            SpecialBlockKind::Pointer => Stmt::PointerBlock(block),
            SpecialBlockKind::Comptime => Stmt::ComptimeBlock(block),
        })
    }

    fn parse_asm_block(&mut self) -> Option<Stmt> {
        let start = self.token_span();
        if !self.expect(TokenKind::AtAsm, "@asm") {
            return None;
        }
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut parts = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            parts.push(self.current_token().lexeme.clone());
            self.advance();
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(Stmt::AsmBlock(
            parts.join(" "),
            self.merge_spans(start, end),
        ))
    }

    fn parse_if_compile_stmt(&mut self) -> Option<IfCompileStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::AtIf, "@if");
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Some(IfCompileStmt {
            condition,
            span: self.merge_spans(start, body.span),
            body,
        })
    }

    fn parse_gc_config_stmt(&mut self) -> Option<GcConfigStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::AtGcConfig, "@gc_config");
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut entries = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            let entry_start = self.token_span();
            let (key, _) = self.consume_ident("GC config key")?;
            let _ = self.expect(TokenKind::Eq, "=");
            let value = self.parse_expression()?;
            let end = value.span();
            entries.push(GcConfigEntry {
                key,
                value,
                span: self.merge_spans(entry_start, end),
            });
            let _ = self.match_kind(TokenKind::Comma);
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(GcConfigStmt {
            entries,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_let_stmt(&mut self) -> Option<Stmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Let, "let");
        let is_mut = self.match_kind(TokenKind::Mut);
        if self.check(TokenKind::LParen) {
            return self.parse_let_destructure(start, is_mut);
        }
        let (name, _) = self.consume_ident("binding name")?;
        let type_hint = if self.match_kind(TokenKind::Colon) {
            self.parse_type_expr()
        } else {
            None
        };
        let value = if self.match_kind(TokenKind::Eq) {
            self.parse_expression()
        } else {
            None
        };
        self.optional_semicolon();
        let end = value
            .as_ref()
            .map(Expr::span)
            .or_else(|| type_hint.as_ref().map(|ty| ty.span()))
            .unwrap_or(start);
        Some(Stmt::Let(LetStmt {
            is_mut,
            name,
            type_hint,
            value,
            span: self.merge_spans(start, end),
        }))
    }

    fn parse_let_destructure(&mut self, start: draton_ast::Span, is_mut: bool) -> Option<Stmt> {
        if !self.expect(TokenKind::LParen, "(") {
            return None;
        }
        let mut names = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RParen) {
            if self.check(TokenKind::Ident) && self.current_token().lexeme == "_" {
                self.advance();
                names.push(DestructureBinding::Wildcard);
            } else {
                let (name, _) = self.consume_ident("binding name")?;
                names.push(DestructureBinding::Name(name));
            }
            if !self.check(TokenKind::RParen) {
                let _ = self.expect(TokenKind::Comma, ",");
            }
        }
        let _ = self.expect(TokenKind::RParen, ")");
        let _ = self.expect(TokenKind::Eq, "=");
        let value = self.parse_expression()?;
        let end = value.span();
        self.optional_semicolon();
        Some(Stmt::LetDestructure(LetDestructureStmt {
            is_mut,
            names,
            value,
            span: self.merge_spans(start, end),
        }))
    }

    fn parse_return_stmt(&mut self) -> Option<ReturnStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Return, "return");
        let value = match self.current_kind() {
            TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof => None,
            _ => self.parse_expression(),
        };
        self.optional_semicolon();
        let end = value.as_ref().map(Expr::span).unwrap_or(start);
        Some(ReturnStmt {
            value,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_if_stmt(&mut self) -> Option<IfStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::If, "if");
        self.parse_if_stmt_tail(start)
    }

    fn parse_elif_stmt(&mut self) -> Option<IfStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Elif, "elif");
        self.parse_if_stmt_tail(start)
    }

    fn parse_if_stmt_tail(&mut self, start: draton_ast::Span) -> Option<IfStmt> {
        let condition = if self.match_kind(TokenKind::LParen) {
            let expr = self.parse_expression()?;
            let _ = self.expect(TokenKind::RParen, ")");
            expr
        } else {
            self.parse_expression()?
        };
        let then_branch = self.parse_block()?;
        let else_branch = if self.match_kind(TokenKind::Else) {
            if self.check(TokenKind::If) {
                self.parse_if_stmt()
                    .map(|stmt| ElseBranch::If(Box::new(stmt)))
            } else if self.check(TokenKind::Elif) {
                self.parse_elif_stmt()
                    .map(|stmt| ElseBranch::If(Box::new(stmt)))
            } else {
                self.parse_block().map(ElseBranch::Block)
            }
        } else if self.check(TokenKind::Elif) {
            let elif_start = self.token_span();
            let _ = self.expect(TokenKind::Elif, "elif");
            self.parse_if_stmt_tail(elif_start)
                .map(|stmt| ElseBranch::If(Box::new(stmt)))
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map(|branch| match branch {
                ElseBranch::If(stmt) => stmt.span,
                ElseBranch::Block(block) => block.span,
            })
            .unwrap_or(then_branch.span);
        Some(IfStmt {
            condition,
            then_branch,
            else_branch,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_for_stmt(&mut self) -> Option<ForStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::For, "for");
        let (name, _) = self.consume_ident("loop variable")?;
        let _ = self.expect(TokenKind::In, "in");
        let iter = self.parse_expression()?;
        let body = self.parse_block()?;
        Some(ForStmt {
            name,
            iter,
            span: self.merge_spans(start, body.span),
            body,
        })
    }

    fn parse_while_stmt(&mut self) -> Option<WhileStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::While, "while");
        let condition = if self.match_kind(TokenKind::LParen) {
            let expr = self.parse_expression()?;
            let _ = self.expect(TokenKind::RParen, ")");
            expr
        } else {
            self.parse_expression()?
        };
        let body = self.parse_block()?;
        Some(WhileStmt {
            condition,
            span: self.merge_spans(start, body.span),
            body,
        })
    }

    fn parse_spawn_stmt(&mut self) -> Option<SpawnStmt> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Spawn, "spawn");
        let body = if self.check(TokenKind::LBrace) {
            SpawnBody::Block(self.parse_block()?)
        } else {
            SpawnBody::Expr(self.parse_expression()?)
        };
        let end = match &body {
            SpawnBody::Block(block) => block.span,
            SpawnBody::Expr(expr) => expr.span(),
        };
        Some(SpawnStmt {
            body,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_expr_stmt_or_assignment(&mut self) -> Option<Stmt> {
        let start = self.token_span();
        let expr = self.parse_expression()?;
        let stmt = match self.current_kind() {
            TokenKind::Eq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::Assign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::PlusEq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::AddAssign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::MinusEq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::SubAssign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::StarEq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::MulAssign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::SlashEq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::DivAssign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::PercentEq => {
                self.advance();
                let value = self.parse_expression()?;
                let end = value.span();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::ModAssign,
                    value: Some(value),
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::PlusPlus => {
                let end = self.token_span();
                self.advance();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::Inc,
                    value: None,
                    span: self.merge_spans(start, end),
                })
            }
            TokenKind::MinusMinus => {
                let end = self.token_span();
                self.advance();
                Stmt::Assign(AssignStmt {
                    target: expr,
                    op: AssignOp::Dec,
                    value: None,
                    span: self.merge_spans(start, end),
                })
            }
            _ => Stmt::Expr(expr),
        };
        self.optional_semicolon();
        Some(stmt)
    }
}

enum SpecialBlockKind {
    Unsafe,
    Pointer,
    Comptime,
}
