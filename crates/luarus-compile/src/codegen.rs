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

    for stmt in &checked.stmts {
        match stmt {
            TStmt::Store { place, value, line } => {
                emit_expr(&mut chunk, value, *line);
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
                    emit_expr(&mut chunk, item, *line);
                    chunk.emit(Op::Write(item.ty()), *line);
                }
            }
        }
    }

    let last = checked.stmts.last().map(|s| match s {
        TStmt::Store { line, .. } | TStmt::Print { line, .. } => *line,
    });
    chunk.emit(Op::Halt, last.unwrap_or(1));
    chunk
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
