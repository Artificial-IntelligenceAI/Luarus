//! The Luarus type checker.
//!
//! Checking is bidirectional. A literal has no type of its own, so it is always
//! *checked against* a type that came from somewhere else — a declaration's
//! annotation, or the other operand of an arithmetic expression. When no such
//! type exists the program is rejected rather than guessed at, which is the
//! whole reason the language annotates in the first place.

use std::collections::HashMap;

use luarus_bytecode::{Const, RtType};
use luarus_syntax::ast::{BinOp, Expr, Modifier, Program, Stmt, UnOp};
use luarus_syntax::diag::line_col;
use luarus_syntax::{Diagnostic, Span};

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

#[derive(Clone, Debug)]
pub enum TStmt {
    Store { place: Place, value: TExpr, line: u32 },
    Print { items: Vec<TExpr>, newline: bool, line: u32 },
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
        scope: HashMap::new(),
        out: Checked::default(),
        errors: Vec::new(),
    };
    cx.run(program);
    if cx.errors.is_empty() {
        Ok(cx.out)
    } else {
        Err(cx.errors)
    }
}

struct Checker<'a> {
    src: &'a str,
    scope: HashMap<String, Binding>,
    out: Checked,
    errors: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn line(&self, span: Span) -> u32 {
        line_col(self.src, span.start).0 as u32
    }

    fn run(&mut self, program: &Program) {
        for stmt in &program.stmts {
            // One bad statement should not suppress the rest of the file.
            if let Err(d) = self.stmt(stmt) {
                self.errors.push(d);
            }
        }
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Var { modifier, ty, name, value, .. } => {
                let declared = RtType::from_name(&ty.text).ok_or_else(|| {
                    Diagnostic::new(ty.span, format!("unknown type `{}`", ty.text))
                        .with_help(TYPE_LIST)
                })?;

                // Check the initialiser before binding the name, so a
                // declaration cannot refer to the variable it is defining.
                let value = self.check(value, declared)?;

                if let Some(prev) = self.scope.get(&name.text) {
                    let (line, _) = line_col(self.src, prev.declared_at.start);
                    return Err(Diagnostic::new(
                        name.span,
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

                self.scope.insert(
                    name.text.clone(),
                    Binding { place, ty: declared, declared_at: name.span },
                );
                let line = self.line(stmt.span());
                self.out.stmts.push(TStmt::Store { place, value, line });
                Ok(())
            }

            Stmt::Assign { name, value, .. } => {
                let binding = self.lookup(&name.text, name.span)?;
                let value = self.check(value, binding.ty)?;
                let line = self.line(stmt.span());
                self.out.stmts.push(TStmt::Store { place: binding.place, value, line });
                Ok(())
            }

            Stmt::Print { items, newline, .. } => {
                let mut checked = Vec::with_capacity(items.len());
                for item in items {
                    // Everything printed is stringified, so a bare literal in a
                    // print list needs no annotation: it is simply text. This is
                    // the one place Luarus converts without being told to.
                    let ty = self.probe(item).unwrap_or(RtType::Str);
                    checked.push(self.check(item, ty)?);
                }
                let line = self.line(stmt.span());
                self.out.stmts.push(TStmt::Print {
                    items: checked,
                    newline: *newline,
                    line,
                });
                Ok(())
            }
        }
    }

    fn lookup(&self, name: &str, span: Span) -> Result<Binding, Diagnostic> {
        self.scope.get(name).cloned().ok_or_else(|| {
            let mut d = Diagnostic::new(span, format!("`({name})` is not declared"));
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
        self.scope
            .keys()
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
            Expr::Literal { .. } | Expr::Escape { .. } => None,
            Expr::Ident(n) => (!self.scope.contains_key(&n.text)).then_some(n),
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
            Expr::Ident(n) => self.scope.get(&n.text).map(|b| b.ty),
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
                    let mut d = Diagnostic::new(*span, le.message);
                    if let Some(h) = le.help {
                        d = d.with_help(h);
                    }
                    Err(d)
                }
            },

            Expr::Escape { text, span } => {
                if expected != RtType::Str {
                    return Err(Diagnostic::new(
                        *span,
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
                        format!(
                            "expected `{}`, but `({})` is `{}`",
                            expected.name(),
                            n.text,
                            b.ty.name()
                        ),
                    )
                    .with_help(
                        "Luarus does not convert between types on its own; every width is \
                         written out",
                    ));
                }
                Ok(TExpr::Load(b.place, b.ty))
            }

            Expr::Group { inner, .. } => self.check(inner, expected),

            Expr::Unary { op: UnOp::Neg, operand, span } => {
                if expected.is_unsigned_int() {
                    return Err(Diagnostic::new(
                        *span,
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

const TYPE_LIST: &str = "the types are i8 i16 i32 i64, u8 u16 u32 u64, f16 f32 f64, bool, str, nil";

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
