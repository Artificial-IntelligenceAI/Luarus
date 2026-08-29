use crate::ast::*;
use crate::lexer::{Lexer, Tok, Token};
use luarus_diag::{Diagnostic, Rule, Span};

/// Parse a whole source file.
///
/// Statements are grouped into chains: `stmt (',' stmt)* 'end'`. A lone
/// statement is a one-element chain and still needs its `end`.
///
/// Errors are collected rather than thrown one at a time: the parser
/// resynchronises at the next `end` so one bad chain does not hide the rest.
pub fn parse(src: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = Lexer::new(src).tokenize().map_err(|d| vec![d])?;
    let mut p = Parser { tokens, pos: 0, errors: Vec::new() };
    let program = p.program();
    if p.errors.is_empty() {
        Ok(program)
    } else {
        Err(p.errors)
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<Diagnostic>,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn at_eof(&self) -> bool {
        self.peek().tok == Tok::Eof
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_word(&self, w: &str) -> bool {
        matches!(&self.peek().tok, Tok::Word(x) if x == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.at_word(w) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn err(&self, span: Span, rule: Rule, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::new(span, rule, msg)
    }

    /// Skip forward to the end of the current construct, so the next one can
    /// still be parsed. A `}` is left in place, since it closes an enclosing
    /// block rather than this statement.
    fn recover(&mut self) {
        while !self.at_eof() {
            if self.peek().tok == Tok::RBrace {
                return;
            }
            if self.eat_word("end") {
                return;
            }
            self.bump();
        }
    }

    fn program(&mut self) -> Program {
        let mut stmts = Vec::new();
        while !self.at_eof() {
            let before = self.pos;
            match self.item() {
                Ok(mut c) => stmts.append(&mut c),
                Err(d) => {
                    self.errors.push(d);
                    self.recover();
                }
            }
            // Recovery stops *at* a `}` rather than consuming it, since normally
            // that brace closes an enclosing block. At the top level there is no
            // enclosing block, so without this the parser would never advance.
            if self.pos == before {
                self.bump();
            }
        }
        Program { stmts }
    }

    /// One thing at statement level: either a braced construct, which closes
    /// itself, or a chain, which closes with `end`.
    fn item(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        if self.at_word("if") {
            let start = self.bump().span;
            return Ok(vec![self.if_stmt(start)?]);
        }
        self.chain()
    }

    /// `if cond { ... elseif cond ... else ... }`.
    ///
    /// The whole chain lives in one brace pair: `elseif` and `else` divide it
    /// rather than opening blocks of their own, so nothing piles up at the end.
    fn if_stmt(&mut self, start: Span) -> Result<Stmt, Diagnostic> {
        let cond = self.expr()?;

        if self.peek().tok != Tok::LBrace {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::BlocksAreBraced, format!("expected `{{` after the condition, found {}", t.tok.describe()))
                .with_help("an `if` body is braced: `if (x) > '5' { ... }`"));
        }
        let open = self.bump().span;

        let mut arms = vec![IfArm { cond, body: self.arm(open)? }];
        while self.at_word("elseif") {
            self.bump();
            let cond = self.expr()?;
            arms.push(IfArm { cond, body: self.arm(open)? });
        }
        let else_arm = if self.eat_word("else") { self.arm(open)? } else { Vec::new() };

        // `else` ends the chain, so anything dividing the block after it is a
        // mistake worth naming rather than a parse failure.
        if self.at_word("elseif") {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::BlocksAreBraced, "`elseif` cannot come after `else`")
                .with_help("`else` is the last arm; move it below the `elseif` arms"));
        }
        if self.at_word("else") {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::BlocksAreBraced, "a block may have only one `else`"));
        }

        if self.peek().tok != Tok::RBrace {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::BlocksAreBraced, format!("expected `}}` to close this `if`, found {}", t.tok.describe())));
        }
        let end = self.bump().span;
        Ok(Stmt::If { arms, else_arm, span: start.to(end) })
    }

    /// Statements up to the next divider or the closing brace.
    fn arm(&mut self, open: Span) -> Result<Vec<Stmt>, Diagnostic> {
        let mut stmts = Vec::new();
        loop {
            if self.peek().tok == Tok::RBrace || self.at_word("else") || self.at_word("elseif") {
                return Ok(stmts);
            }
            if self.at_eof() {
                return Err(self
                    .err(open, Rule::BlocksAreBraced, "unterminated block")
                    .with_help("this `{` is never closed by a `}`"));
            }
            stmts.append(&mut self.item()?);
        }
    }

    /// `stmt (',' stmt)* 'end'`.
    fn chain(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        let mut stmts = vec![self.stmt()?];
        while self.peek().tok == Tok::Comma {
            self.bump();
            stmts.push(self.stmt()?);
        }
        self.expect_end()?;
        Ok(stmts)
    }

    fn stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.peek().span;

        // Modifiers come before `var`: `global var i32 (n) = '0' end`.
        let modifier = if self.at_word("global") {
            self.bump();
            Some(Modifier::Global)
        } else if self.at_word("pub") {
            self.bump();
            Some(Modifier::Pub)
        } else {
            None
        };

        if let Some(m) = modifier {
            if !self.at_word("var") {
                let t = self.peek().clone();
                return Err(self
                    .err(t.span, Rule::StatementForm, format!("expected `var` after `{}`, found {}", m.as_str(), t.tok.describe()))
                    .with_help("modifiers attach to a declaration, as in `global var i32 (n) = '0' end`"));
            }
        }

        if self.eat_word("var") {
            return self.var_stmt(modifier, start);
        }
        if self.eat_word("set") {
            return self.assign_stmt(start);
        }
        if self.eat_word("print") {
            return self.print_stmt(start);
        }

        let t = self.peek().clone();
        if t.tok == Tok::RBrace {
            return Err(self
                .err(t.span, Rule::BlocksAreBraced, "unmatched `}`")
                .with_help("this brace closes a block that was never opened"));
        }
        Err(self
            .err(t.span, Rule::StatementForm, format!("expected a statement, found {}", t.tok.describe()))
            .with_help("a statement starts with `var`, `set`, `print`, `global` or `pub`"))
    }

    fn var_stmt(&mut self, modifier: Option<Modifier>, start: Span) -> Result<Stmt, Diagnostic> {
        let ty = match self.peek().tok.clone() {
            Tok::Word(text) => {
                let span = self.bump().span;
                TypeRef { text, span }
            }
            other => {
                let span = self.peek().span;
                return Err(self
                    .err(span, Rule::TypesMustExist, format!("expected a type after `var`, found {}", other.describe()))
                    .with_help("the type comes before the name: `var i32 (x) = '1' end`"));
            }
        };
        let name = self.ident("after the type in a `var` declaration")?;
        self.expect_assign()?;
        let value = self.expr()?;
        let span = start.to(value.span());
        Ok(Stmt::Var { modifier, ty, name, value, span })
    }

    /// `print [ item item ... ]`. Items are juxtaposed, not separated.
    fn print_stmt(&mut self, start: Span) -> Result<Stmt, Diagnostic> {
        if self.peek().tok != Tok::LBracket {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::PrintTakesBrackets, format!("expected `[` after `print`, found {}", t.tok.describe()))
                .with_help("print takes its values in brackets: `print[\"hello \" (name)] end`"));
        }
        let open = self.bump().span;

        let mut items = Vec::new();
        while self.peek().tok != Tok::RBracket {
            if self.at_eof() {
                return Err(self
                    .err(open, Rule::PrintTakesBrackets, "unterminated `print`")
                    .with_help("this `[` is never closed by a `]`"));
            }
            items.push(self.expr()?);
        }
        let end = self.bump().span;
        Ok(Stmt::Print { items, span: start.to(end) })
    }

    fn assign_stmt(&mut self, start: Span) -> Result<Stmt, Diagnostic> {
        let name = self.ident("after `set`")?;
        self.expect_assign()?;
        let value = self.expr()?;
        let span = start.to(value.span());
        Ok(Stmt::Assign { name, value, span })
    }

    fn ident(&mut self, ctx: &str) -> Result<Name, Diagnostic> {
        match self.peek().tok.clone() {
            Tok::Ident(text) => {
                let span = self.bump().span;
                Ok(Name { text, span })
            }
            other => {
                let span = self.peek().span;
                Err(self
                    .err(span, Rule::NamesAreParenthesised, format!("expected a name {ctx}, found {}", other.describe()))
                    .with_help("names are parenthesised so they can hold spaces and emoji, as in `(item count)`"))
            }
        }
    }

    fn expect_assign(&mut self) -> Result<Span, Diagnostic> {
        if self.peek().tok == Tok::Assign {
            Ok(self.bump().span)
        } else {
            let t = self.peek().clone();
            Err(self.err(t.span, Rule::StatementForm, format!("expected `=`, found {}", t.tok.describe())))
        }
    }

    fn expect_end(&mut self) -> Result<Span, Diagnostic> {
        if self.at_word("end") {
            Ok(self.bump().span)
        } else {
            let t = self.peek().clone();
            Err(self
                .err(t.span, Rule::EndClosesAChain, format!("expected `end`, found {}", t.tok.describe()))
                .with_help(
                    "`end` closes a statement chain; chain statements with `,` or close this one \
                     with `end`",
                ))
        }
    }

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        self.comparison()
    }

    /// Comparisons do not chain: `a < b < c` is rejected rather than misread.
    fn comparison(&mut self) -> Result<Expr, Diagnostic> {
        let lhs = self.additive()?;
        let op = match self.peek().tok {
            Tok::EqEq => BinOp::Eq,
            Tok::BangEq => BinOp::Ne,
            Tok::Lt => BinOp::Lt,
            Tok::LtEq => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::GtEq => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.additive()?;
        let span = lhs.span().to(rhs.span());
        let expr = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };

        if matches!(
            self.peek().tok,
            Tok::EqEq | Tok::BangEq | Tok::Lt | Tok::LtEq | Tok::Gt | Tok::GtEq
        ) {
            let t = self.peek().clone();
            return Err(self
                .err(t.span, Rule::ComparisonsDoNotChain, "comparison operators cannot be chained")
                .with_help("group one side explicitly, as in `[ (a) < (b) ] == (c)`"));
        }
        Ok(expr)
    }

    fn additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = match self.peek().tok {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.multiplicative()?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek().tok {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.unary()?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek().tok == Tok::Minus {
            let start = self.bump().span;
            let operand = self.unary()?;
            let span = start.to(operand.span());
            return Ok(Expr::Unary { op: UnOp::Neg, operand: Box::new(operand), span });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek().tok.clone() {
            Tok::Str(text) => {
                let span = self.bump().span;
                Ok(Expr::Literal { text, span })
            }
            Tok::Ident(text) => {
                let span = self.bump().span;
                Ok(Expr::Ident(Name { text, span }))
            }
            Tok::Escape(text) => {
                let span = self.bump().span;
                Ok(Expr::Escape { text, span })
            }
            // `f16 '5'` — a literal saying its own type, so it can stand where
            // nothing else would supply one.
            Tok::Word(word) => {
                let ty_span = self.bump().span;
                match self.peek().tok.clone() {
                    Tok::Str(text) => {
                        let lit = self.bump().span;
                        Ok(Expr::TypedLiteral {
                            ty: TypeRef { text: word, span: ty_span },
                            text,
                            span: ty_span.to(lit),
                        })
                    }
                    other => Err(self
                        .err(
                            ty_span.to(self.peek().span),
                            Rule::LiteralsNeedAType,
                            format!("expected a literal after `{word}`, found {}", other.describe()),
                        )
                        .with_help("a type in front of a value states that value's type, as in `f16 '5'`")),
                }
            }
            Tok::Pipe => {
                let start = self.bump().span;
                let inner = self.expr()?;
                if self.peek().tok != Tok::Pipe {
                    let t = self.peek().clone();
                    return Err(self
                        .err(t.span, Rule::GroupsArePiped, format!("expected `|` to close this group, found {}", t.tok.describe()))
                        .with_help(
                            "grouping uses pipes: `(...)` is a name and `[...]` is print's list, \
                             so neither was free",
                        ));
                }
                let end = self.bump().span;
                Ok(Expr::Group { inner: Box::new(inner), span: start.to(end) })
            }
            other => {
                let span = self.peek().span;
                Err(self
                    .err(span, Rule::ValuesAreQuoted, format!("expected a value, found {}", other.describe()))
                    .with_help("a value is a quoted literal like `'12'`, a name like `(x)`, or `| ... |`"))
            }
        }
    }
}
