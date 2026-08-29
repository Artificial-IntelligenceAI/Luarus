//! Lowering the checked program to bytecode.
//!
//! By this point every type is known, so lowering is a straight post-order walk:
//! there is nothing left to decide.

use luarus_bytecode::chunk::GlobalInfo;
use luarus_bytecode::{Chunk, Op};
use luarus_syntax::ast::BinOp;

use crate::typeck::{Checked, Place, TExpr, TStmt};

pub fn emit(source_name: &str, checked: &Checked) -> Chunk {
    let mut chunk = Chunk::new(source_name);
    chunk.locals = checked.locals.len();
    chunk.local_names = checked.locals.clone();
    chunk.globals = checked
        .globals
        .iter()
        .map(|(name, ty, exported)| GlobalInfo {
            name: name.clone(),
            ty: *ty,
            exported: *exported,
        })
        .collect();

    emit_block(&mut chunk, &checked.stmts);

    let last = chunk.lines.last().copied().unwrap_or(1);
    chunk.emit(Op::Halt, last);
    chunk
}

fn emit_block(chunk: &mut Chunk, stmts: &[TStmt]) {
    for stmt in stmts {
        match stmt {
            TStmt::Store { place, value, line } => {
                emit_expr(chunk, value, *line);
                let op = match place {
                    Place::Local(slot) => Op::StoreLocal(*slot),
                    Place::Global(idx) => Op::StoreGlobal(*idx),
                };
                chunk.emit(op, *line);
            }
            TStmt::Print { items, line } => {
                // Juxtaposed items are written in order, which produces exactly
                // the same output as concatenating them first.
                for item in items {
                    emit_expr(chunk, item, *line);
                    chunk.emit(Op::Write(item.ty()), *line);
                }
            }
            TStmt::If { cond, then_arm, else_arm, line } => {
                emit_expr(chunk, cond, *line);
                // The destinations are not known yet, so both jumps go out with
                // a placeholder and are corrected once they are reached.
                let to_else = chunk.emit(Op::JumpIfFalse(u32::MAX), *line);
                emit_block(chunk, then_arm);

                if else_arm.is_empty() {
                    let after = chunk.code.len() as u32;
                    chunk.patch_jump(to_else, after);
                } else {
                    let past_else = chunk.emit(Op::Jump(u32::MAX), *line);
                    chunk.patch_jump(to_else, chunk.code.len() as u32);
                    emit_block(chunk, else_arm);
                    let after = chunk.code.len() as u32;
                    chunk.patch_jump(past_else, after);
                }
            }
        }
    }
}

fn emit_expr(chunk: &mut Chunk, e: &TExpr, line: u32) {
    match e {
        TExpr::Const(c, _) => {
            let k = chunk.add_const(c.clone());
            chunk.emit(Op::Const(k), line);
        }
        TExpr::Load(Place::Local(slot), _) => {
            chunk.emit(Op::LoadLocal(*slot), line);
        }
        TExpr::Load(Place::Global(idx), _) => {
            chunk.emit(Op::LoadGlobal(*idx), line);
        }
        TExpr::Neg(inner, ty) => {
            emit_expr(chunk, inner, line);
            chunk.emit(Op::Neg(*ty), line);
        }
        TExpr::Bin { op, operand_ty, lhs, rhs, .. } => {
            emit_expr(chunk, lhs, line);
            emit_expr(chunk, rhs, line);
            let t = *operand_ty;
            let instr = match op {
                BinOp::Add => Op::Add(t),
                BinOp::Sub => Op::Sub(t),
                BinOp::Mul => Op::Mul(t),
                BinOp::Div => Op::Div(t),
                BinOp::Rem => Op::Rem(t),
                BinOp::Eq => Op::Eq(t),
                BinOp::Ne => Op::Ne(t),
                BinOp::Lt => Op::Lt(t),
                BinOp::Le => Op::Le(t),
                BinOp::Gt => Op::Gt(t),
                BinOp::Ge => Op::Ge(t),
            };
            chunk.emit(instr, line);
        }
    }
}
