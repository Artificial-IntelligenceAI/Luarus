use crate::grapheme;
use crate::Diagnostic;

/// 1-based line and column of `offset` within `src`.
///
/// The column counts characters as a reader would: `🧑‍🧑‍🧒‍🧒` advances it by one,
/// not by seven.
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
    (line, grapheme::count(&src[line_start..offset]) + 1)
}

fn line_text(src: &str, offset: usize) -> (usize, &str) {
    let offset = offset.min(src.len());
    let start = src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = src[offset..].find('\n').map(|i| offset + i).unwrap_or(src.len());
    (start, &src[start..end])
}

/// Render a diagnostic: the rule broken, the line, and a caret under it.
pub fn render(src: &str, file: &str, d: &Diagnostic) -> String {
    let (line, col) = line_col(src, d.span.start);
    let (line_start, text) = line_text(src, d.span.start);

    let gutter = line.to_string();
    let pad = " ".repeat(gutter.len());

    // Caret geometry is laid out in terminal cells, not characters: a space is
    // one cell but an emoji is two, so padding by character count would leave
    // the caret short. The column above is still counted in characters.
    let lead = text
        .get(..d.span.start.saturating_sub(line_start))
        .map(grapheme::width)
        .unwrap_or(0);
    let width = src
        .get(d.span.start..d.span.end.min(line_start + text.len()))
        .map(grapheme::width)
        .unwrap_or(1)
        .max(1);

    let mut out = String::new();
    out.push_str(&format!("error[{}]: {}\n", d.rule.slug(), d.message));
    out.push_str(&format!("{pad}--> {file}:{line}:{col}\n"));
    out.push_str(&format!("{pad} |\n"));
    out.push_str(&format!("{gutter} | {text}\n"));
    out.push_str(&format!("{pad} | {}{}\n", " ".repeat(lead), "^".repeat(width)));
    out.push_str(&format!("{pad} = rule: {}\n", d.rule.statement()));
    if let Some(h) = &d.help {
        out.push_str(&format!("{pad} = help: {h}\n"));
    }
    out
}
