use crate::value::RtType;

/// A Luarus instruction.
///
/// Instructions are typed: there is no generic `Add` that inspects its operands
/// at runtime. The checker has already proved both sides are the same type, so
/// `Add(I32)` can do exactly one thing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Push constant pool entry `n`.
    Const(u32),
    /// Push the value in local slot `n`.
    LoadLocal(u32),
    /// Pop and store into local slot `n`.
    StoreLocal(u32),
    /// Push the value of global `n`.
    LoadGlobal(u32),
    /// Pop and store into global `n`.
    StoreGlobal(u32),

    Add(RtType),
    Sub(RtType),
    Mul(RtType),
    Div(RtType),
    Rem(RtType),
    Neg(RtType),

    Eq(RtType),
    Ne(RtType),
    Lt(RtType),
    Le(RtType),
    Gt(RtType),
    Ge(RtType),

    /// Continue at this instruction.
    Jump(u32),
    /// Pop a bool; continue at this instruction if it is false.
    JumpIfFalse(u32),

    /// The tail of a counting loop, in one instruction.
    ///
    /// If `counter` is below `bound`, step it by one and continue at `target`;
    /// otherwise fall through. Stepping only while strictly below means the
    /// increment can never overflow the counter's type, and it leaves the
    /// counter holding the last value the loop used.
    LoopStep { counter: u32, bound: u32, target: u32, ty: RtType },

    /// Fail unless local `n` holds a whole number.
    ///
    /// Only `er` can carry a fraction into a loop bound, and only when the
    /// bound was computed rather than written down — a literal is caught by the
    /// checker instead.
    RequireWhole(u32),

    /// Pop and write, formatting according to the static type. Writes no
    /// newline: every line ending in Luarus is written by hand.
    Write(RtType),
    /// Discard the top of the stack.
    Pop,
    /// Stop execution.
    Halt,
}

impl Op {
    pub fn mnemonic(self) -> &'static str {
        match self {
            Op::Const(_) => "const",
            Op::LoadLocal(_) => "load.local",
            Op::StoreLocal(_) => "store.local",
            Op::LoadGlobal(_) => "load.global",
            Op::StoreGlobal(_) => "store.global",
            Op::Add(_) => "add",
            Op::Sub(_) => "sub",
            Op::Mul(_) => "mul",
            Op::Div(_) => "div",
            Op::Rem(_) => "rem",
            Op::Neg(_) => "neg",
            Op::Eq(_) => "eq",
            Op::Ne(_) => "ne",
            Op::Lt(_) => "lt",
            Op::Le(_) => "le",
            Op::Gt(_) => "gt",
            Op::Ge(_) => "ge",
            Op::Jump(_) => "jump",
            Op::JumpIfFalse(_) => "jump.false",
            Op::LoopStep { .. } => "loop.step",
            Op::RequireWhole(_) => "require.whole",
            Op::Write(_) => "write",
            Op::Pop => "pop",
            Op::Halt => "halt",
        }
    }
}
