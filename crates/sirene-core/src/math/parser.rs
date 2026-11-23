//! The parser (LMATH-2, step 3): tokens to a tree of nodes.
//!
//! It follows KaTeX's grammar: an expression is atoms until something ends it; an atom is a base,
//! a group or one symbol, followed by any superscript, subscript, or primes. It reads tokens
//! through the expander, one ahead, so macros are expanded as the parser reaches them. Commands
//! that the function table lists but the parser does not build yet fail with "not supported yet".

mod arguments;
mod environments;
mod handlers;

use std::ops::Range;

use super::{
    MathError,
    ast::{Node, NodeKind},
    expander::{Expander, token_text},
    functions,
    lexer::{Token, TokenKind},
    limits,
    symbols::{self, AtomGroup, Mode, Name},
    unicode_data,
};

/// What the caller chooses about a formula.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Settings {
    /// Display mode: the aligned environments and `\tag` need it.
    pub display_mode: bool,
}

/// Parses a formula in math mode, in inline mode.
pub(crate) fn parse(src: &str) -> Result<Vec<Node<'_>>, MathError> {
    parse_with(src, Settings::default())
}

/// Parses a formula in math mode.
pub(crate) fn parse_with(src: &str, settings: Settings) -> Result<Vec<Node<'_>>, MathError> {
    let mut parser = Parser {
        src,
        expander: Expander::new(src, Mode::Math)?,
        next: None,
        mode: Mode::Math,
        depth: 0,
        leftright_depth: 0,
        break_on: None,
        display_mode: settings.display_mode,
        current_color: None,
    };
    let body = parser.parse_expression(false)?;
    parser.expect_end()?;
    // An equation tag wraps the formula.
    if parser.expander.has_macro("df@tag") {
        if !settings.display_mode {
            return Err(MathError::new(
                "\\tag works only in display equations",
                src.len()..src.len(),
            ));
        }
        let tag = parser.subparse(vec![Token {
            kind: TokenKind::ControlWord("df@tag"),
            span: 0..0,
        }])?;
        let span = body
            .first()
            .zip(body.last())
            .map_or(0..0, |(a, b)| a.span.start..b.span.end);
        return Ok(vec![Node {
            kind: NodeKind::Tag { body, tag },
            mode: Mode::Text,
            span,
        }]);
    }
    Ok(body)
}

struct Parser<'a> {
    src: &'a str,
    expander: Expander<'a>,
    /// The token ahead, already expanded; `None` once it has been consumed.
    next: Option<Token<'a>>,
    mode: Mode,
    /// How many brace groups and functions are open, bounded by `limits::MAX_DEPTH`.
    depth: usize,
    /// How many `\left` are open, which `\middle` needs.
    leftright_depth: usize,
    /// The token that ends the expression being read, besides the usual ones: `\\` in a table
    /// cell. Functions that read the rest of the group stop at it too.
    break_on: Option<&'static str>,
    display_mode: bool,
    /// The color of the last `\color` in the open groups, which a `\right` after it takes.
    current_color: Option<String>,
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
    // One pass into a new list: a splice per run would move the rest of the list each time.
    let mut input: Vec<Option<Node<'_>>> = std::mem::take(body).into_iter().map(Some).collect();
    fn text_at<'n>(input: &[Option<Node<'n>>], i: usize) -> Option<&'n str> {
        input.get(i)?.as_ref().and_then(text_of)
    }
    let mut i = 0;
    while i < input.len() {
        let run = match (text_at(&input, i), text_at(&input, i + 1)) {
            // `---` if another dash follows, else `--`.
            (Some("-"), Some("-")) if text_at(&input, i + 2) == Some("-") => Some(("---", 3)),
            (Some("-"), Some("-")) => Some(("--", 2)),
            (Some("'"), Some("'")) => Some(("''", 2)),
            (Some("`"), Some("`")) => Some(("``", 2)),
            _ => None,
        };
        match run {
            Some((text, count)) => {
                let start = input[i].as_ref().map_or(0, |n| n.span.start);
                let end = input[i + count - 1].as_ref().map_or(start, |n| n.span.end);
                body.push(Node {
                    kind: NodeKind::Symbol {
                        group: AtomGroup::TextOrd,
                        name: Name::Char(text),
                    },
                    mode: Mode::Text,
                    span: start..end,
                });
                i += count;
            }
            None => {
                body.extend(input[i].take());
                i += 1;
            }
        }
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

    /// Parses `tokens` as a formula of their own at this point, as a tag or a cell is, and
    /// returns its nodes. The token ahead is kept for after.
    fn subparse(&mut self, tokens: Vec<Token<'a>>) -> Result<Vec<Node<'a>>, MathError> {
        let saved = self.next.take();
        // The formula ends at an extra closing brace.
        self.expander.push_token(Token {
            kind: TokenKind::Char("}"),
            span: 0..0,
        });
        self.expander.push_tokens(tokens);
        let body = self.parse_expression(false)?;
        self.expect("}")?;
        self.next = saved;
        Ok(body)
    }

    /// Opens one level of nesting, a brace group or a function, which the parser reads by
    /// recursion: braces and functions share the budget of `limits::MAX_DEPTH`, so the stack
    /// they use is bounded. A level is closed with `self.depth -= 1`; an error ends the parse, so
    /// the paths that fail need not close it.
    fn enter(&mut self, at: &Range<usize>) -> Result<(), MathError> {
        if self.depth >= limits::MAX_DEPTH {
            return Err(MathError::new(
                format!("groups nested more than {} deep", limits::MAX_DEPTH),
                at.clone(),
            ));
        }
        self.depth += 1;
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
        self.parse_expression_until(break_on_infix, None)
    }

    /// Like `parse_expression`, ending also at the token written `until`.
    fn parse_expression_until(
        &mut self,
        break_on_infix: bool,
        until: Option<&'static str>,
    ) -> Result<Vec<Node<'a>>, MathError> {
        let saved = std::mem::replace(&mut self.break_on, until);
        let body = self.expression_body(break_on_infix);
        self.break_on = saved;
        body
    }

    fn expression_body(&mut self, break_on_infix: bool) -> Result<Vec<Node<'a>>, MathError> {
        let mut body = Vec::new();
        loop {
            if self.mode == Mode::Math {
                self.consume_spaces()?;
            }
            let token = self.fetch()?;
            if ends_expression(&token)
                || (break_on_infix && is_infix(&token))
                || self.break_on.is_some_and(|b| token_text(&token.kind) == b)
            {
                break;
            }
            match self.parse_atom()? {
                // Nothing (`\relax`) does not appear in the tree.
                Some(Node {
                    kind: NodeKind::Internal,
                    ..
                }) => {}
                Some(atom) => body.push(atom),
                None => break,
            }
        }
        if self.mode == Mode::Text {
            form_ligatures(&mut body);
        }
        self.split_infix(body)
    }

    /// A base and the scripts that follow it.
    fn parse_atom(&mut self) -> Result<Option<Node<'a>>, MathError> {
        let mut base = self.parse_group("atom")?;
        // Nothing cannot take scripts; they find a base of their own next time.
        if matches!(
            base,
            Some(Node {
                kind: NodeKind::Internal,
                ..
            })
        ) {
            return Ok(base);
        }
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
                TokenKind::ControlWord(control @ ("limits" | "nolimits")) => {
                    let wanted = control == "limits";
                    match base.as_mut().map(|b| &mut b.kind) {
                        Some(NodeKind::Op {
                            limits,
                            always_handle_supsub,
                            ..
                        }) => {
                            *limits = wanted;
                            *always_handle_supsub = true;
                        }
                        Some(NodeKind::OperatorName {
                            limits,
                            always_handle_supsub: true,
                            ..
                        }) => *limits = wanted,
                        // A plain `\operatorname` takes no limit controls, and ignores them.
                        Some(NodeKind::OperatorName { .. }) => {}
                        _ => {
                            return Err(MathError::new(
                                "Limit controls must follow a math operator",
                                token.span,
                            ));
                        }
                    }
                    self.consume();
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
        self.enter(&token.span)?;
        let outer_color = self.current_color.clone();
        let body = self.parse_expression(false)?;
        self.current_color = outer_color;
        self.depth -= 1;
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
            TokenKind::Verb { star, body, .. } => {
                self.consume();
                return Ok(Some(Node {
                    kind: NodeKind::Verb { body, star },
                    mode: Mode::Text,
                    span: token.span,
                }));
            }
            // A space is a symbol in text mode, and is skipped before here in math mode.
            TokenKind::Space if self.mode == Mode::Text => Name::Char(" "),
            // The expander turns a frozen token into `\relax` before the parser sees it.
            TokenKind::Space | TokenKind::Frozen(_) | TokenKind::Eof => return Ok(None),
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
    use super::super::ast::OpContent;
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
                numer,
                denom,
                bar,
                left,
                right,
                ..
            } => format!(
                "frac{}{}[{} / {}]",
                if *bar { "" } else { "0" },
                match (left, right) {
                    (None, None) => String::new(),
                    _ => format!(
                        "<{} {}>",
                        left.as_deref().unwrap_or("."),
                        right.as_deref().unwrap_or(".")
                    ),
                },
                show(numer),
                show(denom)
            ),
            NodeKind::Infix { replace_with, .. } => format!("infix{replace_with}"),
            NodeKind::Sqrt { body, index } => match index {
                Some(index) => format!("sqrt[{}]{}", show(index), show(body)),
                None => format!("sqrt{}", show(body)),
            },
            NodeKind::Styling { style, body, .. } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("{style:?}[{}]", inner.join(" "))
            }
            NodeKind::Text { body, font } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("\\{}[{}]", font.unwrap_or(""), inner.join(" "))
            }
            NodeKind::HtmlMathml { html, mathml } => {
                let part =
                    |nodes: &[Node<'_>]| nodes.iter().map(show).collect::<Vec<_>>().join(" ");
                format!("htmlmathml[{} | {}]", part(html), part(mathml))
            }
            NodeKind::CharCode(c) => format!("char<{c}>"),
            NodeKind::Verb { body, star } => {
                format!("verb{}<{body}>", if *star { "*" } else { "" })
            }
            NodeKind::Raw { string } => format!("raw<{string}>"),
            NodeKind::Url { url } => format!("url<{url}>"),
            NodeKind::Font { font, body } => format!("{font}<{}>", show(body)),
            NodeKind::MClass { class, body, .. } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("{class:?}[{}]", inner.join(" "))
            }
            NodeKind::DelimSizing { size, delim, .. } => format!("big{size}{delim}"),
            NodeKind::LeftRight {
                body, left, right, ..
            } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("left{left}[{}]right{right}", inner.join(" "))
            }
            NodeKind::Middle { delim } => format!("mid{delim}"),
            NodeKind::Array(array) => {
                let rows: Vec<String> = array
                    .body
                    .iter()
                    .map(|row| row.iter().map(show).collect::<Vec<_>>().join(" & "))
                    .collect();
                format!("array[{}]", rows.join(" \\\\ "))
            }
            NodeKind::Cr { size, .. } => match size {
                Some(size) => format!("cr{}{}", size.number, size.unit),
                None => "cr".to_string(),
            },
            NodeKind::Environment { name } => format!("end{name}"),
            NodeKind::Tag { body, tag } => {
                let (b, t): (Vec<_>, Vec<_>) = (
                    body.iter().map(show).collect(),
                    tag.iter().map(show).collect(),
                );
                format!("tag[{}]({})", t.join(" "), b.join(" "))
            }
            NodeKind::Kern { dimension } => format!("kern{}{}", dimension.number, dimension.unit),
            NodeKind::Rule { width, height, .. } => format!(
                "rule{}{}x{}{}",
                width.number, width.unit, height.number, height.unit
            ),
            NodeKind::Color { color, body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("{color}[{}]", inner.join(" "))
            }
            NodeKind::Phantom { body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("phantom[{}]", inner.join(" "))
            }
            NodeKind::VPhantom { body } => format!("vphantom<{}>", show(body)),
            NodeKind::Smash {
                body,
                height,
                depth,
            } => format!(
                "smash{}{}<{}>",
                if *height { "t" } else { "" },
                if *depth { "b" } else { "" },
                show(body)
            ),
            NodeKind::Lap { alignment, body } => format!("{alignment}<{}>", show(body)),
            NodeKind::RaiseBox { dy, body } => {
                format!("raise{}{}<{}>", dy.number, dy.unit, show(body))
            }
            NodeKind::Sizing { size, body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("size{size}[{}]", inner.join(" "))
            }
            NodeKind::Size { value, .. } => format!("{}{}", value.number, value.unit),
            NodeKind::ColorToken(color) => color.clone(),
            NodeKind::Internal => "internal".to_string(),
            NodeKind::Op {
                limits, content, ..
            } => {
                let lim = if *limits { "L" } else { "" };
                match content {
                    OpContent::Name(name) => format!("op{lim}{name}"),
                    OpContent::Body(body) => {
                        let inner: Vec<_> = body.iter().map(show).collect();
                        format!("op{lim}[{}]", inner.join(" "))
                    }
                }
            }
            NodeKind::OperatorName { body, limits, .. } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!(
                    "opname{}[{}]",
                    if *limits { "L" } else { "" },
                    inner.join(" ")
                )
            }
            NodeKind::Accent { label, base, .. } => format!("{label}<{}>", show(base)),
            NodeKind::AccentUnder { label, base } => format!("{label}<{}>", show(base)),
            NodeKind::Overline { body } => format!("over<{}>", show(body)),
            NodeKind::Underline { body } => format!("under<{}>", show(body)),
            NodeKind::Enclose {
                label,
                background_color,
                border_color,
                body,
            } => format!(
                "{label}[{}|{}]<{}>",
                border_color.as_deref().unwrap_or(""),
                background_color.as_deref().unwrap_or(""),
                show(body)
            ),
            NodeKind::Pmb { class, body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("pmb{class:?}[{}]", inner.join(" "))
            }
            NodeKind::HBox { body } => {
                let inner: Vec<_> = body.iter().map(show).collect();
                format!("hbox[{}]", inner.join(" "))
            }
            NodeKind::VCenter { body } => format!("vcenter<{}>", show(body)),
            NodeKind::MathChoice {
                display,
                text,
                script,
                script_script,
            } => {
                let part =
                    |nodes: &[Node<'_>]| nodes.iter().map(show).collect::<Vec<_>>().join(" ");
                format!(
                    "choice[{} | {} | {} | {}]",
                    part(display),
                    part(text),
                    part(script),
                    part(script_script)
                )
            }
            NodeKind::ReflectBox { body } => format!("reflect<{}>", show(body)),
            NodeKind::XArrow { label, body, below } => match below {
                Some(below) => format!("{label}[{}]<{}>", show(below), show(body)),
                None => format!("{label}<{}>", show(body)),
            },
            NodeKind::HorizBrace { label, base, .. } => format!("{label}<{}>", show(base)),
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
        assert_eq!(shown("\\binom ab"), "frac0<( )>[{a} / {b}]");
        // A display or text fraction is wrapped in the style.
        assert_eq!(shown("\\dfrac12"), "Display[frac[{1} / {2}]]");
        assert_eq!(shown("\\tbinom12"), "Text[frac0<( )>[{1} / {2}]]");
        assert_eq!(shown("\\frac12^3"), "(frac[{1} / {2}] ^3)");
    }

    #[test]
    fn infix_commands_split_the_expression() {
        // Everything before is the numerator, everything after the denominator.
        assert_eq!(shown("a+b \\over c"), "frac[{a + b} / {c}]");
        assert_eq!(shown("a \\atop b"), "frac0[{a} / {b}]");
        assert_eq!(shown("a \\choose b"), "frac0<( )>[{a} / {b}]");
        assert_eq!(shown("a \\brace b"), "frac0<\\{ \\}>[{a} / {b}]");
        assert_eq!(shown("a \\brack b"), "frac0<[ ]>[{a} / {b}]");
        // A half that is one group is kept as it is.
        assert_eq!(shown("{a}\\over{b}"), "frac[{a} / {b}]");
        assert_eq!(shown("\\over"), "frac[{} / {}]");
        // Groups, arguments and environments each split their own.
        assert_eq!(shown("x {a \\over b} y"), "x {frac[{a} / {b}]} y");
        assert_eq!(shown("\\sqrt{a \\over b}"), "sqrt{frac[{a} / {b}]}");
        assert_eq!(
            shown("\\displaystyle a \\over b"),
            "frac[{Display[a]} / {b}]"
        );
        assert_eq!(
            error("a \\over b \\over c"),
            "only one infix operator per group"
        );
        assert_eq!(
            error("x^\\over"),
            "Got function '\\over' with no arguments as superscript"
        );
    }

    #[test]
    fn above_sets_the_bar() {
        let nodes = parse("a \\above 2pt b").unwrap();
        let NodeKind::Frac { bar, bar_size, .. } = &nodes[0].kind else {
            unreachable!("a fraction");
        };
        assert!(bar);
        assert_eq!(
            bar_size.as_ref().map(|m| (m.number, m.unit.as_str())),
            Some((2.0, "pt"))
        );
        let nodes = parse("a \\above 0pt b").unwrap();
        let NodeKind::Frac { bar, .. } = &nodes[0].kind else {
            unreachable!("a fraction");
        };
        assert!(!bar);
        assert_eq!(error("a \\above b"), "Invalid size: 'b'");
    }

    #[test]
    fn enclosures() {
        // A box argument is text, in the text style.
        assert_eq!(shown("\\fbox{a}"), "\\fbox[|]<Text[{a}]>");
        assert_eq!(shown("\\colorbox{red}{a}"), "\\colorbox[|red]<Text[{a}]>");
        assert_eq!(
            shown("\\fcolorbox{red}{#00f}{a}"),
            "\\fcolorbox[red|#00f]<Text[{a}]>"
        );
        // The lines take the body as it is.
        assert_eq!(shown("\\cancel{x}"), "\\cancel[|]<{x}>");
        assert_eq!(shown("\\bcancel x"), "\\bcancel[|]<{x}>");
        assert_eq!(shown("\\sout{x}"), "\\sout[|]<{x}>");
        assert_eq!(shown("\\phase{x}"), "\\phase[|]<{x}>");
        assert_eq!(shown("\\angl{x}"), "\\angl[|]<Text[{x}]>");
        assert_eq!(
            error("\\text{\\angl{x}}"),
            "Can't use function '\\angl' in text mode"
        );
        assert_eq!(error("\\colorbox{nope!}{a}"), "Invalid color: 'nope!'");
    }

    #[test]
    fn math_inside_text() {
        assert_eq!(shown("\\text{a $x$ b}"), "\\text[a   Text[x]   b]");
        assert_eq!(shown("\\text{\\(x\\)}"), "\\text[Text[x]]");
        assert_eq!(error("\\text{$x}"), "Expected '$', got 'EOF'");
        assert_eq!(error("$x$"), "Can't use function '$' in math mode");
        assert_eq!(error("\\text{x \\)}"), "Mismatched \\)");
        assert_eq!(error("\\]"), "Can't use function '\\]' in math mode");
    }

    #[test]
    fn verb_is_text_as_written() {
        assert_eq!(shown("\\verb|a b|"), "verb<a b>");
        assert_eq!(shown("\\verb*+x_y+"), "verb*<x_y>");
        assert_eq!(shown("\\text{\\verb!x!}"), "\\text[verb<x>]");
        assert_eq!(parse("x\\verb|a|").unwrap()[1].mode, Mode::Text);
    }

    #[test]
    fn characters_from_numbers() {
        assert_eq!(shown("\\@char{65}"), "char<A>");
        // As `parseInt` reads it: up to the first character that is not a digit. Letters are
        // characters only in text mode.
        assert_eq!(shown("\\text{\\@char{0x41}}"), "\\text[char<A>]");
        assert_eq!(shown("\\text{\\@char{66z}}"), "\\text[char<B>]");
        assert_eq!(
            error("\\text{\\@char{z}}"),
            "\\@char has non-numeric argument z"
        );
        assert_eq!(error("\\@char{z}"), "\\@char has non-numeric argument");
        assert_eq!(
            error("\\@char{\\text{2}}"),
            "\\@char has non-numeric argument"
        );
        assert_eq!(
            error("\\@char{1114112}"),
            "\\@char with invalid code point 1114112"
        );
        assert_eq!(
            error("\\@char{55296}"),
            "\\@char with invalid code point 55296"
        );
    }

    #[test]
    fn links_and_html_are_left_out_as_untrusted() {
        // KaTeX with no trust shows the command's name in the error color.
        let shown_name = |src: &str| match &parse(src).unwrap()[0].kind {
            NodeKind::Color { color, body } => {
                let NodeKind::Text { body, font } = &body[0].kind else {
                    unreachable!("text");
                };
                assert_eq!(*font, None);
                format!(
                    "{color} {}",
                    body.iter().map(show).collect::<Vec<_>>().join("")
                )
            }
            other => unreachable!("a left out command, not {other:?}"),
        };
        assert_eq!(shown_name("\\href{http://a.b/}{x}"), "#cc0000 \\href");
        assert_eq!(shown_name("\\url{http://a.b/}"), "#cc0000 \\url");
        assert_eq!(shown_name("\\htmlId{a}{x}"), "#cc0000 \\htmlId");
        assert_eq!(
            shown_name("\\includegraphics{a.png}"),
            "#cc0000 \\includegraphics"
        );
        // A `%` in the argument is a character.
        assert_eq!(shown("\\url{a%b} c"), shown("\\url{ab} c"));
        // Checks KaTeX makes first.
        assert_eq!(
            error("\\htmlData{a}{x}"),
            "\\htmlData key/value 'a' missing equals sign"
        );
        assert!(parse("\\htmlData{a=1,b=2{,}3}{x}").is_ok());
        assert_eq!(
            error("\\includegraphics[depth=1em]{a.png}"),
            "Invalid key: 'depth' in \\includegraphics."
        );
        assert_eq!(
            error("\\includegraphics[width=wide]{a.png}"),
            "Invalid size: 'wide' in \\includegraphics"
        );
        assert_eq!(
            error("\\includegraphics[height=1zz]{a.png}"),
            "Invalid unit: 'zz' in \\includegraphics."
        );
        assert!(parse("\\includegraphics[width=2, height=1.5em, alt=x]{a.png}").is_ok());
        assert_eq!(shown("\\html@mathml{a}{b}"), "htmlmathml[a | b]");
    }

    #[test]
    fn one_body_on_another() {
        assert_eq!(shown("\\overset{a}{=}"), "Rel[(opL[=] ^{a})]");
        assert_eq!(shown("\\underset{a}{x}"), "Ord[(opL[x] _{a})]");
        assert_eq!(shown("\\stackrel{a}{x}"), "Rel[(opL[x] ^{a})]");
    }

    #[test]
    fn stretchy_arrows_and_braces() {
        assert_eq!(shown("\\xrightarrow{a}"), "\\xrightarrow<{a}>");
        assert_eq!(shown("\\xrightarrow[b]{a}"), "\\xrightarrow[{b}]<{a}>");
        assert_eq!(shown("\\xleftarrow x"), "\\xleftarrow<{x}>");
        assert_eq!(shown("\\overbrace{x}^2"), "(\\overbrace<{x}> ^2)");
        assert_eq!(shown("\\underbracket x"), "\\underbracket<{x}>");
        let nodes = parse("\\underbrace{x}").unwrap();
        let NodeKind::HorizBrace { is_over, .. } = &nodes[0].kind else {
            unreachable!("a brace");
        };
        assert!(!is_over);
        assert_eq!(
            error("\\xrightarrow"),
            "Unexpected end of input in a macro argument, expected '}'"
        );
    }

    #[test]
    fn boxes_and_choices() {
        assert_eq!(shown("\\pmb{x}"), "pmbOrd[x]");
        assert_eq!(shown("\\pmb =").as_str(), "pmbRel[=]");
        assert_eq!(shown("\\hbox{a b}"), "hbox[a   b]");
        assert_eq!(shown("\\vcenter{a}"), "vcenter<{a}>");
        assert_eq!(shown("\\mathchoice{a}{b}{c}{d}"), "choice[a | b | c | d]");
        assert_eq!(shown("\\mathchoice{a b}{}x{d}"), "choice[a b |  | x | d]");
        assert_eq!(shown("\\reflectbox{a}"), "reflect<Text[{a}]>");
        assert_eq!(shown("\\mathreflectbox{a}"), "reflect<{a}>");
    }

    #[test]
    fn genfrac_takes_delimiters_bar_and_style() {
        assert_eq!(
            shown("\\genfrac ( ] {0.06em}{0}{a}{b}"),
            "Display[frac<( ]>[{a} / {b}]]"
        );
        // An empty bar size is the usual bar, and 0 is none.
        assert_eq!(shown("\\genfrac{}{}{}{}{a}{b}"), "frac[{a} / {b}]");
        assert_eq!(
            shown("\\genfrac{}{}{0pt}{3}{a}{b}"),
            "ScriptScript[frac0[{a} / {b}]]"
        );
        // Only an open delimiter on the left and a close one on the right count.
        assert_eq!(shown("\\genfrac ] ( {}{}{a}{b}"), "frac[{a} / {b}]");
        // A style that is not 0 to 3 leaves the fraction in the style it stands in.
        assert_eq!(shown("\\genfrac{}{}{}{9}{a}{b}"), "frac[{a} / {b}]");
        assert_eq!(
            error("\\genfrac{}{}{}{~}{a}{b}"),
            "Expected node of type textord, but got node of type spacing"
        );
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

    /// The deepest nesting the limits allow, braces and functions together, fits in the 2 MiB of a
    /// test thread in a debug build (an optimized build needs a fraction of that, so a 1 MiB
    /// WebAssembly stack is enough).
    #[test]
    fn the_deepest_allowed_nesting_fits_a_small_stack() {
        let run = |source: String| {
            std::thread::Builder::new()
                .stack_size(2 * 1024 * 1024)
                .spawn(move || {
                    parse(&source)
                        .map(|nodes| nodes.len())
                        .map_err(|e| e.message)
                })
                .map(|handle| handle.join())
        };
        // Braces and `\left` together reach both limits.
        let mixed = format!("{}x{}", "\\left({".repeat(60), "}\\right)".repeat(60));
        assert!(matches!(run(mixed), Ok(Ok(_))));
        for source in [
            "{".repeat(100),
            "\\left(".repeat(100),
            "\\displaystyle ".repeat(100),
            "\\bf ".repeat(100),
            "\\color{red}".repeat(100),
            "\\mathbf{".repeat(100),
            "\\frac{".repeat(100),
            "x^{".repeat(100),
        ] {
            assert!(matches!(run(source), Ok(Ok(_))));
        }
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
    fn operators() {
        assert_eq!(shown("\\sum_{i}^n"), "(opL\\sum ^n _{i})");
        assert_eq!(shown("\\int_0^1"), "(op\\int ^1 _0)");
        assert_eq!(shown("\\lim_{x}"), "(opL\\lim _{x})");
        assert_eq!(shown("\\sin x"), "op\\sin x");
        // The Unicode operator is the command.
        assert_eq!(shown("\u{2211}"), "opL\\sum");
        assert_eq!(shown("\\mathop{ab}"), "op[a b]");
        assert_eq!(shown("\\operatorname{foo}"), "opname[f o o]");
        assert_eq!(shown("\\operatorname*{foo}"), "opname[f o o]");
    }

    #[test]
    fn limit_controls_follow_an_operator() {
        assert_eq!(shown("\\int\\limits_0^1"), "(opL\\int ^1 _0)");
        assert_eq!(shown("\\sum\\nolimits_0"), "(op\\sum _0)");
        assert_eq!(shown("\\mathop{x}\\limits^2"), "(opL[x] ^2)");
        assert_eq!(
            error("x\\limits"),
            "Limit controls must follow a math operator"
        );
        assert_eq!(
            error("\\limits"),
            "Limit controls must follow a math operator"
        );
        // A plain `\operatorname` ignores the control; the starred one takes it.
        assert_eq!(shown("\\operatorname{f}\\limits"), "opname[f]");
        assert_eq!(shown("\\operatorname*{f}\\limits"), "opnameL[f]");
    }

    #[test]
    fn kerns_and_rules_read_sizes() {
        assert_eq!(shown("\\kern1em"), "kern1em");
        assert_eq!(shown("\\kern -0.5 em x"), "kern-0.5em x");
        assert_eq!(shown("\\mkern{3mu}"), "kern3mu");
        assert_eq!(shown("\\hskip 2pt"), "kern2pt");
        assert_eq!(shown("\\rule{1em}{2pt}"), "rule1emx2pt");
        assert_eq!(shown("\\rule[3pt]{1em}{2pt}"), "rule1emx2pt");
        assert_eq!(error("\\kern x"), "Invalid size: 'x'");
        assert_eq!(error("\\kern{3}"), "Invalid size: '3'");
        assert_eq!(error("\\kern{3zz}"), "Invalid unit: 'zz'");
        assert_eq!(error("\\kern"), "Invalid size: 'EOF'");
        // A run of digits too long to be a size is not read to its end.
        let long = format!("\\kern{}1em", "0".repeat(100));
        assert!(parse(&long).is_err());
    }

    #[test]
    fn colors_take_names_and_hex() {
        assert_eq!(shown("\\textcolor{red}{x y}"), "red[x y]");
        assert_eq!(shown("\\textcolor{ff0000}{x}"), "#ff0000[x]");
        assert_eq!(shown("a\\color{#00f}b c"), "a #00f[b c]");
        assert_eq!(shown("{\\color{blue}b}c"), "{blue[b]} c");
        assert_eq!(error("\\color{no way}"), "Invalid color: 'no way'");
        assert_eq!(error("\\textcolor{1}{x}"), "Invalid color: '1'");
    }

    #[test]
    fn phantoms_laps_and_boxes() {
        assert_eq!(shown("\\phantom{ab}"), "phantom[a b]");
        assert_eq!(shown("\\vphantom{a}"), "vphantom<{a}>");
        assert_eq!(shown("\\smash{a}"), "smashtb<{a}>");
        assert_eq!(shown("\\smash[t]{a}"), "smasht<{a}>");
        assert_eq!(shown("\\smash[b]{a}"), "smashb<{a}>");
        assert_eq!(shown("\\smash[tb]{a}"), "smashtb<{a}>");
        assert_eq!(shown("\\smash[x]{a}"), "smash<{a}>");
        assert_eq!(shown("\\mathrlap{a}"), "rlap<{a}>");
        assert_eq!(shown("\\mathclap{a}"), "clap<{a}>");
        assert_eq!(shown("\\raisebox{2pt}{ab}"), "raise2pt<Text[{a b}]>");
    }

    #[test]
    fn sizing_and_relax() {
        assert_eq!(shown("a\\Huge b c"), "a size11[b c]");
        assert_eq!(shown("\\tiny x"), "size1[x]");
        // `\relax` is not in the tree, and a script after it has no base.
        assert_eq!(shown("a\\relax b"), "a b");
        assert_eq!(shown("\\relax^2"), "( ^2)");
    }

    fn shown_in_display(src: &str) -> String {
        parse_with(src, Settings { display_mode: true })
            .unwrap()
            .iter()
            .map(show)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn display_error(src: &str) -> String {
        parse_with(src, Settings { display_mode: true })
            .unwrap_err()
            .message
    }

    #[test]
    fn tables() {
        assert_eq!(
            shown("\\begin{array}{cc} a & b \\\\ c & d \\end{array}"),
            "array[Text[{a}] & Text[{b}] \\\\ Text[{c}] & Text[{d}]]"
        );
        // A last empty row, from a `\\` before `\end`, is dropped.
        assert_eq!(
            shown("\\begin{matrix} a \\\\ \\end{matrix}"),
            "array[Text[{a}]]"
        );
        assert_eq!(shown("\\begin{matrix}\\end{matrix}"), "array[]");
        assert_eq!(
            shown("\\begin{pmatrix} a & b \\end{pmatrix}"),
            "left([array[Text[{a}] & Text[{b}]]]right)"
        );
        assert_eq!(
            shown("\\begin{cases} a & b \\\\ c & d \\end{cases}"),
            "left\\{[array[Text[{a}] & Text[{b}] \\\\ Text[{c}] & Text[{d}]]]right."
        );
        assert_eq!(
            shown("\\begin{rcases} a \\end{rcases}"),
            "left.[array[Text[{a}]]]right\\}"
        );
        assert_eq!(
            shown("\\begin{smallmatrix} a \\end{smallmatrix}"),
            "array[Script[{a}]]"
        );
        assert_eq!(
            shown("\\begin{dcases} a \\end{dcases}"),
            "left\\{[array[Display[{a}]]]right."
        );
    }

    #[test]
    fn table_rows_take_gaps_rules_and_the_cr_macro() {
        let src = "\\begin{array}{c} \\hline a \\\\[2pt] \\hdashline b \\cr c \\end{array}";
        let nodes = parse(src).unwrap();
        let NodeKind::Array(array) = &nodes[0].kind else {
            unreachable!("a table");
        };
        assert_eq!(array.body.len(), 3);
        assert_eq!(
            array.hlines_before_row,
            [vec![false], vec![true], vec![], vec![]]
        );
        let gap = |i: usize| {
            array.row_gaps[i]
                .as_ref()
                .map(|m| (m.number, m.unit.as_str()))
        };
        assert_eq!(array.row_gaps.len(), 2);
        assert_eq!(gap(0), Some((2.0, "pt")));
        assert_eq!(gap(1), None);
        assert_eq!(array.arraystretch, 1.0);
    }

    #[test]
    fn the_array_stretch_comes_from_the_macro() {
        let nodes =
            parse("\\newcommand{\\arraystretch}{1.5}\\begin{array}{c}a\\end{array}").unwrap();
        let NodeKind::Array(array) = &nodes[0].kind else {
            unreachable!("a table");
        };
        assert_eq!(array.arraystretch, 1.5);
        assert_eq!(
            error("\\newcommand{\\arraystretch}{x}\\begin{array}{c}a\\end{array}"),
            "Invalid \\arraystretch: x"
        );
    }

    #[test]
    fn aligned_environments() {
        assert_eq!(
            shown("\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}"),
            "array[Display[{a}] & Display[{{} = b}] \\\\ Display[{c}] & Display[{{} = d}]]"
        );
        assert_eq!(
            error("\\begin{align} a \\end{align}"),
            "{align} can be used only in display mode."
        );
        assert!(shown_in_display("\\begin{align} a \\end{align}").starts_with("array["));
        // The pairs of cells are counted as halves, as KaTeX counts them.
        assert_eq!(
            error("\\begin{alignedat}{1} a & b & c & d & e \\end{alignedat}"),
            "Too many math in a row: expected 1, but got 2.5"
        );
        assert_eq!(
            error("\\begin{alignedat}{x} a \\end{alignedat}"),
            "Number of columns should be a positive integer"
        );
        // A column count too large to build the columns for is refused, not allocated.
        assert_eq!(
            error("\\begin{alignedat}{99999999999999999999} a \\end{alignedat}"),
            "Number of columns should be a positive integer"
        );
        assert_eq!(
            error("\\begin{alignedat}{1000000} a \\end{alignedat}"),
            "Number of columns should be at most 256"
        );
        assert_eq!(
            error("\\begin{split} a \\end{split}"),
            "{split} can be used only in display mode."
        );
        assert_eq!(
            display_error("\\begin{split} a & b & c \\end{split}"),
            "Too many tab characters: &"
        );
    }

    #[test]
    fn equation_tags() {
        assert_eq!(shown_in_display("x \\tag{1}"), "tag[\\text[( {1} )]](x)");
        assert_eq!(error("x \\tag{1}"), "\\tag works only in display equations");
        assert_eq!(display_error("x \\tag{1} \\tag{2}"), "Multiple \\tag");
        let nodes = parse_with(
            "\\begin{align} a \\tag{x} \\\\ b \\nonumber \\\\ c \\end{align}",
            Settings { display_mode: true },
        )
        .unwrap();
        let NodeKind::Array(array) = &nodes[0].kind else {
            unreachable!("a table");
        };
        let tags: Vec<_> = array
            .tags
            .iter()
            .flatten()
            .map(|tag| match tag {
                crate::math::ast::RowTag::Numbered(numbered) => numbered.to_string(),
                crate::math::ast::RowTag::Custom(_) => "custom".to_string(),
            })
            .collect();
        assert_eq!(tags, ["custom", "false", "true"]);
    }

    /// Long inputs of the shapes that could cost more than their size (each is under the source
    /// limit) are read in one pass.
    #[test]
    fn long_runs_are_read_in_linear_time() {
        // Twenty thousand `---` ligatures in one text.
        let dashes = format!("\\text{{{}}}", "-".repeat(60_000));
        let nodes = parse(&dashes).unwrap();
        let NodeKind::Text { body, .. } = &nodes[0].kind else {
            unreachable!("text");
        };
        assert_eq!(body.len(), 20_000);
        // Many cells and many rows.
        let cells = format!("\\begin{{matrix}}{}\\end{{matrix}}", "a&".repeat(15_000));
        assert!(parse(&cells).is_ok());
        let rows = format!("\\begin{{matrix}}{}\\end{{matrix}}", "a\\\\".repeat(15_000));
        assert!(parse(&rows).is_ok());
        // A long run of scripts and of groups.
        assert!(parse(&"x^a_b".repeat(10_000)).is_ok());
        assert!(parse(&"{}".repeat(30_000)).is_ok());
    }

    #[test]
    fn line_breaks_outside_tables() {
        assert_eq!(shown("a \\\\ b"), "a cr b");
        assert_eq!(shown("a \\\\[1em] b"), "a cr1em b");
    }

    #[test]
    fn environments_read_their_names_and_ends() {
        assert_eq!(error("\\begin{foo}x\\end{foo}"), "No such environment: foo");
        assert_eq!(
            error("\\begin{matrix} a \\end{pmatrix}"),
            "Mismatch: \\begin{matrix} matched by \\end{pmatrix}"
        );
        assert_eq!(
            error("\\begin{matrix} a"),
            "Expected & or \\\\ or \\cr or \\end"
        );
        assert_eq!(error("\\end{matrix}"), "Expected 'EOF', got '\\end'");
        assert_eq!(
            error("\\hline"),
            "\\hline valid only within array environment"
        );
        assert_eq!(
            error("\\begin{array}{x} a \\end{array}"),
            "Unknown column alignment: x"
        );
        assert_eq!(
            error("\\begin{matrix*}[x] a \\end{matrix*}"),
            "Expected l or c or r"
        );
    }

    #[test]
    fn what_the_parser_lacks_is_said_so() {
        assert!(error("\\begin{CD}a\\end{CD}").ends_with("supported yet"));
        assert_eq!(error("\\bra@ket"), "\\bra@ket is not supported yet");
        assert!(error("x\u{2082}").ends_with("supported yet"));
    }

    #[test]
    fn nesting_is_bounded() {
        let deep = format!("{}{}", "{".repeat(10_000), "}".repeat(10_000));
        assert!(error(&deep).starts_with("groups nested more than"));
    }
}
