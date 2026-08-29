//! The Luarus type checker.
//!
//! Checking is bidirectional. A literal has no type of its own, so it is always
//! *checked against* a type that came from somewhere else — a declaration's
//! annotation, or the other operand of an arithmetic expression. When no such
//! type exists the program is rejected rather than guessed at, which is the
//! whole reason the language annotates in the first place.

use std::collections::HashMap;

use luarus_bytecode::{Const, RtType};
use luarus_diag::{line_col, Diagnostic, Rule, Span};
use luarus_syntax::ast::{BinOp, Expr, Modifier, Program, Stmt, UnOp};

/// Where a binding lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Local(u32),
    Global(u32),
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub place: Place,
    pub ty: RtType,
    pub declared_at: Span,
}

/// An expression with every type resolved and every literal already parsed.
#[derive(Clone, Debug)]
pub enum TExpr {
    Const(Const, RtType),
    Load(Place, RtType),
    Neg(Box<TExpr>, RtType),
    Bin {
        op: BinOp,
        /// The type both operands were checked at.
        operand_ty: RtType,
        result: RtType,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
}

impl TExpr {
    pub fn ty(&self) -> RtType {
        match self {
            TExpr::Const(_, t) | TExpr::Load(_, t) | TExpr::Neg(_, t) => *t,
            TExpr::Bin { result, .. } => *result,
        }
    }
}

/// One checked condition and the statements it guards.
#[derive(Clone, Debug)]
pub struct TIfArm {
    pub cond: TExpr,
    pub body: Vec<TStmt>,
}

#[derive(Clone, Debug)]
pub enum TStmt {
    Store { place: Place, value: TExpr, line: u32 },
    Print { items: Vec<TExpr>, line: u32 },
    If { arms: Vec<TIfArm>, else_arm: Vec<TStmt>, line: u32 },
    /// Count from `from` to `to` inclusive, storing each value into `place`.
    /// `counter` and `bound` are hidden slots the loop needs to do that.
    Loop {
        /// `None` when the loop catches nothing.
        place: Option<Place>,
        ty: RtType,
        from: TExpr,
        to: TExpr,
        /// `to` counts up to and including the bound; `times` stops before it.
        inclusive: bool,
        body: Vec<TStmt>,
        counter: u32,
        bound: u32,
        line: u32,
    },
}

/// A whole checked program, ready for code generation.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub stmts: Vec<TStmt>,
    pub locals: Vec<String>,
    pub globals: Vec<(String, RtType, bool)>,
}

pub fn check_program(src: &str, program: &Program) -> Result<Checked, Vec<Diagnostic>> {
    let mut cx = Checker {
        src,
        scopes: vec![HashMap::new()],
        out: Checked::default(),
        errors: Vec::new(),
    };
    cx.out.stmts = cx.block(&program.stmts);
    if cx.errors.is_empty() {
        Ok(cx.out)
    } else {
        Err(cx.errors)
    }
}

struct Checker<'a> {
    src: &'a str,
    /// Innermost scope last. A block pushes one and pops it on the way out, so
    /// a name declared inside an `if` is gone afterwards.
    scopes: Vec<HashMap<String, Binding>>,
    out: Checked,
    errors: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn line(&self, span: Span) -> u32 {
        line_col(self.src, span.start).0 as u32
    }

    /// Check a run of statements, keeping going past any that fail so one bad
    /// statement does not suppress the rest of the file.
    fn block(&mut self, stmts: &[Stmt]) -> Vec<TStmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match self.stmt(stmt) {
                Ok(t) => out.push(t),
                Err(d) => self.errors.push(d),
            }
        }
        out
    }

    /// Check a block in a scope of its own.
    fn scoped(&mut self, stmts: &[Stmt]) -> Vec<TStmt> {
        self.scopes.push(HashMap::new());
        let out = self.block(stmts);
        self.scopes.pop();
        out
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<TStmt, Diagnostic> {
        match stmt {
            Stmt::Var { modifier, ty, name, value, .. } => {
                let declared = RtType::from_name(&ty.text).ok_or_else(|| {
                    Diagnostic::new(ty.span, Rule::TypesMustExist, format!("unknown type `{}`", ty.text))
                })?;

                // Check the initialiser before binding the name, so a
                // declaration cannot refer to the variable it is defining.
                let value = self.check(value, declared)?;

                if let Some(prev) = self.find(&name.text) {
                    let (line, _) = line_col(self.src, prev.declared_at.start);
                    return Err(Diagnostic::new(
                        name.span,
                        Rule::NamesAreDeclaredOnce,
                        format!("`({})` is already declared", name.text),
                    )
                    .with_help(format!("the first declaration is on line {line}")));
                }

                let place = match modifier {
                    None => {
                        let slot = self.out.locals.len() as u32;
                        self.out.locals.push(name.text.clone());
                        Place::Local(slot)
                    }
                    Some(m) => {
                        let idx = self.out.globals.len() as u32;
                        self.out.globals.push((
                            name.text.clone(),
                            declared,
                            *m == Modifier::Pub,
                        ));
                        Place::Global(idx)
                    }
                };

                self.scopes
                    .last_mut()
                    .expect("a scope is always open")
                    .insert(name.text.clone(), Binding { place, ty: declared, declared_at: name.span });
                let line = self.line(stmt.span());
                Ok(TStmt::Store { place, value, line })
            }

            Stmt::Assign { name, value, .. } => {
                let binding = self.lookup(&name.text, name.span)?;
                let value = self.check(value, binding.ty)?;
                let line = self.line(stmt.span());
                Ok(TStmt::Store { place: binding.place, value, line })
            }

            Stmt::Print { items, .. } => {
                let mut checked = Vec::with_capacity(items.len());
                for item in items {
                    // Everything printed is stringified, so a bare literal in a
                    // print list needs no annotation: it is simply text. This is
                    // the one place Luarus converts without being told to.
                    let ty = self.probe(item).unwrap_or(RtType::Str);
                    checked.push(self.check(item, ty)?);
                }
                let line = self.line(stmt.span());
                Ok(TStmt::Print { items: checked, line })
            }

            Stmt::Loop { perm, target, range, body, .. } => {
                use luarus_syntax::ast::LoopRange;
                // With no `store-in` clause there is no annotation to read the
                // bounds as, so their own type is used. Failing that, a count is
                // never negative and gets `u64`, while a pair of bounds gets
                // `i64` so it can start below zero.
                let declared = match target {
                    Some((ty, _)) => RtType::from_name(&ty.text).ok_or_else(|| {
                        Diagnostic::new(
                            ty.span,
                            Rule::TypesMustExist,
                            format!("unknown type `{}`", ty.text),
                        )
                    })?,
                    None => match range {
                        LoopRange::Between { from, to } => {
                            self.probe(from).or_else(|| self.probe(to)).unwrap_or(RtType::I64)
                        }
                        LoopRange::Times(n) => self.probe(n).unwrap_or(RtType::U64),
                    },
                };

                let range_span = match range {
                    LoopRange::Between { from, .. } => from.span(),
                    LoopRange::Times(n) => n.span(),
                };
                if !declared.is_int() {
                    let span = target.as_ref().map(|(t, _)| t.span).unwrap_or(range_span);
                    return Err(Diagnostic::new(
                        span,
                        Rule::LoopsCountIntegers,
                        format!("a loop cannot count over `{}`", declared.name()),
                    )
                    .with_help("counting steps by one, which only the integer types do exactly"));
                }

                // Bounds are checked before the name exists, so a loop cannot
                // count from itself.
                let (from, to, inclusive) = match range {
                    LoopRange::Between { from, to } => {
                        (self.check(from, declared)?, self.check(to, declared)?, true)
                    }
                    LoopRange::Times(n) => {
                        let zero = if declared.is_unsigned_int() {
                            Const::Uint(0)
                        } else {
                            Const::Int(0)
                        };
                        (TExpr::Const(zero, declared), self.check(n, declared)?, false)
                    }
                };

                let place = match target {
                    None => None,
                    Some((_, name)) => {
                        if let Some(prev) = self.find(&name.text) {
                            let (line, _) = line_col(self.src, prev.declared_at.start);
                            return Err(Diagnostic::new(
                                name.span,
                                Rule::NamesAreDeclaredOnce,
                                format!("`({})` is already declared", name.text),
                            )
                            .with_help(format!("the first declaration is on line {line}")));
                        }
                        let slot = self.out.locals.len() as u32;
                        self.out.locals.push(name.text.clone());
                        Some(Place::Local(slot))
                    }
                };

                let counter = self.hidden_slot("loop counter");
                let bound = self.hidden_slot("loop bound");

                // `perm` binds the name outside the loop, so it survives; `temp`
                // binds it in the body, so it is visible while the loop runs and
                // gone afterwards. With no body, `temp` binds it nowhere.
                self.scopes.push(HashMap::new());
                if let (Some(place), Some((_, name))) = (place, target) {
                    let binding = Binding { place, ty: declared, declared_at: name.span };
                    if *perm {
                        let depth = self.scopes.len() - 2;
                        self.scopes[depth].insert(name.text.clone(), binding);
                    } else {
                        self.scopes.last_mut().expect("just pushed").insert(name.text.clone(), binding);
                    }
                }
                let body = self.block(body);
                self.scopes.pop();

                let line = self.line(stmt.span());
                Ok(TStmt::Loop {
                    place,
                    ty: declared,
                    from,
                    to,
                    inclusive,
                    body,
                    counter,
                    bound,
                    line,
                })
            }

            Stmt::If { arms, else_arm, .. } => {
                let mut checked = Vec::with_capacity(arms.len());
                for arm in arms {
                    // No truthiness: the condition is a bool or it is an error.
                    let cond_ty = self.probe(&arm.cond).unwrap_or(RtType::Bool);
                    if cond_ty != RtType::Bool {
                        self.errors.push(
                            Diagnostic::new(
                                arm.cond.span(),
                                Rule::ConditionsAreBool,
                                format!("this condition is `{}`", cond_ty.name()),
                            )
                            .with_help(format!(
                                "compare it instead, as in `(x) != {} '0'`",
                                cond_ty.name()
                            )),
                        );
                        // Keep checking the body, so one bad condition does not
                        // hide every error inside the arm it guards.
                        self.scoped(&arm.body);
                        continue;
                    }
                    match self.check(&arm.cond, RtType::Bool) {
                        Ok(cond) => {
                            let body = self.scoped(&arm.body);
                            checked.push(TIfArm { cond, body });
                        }
                        Err(d) => {
                            self.errors.push(d);
                            self.scoped(&arm.body);
                        }
                    }
                }
                let else_arm = self.scoped(else_arm);
                let line = self.line(stmt.span());
                Ok(TStmt::If { arms: checked, else_arm, line })
            }
        }
    }

    /// A slot with no name in any scope, for a loop's own bookkeeping.
    fn hidden_slot(&mut self, label: &str) -> u32 {
        let slot = self.out.locals.len() as u32;
        self.out.locals.push(format!("%{label}"));
        slot
    }

    /// Look a name up from the innermost scope outwards.
    fn find(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn lookup(&self, name: &str, span: Span) -> Result<Binding, Diagnostic> {
        self.find(name).cloned().ok_or_else(|| {
            let mut d =
                Diagnostic::new(span, Rule::NamesMustBeDeclared, format!("`({name})` is not declared"));
            if let Some(near) = self.closest(name) {
                d = d.with_help(format!("a variable named `({near})` is declared; did you mean that?"));
            } else {
                d = d.with_help("declare it first, as in `var i32 (x) = '0' end`");
            }
            d
        })
    }

    /// Find a declared name within a small edit distance, for "did you mean".
    fn closest(&self, name: &str) -> Option<String> {
        let budget = (name.chars().count() / 3).max(1);
        self.scopes
            .iter()
            .flat_map(|s| s.keys())
            .map(|k| (edit_distance(k, name), k))
            .filter(|(d, _)| *d <= budget)
            .min_by_key(|(d, _)| *d)
            .map(|(_, k)| k.clone())
    }

    /// The first identifier in `e` that is not declared, if any.
    ///
    /// `probe` returns `None` both for a bare literal and for an unknown name.
    /// Those deserve very different messages, so the error paths ask this first.
    fn first_unresolved<'e>(&self, e: &'e Expr) -> Option<&'e luarus_syntax::ast::Name> {
        match e {
            Expr::Literal { .. } | Expr::Escape { .. } | Expr::TypedLiteral { .. } => None,
            Expr::Ident(n) => self.find(&n.text).is_none().then_some(n),
            Expr::Group { inner, .. } => self.first_unresolved(inner),
            Expr::Unary { operand, .. } => self.first_unresolved(operand),
            Expr::Binary { lhs, rhs, .. } => {
                self.first_unresolved(lhs).or_else(|| self.first_unresolved(rhs))
            }
        }
    }

    /// Work out an expression's type without committing to it.
    ///
    /// Returns `None` exactly when the type would have to come from context —
    /// that is, when the expression bottoms out in bare literals.
    fn probe(&self, e: &Expr) -> Option<RtType> {
        match e {
            Expr::Literal { .. } => None,
            // A bare escape is text whatever the surroundings.
            Expr::Escape { .. } => Some(RtType::Str),
            // A typed literal supplies its own type; that is the whole point.
            Expr::TypedLiteral { ty, .. } => RtType::from_name(&ty.text),
            Expr::Ident(n) => self.find(&n.text).map(|b| b.ty),
            Expr::Group { inner, .. } => self.probe(inner),
            Expr::Unary { operand, .. } => self.probe(operand),
            Expr::Binary { op, lhs, rhs, .. } => {
                if op.is_comparison() {
                    Some(RtType::Bool)
                } else {
                    self.probe(lhs).or_else(|| self.probe(rhs))
                }
            }
        }
    }

    fn check(&mut self, e: &Expr, expected: RtType) -> Result<TExpr, Diagnostic> {
        match e {
            Expr::Literal { text, span } => match crate::literal::parse(text, expected) {
                Ok(c) => Ok(TExpr::Const(c, expected)),
                Err(le) => {
                    let mut d = Diagnostic::new(*span, le.rule, le.message);
                    if let Some(h) = le.help {
                        d = d.with_help(h);
                    }
                    Err(d)
                }
            },

            Expr::TypedLiteral { ty, text, span } => {
                let declared = RtType::from_name(&ty.text).ok_or_else(|| {
                    Diagnostic::new(ty.span, Rule::TypesMustExist, format!("unknown type `{}`", ty.text))
                })?;
                if declared != expected {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::NoImplicitConversion,
                        format!("expected `{}`, but this literal says `{}`", expected.name(), declared.name()),
                    ));
                }
                match crate::literal::parse(text, declared) {
                    Ok(c) => Ok(TExpr::Const(c, declared)),
                    Err(le) => {
                        let mut d = Diagnostic::new(*span, le.rule, le.message);
                        if let Some(h) = le.help {
                            d = d.with_help(h);
                        }
                        Err(d)
                    }
                }
            }

            Expr::Escape { text, span } => {
                if expected != RtType::Str {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::EscapesAreText,
                        format!("an escape is `str`, but `{}` was expected here", expected.name()),
                    ));
                }
                Ok(TExpr::Const(Const::Str(text.clone()), RtType::Str))
            }

            Expr::Ident(n) => {
                let b = self.lookup(&n.text, n.span)?;
                if b.ty != expected {
                    return Err(Diagnostic::new(
                        n.span,
                        Rule::NoImplicitConversion,
                        format!(
                            "expected `{}`, but `({})` is `{}`",
                            expected.name(),
                            n.text,
                            b.ty.name()
                        ),
                    )
                    .with_help(format!(
                        "the two types must match exactly; declare it as `{}`, or use a value \
                         that is already `{}`",
                        b.ty.name(),
                        expected.name()
                    )));
                }
                Ok(TExpr::Load(b.place, b.ty))
            }

            Expr::Group { inner, .. } => self.check(inner, expected),

            Expr::Unary { op: UnOp::Neg, operand, span } => {
                if expected.is_unsigned_int() {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::UnsignedIsNeverNegative,
                        format!("cannot negate a value of unsigned type `{}`", expected.name()),
                    )
                    .with_help(format!(
                        "use a signed type such as `i{}` if the value can be negative",
                        signed_width_hint(expected)
                    )));
                }
                if !expected.is_numeric() {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::ArithmeticIsNumeric,
                        format!("cannot negate a value of type `{}`", expected.name()),
                    ));
                }
                let inner = self.check(operand, expected)?;
                Ok(TExpr::Neg(Box::new(inner), expected))
            }

            Expr::Binary { op, lhs, rhs, span } if op.is_comparison() => {
                if expected != RtType::Bool {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::NoImplicitConversion,
                        format!(
                            "expected `{}`, but `{}` produces `bool`",
                            expected.name(),
                            op.as_str()
                        ),
                    ));
                }
                let operand_ty = match self.probe(lhs).or_else(|| self.probe(rhs)) {
                    Some(t) => t,
                    None => {
                        if let Some(n) =
                            self.first_unresolved(lhs).or_else(|| self.first_unresolved(rhs))
                        {
                            return Err(self.lookup(&n.text, n.span).unwrap_err());
                        }
                        return Err(Diagnostic::new(
                            *span,
                            Rule::LiteralsNeedAType,
                            "cannot tell what type this comparison is over",
                        )
                        .with_help(
                            "at least one side must be a declared name, so the literals on the \
                             other side have a type to be read as",
                        ));
                    }
                };

                if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
                    && !(operand_ty.is_numeric() || operand_ty == RtType::Str)
                {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::ArithmeticIsNumeric,
                        format!("`{}` cannot order values of type `{}`", op.as_str(), operand_ty.name()),
                    ));
                }

                let l = self.check(lhs, operand_ty)?;
                let r = self.check(rhs, operand_ty)?;
                Ok(TExpr::Bin {
                    op: *op,
                    operand_ty,
                    result: RtType::Bool,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                })
            }

            Expr::Binary { op, lhs, rhs, span } => {
                if !expected.is_numeric() {
                    return Err(Diagnostic::new(
                        *span,
                        Rule::ArithmeticIsNumeric,
                        format!(
                            "`{}` is not defined for type `{}`",
                            op.as_str(),
                            expected.name()
                        ),
                    )
                    .with_help("arithmetic works on the numeric types only"));
                }
                let l = self.check(lhs, expected)?;
                let r = self.check(rhs, expected)?;
                Ok(TExpr::Bin {
                    op: *op,
                    operand_ty: expected,
                    result: expected,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                })
            }
        }
    }
}

fn signed_width_hint(ty: RtType) -> u32 {
    match ty {
        RtType::U8 => 16,
        RtType::U16 => 32,
        _ => 64,
    }
}

/// Levenshtein distance, used only for "did you mean" hints.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
