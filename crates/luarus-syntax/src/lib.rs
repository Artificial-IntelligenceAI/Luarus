//! Lexer, AST and parser for Luarus.
//!
//! Luarus is deliberately not Lua-shaped. Three rules drive the whole grammar:
//!
//! * an identifier is always written `(name)`, and the text between the parens
//!   is raw, so names may contain spaces, punctuation and emoji;
//! * a literal is always written `'text'`, and its *type* comes from context, so
//!   `'1000'` is a number under `f16` and text under `str`;
//! * statements chain with `,` and one `end` closes the chain.
//!
//! Because `(...)` is opaque and `[...]` is print's value list, grouping is
//! `|...|`.

pub mod ast;
pub mod lexer;
pub mod parser;

/// Spans, rules and diagnostic rendering live one layer down, so the VM can
/// name the same rules the compiler does.
pub use luarus_diag as diag;
pub use luarus_diag::{Diagnostic, Rule, Span};

pub use ast::{BinOp, Expr, Modifier, Name, Program, Stmt, UnOp};
pub use parser::parse;
