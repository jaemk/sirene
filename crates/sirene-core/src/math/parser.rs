//! The parser (LMATH-2, step 3): tokens to a tree of nodes.
//!
//! It follows KaTeX's grammar: an expression is atoms until something ends it; an atom is a base,
//! a group or one symbol, followed by any superscript, subscript, or primes. It reads tokens
//! through the expander, one ahead, so macros are expanded as the parser reaches them. Commands
//! that the function table lists but the parser does not build yet fail with "not supported yet".

mod handlers;

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
        depth: 0,
        leftright_depth: 0,
    };
    let body = parser.parse_expression(false)?;
    parser.expect_end()?;
    Ok(body)
}

struct Parser<'a> {
    src: &'a str,
    expander: Expander<'a>,
    /// The token ahead, already expanded; `None` once it has been consumed.
    next: Option<Token<'a>>,
    mode: Mode,
    /// How many functions are open, bounded by `limits::MAX_DEPTH` as groups are.
    depth: usize,
    /// How many `\left` are open, which `\middle` needs.
    leftright_depth: usize,
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

/// The name a token is looked up by in the function table.
fn written(kind: &TokenKind<'_>) -> Option<String> {
    match kind {
        TokenKind::ControlWord(_) | TokenKind::ControlSymbol(_) | TokenKind::Char(_) => {
            Some(token_text(kind))
        }
        _ => None,
    }
}

fn is_infix(token: &Token<'_>) -> bool {
    written(&token.kind)
        .and_then(|name| functions::lookup(&name))
        .is_some_and(|f| f.infix)
}

/// In text mode `--`, `---`, ```` `` ```` and `''` are single characters: runs of those
/// one-character symbols become one symbol that holds the run.
fn form_ligatures(body: &mut Vec<Node<'_>>) {
    fn text_of<'n>(node: &Node<'n>) -> Option<&'n str> {
        match node.kind {
            NodeKind::Symbol {
                group: AtomGroup::TextOrd,
                name: Name::Char(text),
            } => Some(text),
            _ => None,
        }
    }
    let mut i = 0;
    while i + 1 < body.len() {
        let (Some(a), Some(b)) = (text_of(&body[i]), text_of(&body[i + 1])) else {
            i += 1;
            continue;
        };
        let run = if a == "-" && b == "-" {
            // `---` if another dash follows, else `--`.
            if body.get(i + 2).and_then(text_of) == Some("-") {
                Some(("---", 3))
            } else {
                Some(("--", 2))
            }
        } else if (a == "'" || a == "`") && a == b {
            Some((if a == "'" { "''" } else { "``" }, 2))
        } else {
            None
        };
        if let Some((text, count)) = run {
            let span = body[i].span.start..body[i + count - 1].span.end;
            body.splice(
                i..i + count,
                [Node {
                    kind: NodeKind::Symbol {
                        group: AtomGroup::TextOrd,
                        name: Name::Char(text),
                    },
                    mode: Mode::Text,
                    span,
                }],
            );
        }
        i += 1;
    }
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

    /// Switches between math and text mode, for the parser and the macros that read the mode.
    fn switch_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.expander.switch_mode(mode);
    }

    /// Atoms up to whatever ends the expression, which is left unread. With `break_on_infix` an
    /// infix command (`\over`) ends it too, as it does for the body of `\displaystyle`.
    fn parse_expression(&mut self, break_on_infix: bool) -> Result<Vec<Node<'a>>, MathError> {
        let mut body = Vec::new();
        loop {
            if self.mode == Mode::Math {
                self.consume_spaces()?;
            }
            let token = self.fetch()?;
            if ends_expression(&token) || (break_on_infix && is_infix(&token)) {
                break;
            }
            match self.parse_atom()? {
                Some(atom) => body.push(atom),
                None => break,
            }
        }
        if self.mode == Mode::Text {
            form_ligatures(&mut body);
        }
        Ok(body)
    }

    /// A base and the scripts that follow it.
    fn parse_atom(&mut self) -> Result<Option<Node<'a>>, MathError> {
        let base = self.parse_group("atom")?;
        // Text mode has no scripts.
        if self.mode == Mode::Text {
            return Ok(base);
        }
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
                    sup = Some(self.parse_script(&token, "superscript")?);
                }
                TokenKind::Char("_") => {
                    if sub.is_some() {
                        return Err(MathError::new("Double subscript", token.span));
                    }
                    sub = Some(self.parse_script(&token, "subscript")?);
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
    fn parse_script(&mut self, token: &Token<'a>, context: &str) -> Result<Node<'a>, MathError> {
        self.consume();
        self.consume_spaces()?;
        match self.parse_group(context)? {
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
            primes.push(self.parse_script(&token, "superscript")?);
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

    /// A braced group, a function with its arguments, or one symbol. `None` where nothing can
    /// start one. `context` says what the group is for (`atom`, `superscript`, `argument to
    /// '\frac'`), which decides whether a function may stand there.
    fn parse_group(&mut self, context: &str) -> Result<Option<Node<'a>>, MathError> {
        let token = self.fetch()?;
        let (closer, semisimple) = match token.kind {
            TokenKind::Char("{") => ("}", false),
            TokenKind::ControlWord("begingroup") => ("\\endgroup", true),
            _ => {
                return match self.parse_function(&token, context)? {
                    Some(node) => Ok(Some(node)),
                    None => self.parse_symbol(),
                };
            }
        };
        self.consume();
        self.expander.begin_group(&token.span)?;
        let body = self.parse_expression(false)?;
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
            TokenKind::Char(text) => Name::Char(text),
            // Handled where scripts are read, so not a symbol and not an error.
            TokenKind::ControlWord("limits" | "nolimits") => return Ok(None),
            TokenKind::ControlWord(word) => Name::Control(word),
            TokenKind::ControlSymbol(c) => {
                // The symbol table keys a control symbol by its character, which is the last
                // character of the token's range.
                let end = token.span.end;
                Name::Control(&self.src[end - c.len_utf8()..end])
            }
            TokenKind::Verb { .. } => return Err(unsupported("\\verb", &token)),
            // A space is a symbol in text mode, and is skipped before here in math mode.
            TokenKind::Space if self.mode == Mode::Text => Name::Char(" "),
            TokenKind::Space | TokenKind::Eof => return Ok(None),
        };
        // A letter with accents is the letter wrapped in accent nodes.
        let (name, marks) = match name {
            Name::Char(text) => self.split_accents(text),
            control => (control, Vec::new()),
        };
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
        let mut node = Node {
            kind: NodeKind::Symbol { group, name },
            mode,
            span: token.span.clone(),
        };
        for mark in marks {
            let Ok(at) = unicode_data::ACCENT_COMMANDS.binary_search_by_key(&mark, |e| e.0) else {
                return Err(MathError::new(
                    format!("Unknown accent ' {mark}'"),
                    token.span,
                ));
            };
            let (_, text, math) = unicode_data::ACCENT_COMMANDS[at];
            let label = if self.mode == Mode::Math && !math.is_empty() {
                math
            } else {
                text
            };
            node = Node {
                kind: NodeKind::Accent {
                    label: label.to_string(),
                    stretchy: false,
                    shifty: true,
                    base: Box::new(node),
                },
                mode: self.mode,
                span: token.span.clone(),
            };
        }
        self.consume();
        Ok(Some(node))
    }

    /// The base of a character and the combining accents on it, in order. A precomposed letter
    /// (`é`) that the symbol table lacks is split by its decomposition first, and a plain `i`
    /// or `j` under an accent loses its dot.
    fn split_accents(&self, text: &'a str) -> (Name<'a>, Vec<char>) {
        let first = text.chars().next();
        let decomposed = first.and_then(|c| {
            let listed = symbols::lookup(self.mode, Name::Char(c.encode_utf8(&mut [0; 4])));
            let at = unicode_data::DECOMPOSED
                .binary_search_by_key(&c, |e| e.0)
                .ok()?;
            listed
                .is_none()
                .then(|| (unicode_data::DECOMPOSED[at].1, c.len_utf8()))
        });
        let (base, marks): (&str, Vec<char>) = match decomposed {
            Some((expanded, skip)) => {
                let (base, marks) = split_marks(expanded);
                let mut marks: Vec<char> = marks.chars().collect();
                marks.extend(text[skip..].chars());
                (base, marks)
            }
            None => {
                let (base, marks) = split_marks(text);
                (base, marks.chars().collect())
            }
        };
        if marks.is_empty() {
            return (Name::Char(base), marks);
        }
        let base = match base {
            "i" => "ı",
            "j" => "ȷ",
            other => other,
        };
        (Name::Char(base), marks)
    }
}

/// A string without its trailing combining diacritical marks, and those marks.
fn split_marks(text: &str) -> (&str, &str) {
    let base = text.trim_end_matches(|c| ('\u{300}'..='\u{36f}').contains(&c));
    text.split_at(base.len())
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
            NodeKind::Frac {
                numer, denom, bar, ..
            } => format!(
                "frac{}[{} / {}]",
                if *bar { "" } else { "0" },
                show(numer),
                show(denom)
            ),
            NodeKind::Sqrt { body, index } => match index {
                Some(index) => format!("sqrt[{}]{}", show(index), show(body)),
                None => format!("sqrt{}", show(body)),
            },
            NodeKind::Styling { style, body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("{style:?}[{}]", inner.join(" "))
            }
            NodeKind::Text { body, font } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("\\{font}[{}]", inner.join(" "))
            }
            NodeKind::Font { font, body } => format!("{font}<{}>", show(body)),
            NodeKind::MClass { class, body, .. } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("{class:?}[{}]", inner.join(" "))
            }
            NodeKind::DelimSizing { size, delim, .. } => format!("big{size}{delim}"),
            NodeKind::LeftRight { body, left, right } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("left{left}[{}]right{right}", inner.join(" "))
            }
            NodeKind::Middle { delim } => format!("mid{delim}"),
            NodeKind::Accent { label, base, .. } => format!("{label}<{}>", show(base)),
            NodeKind::AccentUnder { label, base } => format!("{label}<{}>", show(base)),
            NodeKind::Overline { body } => format!("over<{}>", show(body)),
            NodeKind::Underline { body } => format!("under<{}>", show(body)),
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
    fn fractions() {
        // Each argument is a group, even a single character.
        assert_eq!(shown("\\frac12"), "frac[{1} / {2}]");
        assert_eq!(shown("\\frac{a}{b+c}"), "frac[{a} / {b + c}]");
        assert_eq!(shown("\\binom ab"), "frac0[{a} / {b}]");
        // A display or text fraction is wrapped in the style.
        assert_eq!(shown("\\dfrac12"), "Display[frac[{1} / {2}]]");
        assert_eq!(shown("\\tbinom12"), "Text[frac0[{1} / {2}]]");
        assert_eq!(shown("\\frac12^3"), "(frac[{1} / {2}] ^3)");
    }

    #[test]
    fn roots() {
        assert_eq!(shown("\\sqrt2"), "sqrt2");
        assert_eq!(shown("\\sqrt{x+1}"), "sqrt{x + 1}");
        assert_eq!(shown("\\sqrt[3]{x}"), "sqrt[{3}]{x}");
        // Without an index the body is one group or symbol, and `\sqrt` cannot be a body.
        assert_eq!(
            error("\\sqrt \\sqrt 2"),
            "Got function '\\sqrt' with no arguments as argument to '\\sqrt'"
        );
    }

    #[test]
    fn styles_run_to_the_end_of_the_group() {
        assert_eq!(shown("\\displaystyle x y"), "Display[x y]");
        assert_eq!(shown("{\\scriptstyle x}y"), "{Script[x]} y");
    }

    #[test]
    fn text_is_read_in_text_mode() {
        // The space is a symbol in text mode.
        assert_eq!(shown("\\text{a b}"), "\\text[a   b]");
        assert_eq!(shown("\\textbf{x}"), "\\textbf[x]");
        let nodes = parse("\\text{x}").unwrap();
        let NodeKind::Text { body, .. } = &nodes[0].kind else {
            unreachable!("text");
        };
        assert_eq!(body[0].mode, Mode::Text);
        assert_eq!(nodes[0].mode, Mode::Math);
        // Dashes and quotes join into one character in text.
        assert_eq!(shown("\\text{a--b}"), "\\text[a -- b]");
        assert_eq!(shown("\\text{---}"), "\\text[---]");
        assert_eq!(shown("\\text{``x''}"), "\\text[`` x '']");
        // A function that cannot appear in text says so.
        assert_eq!(
            error("\\text{\\frac12}"),
            "Can't use function '\\frac' in text mode"
        );
    }

    #[test]
    fn a_function_after_a_script_mark_must_be_allowed_as_an_argument() {
        assert_eq!(shown("x^\\frac12"), "(x ^frac[{1} / {2}])");
        assert_eq!(
            error("x^\\displaystyle"),
            "Got function '\\displaystyle' with no arguments as superscript"
        );
        assert_eq!(error("\\sqrt"), "Expected group as argument to '\\sqrt'");
    }

    #[test]
    fn fonts() {
        assert_eq!(shown("\\mathbf{x}"), "mathbf<x>");
        // A group of two stays a group; a group of one is its node.
        assert_eq!(shown("\\mathbf{xy}"), "mathbf<{x y}>");
        assert_eq!(shown("\\Bbb R"), "mathbb<R>");
        assert_eq!(shown("\\bold x"), "mathbf<x>");
        // The old switches set the rest of the group.
        assert_eq!(shown("{\\bf x y}z"), "{mathbf<{x y}>} z");
        assert_eq!(shown("a\\rm b"), "a mathrm<{b}>");
    }

    #[test]
    fn classes() {
        assert_eq!(shown("\\mathbin{+}"), "Bin[+]");
        assert_eq!(shown("\\mathrel{ab}"), "Rel[a b]");
        assert_eq!(shown("\\boldsymbol{+}"), "Bin[boldsymbol<{+}>]");
        assert_eq!(shown("\\boldsymbol{x}"), "Ord[boldsymbol<{x}>]");
        assert_eq!(shown("\\bm{=}"), "Rel[boldsymbol<{=}>]");
        let nodes = parse("\\mathbin{+}\\mathbin{ab}").unwrap();
        let flags: Vec<_> = nodes
            .iter()
            .map(|n| {
                matches!(
                    n.kind,
                    NodeKind::MClass {
                        character_box: true,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(flags, [true, false]);
    }

    #[test]
    fn lines() {
        assert_eq!(shown("\\overline{x}"), "over<{x}>");
        assert_eq!(shown("\\underline x"), "under<{x}>");
    }

    #[test]
    fn delimiters() {
        assert_eq!(shown("\\left( x \\right)"), "left([x]right)");
        assert_eq!(shown("\\left. x \\right\\rangle"), "left.[x]right\\rangle");
        assert_eq!(
            shown("\\left( a \\middle| b \\right)"),
            "left([a mid| b]right)"
        );
        assert_eq!(
            shown("\\left( \\left[ x \\right] \\right)"),
            "left([left[[x]right]]right)"
        );
        assert_eq!(shown("\\big( \\Bigr] \\biggm|"), "big1( big2] big3|");
        assert_eq!(shown("\\bigl{(}"), "big1(");
        assert_eq!(error("\\left( x"), "Expected '\\right', got 'EOF'");
        assert_eq!(error("\\left( x }"), "Expected '\\right', got '}'");
        assert_eq!(error("x \\right)"), "Expected 'EOF', got '\\right'");
        assert_eq!(error("\\middle|"), "\\middle without preceding \\left");
        assert_eq!(
            error("\\left a x \\right)"),
            "Invalid delimiter 'a' after '\\left'"
        );
        assert_eq!(error("\\big{ab}"), "Invalid delimiter type 'ordgroup'");
        assert_eq!(error("\\left"), "Expected group as argument to '\\left'");
    }

    #[test]
    fn function_nesting_is_bounded() {
        let source = "\\left(".repeat(5_000);
        assert!(error(&source).starts_with("groups nested more than"));
        let source = "\\displaystyle".repeat(5_000);
        assert!(error(&source).starts_with("groups nested more than"));
        let source = "\\bf".repeat(20_000);
        assert!(error(&source).starts_with("groups nested more than"));
        // A function that cannot be an argument stops the chain at once.
        assert_eq!(
            error(&"\\mathbin".repeat(5_000)),
            "Got function '\\mathbin' with no arguments as argument to '\\mathbin'"
        );
    }

    #[test]
    fn accents() {
        assert_eq!(shown("\\hat x"), "\\hat<x>");
        assert_eq!(shown("\\widehat{xy}"), "\\widehat<{x y}>");
        assert_eq!(shown("\\underrightarrow x"), "\\underrightarrow<{x}>");
        // A text accent keeps its argument as read, and is text even in math.
        assert_eq!(shown("\\'e"), "\\'<e>");
        assert_eq!(parse("\\'e").unwrap()[0].mode, Mode::Text);
    }

    #[test]
    fn combining_marks_and_precomposed_letters_become_accents() {
        assert_eq!(shown("e\u{301}"), "\\acute<e>");
        assert_eq!(shown("\u{e9}"), "\\acute<e>");
        assert_eq!(shown("e\u{301}\u{302}"), "\\hat<\\acute<e>>");
        // A dotted i loses its dot under an accent.
        assert_eq!(shown("i\u{301}"), "\\acute<\u{131}>");
        // In text, the text-mode command is the label.
        assert_eq!(shown("\\text{\u{e9}}"), "\\text[\\'<e>]");
        // A mark with no command is an error.
        assert_eq!(
            error("e\u{300}\u{334}"),
            "Expected 'EOF', got '\u{334}'"
                .replace("Expected 'EOF', got ", "Unknown accent ' ")
                .replace("'\u{334}'", "\u{334}'")
        );
    }

    #[test]
    fn what_the_parser_lacks_is_said_so() {
        assert_eq!(error("\\overbrace{x}"), "\\overbrace is not supported yet");
        assert_eq!(error("\\sum"), "\\sum is not supported yet");
        assert_eq!(error("\\verb|x|"), "\\verb is not supported yet");
        assert!(error("x\u{2082}").ends_with("supported yet"));
    }

    #[test]
    fn nesting_is_bounded() {
        let deep = format!("{}{}", "{".repeat(10_000), "}".repeat(10_000));
        assert!(error(&deep).starts_with("groups nested more than"));
    }
}
