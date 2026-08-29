use crate::span::Span;

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
    Print {
        /// Juxtaposed items, written back to back inside `[ ... ]`.
        items: Vec<Expr>,
        /// Whether to end with a newline. True exactly when this `print` is the
        /// only statement in its chain, per the chaining rule in the spec.
        newline: bool,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Var { span, .. } | Stmt::Assign { span, .. } | Stmt::Print { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}
