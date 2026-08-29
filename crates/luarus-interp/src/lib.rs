//! A reference interpreter, used as a test oracle for the compiler and VM.
//!
//! This walks the checked syntax tree directly. It has no bytecode, no jumps to
//! patch, no operand stack and no slot numbering — none of the machinery where
//! a compiler goes wrong. It is meant to be slow and obviously correct, so that
//! running a program both ways and comparing the results says something.
//!
//! # What this does and does not test
//!
//! The oracle shares the *front end* with the compiler: parsing, type checking,
//! literal parsing, half-precision conversion and value formatting are the same
//! code in both paths, so a bug in those appears identically on both sides and
//! goes unnoticed. That is deliberate — reimplementing them would only produce
//! divergences that mean nothing.
//!
//! What differs, and is therefore genuinely cross-checked:
//!
//! * control flow — arms and conditions here, against jumps and patched
//!   destinations there;
//! * storage — a map of values here, against numbered slots and globals there;
//! * evaluation order and operand handling — recursion here, against a stack;
//! * integer arithmetic — computed in `i128` and range-checked here, against
//!   `checked_*` at the target width there. Two different routes to the same
//!   answer, so a wrong bound on one side shows up.
//!
//! Serialisation is covered too, since the compiled side runs a chunk that has
//! been encoded and decoded on the way.

use std::io::Write;
use std::rc::Rc;

use luarus_bytecode::value::Value;
use luarus_bytecode::{f16, Const, RtType};
use luarus_compile::typeck::{Checked, Place, TExpr, TIfArm, TStmt};
use luarus_diag::Rule;
use luarus_syntax::ast::BinOp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpError {
    pub rule: Rule,
    pub message: String,
    pub line: u32,
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime error[{}]: {} (line {})", self.rule.slug(), self.message, self.line)
    }
}

impl std::error::Error for InterpError {}

/// Interpret a checked program, writing anything it prints to `out`.
pub fn run(checked: &Checked, out: &mut dyn Write) -> Result<(), InterpError> {
    let mut interp = Interp {
        out,
        locals: vec![None; checked.locals.len()],
        globals: vec![None; checked.globals.len()],
        local_names: &checked.locals,
        global_names: &checked.globals,
    };
    interp.block(&checked.stmts)
}

/// Interpret and capture the output, for comparing against the compiled path.
pub fn run_capturing(checked: &Checked) -> Result<String, InterpError> {
    let mut buf = Vec::new();
    run(checked, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

struct Interp<'a> {
    out: &'a mut dyn Write,
    locals: Vec<Option<Value>>,
    globals: Vec<Option<Value>>,
    local_names: &'a [String],
    global_names: &'a [(String, RtType, bool)],
}

fn err(rule: Rule, line: u32, message: impl Into<String>) -> InterpError {
    InterpError { rule, message: message.into(), line }
}

impl Interp<'_> {
    fn block(&mut self, stmts: &[TStmt]) -> Result<(), InterpError> {
        for stmt in stmts {
            self.stmt(stmt)?;
        }
        Ok(())
    }

    fn stmt(&mut self, stmt: &TStmt) -> Result<(), InterpError> {
        match stmt {
            TStmt::Store { place, value, line } => {
                let v = self.eval(value, *line)?;
                match place {
                    Place::Local(n) => self.locals[*n as usize] = Some(v),
                    Place::Global(n) => self.globals[*n as usize] = Some(v),
                }
                Ok(())
            }

            TStmt::Print { items, line } => {
                for item in items {
                    let v = self.eval(item, *line)?;
                    let text = v.display(item.ty());
                    write!(self.out, "{text}").map_err(|e| {
                        err(Rule::BytecodeIsWellFormed, *line, format!("could not write: {e}"))
                    })?;
                }
                Ok(())
            }

            TStmt::Loop { place, ty, from, to, inclusive, body, line, .. } => {
                // Counting directly, with none of the jumps and hidden slots the
                // compiled form needs — which is the point of the oracle.
                let from = self.eval(from, *line)?;
                let to = self.eval(to, *line)?;

                if *ty == RtType::Er {
                    return self.count_exact(place, &from, &to, *inclusive, body, *line);
                }

                let (Some(mut i), Some(stop)) = (as_int(&from), as_int(&to)) else {
                    return Err(err(Rule::BytecodeIsWellFormed, *line, "loop bounds are not integers"));
                };
                let unsigned = ty.is_unsigned_int();
                while if *inclusive { i <= stop } else { i < stop } {
                    if let Some(place) = place {
                        let v = if unsigned { Value::Uint(i as u64) } else { Value::Int(i as i64) };
                        match place {
                            Place::Local(n) => self.locals[*n as usize] = Some(v),
                            Place::Global(n) => self.globals[*n as usize] = Some(v),
                        }
                    }
                    self.block(body)?;
                    i += 1;
                }
                Ok(())
            }

            TStmt::If { arms, else_arm, line } => {
                for TIfArm { cond, body } in arms {
                    let Value::Bool(taken) = self.eval(cond, *line)? else {
                        return Err(err(Rule::ConditionsAreBool, *line, "condition is not a bool"));
                    };
                    if taken {
                        return self.block(body);
                    }
                }
                self.block(else_arm)
            }
        }
    }

    /// Counting over exact rationals, which are unbounded and so need none of
    /// the care an integer counter takes near the top of its type.
    fn count_exact(
        &mut self,
        place: &Option<Place>,
        from: &Value,
        to: &Value,
        inclusive: bool,
        body: &[TStmt],
        line: u32,
    ) -> Result<(), InterpError> {
        let (Value::Er(start), Value::Er(stop)) = (from, to) else {
            return Err(err(Rule::BytecodeIsWellFormed, line, "loop bounds are not exact"));
        };
        for bound in [start, stop] {
            if !bound.is_integer() {
                return Err(err(
                    Rule::LoopsCountWholeNumbers,
                    line,
                    format!("a loop cannot count from or to `{bound}`"),
                ));
            }
        }

        let one = luarus_num::Rational::one();
        let mut i = (**start).clone();
        loop {
            let keep_going = match i.cmp_to(stop) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => inclusive,
                std::cmp::Ordering::Greater => false,
            };
            if !keep_going {
                return Ok(());
            }
            if let Some(place) = place {
                let v = Value::Er(Rc::new(i.clone()));
                match place {
                    Place::Local(n) => self.locals[*n as usize] = Some(v),
                    Place::Global(n) => self.globals[*n as usize] = Some(v),
                }
            }
            self.block(body)?;
            i = i.add(&one);
        }
    }

    fn eval(&mut self, e: &TExpr, line: u32) -> Result<Value, InterpError> {
        match e {
            TExpr::Const(c, _) => Ok(Value::from_const(c)),

            TExpr::Load(place, _) => {
                let (slot, name) = match place {
                    Place::Local(n) => (
                        self.locals[*n as usize].clone(),
                        self.local_names[*n as usize].clone(),
                    ),
                    Place::Global(n) => (
                        self.globals[*n as usize].clone(),
                        self.global_names[*n as usize].0.clone(),
                    ),
                };
                slot.ok_or_else(|| {
                    err(
                        Rule::AssignBeforeReading,
                        line,
                        format!("`({name})` is read before it is assigned"),
                    )
                })
            }

            TExpr::Neg(inner, ty) => {
                let v = self.eval(inner, line)?;
                negate(v, *ty, line)
            }

            TExpr::Bin { op, operand_ty, lhs, rhs, .. } => {
                let a = self.eval(lhs, line)?;
                let b = self.eval(rhs, line)?;
                if op.is_comparison() {
                    compare(*op, &a, &b, line)
                } else {
                    arith(*op, &a, &b, *operand_ty, line)
                }
            }
        }
    }
}

/// Widen a value to `i128`, which every integer type fits inside.
fn as_int(v: &Value) -> Option<i128> {
    match v {
        Value::Int(a) => Some(*a as i128),
        Value::Uint(a) => Some(*a as i128),
        _ => None,
    }
}

fn fit(v: i128, ty: RtType, line: u32) -> Result<Value, InterpError> {
    let (lo, hi) = ty.int_range().expect("integer type");
    if v < lo || v > hi {
        // The compiled side reaches this by `checked_*` at the target width;
        // here it falls out of an explicit range test on a wider integer.
        return Err(err(Rule::OverflowTraps, line, format!("arithmetic overflowed `{}`", ty.name())));
    }
    Ok(if ty.is_unsigned_int() { Value::Uint(v as u64) } else { Value::Int(v as i64) })
}

fn negate(v: Value, ty: RtType, line: u32) -> Result<Value, InterpError> {
    match (&v, ty) {
        (Value::Int(a), t) if t.is_signed_int() => fit(-(*a as i128), t, line),
        (Value::Er(a), RtType::Er) => Ok(Value::Er(Rc::new(a.neg()))),
        (Value::F32(a), RtType::F16) => Ok(Value::F32(f16::round(-a))),
        (Value::F32(a), _) => Ok(Value::F32(-a)),
        (Value::F64(a), _) => Ok(Value::F64(-a)),
        _ => Err(err(Rule::BytecodeIsWellFormed, line, "cannot negate this value")),
    }
}

fn arith(op: BinOp, a: &Value, b: &Value, ty: RtType, line: u32) -> Result<Value, InterpError> {
    if ty == RtType::Er {
        let (Value::Er(x), Value::Er(y)) = (a, b) else {
            return Err(err(Rule::BytecodeIsWellFormed, line, "expected er operands"));
        };
        let r = match op {
            BinOp::Add => x.add(y),
            BinOp::Sub => x.sub(y),
            BinOp::Mul => x.mul(y),
            BinOp::Div => x
                .div(y)
                .ok_or_else(|| err(Rule::NoDivisionByZero, line, "division by zero"))?,
            BinOp::Rem => x
                .rem(y)
                .ok_or_else(|| err(Rule::NoDivisionByZero, line, "remainder by zero"))?,
            _ => unreachable!("arith with a comparison"),
        };
        return Ok(Value::Er(Rc::new(r)));
    }
    if ty.is_int() {
        let (Some(x), Some(y)) = (as_int(a), as_int(b)) else {
            return Err(err(Rule::BytecodeIsWellFormed, line, "expected integers"));
        };
        // i128 is wide enough for every operand pair except a u64 product, so
        // the multiply is still checked.
        let r = match op {
            BinOp::Add => x.checked_add(y),
            BinOp::Sub => {
                if ty.is_unsigned_int() && y > x {
                    return Err(err(
                        Rule::UnsignedIsNeverNegative,
                        line,
                        format!("subtraction went below zero in `{}`", ty.name()),
                    ));
                }
                x.checked_sub(y)
            }
            BinOp::Mul => x.checked_mul(y),
            BinOp::Div => {
                if y == 0 {
                    return Err(err(Rule::NoDivisionByZero, line, "division by zero"));
                }
                x.checked_div(y)
            }
            BinOp::Rem => {
                if y == 0 {
                    return Err(err(Rule::NoDivisionByZero, line, "remainder by zero"));
                }
                x.checked_rem(y)
            }
            _ => unreachable!("arith with a comparison"),
        };
        let r = r.ok_or_else(|| {
            err(Rule::OverflowTraps, line, format!("arithmetic overflowed `{}`", ty.name()))
        })?;
        return fit(r, ty, line);
    }

    // Floating point is computed at its own width here, where the VM widens to
    // f64 and rounds back. IEEE 754 makes those agree for + - * /, so a
    // disagreement would be a real fault rather than a rounding artefact.
    match (a, b) {
        (Value::F64(x), Value::F64(y)) => Ok(Value::F64(float(op, *x, *y))),
        (Value::F32(x), Value::F32(y)) => {
            let r = float(op, *x, *y);
            Ok(Value::F32(if ty == RtType::F16 { f16::round(r) } else { r }))
        }
        _ => Err(err(Rule::BytecodeIsWellFormed, line, "expected floats")),
    }
}

fn float<T>(op: BinOp, a: T, b: T) -> T
where
    T: std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>
        + std::ops::Rem<Output = T>,
{
    match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => a / b,
        BinOp::Rem => a % b,
        _ => unreachable!("float with a comparison"),
    }
}

fn compare(op: BinOp, a: &Value, b: &Value, line: u32) -> Result<Value, InterpError> {
    use std::cmp::Ordering::*;
    let ord = match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.partial_cmp(y),
        (Value::Uint(x), Value::Uint(y)) => x.partial_cmp(y),
        (Value::F32(x), Value::F32(y)) => x.partial_cmp(y),
        (Value::F64(x), Value::F64(y)) => x.partial_cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.partial_cmp(y),
        (Value::Er(x), Value::Er(y)) => Some(x.cmp_to(y)),
        (Value::Str(x), Value::Str(y)) => Some(x.as_ref().cmp(y.as_ref())),
        (Value::Nil, Value::Nil) => Some(Equal),
        _ => return Err(err(Rule::BytecodeIsWellFormed, line, "cannot compare these values")),
    };
    Ok(Value::Bool(match op {
        BinOp::Eq => ord == Some(Equal),
        BinOp::Ne => ord != Some(Equal),
        BinOp::Lt => ord == Some(Less),
        BinOp::Le => matches!(ord, Some(Less) | Some(Equal)),
        BinOp::Gt => ord == Some(Greater),
        BinOp::Ge => matches!(ord, Some(Greater) | Some(Equal)),
        _ => unreachable!("compare with an arithmetic operator"),
    }))
}

/// Kept so the crate compiles against `Const` even as the value set grows.
const _: fn(&Const) -> Value = Value::from_const;
