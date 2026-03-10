use draton_ast::{AssignOp, BinOp};
use draton_typeck::typed_ast::{
    TypedAssignStmt, TypedDestructureBinding, TypedElseBranch, TypedExpr, TypedExprKind,
    TypedForStmt, TypedIfStmt, TypedLetDestructureStmt, TypedLetStmt, TypedReturnStmt,
    TypedSpawnBody, TypedWhileStmt,
};
use draton_typeck::{TypedBlock, TypedStmt, TypedStmtKind};
use inkwell::values::{BasicValue, BasicValueEnum, PointerValue};
use inkwell::IntPredicate;

use crate::codegen::CodeGen;
use crate::error::CodeGenError;

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn emit_block(
        &mut self,
        block: &TypedBlock,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        self.push_scope();
        let mut last_value = None;
        for stmt in &block.stmts {
            if self.current_block_terminated() {
                break;
            }
            last_value = self.emit_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(last_value)
    }

    pub(crate) fn emit_stmt(
        &mut self,
        stmt: &TypedStmt,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        match &stmt.kind {
            TypedStmtKind::Let(let_stmt) => self.emit_let_stmt(let_stmt),
            TypedStmtKind::LetDestructure(let_stmt) => self.emit_let_destructure(let_stmt),
            TypedStmtKind::Assign(assign) => {
                self.emit_assign_stmt(assign)?;
                Ok(None)
            }
            TypedStmtKind::Return(ret) => {
                self.emit_return_stmt(ret)?;
                Ok(None)
            }
            TypedStmtKind::Expr(expr) => self.emit_expr(expr),
            TypedStmtKind::If(if_stmt) => {
                self.emit_if_stmt(if_stmt)?;
                Ok(None)
            }
            TypedStmtKind::For(for_stmt) => {
                self.emit_for_stmt(for_stmt)?;
                Ok(None)
            }
            TypedStmtKind::While(while_stmt) => {
                self.emit_while_stmt(while_stmt)?;
                Ok(None)
            }
            TypedStmtKind::Block(block)
            | TypedStmtKind::UnsafeBlock(block)
            | TypedStmtKind::PointerBlock(block)
            | TypedStmtKind::ComptimeBlock(block) => self.emit_block(block),
            TypedStmtKind::Spawn(spawn) => match &spawn.body {
                TypedSpawnBody::Expr(expr) => {
                    let _ = self.emit_expr(expr)?;
                    Ok(None)
                }
                TypedSpawnBody::Block(block) => self.emit_block(block),
            },
            TypedStmtKind::AsmBlock(_)
            | TypedStmtKind::IfCompile(_)
            | TypedStmtKind::GcConfig(_) => Ok(None),
        }
    }

    fn emit_let_stmt(
        &mut self,
        let_stmt: &TypedLetStmt,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        if Self::is_void_type(&let_stmt.ty) {
            return Ok(None);
        }
        let function = self.current_function()?;
        let storage = self.create_entry_alloca(
            function,
            self.llvm_basic_type(&let_stmt.ty)?,
            &let_stmt.name,
        )?;
        self.register_gc_root(storage, &let_stmt.ty)?;
        if let Some(value_expr) = &let_stmt.value {
            if let Some(value) = self.emit_expr(value_expr)? {
                self.build_store(storage, value)?;
            }
        } else {
            self.build_store(storage, self.zero_value(&let_stmt.ty)?)?;
        }
        self.define_local(&let_stmt.name, storage);
        Ok(None)
    }

    fn emit_let_destructure(
        &mut self,
        let_stmt: &TypedLetDestructureStmt,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodeGenError> {
        let rhs = self
            .emit_expr(&let_stmt.value)?
            .ok_or_else(|| CodeGenError::UnsupportedStmt("destructure rhs is void".to_string()))?;
        let rhs_struct = rhs.into_struct_value();
        let function = self.current_function()?;

        for (index, binding) in let_stmt.bindings.iter().enumerate() {
            match binding {
                TypedDestructureBinding::Wildcard => {}
                TypedDestructureBinding::Name { name, ty } => {
                    if Self::is_void_type(ty) {
                        continue;
                    }
                    let slot_value = self
                        .builder
                        .build_extract_value(
                            rhs_struct,
                            index as u32,
                            &format!("destructure.{index}"),
                        )
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                    let storage =
                        self.create_entry_alloca(function, self.llvm_basic_type(ty)?, name)?;
                    self.register_gc_root(storage, ty)?;
                    self.build_store(storage, slot_value)?;
                    self.define_local(name, storage);
                }
            }
        }

        Ok(None)
    }

    fn emit_assign_stmt(&mut self, stmt: &TypedAssignStmt) -> Result<(), CodeGenError> {
        let target_ptr = self.emit_lvalue_ptr(&stmt.target)?;
        let value = match stmt.op {
            AssignOp::Assign => stmt
                .value
                .as_ref()
                .and_then(|expr| self.emit_expr(expr).transpose())
                .transpose()?
                .ok_or_else(|| {
                    CodeGenError::UnsupportedStmt("assignment without value".to_string())
                })?,
            AssignOp::Inc | AssignOp::Dec => {
                let current = self.build_load(target_ptr, "assign.cur")?;
                let current_int = current.into_int_value();
                let delta = self.context.i64_type().const_int(1, false);
                let updated = if stmt.op == AssignOp::Inc {
                    self.builder
                        .build_int_add(current_int, delta, "assign.inc")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                } else {
                    self.builder
                        .build_int_sub(current_int, delta, "assign.dec")
                        .map_err(|err| CodeGenError::Llvm(err.to_string()))?
                };
                updated.as_basic_value_enum()
            }
            _ => {
                let rhs = stmt
                    .value
                    .as_ref()
                    .and_then(|expr| self.emit_expr(expr).transpose())
                    .transpose()?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedStmt("compound assignment without rhs".to_string())
                    })?;
                let lhs = self.build_load(target_ptr, "assign.lhs")?;
                self.emit_compound_assignment(stmt.op, lhs, rhs)?
            }
        };
        self.build_store(target_ptr, value)?;
        if matches!(&stmt.target.kind, TypedExprKind::Field(_, _))
            && Self::is_gc_pointer_type(&stmt.target.ty)
        {
            if let TypedExprKind::Field(object_expr, _) = &stmt.target.kind {
                let object_ptr = self
                    .emit_expr(object_expr)?
                    .ok_or_else(|| {
                        CodeGenError::UnsupportedStmt(
                            "field assignment missing object value".to_string(),
                        )
                    })?
                    .into_pointer_value();
                let _ = self.emit_gc_write_barrier(object_ptr, target_ptr, value);
            }
        }
        Ok(())
    }

    fn emit_compound_assignment(
        &self,
        op: AssignOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodeGenError> {
        let lhs = lhs.into_int_value();
        let rhs = rhs.into_int_value();
        let value = match op {
            AssignOp::AddAssign => self
                .builder
                .build_int_add(lhs, rhs, "assign.add")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
            AssignOp::SubAssign => self
                .builder
                .build_int_sub(lhs, rhs, "assign.sub")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
            AssignOp::MulAssign => self
                .builder
                .build_int_mul(lhs, rhs, "assign.mul")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
            AssignOp::DivAssign => self
                .builder
                .build_int_signed_div(lhs, rhs, "assign.div")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
            AssignOp::ModAssign => self
                .builder
                .build_int_signed_rem(lhs, rhs, "assign.mod")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?,
            AssignOp::Assign | AssignOp::Inc | AssignOp::Dec => lhs,
        };
        Ok(value.as_basic_value_enum())
    }

    fn emit_return_stmt(&mut self, stmt: &TypedReturnStmt) -> Result<(), CodeGenError> {
        if let Some(value_expr) = &stmt.value {
            if let Some(value) = self.emit_expr(value_expr)? {
                self.builder
                    .build_return(Some(&value))
                    .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
                return Ok(());
            }
        }
        self.builder
            .build_return(None)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        Ok(())
    }

    fn emit_if_stmt(&mut self, stmt: &TypedIfStmt) -> Result<(), CodeGenError> {
        let function = self.current_function()?;
        let cond = self.emit_expr(&stmt.condition)?.ok_or_else(|| {
            CodeGenError::UnsupportedStmt("if condition without value".to_string())
        })?;
        let then_block = self.context.append_basic_block(function, "if.then");
        let else_block = self.context.append_basic_block(function, "if.else");
        let merge_block = self.context.append_basic_block(function, "if.end");
        self.builder
            .build_conditional_branch(self.to_bool_value(cond)?, then_block, else_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(then_block);
        let _ = self.emit_block(&stmt.then_branch)?;
        if !self.current_block_terminated() {
            self.builder
                .build_unconditional_branch(merge_block)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(else_block);
        if let Some(else_branch) = &stmt.else_branch {
            match else_branch {
                TypedElseBranch::If(if_stmt) => self.emit_if_stmt(if_stmt)?,
                TypedElseBranch::Block(block) => {
                    let _ = self.emit_block(block)?;
                }
            }
        }
        if !self.current_block_terminated() {
            self.builder
                .build_unconditional_branch(merge_block)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(merge_block);
        Ok(())
    }

    fn emit_while_stmt(&mut self, stmt: &TypedWhileStmt) -> Result<(), CodeGenError> {
        let function = self.current_function()?;
        let cond_block = self.context.append_basic_block(function, "while.cond");
        let body_block = self.context.append_basic_block(function, "while.body");
        let end_block = self.context.append_basic_block(function, "while.end");
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(cond_block);
        let cond = self.emit_expr(&stmt.condition)?.ok_or_else(|| {
            CodeGenError::UnsupportedStmt("while condition without value".to_string())
        })?;
        self.builder
            .build_conditional_branch(self.to_bool_value(cond)?, body_block, end_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(body_block);
        let _ = self.emit_block(&stmt.body)?;
        if !self.current_block_terminated() {
            self.emit_safepoint_poll()?;
            self.builder
                .build_unconditional_branch(cond_block)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        }

        self.builder.position_at_end(end_block);
        Ok(())
    }

    fn emit_for_stmt(&mut self, stmt: &TypedForStmt) -> Result<(), CodeGenError> {
        if let TypedExpr {
            kind: draton_typeck::TypedExprKind::BinOp(start, BinOp::Range, end),
            ..
        } = &stmt.iter
        {
            return self.emit_range_for_loop(stmt, start.as_ref(), end.as_ref(), None);
        }
        if let TypedExpr {
            kind: draton_typeck::TypedExprKind::Call(callee, args),
            ..
        } = &stmt.iter
        {
            if matches!(&callee.kind, draton_typeck::TypedExprKind::Ident(name) if name == "range")
            {
                return self.emit_range_for_loop(stmt, &args[0], &args[1], args.get(2));
            }
        }

        let iter_value = self.emit_expr(&stmt.iter)?.ok_or_else(|| {
            CodeGenError::UnsupportedStmt("for iterator without value".to_string())
        })?;
        let iter_struct = iter_value.into_struct_value();
        let len = self
            .builder
            .build_extract_value(iter_struct, 0, "for.len")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_int_value();
        let ptr = self
            .builder
            .build_extract_value(iter_struct, 1, "for.ptr")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?
            .into_pointer_value();
        self.emit_pointer_loop(stmt, len, ptr)
    }

    fn emit_range_for_loop(
        &mut self,
        stmt: &TypedForStmt,
        start: &TypedExpr,
        end: &TypedExpr,
        step: Option<&TypedExpr>,
    ) -> Result<(), CodeGenError> {
        let function = self.current_function()?;
        let preheader = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodeGenError::Llvm("missing for preheader block".to_string()))?;
        let start_val = self
            .emit_expr(start)?
            .ok_or_else(|| CodeGenError::UnsupportedStmt("range start without value".to_string()))?
            .into_int_value();
        let end_val = self
            .emit_expr(end)?
            .ok_or_else(|| CodeGenError::UnsupportedStmt("range end without value".to_string()))?
            .into_int_value();
        let step_val = if let Some(step) = step {
            self.emit_expr(step)?
                .ok_or_else(|| {
                    CodeGenError::UnsupportedStmt("range step without value".to_string())
                })?
                .into_int_value()
        } else {
            self.context.i64_type().const_int(1, false)
        };

        let header = self.context.append_basic_block(function, "for.header");
        let body = self.context.append_basic_block(function, "for.body");
        let end_block = self.context.append_basic_block(function, "for.end");
        self.builder
            .build_unconditional_branch(header)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(header);
        let phi = self
            .builder
            .build_phi(self.context.i64_type(), "for.index")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        phi.add_incoming(&[(&start_val, preheader)]);
        let cmp = self
            .builder
            .build_int_compare(
                IntPredicate::SLT,
                phi.as_basic_value().into_int_value(),
                end_val,
                "for.cmp",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_conditional_branch(cmp, body, end_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(body);
        self.push_scope();
        let loop_alloca =
            self.create_entry_alloca(function, self.context.i64_type().into(), &stmt.name)?;
        self.build_store(loop_alloca, phi.as_basic_value())?;
        self.define_local(&stmt.name, loop_alloca);
        let _ = self.emit_block(&stmt.body)?;
        self.pop_scope();
        if !self.current_block_terminated() {
            self.emit_safepoint_poll()?;
            let next = self
                .builder
                .build_int_add(phi.as_basic_value().into_int_value(), step_val, "for.next")
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            let current_body = self
                .builder
                .get_insert_block()
                .ok_or_else(|| CodeGenError::Llvm("missing for.body block".to_string()))?;
            self.builder
                .build_unconditional_branch(header)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            phi.add_incoming(&[(&next, current_body)]);
        }

        self.builder.position_at_end(end_block);
        Ok(())
    }

    fn emit_pointer_loop(
        &mut self,
        stmt: &TypedForStmt,
        len: inkwell::values::IntValue<'ctx>,
        ptr: PointerValue<'ctx>,
    ) -> Result<(), CodeGenError> {
        let function = self.current_function()?;
        let preheader = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodeGenError::Llvm("missing for preheader block".to_string()))?;
        let header = self.context.append_basic_block(function, "for.header");
        let body = self.context.append_basic_block(function, "for.body");
        let end_block = self.context.append_basic_block(function, "for.end");
        self.builder
            .build_unconditional_branch(header)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(header);
        let phi = self
            .builder
            .build_phi(self.context.i64_type(), "for.index")
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        let zero = self.context.i64_type().const_zero();
        phi.add_incoming(&[(&zero, preheader)]);
        let cmp = self
            .builder
            .build_int_compare(
                IntPredicate::SLT,
                phi.as_basic_value().into_int_value(),
                len,
                "for.cmp",
            )
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
        self.builder
            .build_conditional_branch(cmp, body, end_block)
            .map_err(|err| CodeGenError::Llvm(err.to_string()))?;

        self.builder.position_at_end(body);
        self.push_scope();
        let gep = unsafe {
            self.builder
                .build_gep(
                    ptr,
                    &[phi.as_basic_value().into_int_value()],
                    "for.elem.ptr",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?
        };
        let elem = self.build_load(gep, "for.elem")?;
        let alloca = self.create_entry_alloca(function, elem.get_type(), &stmt.name)?;
        self.build_store(alloca, elem)?;
        self.define_local(&stmt.name, alloca);
        let _ = self.emit_block(&stmt.body)?;
        self.pop_scope();
        if !self.current_block_terminated() {
            self.emit_safepoint_poll()?;
            let next = self
                .builder
                .build_int_add(
                    phi.as_basic_value().into_int_value(),
                    self.context.i64_type().const_int(1, false),
                    "for.next",
                )
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            let body_block = self
                .builder
                .get_insert_block()
                .ok_or_else(|| CodeGenError::Llvm("missing for.body block".to_string()))?;
            self.builder
                .build_unconditional_branch(header)
                .map_err(|err| CodeGenError::Llvm(err.to_string()))?;
            phi.add_incoming(&[(&next, body_block)]);
        }
        self.builder.position_at_end(end_block);
        Ok(())
    }
}
