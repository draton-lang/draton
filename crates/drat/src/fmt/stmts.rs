use draton_ast::{
    AssignOp, Block, DestructureBinding, ElseBranch, ForStmt, IfStmt, SpawnBody, SpawnStmt, Stmt,
    WhileStmt,
};

use super::Printer;

impl Printer {
    pub(crate) fn fmt_block(&mut self, block: &Block) {
        self.write("{");
        if block.stmts.is_empty() {
            self.write(" }");
            return;
        }
        self.newline();
        self.push_indent();
        for stmt in &block.stmts {
            self.write_indent();
            self.fmt_stmt(stmt);
            self.newline();
        }
        self.pop_indent();
        self.write_indent();
        self.write("}");
    }

    pub(crate) fn fmt_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(let_stmt) => {
                if let_stmt.is_mut {
                    self.write("let mut ");
                } else {
                    self.write("let ");
                }
                self.write(&let_stmt.name);
                if let Some(type_hint) = &let_stmt.type_hint {
                    self.write(": ");
                    self.fmt_type_expr(type_hint);
                }
                if let Some(value) = &let_stmt.value {
                    self.write(" = ");
                    self.fmt_expr(value);
                }
            }
            Stmt::LetDestructure(let_stmt) => {
                if let_stmt.is_mut {
                    self.write("let mut (");
                } else {
                    self.write("let (");
                }
                for (index, binding) in let_stmt.names.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    match binding {
                        DestructureBinding::Name(name) => self.write(name),
                        DestructureBinding::Wildcard => self.write("_"),
                    }
                }
                self.write(") = ");
                self.fmt_expr(&let_stmt.value);
            }
            Stmt::Assign(assign) => {
                self.fmt_expr(&assign.target);
                self.write(match assign.op {
                    AssignOp::Inc => "++",
                    AssignOp::Dec => "--",
                    AssignOp::Assign => " = ",
                    AssignOp::AddAssign => " += ",
                    AssignOp::SubAssign => " -= ",
                    AssignOp::MulAssign => " *= ",
                    AssignOp::DivAssign => " /= ",
                    AssignOp::ModAssign => " %= ",
                });
                if let Some(value) = &assign.value {
                    self.fmt_expr(value);
                }
            }
            Stmt::Return(return_stmt) => {
                self.write("return");
                if let Some(value) = &return_stmt.value {
                    self.write(" ");
                    self.fmt_expr(value);
                }
            }
            Stmt::Expr(expr) => self.fmt_expr(expr),
            Stmt::If(if_stmt) => self.fmt_if(if_stmt),
            Stmt::For(for_stmt) => self.fmt_for(for_stmt),
            Stmt::While(while_stmt) => self.fmt_while(while_stmt),
            Stmt::Spawn(spawn_stmt) => self.fmt_spawn(spawn_stmt),
            Stmt::Block(block) => self.fmt_block(block),
            Stmt::UnsafeBlock(block) => {
                self.write("@unsafe ");
                self.fmt_block(block);
            }
            Stmt::PointerBlock(block) => {
                self.write("@pointer ");
                self.fmt_block(block);
            }
            Stmt::AsmBlock(code, _) => {
                self.write("@asm { ");
                self.write(code);
                self.write(" }");
            }
            Stmt::ComptimeBlock(block) => {
                self.write("@comptime ");
                self.fmt_block(block);
            }
            Stmt::IfCompile(if_compile) => {
                self.write("@if ");
                self.fmt_expr(&if_compile.condition);
                self.write(" ");
                self.fmt_block(&if_compile.body);
            }
            Stmt::GcConfig(gc_config) => {
                self.write("@gc_config {");
                if gc_config.entries.is_empty() {
                    self.write(" }");
                } else {
                    self.newline();
                    self.push_indent();
                    for entry in &gc_config.entries {
                        self.write_indent();
                        self.write(&entry.key);
                        self.write(" = ");
                        self.fmt_expr(&entry.value);
                        self.newline();
                    }
                    self.pop_indent();
                    self.write_indent();
                    self.write("}");
                }
            }
            Stmt::TypeBlock(type_block) => self.fmt_type_block(type_block),
        }
    }

    fn fmt_if(&mut self, if_stmt: &IfStmt) {
        self.write("if (");
        self.fmt_expr(&if_stmt.condition);
        self.write(") ");
        self.fmt_block(&if_stmt.then_branch);
        if let Some(else_branch) = &if_stmt.else_branch {
            self.write(" else ");
            match else_branch {
                ElseBranch::If(child) => self.fmt_if(child),
                ElseBranch::Block(block) => self.fmt_block(block),
            }
        }
    }

    fn fmt_for(&mut self, for_stmt: &ForStmt) {
        self.write("for ");
        self.write(&for_stmt.name);
        self.write(" in ");
        self.fmt_expr(&for_stmt.iter);
        self.write(" ");
        self.fmt_block(&for_stmt.body);
    }

    fn fmt_while(&mut self, while_stmt: &WhileStmt) {
        self.write("while (");
        self.fmt_expr(&while_stmt.condition);
        self.write(") ");
        self.fmt_block(&while_stmt.body);
    }

    fn fmt_spawn(&mut self, spawn_stmt: &SpawnStmt) {
        self.write("spawn ");
        match &spawn_stmt.body {
            SpawnBody::Expr(expr) => self.fmt_expr(expr),
            SpawnBody::Block(block) => self.fmt_block(block),
        }
    }
}
