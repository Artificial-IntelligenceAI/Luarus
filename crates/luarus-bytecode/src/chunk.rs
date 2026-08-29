use crate::op::Op;
use crate::value::{Const, RtType};

/// A global variable as recorded in a compiled chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalInfo {
    pub name: String,
    pub ty: RtType,
    /// `pub var` is exported from the module; plain `global var` is not.
    pub exported: bool,
}

/// One compiled unit: the Luarus equivalent of a `.class` file.
#[derive(Clone, Debug, Default)]
pub struct Chunk {
    /// Where this chunk came from, kept only for error messages.
    pub source: String,
    pub consts: Vec<Const>,
    pub code: Vec<Op>,
    /// Line number for each instruction, parallel to `code`.
    pub lines: Vec<u32>,
    /// How many local slots the frame needs.
    pub locals: usize,
    pub globals: Vec<GlobalInfo>,
    /// Debug names for local slots, parallel to slot index.
    pub local_names: Vec<String>,
}

impl Chunk {
    pub fn new(source: impl Into<String>) -> Self {
        Chunk { source: source.into(), ..Default::default() }
    }

    /// Add a constant, reusing an identical existing entry.
    pub fn add_const(&mut self, c: Const) -> u32 {
        if let Some(i) = self.consts.iter().position(|e| e == &c) {
            return i as u32;
        }
        self.consts.push(c);
        (self.consts.len() - 1) as u32
    }

    pub fn emit(&mut self, op: Op, line: u32) -> usize {
        self.code.push(op);
        self.lines.push(line);
        self.code.len() - 1
    }

    /// Point a previously emitted jump at `target`.
    ///
    /// A forward jump cannot know its destination when it is emitted, so it goes
    /// out with a placeholder and is corrected once the destination is reached.
    pub fn patch_jump(&mut self, at: usize, target: u32) {
        match &mut self.code[at] {
            Op::Jump(t) | Op::JumpIfFalse(t) => *t = target,
            other => panic!("tried to patch {other:?}, which is not a jump"),
        }
    }

    /// A human-readable listing, in the spirit of `javap -c`.
    pub fn disassemble(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("chunk {}\n", self.source));
        out.push_str(&format!("  locals: {}\n", self.locals));

        if !self.local_names.is_empty() {
            out.push_str("  local slots:\n");
            for (i, n) in self.local_names.iter().enumerate() {
                out.push_str(&format!("    {i:>3}  ({n})\n"));
            }
        }
        if !self.globals.is_empty() {
            out.push_str("  globals:\n");
            for (i, g) in self.globals.iter().enumerate() {
                let vis = if g.exported { "pub" } else { "global" };
                out.push_str(&format!("    {i:>3}  {vis} {} ({})\n", g.ty.name(), g.name));
            }
        }
        if !self.consts.is_empty() {
            out.push_str("  constants:\n");
            for (i, c) in self.consts.iter().enumerate() {
                out.push_str(&format!("    {i:>3}  {}\n", show_const(c)));
            }
        }

        out.push_str("  code:\n");
        for (i, op) in self.code.iter().enumerate() {
            let line = self.lines.get(i).copied().unwrap_or(0);
            out.push_str(&format!("  {i:>5}  {line:>5}  {}\n", show_op(self, *op)));
        }
        out
    }
}

fn show_const(c: &Const) -> String {
    match c {
        Const::Int(v) => format!("int    {v}"),
        Const::Uint(v) => format!("uint   {v}"),
        Const::F16(bits) => format!("f16    {} (0x{bits:04x})", crate::f16::to_f32(*bits)),
        Const::F32(v) => format!("f32    {v}"),
        Const::F64(v) => format!("f64    {v}"),
        Const::Er(r) => format!("er     {r}"),
        Const::Bool(v) => format!("bool   {v}"),
        Const::Str(s) => format!("str    {s:?}"),
        Const::Nil => "nil".to_string(),
    }
}

fn show_op(chunk: &Chunk, op: Op) -> String {
    let m = op.mnemonic();
    match op {
        Op::Const(n) => {
            let c = chunk.consts.get(n as usize).map(show_const).unwrap_or_default();
            format!("{m:<14} {n}    -- {c}")
        }
        Op::LoadLocal(n) | Op::StoreLocal(n) | Op::RequireWhole(n) => {
            let name = chunk.local_names.get(n as usize).cloned().unwrap_or_default();
            format!("{m:<14} {n}    -- ({name})")
        }
        Op::Jump(t) | Op::JumpIfFalse(t) => format!("{m:<14} {t}"),
        Op::LoadGlobal(n) | Op::StoreGlobal(n) => {
            let name = chunk.globals.get(n as usize).map(|g| g.name.clone()).unwrap_or_default();
            format!("{m:<14} {n}    -- ({name})")
        }
        Op::Add(t)
        | Op::Sub(t)
        | Op::Mul(t)
        | Op::Div(t)
        | Op::Rem(t)
        | Op::Neg(t)
        | Op::Eq(t)
        | Op::Ne(t)
        | Op::Lt(t)
        | Op::Le(t)
        | Op::Gt(t)
        | Op::Ge(t)
        | Op::Write(t) => format!("{m}.{}", t.name()),
        Op::Pop | Op::Halt => m.to_string(),
    }
}
