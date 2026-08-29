use luarus_diag::{Diagnostic, Rule, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A bare word: a keyword, a modifier, or a type name. The parser decides.
    Word(String),
    /// An identifier, written `(name)`. Holds the unescaped name.
    Ident(String),
    /// A literal, written `'text'`. Holds the unescaped text, still untyped.
    Str(String),
    /// A bare escape such as `\n`, outside any quotes. Always `str`.
    Escape(String),

    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    /// `|`, which both opens and closes a group.
    Pipe,
    Comma,
    Eof,
}

impl Tok {
    /// How this token should be named in an error message.
    pub fn describe(&self) -> String {
        match self {
            Tok::Word(w) => format!("`{w}`"),
            Tok::Ident(n) => format!("identifier `({n})`"),
            Tok::Str(_) => "a literal".to_string(),
            Tok::Escape(_) => "an escape".to_string(),
            Tok::Assign => "`=`".into(),
            Tok::Plus => "`+`".into(),
            Tok::Minus => "`-`".into(),
            Tok::Star => "`*`".into(),
            Tok::Slash => "`/`".into(),
            Tok::Percent => "`%`".into(),
            Tok::EqEq => "`==`".into(),
            Tok::BangEq => "`!=`".into(),
            Tok::Lt => "`<`".into(),
            Tok::LtEq => "`<=`".into(),
            Tok::Gt => "`>`".into(),
            Tok::GtEq => "`>=`".into(),
            Tok::LBracket => "`[`".into(),
            Tok::RBracket => "`]`".into(),
            Tok::LBrace => "`{`".into(),
            Tok::RBrace => "`}`".into(),
            Tok::Pipe => "`|`".into(),
            Tok::Comma => "`,`".into(),
            Tok::Eof => "end of file".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, bytes: src.as_bytes(), pos: 0 }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token()?;
            let done = t.tok == Tok::Eof;
            out.push(t);
            if done {
                return Ok(out);
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.bytes.get(self.pos + 1).copied()
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_whitespace() => self.pos += 1,
                // `--` runs to end of line, a deliberate nod to Lua.
                Some(b'-') if self.peek2() == Some(b'-') => {
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => return,
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_trivia();
        let start = self.pos;
        let Some(b) = self.peek() else {
            return Ok(Token { tok: Tok::Eof, span: Span::new(start, start) });
        };

        // Two-character operators first, so `==` never lexes as two `=`.
        let two = |a: u8, t: Tok| -> Option<Tok> {
            if self.peek2() == Some(a) {
                Some(t)
            } else {
                None
            }
        };
        let simple = match b {
            b'=' => two(b'=', Tok::EqEq).map(|t| (t, 2)).or(Some((Tok::Assign, 1))),
            b'!' => two(b'=', Tok::BangEq).map(|t| (t, 2)),
            b'<' => two(b'=', Tok::LtEq).map(|t| (t, 2)).or(Some((Tok::Lt, 1))),
            b'>' => two(b'=', Tok::GtEq).map(|t| (t, 2)).or(Some((Tok::Gt, 1))),
            b'+' => Some((Tok::Plus, 1)),
            b'-' => Some((Tok::Minus, 1)),
            b'*' => Some((Tok::Star, 1)),
            b'/' => Some((Tok::Slash, 1)),
            b'%' => Some((Tok::Percent, 1)),
            b'[' => Some((Tok::LBracket, 1)),
            b']' => Some((Tok::RBracket, 1)),
            b'{' => Some((Tok::LBrace, 1)),
            b'}' => Some((Tok::RBrace, 1)),
            b'|' => Some((Tok::Pipe, 1)),
            b',' => Some((Tok::Comma, 1)),
            _ => None,
        };
        if let Some((tok, len)) = simple {
            self.pos += len;
            return Ok(Token { tok, span: Span::new(start, self.pos) });
        }
        if b == b'!' {
            return Err(Diagnostic::new(Span::new(start, start + 1), Rule::LexicalForm, "unexpected `!`")
                .with_help("`!` only appears as part of the `!=` operator"));
        }

        match b {
            b'(' => self.lex_ident(start),
            b'\'' | b'"' => self.lex_literal(start, b),
            b'\\' => self.lex_escape(start),
            _ => self.lex_word(start),
        }
    }

    /// `(name)` — raw text up to the matching `)`. `\)` and `\\` escape.
    fn lex_ident(&mut self, start: usize) -> Result<Token, Diagnostic> {
        self.pos += 1; // consume `(`
        let mut name = String::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(Diagnostic::new(
                    Span::new(start, self.pos),
                    Rule::NamesAreParenthesised,
                    "unterminated identifier",
                )
                .with_help("this `(` is never closed by a `)`"));
            };
            match b {
                b')' => {
                    self.pos += 1;
                    let span = Span::new(start, self.pos);
                    if name.is_empty() {
                        return Err(Diagnostic::new(span, Rule::NamesAreParenthesised, "empty identifier")
                            .with_help("a name must have at least one character, as in `(x)`"));
                    }
                    return Ok(Token { tok: Tok::Ident(name), span });
                }
                b'\\' => {
                    self.pos += 1;
                    match self.peek() {
                        Some(c @ (b')' | b'\\' | b'(')) => {
                            name.push(c as char);
                            self.pos += 1;
                        }
                        _ => {
                            return Err(Diagnostic::new(
                                Span::new(self.pos - 1, self.pos),
                                Rule::NamesAreParenthesised,
                                "invalid escape in identifier",
                            )
                            .with_help("inside `(...)` only `\\(`, `\\)` and `\\\\` are escapes"))
                        }
                    }
                }
                b'\n' => {
                    return Err(Diagnostic::new(
                        Span::new(start, self.pos),
                        Rule::NamesAreParenthesised,
                        "unterminated identifier",
                    )
                    .with_help("an identifier cannot span multiple lines"))
                }
                _ => {
                    // Copy one whole UTF-8 character so emoji survive intact.
                    let ch = self.src[self.pos..].chars().next().unwrap();
                    name.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    /// `'text'` — the text is kept raw; its type is decided later, by context.
    fn lex_literal(&mut self, start: usize, quote: u8) -> Result<Token, Diagnostic> {
        self.pos += 1; // consume the opening quote
        let mut text = String::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(Diagnostic::new(
                    Span::new(start, self.pos),
                    Rule::ValuesAreQuoted,
                    "unterminated literal",
                )
                .with_help("this quote is never closed"));
            };
            if b == quote {
                self.pos += 1;
                return Ok(Token { tok: Tok::Str(text), span: Span::new(start, self.pos) });
            }
            if b == b'\\' {
                self.pos += 1;
                let esc = self.peek().ok_or_else(|| {
                    Diagnostic::new(
                        Span::new(start, self.pos),
                        Rule::ValuesAreQuoted,
                        "unterminated literal",
                    )
                })?;
                let ch = match esc {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'0' => '\0',
                    b'\\' => '\\',
                    b'\'' => '\'',
                    b'"' => '"',
                    _ => {
                        return Err(Diagnostic::new(
                            Span::new(self.pos - 1, self.pos + 1),
                            Rule::ValuesAreQuoted,
                            format!("invalid escape `\\{}`", esc as char),
                        )
                        .with_help("valid escapes are \\n \\t \\r \\0 \\\\ \\' \\\""))
                    }
                };
                text.push(ch);
                self.pos += 1;
                continue;
            }
            let ch = self.src[self.pos..].chars().next().unwrap();
            text.push(ch);
            self.pos += ch.len_utf8();
        }
    }

    /// A bare escape outside quotes, as in `print["1" \n]`.
    fn lex_escape(&mut self, start: usize) -> Result<Token, Diagnostic> {
        self.pos += 1; // consume the backslash
        let Some(c) = self.peek() else {
            return Err(Diagnostic::new(
                Span::new(start, self.pos),
                Rule::EscapesAreText,
                "escape is missing its letter",
            )
                .with_help("a bare escape is written `\\n`, `\\t`, `\\r`, `\\0` or `\\\\`"));
        };
        let text = match c {
            b'n' => "\n",
            b't' => "\t",
            b'r' => "\r",
            b'0' => "\0",
            b'\\' => "\\",
            _ => {
                self.pos += 1;
                return Err(Diagnostic::new(
                    Span::new(start, self.pos),
                    Rule::EscapesAreText,
                    format!("invalid escape `\\{}`", c as char),
                )
                .with_help("a bare escape is written `\\n`, `\\t`, `\\r`, `\\0` or `\\\\`"));
            }
        };
        self.pos += 1;
        Ok(Token { tok: Tok::Escape(text.to_string()), span: Span::new(start, self.pos) })
    }

    /// A bare word: `var`, `end`, `global`, a type name, and so on.
    fn lex_word(&mut self, start: usize) -> Result<Token, Diagnostic> {
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
                continue;
            }
            // A hyphen joins two words, so `store-in` is one keyword. It only
            // does so between letters, which keeps `'5'-'3'` and `i32 -'1'`
            // reading as subtraction and negation.
            if b == b'-' && self.peek2().is_some_and(|c| c.is_ascii_alphabetic()) {
                self.pos += 1;
                continue;
            }
            break;
        }
        if self.pos == start {
            let ch = self.src[start..].chars().next().unwrap();
            self.pos = start + ch.len_utf8();
            return Err(Diagnostic::new(
                Span::new(start, self.pos),
                Rule::LexicalForm,
                format!("unexpected character `{ch}`"),
            )
            .with_help("names go in parentheses and values go in quotes, as in `var i32 (x) = '1' end`"));
        }
        Ok(Token {
            tok: Tok::Word(self.src[start..self.pos].to_string()),
            span: Span::new(start, self.pos),
        })
    }
}
