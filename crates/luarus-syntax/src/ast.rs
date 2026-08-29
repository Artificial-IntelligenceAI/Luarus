use luarus_diag::Span;

/// A visibility/scope modifier written *before* `var`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    /// Lift the binding out of local scope into the module's globals.
    Global,
    /// Global, and also exported from the module.
    Pub,
}

impl Modifier {
    pub fn as_str(self) -> &'static str {
        match self {
            Modifier::Global => "global",
            Modifier::Pub => "pub",
        }
    }
}

/// An identifier occurrence. `text` is the raw name, which may contain
/// spaces, punctuation or emoji.
#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    /// Comparisons yield `bool` regardless of what they compare.
    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
}

#[derive(Clone, Debug)]
pub enum Expr {
    /// A quoted literal. Still untyped: `'1000'` is a number or text depending
    /// on the type it is checked against.
    Literal { text: String, span: Span },
    Ident(Name),
    /// A bare escape such as `\n`. Always `str`, whatever the context.
    Escape { text: String, span: Span },
    /// A literal that states its own type, as in `f16 '5'`. Usable anywhere,
    /// including where nothing else would supply one.
    TypedLiteral { ty: TypeRef, text: String, span: Span },
    Unary { op: UnOp, operand: Box<Expr>, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// `| ... |`. Grouping uses pipes: `(...)` means an identifier and `[...]`
    /// is print's argument list, so neither was available.
    Group { inner: Box<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. }
            | Expr::Escape { span, .. }
            | Expr::TypedLiteral { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Group { span, .. } => *span,
            Expr::Ident(n) => n.span,
        }
    }
}

/// A type as written in the source. Resolution to a real type happens later.
#[derive(Clone, Debug)]
pub struct TypeRef {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Var {
        modifier: Option<Modifier>,
        ty: TypeRef,
        name: Name,
        value: Expr,
        span: Span,
    },
    Assign {
        name: Name,
        value: Expr,
        span: Span,
    },
    /// `if cond { ... elseif cond ... else ... }`.
    ///
    /// One brace pair holds the whole chain; `elseif` and `else` divide it
    /// rather than opening blocks of their own, so a chain of any length closes
    /// with a single `}`.
    If {
        /// The `if` arm followed by every `elseif` arm, in order.
        arms: Vec<IfArm>,
        else_arm: Vec<Stmt>,
        span: Span,
    },
    /// `loop perm store-in i32 (i) = '0' to '10' end`, or
    /// `loop temp = '0' to '10' { ... }`.
    ///
    /// Counts from one bound to the other, inclusive at both ends. Both the
    /// target and the body are optional, and each does a different job: the
    /// target catches the values, the body runs once per value.
    Loop {
        /// `perm` keeps the target alive after the loop; `temp` confines it.
        perm: bool,
        /// `None` when there is no `store-in` clause, so nothing is caught.
        target: Option<(TypeRef, Name)>,
        range: LoopRange,
        /// Empty when the loop is written with `end` rather than braces.
        body: Vec<Stmt>,
        span: Span,
    },
    Print {
        /// Juxtaposed items, written back to back inside `[ ... ]`.
        /// `print` writes exactly these and nothing else — no separators and no
        /// newline. A line ending is written with `\n` like any other value.
        items: Vec<Expr>,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Var { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Print { span, .. }
            | Stmt::If { span, .. }
            | Stmt::Loop { span, .. } => *span,
        }
    }
}

/// How a loop says which values to count.
#[derive(Clone, Debug)]
pub enum LoopRange {
    /// `= from to to` — inclusive at both ends.
    Between { from: Expr, to: Expr },
    /// `= n times` — `n` values, counting from zero, so the last is `n - 1`.
    Times(Expr),
}

/// One condition and the statements it guards.
#[derive(Clone, Debug)]
pub struct IfArm {
    pub cond: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}
