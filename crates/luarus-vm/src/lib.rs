//! The Luarus virtual machine.
//!
//! A typed operand stack. Because the checker has already proved both operands
//! of every instruction share a type, the VM never inspects a value to decide
//! what an instruction means — the instruction says so itself.

use std::fmt;
use std::io::Write;

use luarus_bytecode::value::Value;
use luarus_bytecode::{f16, Chunk, Op, RtType};

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
    pub line: u32,
    pub source: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime error: {}\n  --> {}:{}", self.message, self.source, self.line)
    }
}

impl std::error::Error for RuntimeError {}

/// Run a chunk, writing anything it prints to `out`.
pub fn run(chunk: &Chunk, out: &mut dyn Write) -> Result<(), RuntimeError> {
    Vm::new(chunk, out).run()
}

/// Run a chunk and capture its output, for tests and tooling.
pub fn run_capturing(chunk: &Chunk) -> Result<String, RuntimeError> {
    let mut buf = Vec::new();
    run(chunk, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

struct Vm<'a> {
    chunk: &'a Chunk,
    out: &'a mut dyn Write,
    stack: Vec<Value>,
    locals: Vec<Option<Value>>,
    globals: Vec<Option<Value>>,
    pc: usize,
}

impl<'a> Vm<'a> {
    fn new(chunk: &'a Chunk, out: &'a mut dyn Write) -> Self {
        Vm {
            chunk,
            out,
            stack: Vec::with_capacity(16),
            locals: vec![None; chunk.locals],
            globals: vec![None; chunk.globals.len()],
            pc: 0,
        }
    }

    fn err(&self, message: impl Into<String>) -> RuntimeError {
        // `pc` has already advanced past the failing instruction.
        let idx = self.pc.saturating_sub(1);
        RuntimeError {
            message: message.into(),
            line: self.chunk.lines.get(idx).copied().unwrap_or(0),
            source: self.chunk.source.clone(),
        }
    }

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or_else(|| self.err("operand stack underflow"))
    }

    fn run(mut self) -> Result<(), RuntimeError> {
        while self.pc < self.chunk.code.len() {
            let op = self.chunk.code[self.pc];
            self.pc += 1;
            match op {
                Op::Halt => return Ok(()),
                Op::Pop => {
                    self.pop()?;
                }
                Op::Const(k) => {
                    let c = self
                        .chunk
                        .consts
                        .get(k as usize)
                        .ok_or_else(|| self.err(format!("constant {k} is out of range")))?;
                    self.stack.push(Value::from_const(c));
                }
                Op::LoadLocal(n) => {
                    let v = self
                        .locals
                        .get(n as usize)
                        .ok_or_else(|| self.err(format!("local slot {n} is out of range")))?
                        .clone()
                        .ok_or_else(|| {
                            let name = self.name_of_local(n);
                            self.err(format!("`({name})` is read before it is assigned"))
                        })?;
                    self.stack.push(v);
                }
                Op::StoreLocal(n) => {
                    let v = self.pop()?;
                    let slot = self
                        .locals
                        .get_mut(n as usize)
                        .ok_or_else(|| RuntimeError {
                            message: format!("local slot {n} is out of range"),
                            line: 0,
                            source: String::new(),
                        })?;
                    *slot = Some(v);
                }
                Op::LoadGlobal(n) => {
                    let v = self
                        .globals
                        .get(n as usize)
                        .ok_or_else(|| self.err(format!("global {n} is out of range")))?
                        .clone()
                        .ok_or_else(|| {
                            let name = self.name_of_global(n);
                            self.err(format!("`({name})` is read before it is assigned"))
                        })?;
                    self.stack.push(v);
                }
                Op::StoreGlobal(n) => {
                    let v = self.pop()?;
                    let slot = self.globals.get_mut(n as usize).ok_or(RuntimeError {
                        message: format!("global {n} is out of range"),
                        line: 0,
                        source: String::new(),
                    })?;
                    *slot = Some(v);
                }
                Op::Print(ty) => {
                    let v = self.pop()?;
                    let text = v.display(ty);
                    writeln!(self.out, "{text}").map_err(|e| RuntimeError {
                        message: format!("could not write output: {e}"),
                        line: 0,
                        source: self.chunk.source.clone(),
                    })?;
                }
                Op::Neg(ty) => {
                    let v = self.pop()?;
                    let r = self.negate(v, ty)?;
                    self.stack.push(r);
                }
                Op::Add(t) | Op::Sub(t) | Op::Mul(t) | Op::Div(t) | Op::Rem(t) => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = self.arith(op, lhs, rhs, t)?;
                    self.stack.push(r);
                }
                Op::Eq(t) | Op::Ne(t) | Op::Lt(t) | Op::Le(t) | Op::Gt(t) | Op::Ge(t) => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = self.compare(op, lhs, rhs, t)?;
                    self.stack.push(Value::Bool(r));
                }
            }
        }
        Ok(())
    }

    fn name_of_local(&self, n: u32) -> String {
        self.chunk.local_names.get(n as usize).cloned().unwrap_or_else(|| n.to_string())
    }

    fn name_of_global(&self, n: u32) -> String {
        self.chunk.globals.get(n as usize).map(|g| g.name.clone()).unwrap_or_else(|| n.to_string())
    }

    fn negate(&self, v: Value, ty: RtType) -> Result<Value, RuntimeError> {
        Ok(match (v, ty) {
            (Value::Int(a), t) if t.is_signed_int() => {
                let r = a.checked_neg().ok_or_else(|| self.overflow("negation", t))?;
                Value::Int(self.fit_signed(r, t)?)
            }
            (Value::F32(a), RtType::F16) => Value::F32(f16::round(-a)),
            (Value::F32(a), _) => Value::F32(-a),
            (Value::F64(a), _) => Value::F64(-a),
            _ => return Err(self.err(format!("cannot negate a `{}`", ty.name()))),
        })
    }

    fn overflow(&self, what: &str, ty: RtType) -> RuntimeError {
        self.err(format!(
            "{what} overflowed `{}`; Luarus traps on overflow rather than wrapping",
            ty.name()
        ))
    }

    /// Narrow a signed result to `ty`, or report overflow.
    fn fit_signed(&self, v: i64, ty: RtType) -> Result<i64, RuntimeError> {
        let (lo, hi) = ty.int_range().expect("signed integer type");
        if (v as i128) < lo || (v as i128) > hi {
            return Err(self.overflow("arithmetic", ty));
        }
        Ok(v)
    }

    fn fit_unsigned(&self, v: u64, ty: RtType) -> Result<u64, RuntimeError> {
        let (_, hi) = ty.int_range().expect("unsigned integer type");
        if (v as i128) > hi {
            return Err(self.overflow("arithmetic", ty));
        }
        Ok(v)
    }

    fn arith(&self, op: Op, lhs: Value, rhs: Value, ty: RtType) -> Result<Value, RuntimeError> {
        if ty.is_signed_int() {
            let (Value::Int(a), Value::Int(b)) = (&lhs, &rhs) else {
                return Err(self.err("expected signed integers"));
            };
            let (a, b) = (*a, *b);
            let r = match op {
                Op::Add(_) => a.checked_add(b),
                Op::Sub(_) => a.checked_sub(b),
                Op::Mul(_) => a.checked_mul(b),
                Op::Div(_) => {
                    if b == 0 {
                        return Err(self.err("division by zero"));
                    }
                    a.checked_div(b)
                }
                Op::Rem(_) => {
                    if b == 0 {
                        return Err(self.err("remainder by zero"));
                    }
                    a.checked_rem(b)
                }
                _ => unreachable!("arith called with {op:?}"),
            }
            .ok_or_else(|| self.overflow("arithmetic", ty))?;
            return Ok(Value::Int(self.fit_signed(r, ty)?));
        }

        if ty.is_unsigned_int() {
            let (Value::Uint(a), Value::Uint(b)) = (&lhs, &rhs) else {
                return Err(self.err("expected unsigned integers"));
            };
            let (a, b) = (*a, *b);
            let r = match op {
                Op::Add(_) => a.checked_add(b),
                Op::Sub(_) => a.checked_sub(b).ok_or_else(|| {
                    self.err(format!(
                        "subtraction went below zero in unsigned type `{}`",
                        ty.name()
                    ))
                }).map(Some)?,
                Op::Mul(_) => a.checked_mul(b),
                Op::Div(_) => {
                    if b == 0 {
                        return Err(self.err("division by zero"));
                    }
                    a.checked_div(b)
                }
                Op::Rem(_) => {
                    if b == 0 {
                        return Err(self.err("remainder by zero"));
                    }
                    a.checked_rem(b)
                }
                _ => unreachable!("arith called with {op:?}"),
            }
            .ok_or_else(|| self.overflow("arithmetic", ty))?;
            return Ok(Value::Uint(self.fit_unsigned(r, ty)?));
        }

        // Floating point follows IEEE 754: no traps, infinities and NaN instead.
        match ty {
            RtType::F64 => {
                let (Value::F64(a), Value::F64(b)) = (&lhs, &rhs) else {
                    return Err(self.err("expected f64 operands"));
                };
                let r = float_op(op, *a, *b);
                Ok(Value::F64(r))
            }
            RtType::F32 | RtType::F16 => {
                let (Value::F32(a), Value::F32(b)) = (&lhs, &rhs) else {
                    return Err(self.err("expected f32 operands"));
                };
                let r = float_op(op, *a as f64, *b as f64) as f32;
                // Half precision is re-rounded after every step, so the extra
                // range of the f32 carrier never leaks into results.
                Ok(Value::F32(if ty == RtType::F16 { f16::round(r) } else { r }))
            }
            _ => Err(self.err(format!("`{}` has no arithmetic", ty.name()))),
        }
    }

    fn compare(&self, op: Op, lhs: Value, rhs: Value, ty: RtType) -> Result<bool, RuntimeError> {
        let ord = match (&lhs, &rhs) {
            (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
            (Value::Uint(a), Value::Uint(b)) => a.partial_cmp(b),
            (Value::F32(a), Value::F32(b)) => a.partial_cmp(b),
            (Value::F64(a), Value::F64(b)) => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::Str(a), Value::Str(b)) => Some(a.as_ref().cmp(b.as_ref())),
            (Value::Nil, Value::Nil) => Some(std::cmp::Ordering::Equal),
            _ => return Err(self.err(format!("cannot compare these `{}` values", ty.name()))),
        };

        use std::cmp::Ordering::*;
        Ok(match op {
            // NaN compares unequal to everything, itself included.
            Op::Eq(_) => ord == Some(Equal),
            Op::Ne(_) => ord != Some(Equal),
            Op::Lt(_) => ord == Some(Less),
            Op::Le(_) => matches!(ord, Some(Less) | Some(Equal)),
            Op::Gt(_) => ord == Some(Greater),
            Op::Ge(_) => matches!(ord, Some(Greater) | Some(Equal)),
            _ => unreachable!("compare called with {op:?}"),
        })
    }
}

fn float_op(op: Op, a: f64, b: f64) -> f64 {
    match op {
        Op::Add(_) => a + b,
        Op::Sub(_) => a - b,
        Op::Mul(_) => a * b,
        Op::Div(_) => a / b,
        Op::Rem(_) => a % b,
        _ => unreachable!("float_op called with {op:?}"),
    }
}
