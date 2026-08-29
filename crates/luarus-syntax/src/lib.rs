//! Lexer, AST and parser for Luarus.
//!
//! Luarus is deliberately not Lua-shaped. Two rules drive the whole grammar:
//!
//! * an identifier is always written `(name)`, and the text between the parens is
//!   raw, so names may contain spaces, punctuation and emoji;
//! * a literal is always written `'text'`, and its *type* comes from context, so
//!   `'1000'` is a number under `f16` and text under `str`.
//!
//! Because `(...)` is opaque it cannot also mean grouping, so grouping is `[...]`.
//! Every statement is terminated by `end`.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod span;

pub use ast::{BinOp, Expr, Modifier, Name, Program, Stmt, UnOp};
pub use diag::Diagnostic;
pub use parser::parse;
pub use span::Span;
