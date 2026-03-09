use draton_ast::{
    ClassDef, ClassMember, ConstDef, EnumDef, ErrorDef, ExternBlock, FieldDef, FnDef, ImportDef,
    ImportItem, InterfaceDef, Item, Param, TypeBlock, TypeExpr, TypeMember,
};
use draton_lexer::TokenKind;

use crate::Parser;

impl Parser {
    pub(crate) fn parse_item(&mut self) -> Option<Item> {
        self.skip_doc_comments();

        let is_pub = self.match_kind(TokenKind::Pub);

        let item = match self.current_kind() {
            TokenKind::Fn => self.parse_fn_def(is_pub, true).map(Item::Fn),
            TokenKind::Class => self.parse_class_def().map(Item::Class),
            TokenKind::Interface => self.parse_interface_def().map(Item::Interface),
            TokenKind::Enum => self.parse_enum_def().map(Item::Enum),
            TokenKind::Error => self.parse_error_def().map(Item::Error),
            TokenKind::Const => self.parse_const_def().map(Item::Const),
            TokenKind::Import => self.parse_import_def().map(Item::Import),
            TokenKind::AtType => self.parse_type_block().map(Item::TypeBlock),
            TokenKind::AtExtern => self.parse_extern_block().map(Item::Extern),
            TokenKind::AtPanicHandler => self.parse_handler_item(true),
            TokenKind::AtOomHandler => self.parse_handler_item(false),
            _ => {
                let token = self.current_token().clone();
                self.error_unexpected(&token, "top-level item");
                None
            }
        };

        if is_pub && item.is_none() {
            self.synchronize_top_level();
        }

        item
    }

    fn parse_handler_item(&mut self, panic_handler: bool) -> Option<Item> {
        self.advance();
        let function = self.parse_fn_def(false, true)?;
        Some(if panic_handler {
            Item::PanicHandler(function)
        } else {
            Item::OomHandler(function)
        })
    }

    pub(crate) fn parse_fn_def(&mut self, is_pub: bool, allow_body: bool) -> Option<FnDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Fn, "fn") {
            return None;
        }

        let (name, _) = self.consume_ident("function name")?;
        let params = self.parse_param_list()?;
        let ret_type = if self.match_kind(TokenKind::Arrow) {
            self.parse_type_expr()
        } else {
            None
        };

        let body = if allow_body && self.check(TokenKind::LBrace) {
            self.parse_block()
        } else {
            None
        };

        let end = body
            .as_ref()
            .map(|block| block.span)
            .or_else(|| ret_type.as_ref().map(TypeExpr::span))
            .or_else(|| params.last().map(|param| param.span))
            .unwrap_or(start);

        Some(FnDef {
            is_pub,
            name,
            params,
            ret_type,
            body,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_param_list(&mut self) -> Option<Vec<Param>> {
        if !self.expect(TokenKind::LParen, "(") {
            return None;
        }

        let mut params = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RParen) {
            let start = self.token_span();
            let (name, _) = self.consume_ident("parameter name")?;
            let type_hint = if self.match_kind(TokenKind::Colon) {
                self.parse_type_expr()
            } else {
                None
            };
            let end = type_hint.as_ref().map(TypeExpr::span).unwrap_or(start);
            params.push(Param {
                name,
                type_hint,
                span: self.merge_spans(start, end),
            });

            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }

        let _ = self.expect(TokenKind::RParen, ")");
        Some(params)
    }

    fn parse_class_def(&mut self) -> Option<ClassDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Class, "class") {
            return None;
        }

        let (name, _) = self.consume_ident("class name")?;
        let extends = if self.match_kind(TokenKind::Extends) {
            self.consume_ident("base class").map(|value| value.0)
        } else {
            None
        };

        let mut implements = Vec::new();
        if self.match_kind(TokenKind::Implements) {
            while !self.is_eof() {
                if let Some((iface, _)) = self.consume_ident("interface name") {
                    implements.push(iface);
                }
                if !self.match_kind(TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut members = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            self.skip_doc_comments();
            match self.current_kind() {
                TokenKind::Let => {
                    if let Some(field) = self.parse_class_field() {
                        members.push(ClassMember::Field(field));
                    } else {
                        self.synchronize_stmt();
                    }
                }
                TokenKind::Fn => {
                    if let Some(method) = self.parse_fn_def(false, true) {
                        members.push(ClassMember::Method(method));
                    } else {
                        self.synchronize_stmt();
                    }
                }
                _ => {
                    let token = self.current_token().clone();
                    self.error_unexpected(&token, "class member");
                    self.synchronize_stmt();
                }
            }
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(ClassDef {
            name,
            extends,
            implements,
            members,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_class_field(&mut self) -> Option<FieldDef> {
        let start = self.token_span();
        let _ = self.expect(TokenKind::Let, "let");
        let is_mut = self.match_kind(TokenKind::Mut);
        let (name, _) = self.consume_ident("field name")?;
        let type_hint = if self.match_kind(TokenKind::Colon) {
            self.parse_type_expr()
        } else {
            None
        };
        self.optional_semicolon();
        let end = type_hint.as_ref().map(TypeExpr::span).unwrap_or(start);
        Some(FieldDef {
            is_mut,
            name,
            type_hint,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_interface_def(&mut self) -> Option<InterfaceDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Interface, "interface") {
            return None;
        }

        let (name, _) = self.consume_ident("interface name")?;
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut methods = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            if let Some(method) = self.parse_fn_def(false, false) {
                methods.push(method);
                self.optional_semicolon();
            } else {
                self.synchronize_stmt();
            }
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(InterfaceDef {
            name,
            methods,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_enum_def(&mut self) -> Option<EnumDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Enum, "enum") {
            return None;
        }

        let (name, _) = self.consume_ident("enum name")?;
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut variants = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            if let Some((variant, _)) = self.consume_ident("enum variant") {
                variants.push(variant);
            }
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(EnumDef {
            name,
            variants,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_error_def(&mut self) -> Option<ErrorDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Error, "error") {
            return None;
        }

        let (name, _) = self.consume_ident("error name")?;
        let fields = self.parse_param_list()?;
        let end = fields.last().map(|field| field.span).unwrap_or(start);
        Some(ErrorDef {
            name,
            fields,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_const_def(&mut self) -> Option<ConstDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Const, "const") {
            return None;
        }

        let (name, _) = self.consume_ident("const name")?;
        let _ = self.expect(TokenKind::Eq, "=");
        let value = self.parse_expression()?;
        self.optional_semicolon();
        let end = value.span();
        Some(ConstDef {
            name,
            value,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_import_def(&mut self) -> Option<ImportDef> {
        let start = self.token_span();
        if !self.expect(TokenKind::Import, "import") {
            return None;
        }
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut items = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            let item_start = self.token_span();
            let (name, _) = self.consume_ident("import name")?;
            let alias = if self.match_kind(TokenKind::As) {
                self.consume_ident("import alias").map(|value| value.0)
            } else {
                None
            };
            let item_end = self
                .previous_token()
                .map(|token| self.convert_span(token.span))
                .unwrap_or(item_start);
            items.push(ImportItem {
                name,
                alias,
                span: self.merge_spans(item_start, item_end),
            });
            let _ = self.match_kind(TokenKind::Comma);
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(ImportDef {
            items,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_extern_block(&mut self) -> Option<ExternBlock> {
        let start = self.token_span();
        if !self.expect(TokenKind::AtExtern, "@extern") {
            return None;
        }

        let abi = match self.parse_expression()? {
            draton_ast::Expr::StrLit(value, _) => value,
            expr => {
                self.errors.push(crate::ParseError::InvalidExpr {
                    line: expr.span().line,
                    col: expr.span().col,
                });
                return None;
            }
        };

        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut functions = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            if let Some(function) = self.parse_fn_def(false, false) {
                functions.push(function);
                self.optional_semicolon();
            } else {
                self.synchronize_stmt();
            }
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(ExternBlock {
            abi,
            functions,
            span: self.merge_spans(start, end),
        })
    }

    fn parse_type_block(&mut self) -> Option<TypeBlock> {
        let start = self.token_span();
        if !self.expect(TokenKind::AtType, "@type") {
            return None;
        }
        if !self.expect(TokenKind::LBrace, "{") {
            return None;
        }

        let mut members = Vec::new();
        while !self.is_eof() && !self.check(TokenKind::RBrace) {
            match self.current_kind() {
                TokenKind::Fn => {
                    if let Some(signature) = self.parse_fn_def(false, false) {
                        members.push(TypeMember::Function(signature));
                    } else {
                        self.synchronize_stmt();
                    }
                }
                TokenKind::Ident => {
                    let member_start = self.token_span();
                    if let Some((name, _)) = self.consume_ident("type binding name") {
                        let _ = self.expect(TokenKind::Colon, ":");
                        if let Some(type_expr) = self.parse_type_expr() {
                            let end = type_expr.span();
                            members.push(TypeMember::Binding {
                                name,
                                type_expr,
                                span: self.merge_spans(member_start, end),
                            });
                        }
                    }
                }
                _ => {
                    let token = self.current_token().clone();
                    self.error_unexpected(&token, "type block member");
                    self.synchronize_stmt();
                }
            }
            let _ = self.match_kind(TokenKind::Comma);
        }

        let end = self.token_span();
        let _ = self.expect(TokenKind::RBrace, "}");
        Some(TypeBlock {
            members,
            span: self.merge_spans(start, end),
        })
    }

    pub(crate) fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        self.skip_doc_comments();
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::AtPointer => {
                self.advance();
                Some(TypeExpr::Pointer(self.convert_span(token.span)))
            }
            TokenKind::Ident => {
                self.advance();
                let name = token.lexeme;
                let start = self.convert_span(token.span);
                if self.match_kind(TokenKind::LBracket) {
                    let mut args = Vec::new();
                    while !self.is_eof() && !self.check(TokenKind::RBracket) {
                        if let Some(arg) = self.parse_type_expr() {
                            args.push(arg);
                        }
                        if !self.match_kind(TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.token_span();
                    let _ = self.expect(TokenKind::RBracket, "]");
                    Some(TypeExpr::Generic(name, args, self.merge_spans(start, end)))
                } else if name == "_" {
                    Some(TypeExpr::Infer(start))
                } else {
                    Some(TypeExpr::Named(name, start))
                }
            }
            _ => {
                self.error_unexpected(&token, "type expression");
                None
            }
        }
    }
}
