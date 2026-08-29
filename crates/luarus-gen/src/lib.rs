//! A generator of valid Luarus programs, for differential testing.
//!
//! The programs must **type-check**, or the exercise is pointless: an invalid
//! program is rejected identically by both the compiler and the reference
//! interpreter, which says nothing about either. So generation is type-directed
//! — an expression is built to order for a type that is wanted, rather than
//! built freely and checked afterwards.
//!
//! That means the generator has to obey the language's rules by construction:
//!
//! * both operands of an arithmetic expression share a type, since nothing
//!   converts implicitly;
//! * a condition is a `bool`, since there is no truthiness;
//! * an unsigned value is never negated;
//! * comparisons do not chain;
//! * a name is declared once, and only names still in scope are referenced;
//! * a literal appears bare only where a declared type gives it one, and says
//!   its own type everywhere else.
//!
//! A generated program may still *fail at run time* — overflow, division by
//! zero — and that is welcome: the two paths must then agree on the failure.

pub mod rng;

use luarus_bytecode::RtType;
use rng::Rng;

/// The types worth generating over, weighted by how much they exercise.
const TYPES: &[RtType] = &[
    RtType::I32,
    RtType::I32,
    RtType::I64,
    RtType::I64,
    RtType::F64,
    RtType::F64,
    RtType::Str,
    RtType::Bool,
    RtType::U8,
    RtType::U32,
    RtType::U64,
    RtType::I8,
    RtType::I16,
    RtType::F32,
    RtType::F16,
];

/// Names that stress the lexer rather than the type checker.
const EXOTIC: &[&str] = &["v with spaces", "🎯 score", "🧑‍🧑‍🧒‍🧒", "Δt", "a + b end", "end"];

#[derive(Clone, Debug)]
struct Var {
    name: String,
    ty: RtType,
}

pub struct Config {
    pub max_stmts: usize,
    pub max_depth: usize,
    pub max_block_stmts: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config { max_stmts: 12, max_depth: 3, max_block_stmts: 3 }
    }
}

/// Generate one program from a seed. The same seed always gives the same text.
pub fn program(seed: u64) -> String {
    program_with(seed, &Config::default())
}

pub fn program_with(seed: u64, config: &Config) -> String {
    let mut g = Gen {
        rng: Rng::new(seed),
        scopes: vec![Vec::new()],
        next_id: 0,
        config,
        out: String::new(),
    };
    g.out.push_str(&format!("-- generated from seed {seed}\n"));
    let n = 1 + g.rng.below(config.max_stmts);
    for _ in 0..n {
        let stmt = g.statement(0);
        g.out.push_str(&stmt);
    }
    // Something at the end that always runs, so a program is never empty output.
    g.out.push_str("print[\"end\" \\n] end\n");
    g.out
}

struct Gen<'a> {
    rng: Rng,
    scopes: Vec<Vec<Var>>,
    next_id: usize,
    config: &'a Config,
    out: String,
}

impl Gen<'_> {
    // ---------------------------------------------------------------- names

    fn fresh_name(&mut self) -> String {
        let id = self.next_id;
        self.next_id += 1;
        // Mostly plain, occasionally something that only Luarus would accept.
        if self.rng.chance(1, 8) {
            format!("{} {id}", self.rng.pick(EXOTIC))
        } else {
            format!("v{id}")
        }
    }

    fn visible(&self) -> impl Iterator<Item = &Var> {
        self.scopes.iter().flatten()
    }

    fn any_of_type(&mut self, ty: RtType) -> Option<String> {
        let names: Vec<String> =
            self.visible().filter(|v| v.ty == ty).map(|v| v.name.clone()).collect();
        if names.is_empty() {
            return None;
        }
        Some(self.rng.pick(&names).clone())
    }

    /// A type that some visible variable already has, for comparisons.
    fn a_live_type(&mut self) -> Option<RtType> {
        let tys: Vec<RtType> = self.visible().map(|v| v.ty).collect();
        if tys.is_empty() {
            return None;
        }
        Some(*self.rng.pick(&tys))
    }

    // ------------------------------------------------------------- literals

    /// A literal body for `ty`, kept small so that programs usually survive
    /// long enough to exercise more than their first statement.
    fn literal(&mut self, ty: RtType) -> String {
        match ty {
            RtType::Bool => if self.rng.chance(1, 2) { "true" } else { "false" }.to_string(),
            RtType::Nil => "nil".to_string(),
            RtType::Str => {
                let words = ["a", "hi", "luarus", "", "x y", "🎯", "tab\\there"];
                self.rng.pick(&words).to_string()
            }
            t if t.is_float() => {
                let v = ["0", "1", "1.5", "0.25", "2", "10", "0.5", "3"];
                let body = self.rng.pick(&v).to_string();
                if self.rng.chance(1, 6) { format!("-{body}") } else { body }
            }
            t if t.is_unsigned_int() => self.rng.between(0, 9).to_string(),
            t => {
                let _ = t;
                // Underscores and radix prefixes now and then, to keep the
                // literal parser in the loop.
                match self.rng.below(12) {
                    0 => "0xf".to_string(),
                    1 => "0b101".to_string(),
                    2 => "1_0".to_string(),
                    _ => self.rng.between(-9, 9).to_string(),
                }
            }
        }
    }

    // ---------------------------------------------------------- expressions

    /// A leaf: a variable if one is in scope, else a literal.
    ///
    /// `bare_ok` says whether the surrounding context supplies a type. Where it
    /// does not, the literal has to say its own.
    fn leaf(&mut self, ty: RtType, bare_ok: bool) -> String {
        if self.rng.chance(3, 5) {
            if let Some(name) = self.any_of_type(ty) {
                return format!("({name})");
            }
        }
        let body = self.literal(ty);
        if bare_ok && self.rng.chance(1, 2) {
            format!("'{body}'")
        } else {
            format!("{} '{}'", ty.name(), body)
        }
    }

    fn expr(&mut self, ty: RtType, depth: usize, bare_ok: bool) -> String {
        if depth >= self.config.max_depth || self.rng.chance(2, 5) {
            return self.leaf(ty, bare_ok);
        }
        match ty {
            RtType::Bool => self.bool_expr(depth, bare_ok),
            t if t.is_numeric() => self.numeric_expr(t, depth, bare_ok),
            // `str` and `nil` have no operators, so grouping is all that is left.
            t => {
                let inner = self.expr(t, depth + 1, bare_ok);
                format!("| {inner} |")
            }
        }
    }

    fn numeric_expr(&mut self, ty: RtType, depth: usize, bare_ok: bool) -> String {
        match self.rng.below(12) {
            // Division and remainder are rarer: they end a program more often
            // than they extend it.
            0 => {
                let a = self.expr(ty, depth + 1, bare_ok);
                let b = self.expr(ty, depth + 1, bare_ok);
                let op = if self.rng.chance(1, 2) { "/" } else { "%" };
                format!("{a} {op} {b}")
            }
            1..=6 => {
                let a = self.expr(ty, depth + 1, bare_ok);
                let b = self.expr(ty, depth + 1, bare_ok);
                let op = *self.rng.pick(&["+", "-", "*"]);
                format!("{a} {op} {b}")
            }
            7..=8 if ty.is_signed_int() || ty.is_float() => {
                // Negation is only legal on signed and floating types.
                let inner = self.expr(ty, depth + 1, bare_ok);
                format!("-| {inner} |")
            }
            9..=10 => {
                let inner = self.expr(ty, depth + 1, bare_ok);
                format!("| {inner} |")
            }
            _ => self.leaf(ty, bare_ok),
        }
    }

    fn bool_expr(&mut self, depth: usize, bare_ok: bool) -> String {
        if self.rng.chance(1, 4) {
            return self.leaf(RtType::Bool, bare_ok);
        }
        // A comparison needs both sides at one type, and its own type is bool,
        // so the operands cannot take a type from context: they must carry one.
        let ty = match self.a_live_type() {
            Some(t) if self.rng.chance(2, 3) => t,
            _ => *self.rng.pick(TYPES),
        };
        let ordered = ty.is_numeric() || ty == RtType::Str;
        let ops: &[&str] =
            if ordered { &["==", "!=", "<", "<=", ">", ">="] } else { &["==", "!="] };
        let op = *self.rng.pick(ops);

        // Equal operands now and then, so `<` and `<=` are told apart.
        if self.rng.chance(1, 5) {
            if let Some(name) = self.any_of_type(ty) {
                return format!("({name}) {op} ({name})");
            }
        }
        let a = self.operand(ty, depth + 1);
        let b = self.operand(ty, depth + 1);
        format!("{a} {op} {b}")
    }

    /// One side of a comparison.
    ///
    /// Comparing two bools is the awkward case: a comparison is itself a bool,
    /// so an operand generated freely could be another comparison and the whole
    /// thing would be a chain, which Luarus rejects. Grouping breaks the chain.
    fn operand(&mut self, ty: RtType, depth: usize) -> String {
        if ty != RtType::Bool {
            return self.expr(ty, depth, false);
        }
        if self.rng.chance(1, 3) {
            let inner = self.bool_expr(depth + 1, false);
            format!("| {inner} |")
        } else {
            self.leaf(RtType::Bool, false)
        }
    }

    // ----------------------------------------------------------- statements

    fn declare(&mut self, ty: RtType) -> String {
        let name = self.fresh_name();
        self.scopes.last_mut().expect("a scope is open").push(Var { name: name.clone(), ty });
        name
    }

    /// A simple statement, without its `end`, so it can be chained.
    fn simple(&mut self) -> String {
        match self.rng.below(10) {
            0..=4 => {
                let ty = *self.rng.pick(TYPES);
                // The declared type gives bare literals somewhere to get one.
                let value = self.expr(ty, 1, true);
                let modifier = match self.rng.below(12) {
                    0 => "global ",
                    1 => "pub ",
                    _ => "",
                };
                let name = self.declare(ty);
                format!("{modifier}var {} ({name}) = {value}", ty.name())
            }
            5..=6 => {
                // Assignment, if there is anything to assign to.
                let live: Vec<Var> = self.visible().cloned().collect();
                if live.is_empty() {
                    return self.print_stmt();
                }
                let v = self.rng.pick(&live).clone();
                let value = self.expr(v.ty, 1, true);
                format!("set ({}) = {value}", v.name)
            }
            _ => self.print_stmt(),
        }
    }

    /// A counting loop. Bounds are kept to small literals so a generated
    /// program cannot spend the afternoon counting, and nesting is bounded by
    /// the statement depth.
    fn loop_stmt(&mut self, depth: usize) -> String {
        let ints = [
            RtType::I32,
            RtType::I64,
            RtType::I8,
            RtType::I16,
            RtType::U8,
            RtType::U32,
            RtType::U64,
        ];
        let ty = *self.rng.pick(&ints);
        let (lo, hi) = if ty.is_unsigned_int() { (0, 9) } else { (-5, 9) };
        let from = self.rng.between(lo, hi);
        // Sometimes count backwards, which is an empty range and leaves the
        // target unassigned — worth generating, since reading it must then fail
        // the same way on both paths. The upper bound stays inside the type:
        // an unsigned loop cannot count down past zero.
        let to = if self.rng.chance(1, 8) && from > lo {
            self.rng.between(lo, from - 1)
        } else {
            self.rng.between(from, hi)
        };

        let pad = "  ".repeat(depth);
        let perm = self.rng.chance(1, 2);
        let keyword = match (perm, self.rng.chance(1, 2)) {
            (true, _) => "loop perm",
            (false, true) => "loop temp",
            (false, false) => "loop",
        };
        let with_body = self.rng.chance(1, 2);

        // Without `store-in` there is nothing to catch the values, and the
        // bounds fall back to their own type.
        let target = if self.rng.chance(4, 5) {
            // A `perm` target outlives the loop, so it joins the enclosing
            // scope now; a `temp` one is bound inside the body below, if there
            // is one, and nowhere at all if there is not.
            let name = if perm { self.declare(ty) } else { self.fresh_name() };
            let clause = format!(" store-in {} ({name})", ty.name());
            Some((name, clause))
        } else {
            None
        };
        let clause = target.as_ref().map(|(_, c)| c.clone()).unwrap_or_default();
        let header = format!("{pad}{keyword}{clause} = '{from}' to '{to}'");

        if !with_body {
            return format!("{header} end\n");
        }

        // A `temp` target is visible inside the body and nowhere else.
        self.scopes.push(Vec::new());
        if !perm {
            if let Some((name, _)) = &target {
                self.scopes.last_mut().expect("just pushed").push(Var { name: name.clone(), ty });
            }
        }
        let n = 1 + self.rng.below(self.config.max_block_stmts);
        let mut body = String::new();
        for _ in 0..n {
            let s = self.statement(depth + 1);
            body.push_str(&s);
        }
        self.scopes.pop();
        format!("{header} {{\n{body}{pad}}}\n")
    }

    fn print_stmt(&mut self) -> String {
        let n = 1 + self.rng.below(3);
        let mut items = Vec::with_capacity(n + 1);
        for _ in 0..n {
            let ty = *self.rng.pick(TYPES);
            // A print list supplies no type, so nothing here may be bare.
            let item = self.expr(ty, 2, false);
            // Juxtaposition binds looser than every operator, so an item that
            // begins with `-` would be read as subtracting from the item before
            // it. Grouping keeps it a separate value.
            items.push(if item.starts_with('-') { format!("| {item} |") } else { item });
        }
        if self.rng.chance(3, 4) {
            items.push("\\n".to_string());
        }
        format!("print[{}]", items.join(" "))
    }

    fn statement(&mut self, depth: usize) -> String {
        let pad = "  ".repeat(depth);
        if depth < 2 && self.rng.chance(1, 5) {
            return self.if_stmt(depth);
        }
        if depth < 2 && self.rng.chance(1, 6) {
            return self.loop_stmt(depth);
        }
        // Chain two or three simple statements under one `end` now and then.
        let n = if self.rng.chance(1, 5) { 1 + self.rng.below(2) } else { 0 };
        let mut parts = vec![self.simple()];
        for _ in 0..n {
            parts.push(self.simple());
        }
        format!("{pad}{} end\n", parts.join(", "))
    }

    fn arm_body(&mut self, depth: usize) -> String {
        self.scopes.push(Vec::new());
        let n = 1 + self.rng.below(self.config.max_block_stmts);
        let mut body = String::new();
        for _ in 0..n {
            let s = self.statement(depth + 1);
            body.push_str(&s);
        }
        self.scopes.pop();
        body
    }

    fn if_stmt(&mut self, depth: usize) -> String {
        let pad = "  ".repeat(depth);
        let cond = self.bool_expr(1, false);
        let mut out = format!("{pad}if {cond} {{\n");
        out.push_str(&self.arm_body(depth));

        for _ in 0..self.rng.below(3) {
            let cond = self.bool_expr(1, false);
            out.push_str(&format!("{pad}elseif {cond}\n"));
            out.push_str(&self.arm_body(depth));
        }
        if self.rng.chance(1, 2) {
            out.push_str(&format!("{pad}else\n"));
            out.push_str(&self.arm_body(depth));
        }
        out.push_str(&format!("{pad}}}\n"));
        out
    }
}

/// Reduce a failing program by deleting lines, keeping any smaller version that
/// still fails.
///
/// Line deletion breaks braces as often as not; `still_fails` is expected to
/// reject anything that no longer compiles, which filters those out.
pub fn shrink(src: &str, mut still_fails: impl FnMut(&str) -> bool) -> String {
    let mut best: Vec<&str> = src.lines().collect();
    let mut improved = true;
    while improved {
        improved = false;
        let mut i = 0;
        while i < best.len() {
            let mut candidate = best.clone();
            candidate.remove(i);
            let text = candidate.join("\n");
            if !text.trim().is_empty() && still_fails(&text) {
                best = candidate;
                improved = true;
            } else {
                i += 1;
            }
        }
    }
    best.join("\n")
}
