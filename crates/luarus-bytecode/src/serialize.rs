//! The `.lrb` container format.
//!
//! Hand-rolled little-endian encoding, so the compiler and VM carry no
//! third-party dependencies at all.

use crate::chunk::{Chunk, GlobalInfo};
use crate::op::Op;
use crate::value::{Const, RtType};

pub const MAGIC: [u8; 4] = *b"LRSB";
pub const VERSION: u16 = 6;

#[derive(Debug)]
pub struct DecodeError(pub String);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed bytecode: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

// ---------------------------------------------------------------- encoding

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
    fn ty(&mut self, t: RtType) {
        self.u8(t.tag());
    }
    fn limbs(&mut self, limbs: &[u32]) {
        self.u32(limbs.len() as u32);
        for l in limbs {
            self.u32(*l);
        }
    }
}

pub fn encode(chunk: &Chunk) -> Vec<u8> {
    let mut w = Writer { buf: Vec::new() };
    w.buf.extend_from_slice(&MAGIC);
    w.u16(VERSION);

    w.str(&chunk.source);
    w.u32(chunk.locals as u32);

    w.u32(chunk.local_names.len() as u32);
    for n in &chunk.local_names {
        w.str(n);
    }

    w.u32(chunk.globals.len() as u32);
    for g in &chunk.globals {
        w.str(&g.name);
        w.ty(g.ty);
        w.u8(g.exported as u8);
    }

    w.u32(chunk.consts.len() as u32);
    for c in &chunk.consts {
        match c {
            Const::Int(v) => {
                w.u8(0);
                w.i64(*v);
            }
            Const::Uint(v) => {
                w.u8(1);
                w.u64(*v);
            }
            Const::F16(bits) => {
                w.u8(2);
                w.u16(*bits);
            }
            Const::F32(v) => {
                w.u8(3);
                w.u32(v.to_bits());
            }
            Const::F64(v) => {
                w.u8(4);
                w.u64(v.to_bits());
            }
            Const::Er(r) => {
                w.u8(8);
                w.u8(r.is_negative() as u8);
                w.limbs(r.numerator().limbs());
                w.limbs(r.denominator().limbs());
            }
            Const::Bool(v) => {
                w.u8(5);
                w.u8(*v as u8);
            }
            Const::Str(s) => {
                w.u8(6);
                w.str(s);
            }
            Const::Nil => w.u8(7),
        }
    }

    w.u32(chunk.code.len() as u32);
    for op in &chunk.code {
        encode_op(&mut w, *op);
    }

    w.u32(chunk.lines.len() as u32);
    for l in &chunk.lines {
        w.u32(*l);
    }

    w.buf
}

fn encode_op(w: &mut Writer, op: Op) {
    match op {
        Op::Const(n) => {
            w.u8(0);
            w.u32(n);
        }
        Op::LoadLocal(n) => {
            w.u8(1);
            w.u32(n);
        }
        Op::StoreLocal(n) => {
            w.u8(2);
            w.u32(n);
        }
        Op::LoadGlobal(n) => {
            w.u8(3);
            w.u32(n);
        }
        Op::StoreGlobal(n) => {
            w.u8(4);
            w.u32(n);
        }
        Op::Add(t) => {
            w.u8(5);
            w.ty(t);
        }
        Op::Sub(t) => {
            w.u8(6);
            w.ty(t);
        }
        Op::Mul(t) => {
            w.u8(7);
            w.ty(t);
        }
        Op::Div(t) => {
            w.u8(8);
            w.ty(t);
        }
        Op::Rem(t) => {
            w.u8(9);
            w.ty(t);
        }
        Op::Neg(t) => {
            w.u8(10);
            w.ty(t);
        }
        Op::Eq(t) => {
            w.u8(11);
            w.ty(t);
        }
        Op::Ne(t) => {
            w.u8(12);
            w.ty(t);
        }
        Op::Lt(t) => {
            w.u8(13);
            w.ty(t);
        }
        Op::Le(t) => {
            w.u8(14);
            w.ty(t);
        }
        Op::Gt(t) => {
            w.u8(15);
            w.ty(t);
        }
        Op::Ge(t) => {
            w.u8(16);
            w.ty(t);
        }
        Op::Write(t) => {
            w.u8(17);
            w.ty(t);
        }
        Op::Pop => w.u8(18),
        Op::Halt => w.u8(19),
        Op::Jump(t) => {
            w.u8(20);
            w.u32(t);
        }
        Op::JumpIfFalse(t) => {
            w.u8(21);
            w.u32(t);
        }
        Op::RequireWhole(n) => {
            w.u8(22);
            w.u32(n);
        }
        Op::LoopStep { counter, bound, target, ty } => {
            w.u8(23);
            w.u32(counter);
            w.u32(bound);
            w.u32(target);
            w.ty(ty);
        }
    }
}

// ---------------------------------------------------------------- decoding

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.pos.checked_add(n).ok_or_else(|| DecodeError("length overflow".into()))?;
        let s = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| DecodeError(format!("unexpected end of file at byte {}", self.pos)))?;
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, DecodeError> {
        let n = self.u32()? as usize;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError("string is not valid UTF-8".into()))
    }
    fn ty(&mut self) -> Result<RtType, DecodeError> {
        let tag = self.u8()?;
        RtType::from_tag(tag).ok_or_else(|| DecodeError(format!("unknown type tag {tag}")))
    }
    fn limbs(&mut self) -> Result<Vec<u32>, DecodeError> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.u32()?);
        }
        Ok(out)
    }

    /// Guard against a corrupt length field asking us to preallocate gigabytes.
    fn count(&mut self) -> Result<usize, DecodeError> {
        let n = self.u32()? as usize;
        if n > self.buf.len() - self.pos + 1 {
            return Err(DecodeError(format!("declared count {n} exceeds remaining input")));
        }
        Ok(n)
    }
}

pub fn decode(buf: &[u8]) -> Result<Chunk, DecodeError> {
    let mut r = Reader { buf, pos: 0 };
    if r.take(4)? != MAGIC {
        return Err(DecodeError("not a Luarus bytecode file (bad magic)".into()));
    }
    let version = r.u16()?;
    if version != VERSION {
        return Err(DecodeError(format!(
            "bytecode version {version}, but this VM speaks version {VERSION}"
        )));
    }

    let source = r.str()?;
    let locals = r.u32()? as usize;

    let n = r.count()?;
    let mut local_names = Vec::with_capacity(n);
    for _ in 0..n {
        local_names.push(r.str()?);
    }

    let n = r.count()?;
    let mut globals = Vec::with_capacity(n);
    for _ in 0..n {
        let name = r.str()?;
        let ty = r.ty()?;
        let exported = r.u8()? != 0;
        globals.push(GlobalInfo { name, ty, exported });
    }

    let n = r.count()?;
    let mut consts = Vec::with_capacity(n);
    for _ in 0..n {
        let tag = r.u8()?;
        consts.push(match tag {
            0 => Const::Int(r.i64()?),
            1 => Const::Uint(r.u64()?),
            2 => Const::F16(r.u16()?),
            3 => Const::F32(f32::from_bits(r.u32()?)),
            4 => Const::F64(f64::from_bits(r.u64()?)),
            5 => Const::Bool(r.u8()? != 0),
            6 => Const::Str(r.str()?),
            7 => Const::Nil,
            8 => {
                let negative = r.u8()? != 0;
                let num = luarus_num::BigUint::from_limbs(r.limbs()?);
                let den = luarus_num::BigUint::from_limbs(r.limbs()?);
                Const::Er(
                    luarus_num::Rational::new(negative, num, den)
                        .ok_or_else(|| DecodeError("an er constant has a zero denominator".into()))?,
                )
            }
            _ => return Err(DecodeError(format!("unknown constant tag {tag}"))),
        });
    }

    let n = r.count()?;
    let mut code = Vec::with_capacity(n);
    for _ in 0..n {
        code.push(decode_op(&mut r)?);
    }

    let n = r.count()?;
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        lines.push(r.u32()?);
    }

    Ok(Chunk { source, consts, code, lines, locals, globals, local_names })
}

fn decode_op(r: &mut Reader) -> Result<Op, DecodeError> {
    let code = r.u8()?;
    Ok(match code {
        0 => Op::Const(r.u32()?),
        1 => Op::LoadLocal(r.u32()?),
        2 => Op::StoreLocal(r.u32()?),
        3 => Op::LoadGlobal(r.u32()?),
        4 => Op::StoreGlobal(r.u32()?),
        5 => Op::Add(r.ty()?),
        6 => Op::Sub(r.ty()?),
        7 => Op::Mul(r.ty()?),
        8 => Op::Div(r.ty()?),
        9 => Op::Rem(r.ty()?),
        10 => Op::Neg(r.ty()?),
        11 => Op::Eq(r.ty()?),
        12 => Op::Ne(r.ty()?),
        13 => Op::Lt(r.ty()?),
        14 => Op::Le(r.ty()?),
        15 => Op::Gt(r.ty()?),
        16 => Op::Ge(r.ty()?),
        17 => Op::Write(r.ty()?),
        18 => Op::Pop,
        19 => Op::Halt,
        20 => Op::Jump(r.u32()?),
        21 => Op::JumpIfFalse(r.u32()?),
        22 => Op::RequireWhole(r.u32()?),
        23 => Op::LoopStep {
            counter: r.u32()?,
            bound: r.u32()?,
            target: r.u32()?,
            ty: r.ty()?,
        },
        _ => return Err(DecodeError(format!("unknown opcode {code}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_chunk() {
        let mut c = Chunk::new("test.lrs");
        c.locals = 2;
        c.local_names = vec!["x".into(), "count 🎉".into()];
        c.globals = vec![GlobalInfo { name: "g".into(), ty: RtType::I32, exported: true }];
        let k = c.add_const(Const::Int(1000));
        c.emit(Op::Const(k), 1);
        c.emit(Op::Add(RtType::F16), 1);
        c.emit(Op::Halt, 2);

        let decoded = decode(&encode(&c)).expect("round trip");
        assert_eq!(decoded.code, c.code);
        assert_eq!(decoded.consts, c.consts);
        assert_eq!(decoded.local_names, c.local_names);
        assert_eq!(decoded.globals, c.globals);
        assert_eq!(decoded.lines, c.lines);
    }

    #[test]
    fn rejects_foreign_files() {
        assert!(decode(b"not bytecode at all").is_err());
    }
}
