use draton_ast::{BinOp, Expr, FStrPart, MatchArmBody, TypeExpr, UnOp};

use super::Printer;

impl Printer {
    pub(crate) fn fmt_expr(&mut self, expr: &Expr) {
        self.fmt_expr_prec(expr, 0);
    }

    pub(crate) fn fmt_type_expr(&mut self, type_expr: &TypeExpr) {
        match type_expr {
            TypeExpr::Named(name, _) => self.write(name),
            TypeExpr::Generic(name, args, _) => {
                self.write(name);
                self.write("[");
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(arg);
                }
                self.write("]");
            }
            TypeExpr::Fn(params, ret, _) => {
                self.write("fn(");
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(param);
                }
                self.write(") -> ");
                self.fmt_type_expr(ret);
            }
            TypeExpr::Pointer(_) => self.write("@pointer"),
            TypeExpr::Infer(_) => self.write("_"),
        }
    }

    fn fmt_expr_prec(&mut self, expr: &Expr, parent_prec: u8) {
        match expr {
            Expr::IntLit(value, _) => self.write(&value.to_string()),
            Expr::FloatLit(value, _) => self.write(&format_float(*value)),
            Expr::StrLit(value, _) => self.write_escaped_string(value),
            Expr::FStrLit(parts, _) => self.fmt_fstring(parts),
            Expr::BoolLit(value, _) => self.write(if *value { "true" } else { "false" }),
            Expr::NoneLit(_) => self.write("None"),
            Expr::Ident(name, _) => self.write(name),
            Expr::Array(items, _) => {
                self.write("[");
                self.fmt_expr_list(items);
                self.write("]");
            }
            Expr::Map(entries, _) => {
                self.write("{");
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.fmt_expr(key);
                    self.write(": ");
                    self.fmt_expr(value);
                }
                self.write("}");
            }
            Expr::Set(items, _) => {
                self.write("{");
                self.fmt_expr_list(items);
                self.write("}");
            }
            Expr::Tuple(items, _) => {
                self.write("(");
                self.fmt_expr_list(items);
                self.write(")");
            }
            Expr::BinOp(lhs, op, rhs, _) => {
                let precedence = binop_precedence(*op);
                let needs_paren = precedence < parent_prec;
                if needs_paren {
                    self.write("(");
                }
                self.fmt_expr_prec(lhs, precedence);
                self.write(" ");
                self.write(binop_str(*op));
                self.write(" ");
                self.fmt_expr_prec(rhs, precedence.saturating_add(1));
                if needs_paren {
                    self.write(")");
                }
            }
            Expr::UnOp(op, inner, _) => {
                let precedence = 11;
                let needs_paren = precedence < parent_prec;
                if needs_paren {
                    self.write("(");
                }
                self.write(unop_str(*op));
                self.fmt_expr_prec(inner, precedence);
                if needs_paren {
                    self.write(")");
                }
            }
            Expr::Call(callee, args, _) => {
                self.fmt_expr_prec(callee, 12);
                self.write("(");
                self.fmt_expr_list(args);
                self.write(")");
            }
            Expr::MethodCall(target, name, args, _) => {
                self.fmt_expr_prec(target, 12);
                self.write(".");
                self.write(name);
                self.write("(");
                self.fmt_expr_list(args);
                self.write(")");
            }
            Expr::Field(target, field, _) => {
                self.fmt_expr_prec(target, 12);
                self.write(".");
                self.write(field);
            }
            Expr::Index(target, index, _) => {
                self.fmt_expr_prec(target, 12);
                self.write("[");
                self.fmt_expr(index);
                self.write("]");
            }
            Expr::Lambda(params, body, _) => {
                self.write("lambda ");
                self.write(&params.join(", "));
                self.write(" => ");
                self.fmt_expr(body);
            }
            Expr::Cast(inner, type_expr, _) => {
                let precedence = 10;
                let needs_paren = precedence < parent_prec;
                if needs_paren {
                    self.write("(");
                }
                self.fmt_expr_prec(inner, precedence);
                self.write(" as ");
                self.fmt_type_expr(type_expr);
                if needs_paren {
                    self.write(")");
                }
            }
            Expr::Match(subject, arms, _) => {
                self.write("match ");
                self.fmt_expr(subject);
                self.write(" {");
                if arms.is_empty() {
                    self.write(" }");
                    return;
                }
                self.newline();
                self.push_indent();
                for arm in arms {
                    self.write_indent();
                    self.fmt_expr(&arm.pattern);
                    self.write(" => ");
                    match &arm.body {
                        MatchArmBody::Expr(expr) => self.fmt_expr(expr),
                        MatchArmBody::Block(block) => self.fmt_block(block),
                    }
                    self.newline();
                }
                self.pop_indent();
                self.write_indent();
                self.write("}");
            }
            Expr::Ok(value, _) => {
                self.write("Ok(");
                self.fmt_expr(value);
                self.write(")");
            }
            Expr::Err(value, _) => {
                self.write("Err(");
                self.fmt_expr(value);
                self.write(")");
            }
            Expr::Nullish(lhs, rhs, _) => {
                let precedence = 1;
                let needs_paren = precedence < parent_prec;
                if needs_paren {
                    self.write("(");
                }
                self.fmt_expr_prec(lhs, precedence);
                self.write(" ?? ");
                self.fmt_expr_prec(rhs, precedence.saturating_add(1));
                if needs_paren {
                    self.write(")");
                }
            }
            Expr::Chan(type_expr, _) => {
                self.write("chan[");
                self.fmt_type_expr(type_expr);
                self.write("]");
            }
        }
    }

    fn fmt_expr_list(&mut self, exprs: &[Expr]) {
        for (index, expr) in exprs.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.fmt_expr(expr);
        }
    }

    fn fmt_fstring(&mut self, parts: &[FStrPart]) {
        self.write("f\"");
        for part in parts {
            match part {
                FStrPart::Literal(text) => self.write(text),
                FStrPart::Interp(expr) => {
                    self.write("{");
                    self.fmt_expr(expr);
                    self.write("}");
                }
            }
        }
        self.write("\"");
    }

    fn write_escaped_string(&mut self, value: &str) {
        self.write("\"");
        for ch in value.chars() {
            match ch {
                '\\' => self.write("\\\\"),
                '"' => self.write("\\\""),
                '\n' => self.write("\\n"),
                '\r' => self.write("\\r"),
                '\t' => self.write("\\t"),
                _ => self.write(&ch.to_string()),
            }
        }
        self.write("\"");
    }
}

fn format_float(value: f64) -> String {
    let rendered = value.to_string();
    if rendered.contains('.') {
        rendered
    } else {
        format!("{rendered}.0")
    }
}

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::Range => "..",
    }
}

fn unop_str(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "-",
        UnOp::Not => "!",
        UnOp::BitNot => "~",
        UnOp::Ref => "&",
        UnOp::Deref => "*",
    }
}

fn binop_precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Range => 1,
        BinOp::Or => 2,
        BinOp::And => 3,
        BinOp::Eq | BinOp::Ne => 4,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 5,
        BinOp::BitOr | BinOp::BitXor => 6,
        BinOp::BitAnd => 7,
        BinOp::Shl | BinOp::Shr => 8,
        BinOp::Add | BinOp::Sub => 9,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 10,
    }
}
