//! Spans, rules, diagnostics, and the text measurement they depend on.
//!
//! This sits below both the compiler and the VM, because both report errors and
//! both name the rule that was broken.

pub mod grapheme;
pub mod rule;
mod render;
mod span;

pub use render::{line_col, render};
pub use rule::Rule;
pub use span::Span;

/// A compile error: a rule that was broken, and the source text that broke it.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub rule: Rule,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, rule: Rule, message: impl Into<String>) -> Self {
        Diagnostic { span, rule, message: message.into(), help: None }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}
