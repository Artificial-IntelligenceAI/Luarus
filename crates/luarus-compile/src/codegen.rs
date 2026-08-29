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
            TStmt::If { arms, else_arm, line } => {
                // Each arm tests, runs, and jumps clear of the rest. Jump
                // destinations are unknown when the jumps are emitted, so they
                // go out with a placeholder and are corrected on arrival.
                let mut to_end = Vec::with_capacity(arms.len());
                for (i, arm) in arms.iter().enumerate() {
                    emit_expr(chunk, &arm.cond, *line);
                    let to_next = chunk.emit(Op::JumpIfFalse(u32::MAX), *line);
                    emit_block(chunk, &arm.body);

                    // The last arm needs no jump when nothing follows it.
                    let more_follows = i + 1 < arms.len() || !else_arm.is_empty();
                    if more_follows {
                        to_end.push(chunk.emit(Op::Jump(u32::MAX), *line));
                    }
                    let next = chunk.code.len() as u32;
                    chunk.patch_jump(to_next, next);
                }
                emit_block(chunk, else_arm);

                let after = chunk.code.len() as u32;
                for j in to_end {
                    chunk.patch_jump(j, after);
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
