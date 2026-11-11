//! Parser for Mermaid railroad diagrams in each of their four notations, producing a lossless
//! rowan tree (MRAIL-1).
//!
//! Unlike the line-oriented parsers, a rule may span lines: whitespace, newlines, and comments
//! are allowed between any two tokens, and `;` ends a rule.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use super::Notation;
use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, MAX_DEPTH, too_deep};
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &[
    "railroad-beta",
    "railroad-ebnf-beta",
    "railroad-abnf-beta",
    "railroad-peg-beta",
];

/// The IR's constructors.
pub(crate) const CONSTRUCTORS: &[&str] = &[
    "terminal",
    "nonterminal",
    "special",
    "sequence",
    "choice",
    "optional",
    "zeroOrMore",
    "oneOrMore",
];

pub(crate) fn parse(src: &str, notation: Notation) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
        notation,
        failed: false,
        depth: 0,
    };
    p.start(ROOT);
    p.header(HEADERS, "railroad-beta", header_len);
    if !p.diagnostics.is_empty() {
        // Without the header, the rest is not read.
        let len = p.rest().len();
        p.start(ERROR);
        p.token(ERROR_TOKEN, len);
        p.finish();
    }
    loop {
        p.trivia(true);
        if p.at_eof() {
            break;
        }
        p.statement();
    }
    p.finish();
    p.base.finish_tree()
}

fn header_len(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(s.len())
}

/// The length of an identifier at the start of `s`: `[A-Za-z][A-Za-z0-9-]*` in ABNF, and
/// `[A-Za-z_][A-Za-z0-9_-]*` otherwise.
fn ident_len(s: &str, notation: Notation) -> usize {
    let abnf = notation == Notation::Abnf;
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() || (c == '_' && !abnf) => {}
        _ => return 0,
    }
    chars
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '-' || (c == '_' && !abnf)))
        .map_or(s.len(), |(i, _)| i)
}

/// The length of a string at the start of `s`, which may span lines: `"..."` in ABNF, and
/// `"..."` or `'...'` with `\` escapes otherwise. `Err` with the length to the end of the line
/// when it does not close.
fn string_len(s: &str, notation: Notation) -> Option<Result<usize, usize>> {
    let q = s
        .chars()
        .next()
        .filter(|&c| c == '"' || (c == '\'' && notation != Notation::Abnf))?;
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if c == '\\' && notation != Notation::Abnf {
            escaped = true;
        } else if c == q {
            return Some(Ok(i + 1));
        }
    }
    Some(Err(scan::line_len(s)))
}

/// The length of an ABNF number such as `%x41`, `%d65`, `%b1000001`, `%x30-39`, or `%x0D.0A`.
fn numval_len(s: &str) -> usize {
    let Some(body) = s.strip_prefix('%') else {
        return 0;
    };
    if !body.starts_with(['x', 'X', 'd', 'D', 'b', 'B']) {
        return 0;
    }
    let hex = |s: &str| s.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(s.len());
    let mut n = 2;
    let first = hex(&s[n..]);
    if first == 0 {
        return 0;
    }
    n += first;
    while s[n..].starts_with(['-', '.']) {
        let more = hex(&s[n + 1..]);
        if more == 0 {
            break;
        }
        n += 1 + more;
    }
    n
}

/// The length of an ABNF repeat such as `*`, `1*`, `2*4`, or `3` before an element.
fn repeat_len(s: &str) -> usize {
    let digits = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let lo = digits(s);
    if s[lo..].starts_with('*') {
        lo + 1 + digits(&s[lo + 1..])
    } else {
        lo
    }
}

/// The length of an EBNF special sequence `? text ?` at the start of `s`: text without `?` or
/// `;` that is not blank.
fn special_len(s: &str) -> Option<usize> {
    let body = s.strip_prefix('?')?;
    let end = body.find(['?', ';'])?;
    (body[end..].starts_with('?') && !body[..end].trim().is_empty()).then_some(end + 2)
}

struct Parser<'a> {
    base: Base<'a>,
    notation: Notation,
    /// Whether the current rule has failed; the rest of it is skipped.
    failed: bool,
    /// How many groups or constructors are open around the cursor (LANGS-8).
    depth: usize,
}

impl<'a> Deref for Parser<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.base
    }
}

impl DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl<'a> Parser<'a> {
    /// Whitespace, newlines, comments, and directives. `rule_start` is whether a rule may
    /// start here, where an ABNF `;` always starts a comment.
    fn trivia(&mut self, rule_start: bool) {
        loop {
            let r = self.rest();
            let ws = r
                .find(|c: char| !matches!(c, ' ' | '\t' | '\r' | '\n'))
                .unwrap_or(r.len());
            let line = scan::line_len(r);
            if ws > 0 {
                let nl = r[..ws].contains('\n');
                self.token(if nl { NEWLINE } else { WHITESPACE }, ws);
            } else if let Some(body) = r.strip_prefix("%%{") {
                match body.find("}%%") {
                    Some(i) => self.token(DIRECTIVE, i + 6),
                    None => {
                        let start = self.pos;
                        self.token(DIRECTIVE, line);
                        self.error(start, start + 3, "unclosed directive: expected `}%%`");
                    }
                }
            } else if r.starts_with("%%") {
                self.token(COMMENT, line);
            } else if let Some(n) = self.comment_len(r, rule_start) {
                self.token(COMMENT, n);
            } else {
                return;
            }
        }
    }

    /// The length of a comment in this notation at the start of `r`.
    fn comment_len(&mut self, r: &str, rule_start: bool) -> Option<usize> {
        let line = scan::line_len(r);
        let block = |open: &str, close: &str| -> Option<Result<usize, ()>> {
            let body = r.strip_prefix(open)?;
            Some(
                body.find(close)
                    .map(|i| open.len() + i + close.len())
                    .ok_or(()),
            )
        };
        let found = match self.notation {
            Notation::Ir => block("/*", "*/"),
            Notation::Ebnf => block("/*", "*/").or_else(|| block("(*", "*)")),
            Notation::Peg => r.starts_with('#').then_some(Ok(line)),
            Notation::Abnf => {
                // A `;` with text after it on its line starts a comment, as does any `;` where
                // a rule may start; otherwise it ends a rule.
                let text_after = r.get(1..line).is_some_and(|t| !t.trim().is_empty());
                (r.starts_with(';') && (rule_start || text_after)).then_some(Ok(line))
            }
        };
        match found? {
            Ok(n) => Some(n),
            Err(()) => {
                let start = self.pos;
                let close = if r.starts_with("(*") { "*)" } else { "*/" };
                self.error(
                    start,
                    start + 2,
                    format!("unclosed comment: expected `{close}`"),
                );
                Some(r.len())
            }
        }
    }

    fn statement(&mut self) {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        if line == "title" || line.starts_with("title ") || line.starts_with("title\t") {
            self.title();
            return;
        }
        let after = |kw: &str| line.strip_prefix(kw).map(str::trim_start);
        if after("accTitle").is_some_and(|t| t.starts_with(':'))
            || after("accDescr").is_some_and(|t| t.starts_with([':', '{']))
        {
            self.acc();
            return;
        }
        self.rule();
    }

    /// `title text`, up to a `%%` comment.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let end = line.find("%%").unwrap_or(line.len());
        let n = scan::trim_end_len(r, end);
        self.token(TEXT, n);
        self.finish();
    }

    /// `name = definition ;`, with the notation's assignment operator.
    fn rule(&mut self) {
        self.failed = false;
        self.start(RULE);
        let n = ident_len(self.rest(), self.notation);
        if n == 0 {
            self.fail_here("expected a rule name");
        } else {
            self.token(IDENT, n);
            self.trivia(false);
            let r = self.rest();
            let op = match self.notation {
                Notation::Peg => r.starts_with("<-").then_some(2),
                Notation::Ebnf if r.starts_with("::=") => Some(3),
                _ => r.starts_with('=').then_some(1),
            };
            match op {
                Some(n) => {
                    self.token(EQ, n);
                    self.trivia(false);
                    if self.notation == Notation::Ir {
                        self.call();
                    } else {
                        self.choice();
                    }
                    if !self.failed {
                        self.trivia(false);
                        if self.rest().starts_with(';') {
                            self.token(SEMI, 1);
                        } else if self.notation == Notation::Abnf {
                            self.fail_here(
                                "expected `;` to end the rule (a `;` with text after it on its \
                                 line starts a comment)",
                            );
                        } else {
                            self.fail_here("expected `;` to end the rule");
                        }
                    }
                }
                None => {
                    let what = match self.notation {
                        Notation::Peg => "`<-`",
                        Notation::Ebnf => "`=` or `::=`",
                        _ => "`=`",
                    };
                    self.fail_here(&format!("expected {what} after the rule name"));
                }
            }
        }
        if self.failed {
            // Skip to the end of the rule.
            let r = self.rest();
            let n = r.find(';').unwrap_or(r.len());
            if n > 0 {
                self.start(ERROR);
                self.token(ERROR_TOKEN, n);
                self.finish();
            }
            let semi = usize::from(self.rest().starts_with(';'));
            self.token(SEMI, semi);
        }
        self.finish();
    }

    /// Reports `message` at the next token and fails the rule.
    fn fail_here(&mut self, message: &str) {
        let start = self.pos;
        let r = self.rest();
        let n = match r.chars().next() {
            None => 0,
            Some(c) if c.is_alphanumeric() || c == '_' => r
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .unwrap_or(r.len()),
            Some(c) => c.len_utf8(),
        };
        self.error(start, start + n, message);
        self.failed = true;
    }

    /// The alternatives operator in this notation.
    fn alt(&self) -> char {
        match self.notation {
            Notation::Ebnf => '|',
            _ => '/',
        }
    }

    /// Sequences separated by `|` or `/`.
    fn choice(&mut self) {
        self.start(RR_CHOICE);
        self.seq();
        while !self.failed {
            self.trivia(false);
            if !self.rest().starts_with(self.alt()) {
                break;
            }
            self.token(RR_OP, 1);
            self.trivia(false);
            self.seq();
        }
        self.finish();
    }

    /// Elements one after another, with optional commas between them in EBNF.
    fn seq(&mut self) {
        self.start(RR_SEQ);
        self.term();
        while !self.failed {
            self.trivia(false);
            if self.notation == Notation::Ebnf && self.rest().starts_with(',') {
                self.token(COMMA, 1);
                self.trivia(false);
                self.term();
            } else if self.at_term() {
                self.term();
            } else {
                break;
            }
        }
        self.finish();
    }

    /// Whether an element may start here.
    fn at_term(&self) -> bool {
        let r = self.rest();
        let n = self.notation;
        ident_len(r, n) > 0
            || string_len(r, n).is_some()
            || match n {
                Notation::Ebnf => r.starts_with(['(', '[', '{']) || special_len(r).is_some(),
                Notation::Abnf => {
                    r.starts_with(['(', '[']) || numval_len(r) > 0 || repeat_len(r) > 0
                }
                Notation::Peg => r.starts_with(['(', '.', '&', '!']),
                Notation::Ir => false,
            }
    }

    /// An element with its notation's operators: an ABNF repeat or PEG predicate before it,
    /// and EBNF or PEG operators after it.
    fn term(&mut self) {
        self.start(RR_TERM);
        match self.notation {
            Notation::Abnf => {
                let n = repeat_len(self.rest());
                if n > 0 {
                    self.token(RR_REPEAT, n);
                    self.trivia(false);
                }
            }
            Notation::Peg if self.rest().starts_with(['&', '!']) => {
                self.token(RR_OP, 1);
                self.trivia(false);
            }
            _ => {}
        }
        if self.primary() {
            self.postfix();
        }
        self.finish();
    }

    /// EBNF's `?`, `*`, `+`, and `- except`, any number of times, or one of PEG's `?`, `*`,
    /// and `+`.
    fn postfix(&mut self) {
        let mut count = 0;
        while !self.failed {
            let before = self.pos;
            let spaced = {
                let r = self.rest();
                r.starts_with(|c: char| c.is_whitespace())
                    || r.starts_with("%%")
                    || self.comment_len_peek(r)
            };
            let r = &self.src[before..];
            // Look past trivia without consuming it unless an operator follows.
            let skip = self.trivia_len(r);
            let after = &r[skip..];
            let op = match self.notation {
                Notation::Ebnf => {
                    if after.starts_with('?') && spaced && special_len(after).is_some() {
                        // `a ? text ?` is `a` then a special sequence.
                        None
                    } else if after.starts_with(['?', '*', '+', '-']) {
                        after.chars().next()
                    } else {
                        None
                    }
                }
                Notation::Peg if count == 0 => after
                    .starts_with(['?', '*', '+'])
                    .then(|| after.chars().next())
                    .flatten(),
                _ => None,
            };
            let Some(op) = op else { return };
            self.trivia(false);
            self.token(RR_OP, 1);
            count += 1;
            if op == '-' {
                self.trivia(false);
                if !self.primary() {
                    return;
                }
            }
        }
    }

    fn comment_len_peek(&self, r: &str) -> bool {
        match self.notation {
            Notation::Ir => r.starts_with("/*"),
            Notation::Ebnf => r.starts_with("/*") || r.starts_with("(*"),
            Notation::Peg => r.starts_with('#'),
            Notation::Abnf => r.starts_with(';'),
        }
    }

    /// The length of the trivia at the start of `r`, without consuming it.
    fn trivia_len(&self, r: &str) -> usize {
        let mut n = 0;
        loop {
            let s = &r[n..];
            let ws = s.find(|c: char| !c.is_whitespace()).unwrap_or(s.len());
            if ws > 0 {
                n += ws;
                continue;
            }
            let line = scan::line_len(s);
            let close = |open: &str, close: &str| {
                s.strip_prefix(open).map(|b| {
                    b.find(close)
                        .map_or(s.len(), |i| open.len() + i + close.len())
                })
            };
            let more = if let Some(b) = s.strip_prefix("%%{") {
                Some(b.find("}%%").map_or(line, |i| i + 6))
            } else if s.starts_with("%%") {
                Some(line)
            } else {
                match self.notation {
                    Notation::Ir => close("/*", "*/"),
                    Notation::Ebnf => close("/*", "*/").or_else(|| close("(*", "*)")),
                    Notation::Peg => s.starts_with('#').then_some(line),
                    Notation::Abnf => {
                        let text_after = s.get(1..line).is_some_and(|t| !t.trim().is_empty());
                        (s.starts_with(';') && text_after).then_some(line)
                    }
                }
            };
            match more {
                Some(m) if m > 0 => n += m,
                _ => return n,
            }
        }
    }

    /// A string, name, or bracketed group, and the notation's other elements. Returns whether
    /// one was read; otherwise the rule fails.
    fn primary(&mut self) -> bool {
        let r = self.rest();
        let notation = self.notation;
        if let Some(len) = string_len(r, notation) {
            return match len {
                Ok(n) => {
                    self.token(STRING, n);
                    true
                }
                Err(_) => {
                    self.fail_here("unclosed string: expected a closing quote");
                    false
                }
            };
        }
        let n = ident_len(r, notation);
        if n > 0 {
            self.token(IDENT, n);
            return true;
        }
        match notation {
            Notation::Abnf if numval_len(r) > 0 => {
                self.token(RR_NUMVAL, numval_len(r));
                return true;
            }
            Notation::Ebnf => {
                if let Some(n) = special_len(r) {
                    self.token(RR_SPECIAL, n);
                    return true;
                }
            }
            Notation::Peg if r.starts_with('.') => {
                self.token(DOT, 1);
                return true;
            }
            _ => {}
        }
        let close = match r.chars().next() {
            Some('(') => Some((L_PAREN, ')', R_PAREN)),
            Some('[') if matches!(notation, Notation::Ebnf | Notation::Abnf) => {
                Some((L_BRACKET, ']', R_BRACKET))
            }
            Some('{') if notation == Notation::Ebnf => Some((L_BRACE, '}', R_BRACE)),
            _ => None,
        };
        let Some((open, close, close_kind)) = close else {
            self.fail_here(match notation {
                Notation::Ebnf => "expected a string, a rule name, `(`, `[`, `{`, or `? text ?`",
                Notation::Abnf => "expected a string, a rule name, a `%x` number, `(`, or `[`",
                Notation::Peg => "expected a string, a rule name, `(`, or `.`",
                Notation::Ir => "expected an expression",
            });
            return false;
        };
        if self.depth >= MAX_DEPTH {
            self.fail_here(&too_deep("groups", MAX_DEPTH));
            return false;
        }
        self.start(RR_GROUP);
        self.token(open, 1);
        self.trivia(false);
        self.depth += 1;
        self.choice();
        self.depth -= 1;
        if !self.failed {
            self.trivia(false);
            if self.rest().starts_with(close) {
                self.token(close_kind, 1);
            } else {
                self.fail_here(&format!("expected `{close}`"));
            }
        }
        self.finish();
        !self.failed
    }

    /// An IR constructor such as `choice(a, b)`.
    fn call(&mut self) {
        self.start(RR_CALL);
        let r = self.rest();
        let n = ident_len(r, self.notation);
        let name = &r[..n];
        if !CONSTRUCTORS.contains(&name) {
            self.fail_here(
                "expected `terminal`, `nonterminal`, `special`, `sequence`, `choice`, \
                 `optional`, `zeroOrMore`, or `oneOrMore`",
            );
            self.finish();
            return;
        }
        if self.depth >= MAX_DEPTH {
            self.fail_here(&too_deep("expressions", MAX_DEPTH));
            self.finish();
            return;
        }
        self.token(RAILROAD_KW, n);
        self.trivia(false);
        if !self.rest().starts_with('(') {
            self.fail_here("expected `(`");
            self.finish();
            return;
        }
        self.token(L_PAREN, 1);
        self.trivia(false);
        self.depth += 1;
        match name {
            "terminal" | "nonterminal" | "special" => {
                match string_len(self.rest(), self.notation) {
                    Some(Ok(n)) => self.token(STRING, n),
                    Some(Err(_)) => self.fail_here("unclosed string: expected a closing quote"),
                    None => self.fail_here("expected a string"),
                }
            }
            "optional" | "zeroOrMore" | "oneOrMore" => self.call(),
            _ => {
                self.call();
                while !self.failed {
                    self.trivia(false);
                    if !self.rest().starts_with(',') {
                        break;
                    }
                    self.token(COMMA, 1);
                    self.trivia(false);
                    self.call();
                }
            }
        }
        self.depth -= 1;
        if !self.failed {
            self.trivia(false);
            if self.rest().starts_with(')') {
                self.token(R_PAREN, 1);
            } else {
                self.fail_here("expected `)`");
            }
        }
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxNode;

    fn tree(src: &str, notation: Notation) -> (String, Vec<String>) {
        let (green, d) = parse(src, notation);
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.to_string(), src);
        (
            format!("{root:#?}"),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    #[test]
    fn lexemes() {
        assert_eq!(ident_len("a-b_c d", Notation::Ebnf), 5);
        assert_eq!(ident_len("_a", Notation::Abnf), 0);
        assert_eq!(ident_len("a_b", Notation::Abnf), 1);
        assert_eq!(string_len("\"a\\\"b\" c", Notation::Ebnf), Some(Ok(6)));
        assert_eq!(string_len("\"a\\\"b\" c", Notation::Abnf), Some(Ok(4)));
        assert_eq!(string_len("'a'", Notation::Abnf), None);
        assert_eq!(string_len("'a\nb' c", Notation::Peg), Some(Ok(5)));
        assert_eq!(string_len("'a\nb", Notation::Peg), Some(Err(2)));
        assert_eq!(numval_len("%x30-39 a"), 7);
        assert_eq!(numval_len("%x0D.0A"), 7);
        assert_eq!(numval_len("%d65"), 4);
        assert_eq!(numval_len("%q1"), 0);
        assert_eq!(repeat_len("1*DIGIT"), 2);
        assert_eq!(repeat_len("2*4a"), 3);
        assert_eq!(repeat_len("*a"), 1);
        assert_eq!(repeat_len("3a"), 1);
        assert_eq!(repeat_len("a"), 0);
        assert_eq!(special_len("? any char ? ;"), Some(12));
        assert_eq!(special_len("? ?"), None);
        assert_eq!(special_len("? a ; ?"), None);
    }

    #[test]
    fn notations() {
        let (t, d) = tree(
            "railroad-ebnf-beta\ntitle \"T\"\n(* c *) a ::= \"x\", b? | [c] {d}+ - e /* c */;\n\
             s = ? any ? ;\n",
            Notation::Ebnf,
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("RULE@").count(), 2);
        assert_eq!(t.matches("RR_GROUP@").count(), 2);
        assert!(t.contains("RR_SPECIAL@"), "{t}");
        assert!(t.contains("TITLE_STMT@"), "{t}");

        let (t, d) = tree(
            "railroad-abnf-beta\n; comment\na = 1*( b / \"c\" ) [%x30-39] 3d ; text\n  e ;\n",
            Notation::Abnf,
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("RR_REPEAT@").count(), 2);
        assert!(t.contains("RR_NUMVAL@"), "{t}");
        assert_eq!(t.matches("COMMENT@").count(), 2);

        let (t, d) = tree(
            "railroad-peg-beta\n# c\nA <- !B C* / &'d' . ;\n",
            Notation::Peg,
        );
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("DOT@"), "{t}");

        let (t, d) = tree(
            "railroad-beta\naccTitle: x\nr = sequence(terminal(\"a\"),\n  zeroOrMore(nonterminal('b'))) ;\n",
            Notation::Ir,
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("RR_CALL@").count(), 4);
        assert!(t.contains("ACC_TITLE_STMT@"), "{t}");
    }

    #[test]
    fn special_or_optional() {
        // `?` right after an element is optional; after a space, a special sequence when one
        // closes.
        let (t, _) = tree("railroad-ebnf-beta\na = b? c? ;\n", Notation::Ebnf);
        assert!(!t.contains("RR_SPECIAL@"), "{t}");
        let (t, _) = tree("railroad-ebnf-beta\na = b ? c ? ;\n", Notation::Ebnf);
        assert!(t.contains("RR_SPECIAL@"), "{t}");
    }

    #[test]
    fn errors() {
        let e = |src: &str, n: Notation| tree(src, n).1;
        assert_eq!(
            e("railroad\n", Notation::Ir),
            ["expected `railroad-beta` header"]
        );
        assert_eq!(
            e(
                "railroad-ebnf-beta\na = b\nc = d ;\ne = f ;\n",
                Notation::Ebnf
            ),
            ["expected `;` to end the rule"]
        );
        assert_eq!(
            e("railroad-abnf-beta\na = b ; c\n", Notation::Abnf),
            [
                "expected `;` to end the rule (a `;` with text after it on its line starts a \
                 comment)"
            ]
        );
        assert_eq!(
            e("railroad-peg-beta\na = b ;\n", Notation::Peg),
            ["expected `<-` after the rule name"]
        );
        assert_eq!(
            e("railroad-ebnf-beta\na = ( b ;\nc = \"d ;\n", Notation::Ebnf),
            ["expected `)`", "unclosed string: expected a closing quote"]
        );
        assert_eq!(
            e("railroad-beta\na = loop(terminal(\"x\")) ;\n", Notation::Ir),
            [
                "expected `terminal`, `nonterminal`, `special`, `sequence`, `choice`, \
                 `optional`, `zeroOrMore`, or `oneOrMore`"
            ]
        );
        assert_eq!(
            e("railroad-ebnf-beta\n= a ;\nb = | ;\n/* x\n", Notation::Ebnf),
            [
                "expected a rule name",
                "expected a string, a rule name, `(`, `[`, `{`, or `? text ?`",
                "unclosed comment: expected `*/`"
            ]
        );
    }
}
