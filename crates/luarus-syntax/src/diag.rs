use crate::span::Span;

/// A compile error, pointing at the source text that caused it.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Diagnostic { span, message: message.into(), help: None }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// 1-based line and column of `offset` within `src`.
pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let mut line = 1usize;
    let mut line_start = 0usize;
    for (i, b) in src.as_bytes().iter().enumerate().take(offset) {
        if *b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let col = src[line_start..offset].chars().count() + 1;
    (line, col)
}

fn line_text(src: &str, offset: usize) -> (usize, &str) {
    let offset = offset.min(src.len());
    let start = src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = src[offset..].find('\n').map(|i| offset + i).unwrap_or(src.len());
    (start, &src[start..end])
}

/// Render a diagnostic as a multi-line, caret-underlined message.
pub fn render(src: &str, file: &str, d: &Diagnostic) -> String {
    let (line, col) = line_col(src, d.span.start);
    let (line_start, text) = line_text(src, d.span.start);

    let gutter = line.to_string();
    let pad = " ".repeat(gutter.len());

    // Column offsets are counted in characters so that emoji in identifiers do
    // not push the caret out of alignment.
    let lead: usize = text
        .get(..d.span.start.saturating_sub(line_start))
        .map(|s| s.chars().count())
        .unwrap_or(0);
    let width = src
        .get(d.span.start..d.span.end.min(line_start + text.len()))
        .map(|s| s.chars().count())
        .unwrap_or(1)
        .max(1);

    let mut out = String::new();
    out.push_str(&format!("error: {}\n", d.message));
    out.push_str(&format!("{pad}--> {file}:{line}:{col}\n"));
    out.push_str(&format!("{pad} |\n"));
    out.push_str(&format!("{gutter} | {text}\n"));
    out.push_str(&format!("{pad} | {}{}\n", " ".repeat(lead), "^".repeat(width)));
    if let Some(h) = &d.help {
        out.push_str(&format!("{pad} = help: {h}\n"));
    }
    out
}
