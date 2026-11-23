//! The parser (LMATH-2, step 3): tokens to a tree of nodes.
//!
//! It follows KaTeX's grammar: an expression is atoms until something ends it; an atom is a base,
//! a group or one symbol, followed by any superscript, subscript, or primes. It reads tokens
//! through the expander, one ahead, so macros are expanded as the parser reaches them. Commands
//! that the function table lists but the parser does not build yet fail with "not supported yet".

use std::ops::Range;

use super::{
    MathError,
    ast::{Node, NodeKind},
    expander::{Expander, token_text},
    functions,
    lexer::{Token, TokenKind},
    symbols::{self, AtomGroup, Mode, Name},
    unicode_data,
};

/// Parses a formula in math mode.
pub(crate) fn parse(src: &str) -> Result<Vec<Node<'_>>, MathError> {
    let mut parser = Parser {
        src,
        expander: Expander::new(src, Mode::Math)?,
        next: None,
        mode: Mode::Math,
    };
    let body = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(body)
}

struct Parser<'a> {
    src: &'a str,
    expander: Expander<'a>,
    /// The token ahead, already expanded; `None` once it has been consumed.
    next: Option<Token<'a>>,
    mode: Mode,
}

/// Whether `token` ends an expression: a closing brace, `\end`, `\right`, `\endgroup`, `&`, or
/// the end of the input.
fn ends_expression(token: &Token<'_>) -> bool {
    match token.kind {
        TokenKind::Char(text) => text == "}" || text == "&",
        TokenKind::ControlWord(word) => matches!(word, "end" | "right" | "endgroup"),
        TokenKind::Eof => true,
        _ => false,
    }
}

fn is_char(token: &Token<'_>, text: &str) -> bool {
    matches!(token.kind, TokenKind::Char(c) if c == text)
}

impl<'a> Parser<'a> {
    /// The next token, expanding macros to reach it, without consuming it.
    fn fetch(&mut self) -> Result<Token<'a>, MathError> {
        if self.next.is_none() {
            self.next = Some(self.expander.expand_next_token()?);
        }
        match &self.next {
            Some(token) => Ok(token.clone()),
            None => Err(MathError::new("Empty token", 0..0)),
        }
    }

    /// Takes the token `fetch` returned.
    fn consume(&mut self) {
        self.next = None;
    }

    fn consume_spaces(&mut self) -> Result<(), MathError> {
        while self.fetch()?.kind == TokenKind::Space {
            self.consume();
        }
        Ok(())
    }

    /// Consumes the token if it reads `text`, and fails with KaTeX's message if not.
    fn expect(&mut self, text: &str) -> Result<Token<'a>, MathError> {
        let token = self.fetch()?;
        let got = token_text(&token.kind);
        if got != text {
            return Err(MathError::new(
                format!("Expected '{text}', got '{got}'"),
                token.span,
            ));
        }
        self.consume();
        Ok(token)
    }

    fn expect_end(&mut self) -> Result<(), MathError> {
        let token = self.fetch()?;
        if token.kind != TokenKind::Eof {
            return Err(MathError::new(
                format!("Expected 'EOF', got '{}'", token_text(&token.kind)),
                token.span,
            ));
        }
        Ok(())
    }

    /// Atoms up to whatever ends the expression, which is left unread.
    fn parse_expression(&mut self) -> Result<Vec<Node<'a>>, MathError> {
        let mut body = Vec::new();
        loop {
            if self.mode == Mode::Math {
                self.consume_spaces()?;
            }
            if ends_expression(&self.fetch()?) {
                break;
            }
            match self.parse_atom()? {
                Some(atom) => body.push(atom),
                None => break,
            }
        }
        Ok(body)
    }

    /// A base and the scripts that follow it.
    fn parse_atom(&mut self) -> Result<Option<Node<'a>>, MathError> {
        let base = self.parse_group()?;
        let mut sup: Option<Node<'a>> = None;
        let mut sub: Option<Node<'a>> = None;
        loop {
            self.consume_spaces()?;
            let token = self.fetch()?;
            match token.kind {
                TokenKind::Char("^") => {
                    if sup.is_some() {
                        return Err(MathError::new("Double superscript", token.span));
                    }
                    sup = Some(self.parse_script(&token)?);
                }
                TokenKind::Char("_") => {
                    if sub.is_some() {
                        return Err(MathError::new("Double subscript", token.span));
                    }
                    sub = Some(self.parse_script(&token)?);
                }
                TokenKind::Char("'") => {
                    if sup.is_some() {
                        return Err(MathError::new("Double superscript", token.span));
                    }
                    sup = Some(self.parse_primes(&token)?);
                }
                TokenKind::Char(text)
                    if text
                        .chars()
                        .next()
                        .is_some_and(|c| unicode_data::SCRIPTS.binary_search(&c).is_ok()) =>
                {
                    return Err(MathError::new(
                        "subscript and superscript characters are not supported yet",
                        token.span,
                    ));
                }
                TokenKind::ControlWord("limits" | "nolimits") => {
                    return Err(MathError::new(
                        "Limit controls must follow a math operator",
                        token.span,
                    ));
                }
                _ => break,
            }
        }
        if sup.is_none() && sub.is_none() {
            return Ok(base);
        }
        let start = base.as_ref().map_or_else(
            || {
                let first = sup.iter().chain(&sub).map(|n| n.span.start).min();
                first.unwrap_or(0)
            },
            |b| b.span.start,
        );
        let end = base
            .iter()
            .chain(&sup)
            .chain(&sub)
            .map(|n| n.span.end)
            .max()
            .unwrap_or(start);
        Ok(Some(Node {
            kind: NodeKind::SupSub {
                base: base.map(Box::new),
                sup: sup.map(Box::new),
                sub: sub.map(Box::new),
            },
            mode: self.mode,
            span: start..end,
        }))
    }

    /// The group after `^` or `_`; `token` is that character.
    fn parse_script(&mut self, token: &Token<'a>) -> Result<Node<'a>, MathError> {
        self.consume();
        self.consume_spaces()?;
        match self.parse_group()? {
            Some(group) => Ok(group),
            None => Err(MathError::new(
                format!("Expected group after '{}'", token_text(&token.kind)),
                token.span.clone(),
            )),
        }
    }

    /// One or more `'`, then an optional superscript that follows them directly, as a group of
    /// `\prime` symbols (`x'^2` is `x^{\prime 2}`).
    fn parse_primes(&mut self, first: &Token<'a>) -> Result<Node<'a>, MathError> {
        let prime = |span: Range<usize>| Node {
            kind: NodeKind::Symbol {
                group: AtomGroup::TextOrd,
                name: Name::Control("prime"),
            },
            mode: Mode::Math,
            span,
        };
        let mut primes = vec![prime(first.span.clone())];
        self.consume();
        loop {
            let token = self.fetch()?;
            if !is_char(&token, "'") {
                break;
            }
            primes.push(prime(token.span));
            self.consume();
        }
        let token = self.fetch()?;
        if is_char(&token, "^") {
            primes.push(self.parse_script(&token)?);
        }
        let end = primes.last().map_or(first.span.end, |n| n.span.end);
        Ok(Node {
            kind: NodeKind::Group {
                body: primes,
                semisimple: false,
            },
            mode: self.mode,
            span: first.span.start..end,
        })
    }

    /// A braced group, or one symbol. `None` where nothing can start a group.
    fn parse_group(&mut self) -> Result<Option<Node<'a>>, MathError> {
        let token = self.fetch()?;
        let (closer, semisimple) = match token.kind {
            TokenKind::Char("{") => ("}", false),
            TokenKind::ControlWord("begingroup") => ("\\endgroup", true),
            _ => return self.parse_symbol(),
        };
        self.consume();
        self.expander.begin_group(&token.span)?;
        let body = self.parse_expression()?;
        // The closer is consumed, but the token after it is not read until the group's
        // definitions have ended.
        let close = self.expect(closer)?;
        self.expander.end_group(&close.span)?;
        Ok(Some(Node {
            kind: NodeKind::Group { body, semisimple },
            mode: self.mode,
            span: token.span.start..close.span.end,
        }))
    }

    /// One symbol, if the next token is one.
    fn parse_symbol(&mut self) -> Result<Option<Node<'a>>, MathError> {
        let token = self.fetch()?;
        let name = match token.kind {
            TokenKind::Char(text) => {
                if functions::lookup(text).is_some() {
                    return Err(unsupported(text, &token));
                }
                Name::Char(text)
            }
            TokenKind::ControlWord(word) => {
                let written = format!("\\{word}");
                if functions::lookup(&written).is_some() {
                    return Err(unsupported(&written, &token));
                }
                Name::Control(word)
            }
            TokenKind::ControlSymbol(c) => {
                let written = format!("\\{c}");
                if functions::lookup(&written).is_some() {
                    return Err(unsupported(&written, &token));
                }
                // The symbol table keys a control symbol by its character, which is the last
                // character of the token's range.
                let end = token.span.end;
                Name::Control(&self.src[end - c.len_utf8()..end])
            }
            TokenKind::Verb { .. } => return Err(unsupported("\\verb", &token)),
            TokenKind::Space | TokenKind::Eof => return Ok(None),
        };
        // An accented letter is a base and an accent, and a combining mark makes one.
        if let Name::Char(text) = name {
            let first = text.chars().next();
            let listed = |c: char| {
                symbols::lookup(self.mode, Name::Char(c.encode_utf8(&mut [0; 4]))).is_some()
            };
            if first.is_some_and(|c| unicode_data::ACCENTED.binary_search(&c).is_ok() && !listed(c))
                || (text.chars().count() > 1 && symbols::lookup(self.mode, name).is_none())
            {
                return Err(MathError::new(
                    "accented characters are not supported yet",
                    token.span,
                ));
            }
        }
        let (group, mode) = match (symbols::lookup(self.mode, name), name) {
            (Some(symbol), _) => (symbol.group, self.mode),
            // A character the table does not list is an ordinary symbol set as text, if it is
            // not ASCII.
            (None, Name::Char(text)) if !text.is_ascii() => (AtomGroup::TextOrd, Mode::Text),
            (None, Name::Control(_)) => {
                return Err(MathError::new(
                    format!("Undefined control sequence: {}", token_text(&token.kind)),
                    token.span,
                ));
            }
            // An ASCII character with no symbol ends the expression, and the caller reports it.
            (None, Name::Char(_)) => return Ok(None),
        };
        if matches!(group, AtomGroup::Op | AtomGroup::Accent) {
            return Err(unsupported(&token_text(&token.kind), &token));
        }
        self.consume();
        Ok(Some(Node {
            kind: NodeKind::Symbol { group, name },
            mode,
            span: token.span,
        }))
    }
}

fn unsupported(name: &str, token: &Token<'_>) -> MathError {
    MathError::new(format!("{name} is not supported yet"), token.span.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node as a short string: `a` for a symbol, `{..}` for a group, `(base ^sup _sub)`.
    fn show(node: &Node<'_>) -> String {
        match &node.kind {
            NodeKind::Symbol { name, .. } => match name {
                Name::Char(text) => (*text).into(),
                Name::Control(name) => format!("\\{name}"),
            },
            NodeKind::Group { body, semisimple } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                let (open, close) = if *semisimple { ("<", ">") } else { ("{", "}") };
                format!("{open}{}{close}", inner.join(" "))
            }
            NodeKind::SupSub { base, sup, sub } => {
                let part = |label: &str, n: &Option<Box<Node<'_>>>| {
                    n.as_deref()
                        .map(|n| format!(" {label}{}", show(n)))
                        .unwrap_or_default()
                };
                let base = base.as_deref().map(show).unwrap_or_default();
                format!("({base}{}{})", part("^", sup), part("_", sub))
            }
        }
    }

    fn shown(src: &str) -> String {
        parse(src)
            .unwrap()
            .iter()
            .map(show)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn error(src: &str) -> String {
        parse(src).unwrap_err().message
    }

    #[test]
    fn symbols_and_groups() {
        assert_eq!(shown("x + y"), "x + y");
        assert_eq!(shown("a{b{c}}d"), "a {b {c}} d");
        assert_eq!(shown(""), "");
        assert_eq!(shown("\\alpha\\leq"), "\\alpha \\leq");
    }

    #[test]
    fn scripts() {
        assert_eq!(shown("x^2_i"), "(x ^2 _i)");
        assert_eq!(shown("x_i^2"), "(x ^2 _i)");
        assert_eq!(shown("x ^ {a b}"), "(x ^{a b})");
        assert_eq!(shown("^2"), "( ^2)");
        assert_eq!(shown("{x}^2"), "({x} ^2)");
        // A script reads the symbol after any spaces.
        assert_eq!(shown("x^ 2"), "(x ^2)");
    }

    #[test]
    fn primes_become_a_group_of_prime_symbols() {
        assert_eq!(shown("f'"), "(f ^{\\prime})");
        assert_eq!(shown("f''"), "(f ^{\\prime \\prime})");
        assert_eq!(shown("f'^2"), "(f ^{\\prime 2})");
        assert_eq!(shown("f'_1"), "(f ^{\\prime} _1)");
    }

    #[test]
    fn begingroup_is_a_semisimple_group() {
        assert_eq!(shown("\\begingroup xy \\endgroup"), "<x y>");
    }

    #[test]
    fn macros_expand_where_the_parser_reaches_them() {
        assert_eq!(shown("\\newcommand{\\f}[1]{#1^2}\\f{x}"), "(x ^2)");
        // A definition ends with its group.
        assert_eq!(shown("{\\def\\a{x}\\a}"), "{x}");
        assert_eq!(
            error("{\\def\\a{x}\\a}\\a"),
            "Undefined control sequence: \\a"
        );
    }

    #[test]
    fn spans_cover_the_source_read() {
        let nodes = parse("ab^{cd}").unwrap();
        assert_eq!(nodes[0].span, 0..1);
        assert_eq!(nodes[1].span, 1..7);
        let NodeKind::SupSub { sup, .. } = &nodes[1].kind else {
            unreachable!("a script");
        };
        assert_eq!(sup.as_ref().map(|s| s.span.clone()), Some(3..7));
        let primes = parse("f''").unwrap();
        assert_eq!(primes[0].span, 0..3);
    }

    #[test]
    fn non_ascii_characters_outside_the_table_are_text() {
        let nodes = parse("\u{8a66}").unwrap();
        assert_eq!(nodes[0].mode, Mode::Text);
        assert!(matches!(
            nodes[0].kind,
            NodeKind::Symbol {
                group: AtomGroup::TextOrd,
                ..
            }
        ));
    }

    #[test]
    fn errors_read_as_katex_s_do() {
        assert_eq!(error("x^"), "Expected group after '^'");
        assert_eq!(error("x^2^3"), "Double superscript");
        assert_eq!(error("x_2_3"), "Double subscript");
        assert_eq!(error("x'^2^3"), "Double superscript");
        assert_eq!(error("{x"), "Expected '}', got 'EOF'");
        assert_eq!(error("x}"), "Expected 'EOF', got '}'");
        assert_eq!(error("\\foo"), "Undefined control sequence: \\foo");
        assert_eq!(
            error("x\\limits"),
            "Limit controls must follow a math operator"
        );
        assert_eq!(error("\\begingroup x}"), "Expected '\\endgroup', got '}'");
        assert_eq!(error("#"), "Expected 'EOF', got '#'");
    }

    #[test]
    fn what_the_parser_lacks_is_said_so() {
        assert_eq!(error("\\frac12"), "\\frac is not supported yet");
        assert_eq!(error("\\sum"), "\\sum is not supported yet");
        assert_eq!(error("\\verb|x|"), "\\verb is not supported yet");
        assert!(error("\u{e9}").ends_with("supported yet"));
        assert!(error("x\u{2082}").ends_with("supported yet"));
    }

    #[test]
    fn nesting_is_bounded() {
        let deep = format!("{}{}", "{".repeat(10_000), "}".repeat(10_000));
        assert!(error(&deep).starts_with("groups nested more than"));
    }
}
