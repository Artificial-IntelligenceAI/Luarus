use crate::f16;
use std::rc::Rc;

/// Every type Luarus has at runtime. There is no `number`: a program says which
/// width it means, which is the point of the language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RtType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F16,
    F32,
    F64,
    Bool,
    Str,
    Nil,
}

impl RtType {
    pub fn name(self) -> &'static str {
        match self {
            RtType::I8 => "i8",
            RtType::I16 => "i16",
            RtType::I32 => "i32",
            RtType::I64 => "i64",
            RtType::U8 => "u8",
            RtType::U16 => "u16",
            RtType::U32 => "u32",
            RtType::U64 => "u64",
            RtType::F16 => "f16",
            RtType::F32 => "f32",
            RtType::F64 => "f64",
            RtType::Bool => "bool",
            RtType::Str => "str",
            RtType::Nil => "nil",
        }
    }

    pub fn from_name(name: &str) -> Option<RtType> {
        Some(match name {
            "i8" => RtType::I8,
            "i16" => RtType::I16,
            "i32" => RtType::I32,
            "i64" => RtType::I64,
            "u8" => RtType::U8,
            "u16" => RtType::U16,
            "u32" => RtType::U32,
            "u64" => RtType::U64,
            "f16" => RtType::F16,
            "f32" => RtType::F32,
            "f64" => RtType::F64,
            "bool" => RtType::Bool,
            "str" => RtType::Str,
            "nil" => RtType::Nil,
            _ => return None,
        })
    }

    pub fn is_signed_int(self) -> bool {
        matches!(self, RtType::I8 | RtType::I16 | RtType::I32 | RtType::I64)
    }

    pub fn is_unsigned_int(self) -> bool {
        matches!(self, RtType::U8 | RtType::U16 | RtType::U32 | RtType::U64)
    }

    pub fn is_int(self) -> bool {
        self.is_signed_int() || self.is_unsigned_int()
    }

    pub fn is_float(self) -> bool {
        matches!(self, RtType::F16 | RtType::F32 | RtType::F64)
    }

    pub fn is_numeric(self) -> bool {
        self.is_int() || self.is_float()
    }

    /// Inclusive bounds for an integer type.
    pub fn int_range(self) -> Option<(i128, i128)> {
        Some(match self {
            RtType::I8 => (i8::MIN as i128, i8::MAX as i128),
            RtType::I16 => (i16::MIN as i128, i16::MAX as i128),
            RtType::I32 => (i32::MIN as i128, i32::MAX as i128),
            RtType::I64 => (i64::MIN as i128, i64::MAX as i128),
            RtType::U8 => (0, u8::MAX as i128),
            RtType::U16 => (0, u16::MAX as i128),
            RtType::U32 => (0, u32::MAX as i128),
            RtType::U64 => (0, u64::MAX as i128),
            _ => return None,
        })
    }

    pub fn tag(self) -> u8 {
        self as u8
    }

    pub fn from_tag(tag: u8) -> Option<RtType> {
        Some(match tag {
            0 => RtType::I8,
            1 => RtType::I16,
            2 => RtType::I32,
            3 => RtType::I64,
            4 => RtType::U8,
            5 => RtType::U16,
            6 => RtType::U32,
            7 => RtType::U64,
            8 => RtType::F16,
            9 => RtType::F32,
            10 => RtType::F64,
            11 => RtType::Bool,
            12 => RtType::Str,
            13 => RtType::Nil,
            _ => return None,
        })
    }
}

/// A constant baked into a chunk's constant pool.
#[derive(Clone, Debug)]
pub enum Const {
    /// Any signed integer type; the width lives in the instruction, not here.
    Int(i64),
    /// Any unsigned integer type.
    Uint(u64),
    /// Half precision, stored as raw binary16 bits.
    F16(u16),
    F32(f32),
    F64(f64),
    Bool(bool),
    Str(String),
    Nil,
}

/// Constants compare by their **bits**, not by IEEE equality.
///
/// The constant pool deduplicates on this, and `0.0 == -0.0` is true in IEEE
/// arithmetic — so comparing by value would collapse two genuinely different
/// constants into one, and whichever landed in the pool first would win for
/// both. It also lets a NaN survive a round trip through the pool, which `==`
/// would never admit.
impl PartialEq for Const {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Const::Int(a), Const::Int(b)) => a == b,
            (Const::Uint(a), Const::Uint(b)) => a == b,
            (Const::F16(a), Const::F16(b)) => a == b,
            (Const::F32(a), Const::F32(b)) => a.to_bits() == b.to_bits(),
            (Const::F64(a), Const::F64(b)) => a.to_bits() == b.to_bits(),
            (Const::Bool(a), Const::Bool(b)) => a == b,
            (Const::Str(a), Const::Str(b)) => a == b,
            (Const::Nil, Const::Nil) => true,
            _ => false,
        }
    }
}

/// A value on the VM's operand stack.
#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Uint(u64),
    /// Covers both `f16` and `f32`. An `f16` is kept rounded to half precision
    /// after every operation, so the extra range is never observable.
    F32(f32),
    F64(f64),
    Bool(bool),
    Str(Rc<str>),
    Nil,
}

impl Value {
    pub fn from_const(c: &Const) -> Value {
        match c {
            Const::Int(v) => Value::Int(*v),
            Const::Uint(v) => Value::Uint(*v),
            Const::F16(bits) => Value::F32(f16::to_f32(*bits)),
            Const::F32(v) => Value::F32(*v),
            Const::F64(v) => Value::F64(*v),
            Const::Bool(v) => Value::Bool(*v),
            Const::Str(s) => Value::Str(Rc::from(s.as_str())),
            Const::Nil => Value::Nil,
        }
    }

    /// Render for `print`, using the static type so widths display honestly.
    pub fn display(&self, ty: RtType) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Uint(v) => v.to_string(),
            Value::F32(v) => format_float(*v as f64, ty),
            Value::F64(v) => format_float(*v, ty),
            Value::Bool(v) => v.to_string(),
            Value::Str(s) => s.to_string(),
            Value::Nil => "nil".to_string(),
        }
    }
}

fn format_float(v: f64, _ty: RtType) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == v.trunc() && v.abs() < 1e15 {
        // Floats print with a trailing `.0` so `f64` never looks like an integer.
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}
