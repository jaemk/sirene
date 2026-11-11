//! Line-oriented parser for Mermaid Wardley maps, producing a lossless rowan tree (MWARD-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["wardley-beta"];

/// Statement keywords; a line starting with one of these, then a space, is that statement.
const KEYWORDS: &[&str] = &[
    "size",
    "evolution",
    "anchor",
    "component",
    "evolve",
    "pipeline",
    "note",
    "annotations",
    "annotation",
    "accelerator",
    "deaccelerator",
];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
    };
    p.start(ROOT);
    p.header(HEADERS, "wardley-beta", header_len);
    if !p.diagnostics.is_empty() {
        // Without the header, the rest is not read.
        let len = p.rest().len();
        p.start(ERROR);
        p.token(ERROR_TOKEN, len);
        p.finish();
    }
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        p.statement();
    }
    p.finish();
    p.base.finish_tree()
}

/// The length of the header word: letters, digits, and `-`.
fn header_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(s.len())
}

/// The length of a lowercase keyword at the start of `s`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(s.len())
}

/// Whether `c` may continue a name: letters, digits, `_`, `(`, `)`, `&`, or a `-` not before
/// `>`.
fn name_char(s: &str, i: usize, c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | '(' | ')' | '&')
        || (c == '-' && !s[i + 1..].starts_with('>'))
}

/// The length of a run of name characters at the start of `s`.
fn run_len(s: &str) -> usize {
    s.char_indices()
        .find(|&(i, c)| !name_char(s, i, c))
        .map_or(s.len(), |(i, _)| i)
}

/// The length of an unquoted name, as Mermaid reads one: a letter or `_`, then name
/// characters, with single words after spaces when each starts with a letter or `(`.
pub(crate) fn name_len(s: &str) -> usize {
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return 0,
    }
    let mut n = 1 + run_len(&s[1..]);
    loop {
        let after = &s[n..];
        let ws = scan::ws_len(after);
        if ws == 0 {
            return n;
        }
        match after[ws..].chars().next() {
            Some(c) if c.is_ascii_alphabetic() || c == '(' => {
                n += ws + 1 + run_len(&after[ws + 1..]);
            }
            _ => return n,
        }
    }
}

/// The length of an unsigned number such as `3` or `0.25`.
fn number_len(s: &str) -> usize {
    let int = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if int == 0 {
        return 0;
    }
    let Some(frac) = s[int..].strip_prefix('.') else {
        return int;
    };
    let digits = frac
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(frac.len());
    if digits == 0 { int } else { int + 1 + digits }
}

/// The length of an integer with an optional `-`.
fn signed_int_len(s: &str) -> usize {
    let sign = usize::from(s.starts_with('-'));
    let digits = s[sign..]
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(s.len() - sign);
    if digits == 0 { 0 } else { sign + digits }
}

/// The length of a link's flow marker at the start of `s`, and whether it may be followed by
/// an arrow: `+<>`, `+>`, `+<` (ports), or `+'label'<>`, `+'label'<`, `+'label'>` (arrows).
fn port_len(s: &str) -> Option<(usize, bool)> {
    for p in ["+<>", "+>", "+<"] {
        if s.starts_with(p) {
            return Some((p.len(), true));
        }
    }
    let body = s.strip_prefix("+'")?;
    let line = &body[..scan::line_len(body)];
    let close = line.find('\'')?;
    let after = &line[close + 1..];
    let tail = ["<>", "<", ">"]
        .into_iter()
        .find(|t| after.starts_with(t))?;
    Some((2 + close + 1 + tail.len(), false))
}

/// The length of a plain arrow: `-->`, `-.->`, `->`, or `>`.
fn arrow_len(s: &str) -> usize {
    ["-.->", "-->", "->", ">"]
        .into_iter()
        .find(|a| s.starts_with(a))
        .map_or(0, str::len)
}

struct Parser<'a> {
    base: Base<'a>,
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

impl Parser<'_> {
    /// The statement keyword at the cursor, when it is followed by whitespace or `[`.
    fn keyword(&self) -> Option<&'static str> {
        let r = self.rest();
        let n = word_len(r);
        let next = r[n..].chars().next();
        if !matches!(next, Some(' ' | '\t' | '[')) {
            return None;
        }
        KEYWORDS.iter().copied().find(|k| *k == &r[..n])
    }

    fn statement(&mut self) {
        let r = self.rest();
        if r.starts_with("accTitle") || r.starts_with("accDescr") {
            self.acc();
            return;
        }
        if r.starts_with("title")
            && matches!(r[5..].chars().next(), None | Some(' ' | '\t' | '\n' | '\r'))
        {
            self.title();
            return;
        }
        match self.keyword() {
            Some("size") => self.size(),
            Some("evolution") => self.evolution(),
            Some(kw @ ("anchor" | "component")) => self.component(kw),
            Some("evolve") => self.evolve(),
            Some("pipeline") => self.pipeline(),
            Some("note") => self.note(),
            Some("annotations") => self.annotations(),
            Some("annotation") => self.annotation(),
            Some(kw @ ("accelerator" | "deaccelerator")) => self.accelerator(kw),
            _ if self.name_len() > 0 => self.link(),
            _ => self.fail(
                "expected a component, an anchor, a link such as `A -> B`, or a statement such as \
                 `evolve` or `note`",
            ),
        }
    }

    /// Requires the statement to end here, allowing a trailing `%%` comment.
    fn eol(&mut self) {
        self.ws();
        if self.rest().starts_with("%%") {
            let n = scan::line_len(self.rest());
            self.token(COMMENT, n);
        }
        self.end();
    }

    /// `title text`, up to a `%%` comment.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let end = line.find("%%").unwrap_or(line.len());
        let len = scan::trim_end_len(r, end);
        self.token(TEXT, len);
        self.eol();
        self.finish();
    }

    fn kw(&mut self, kw: &str) {
        self.token(WARDLEY_KW, kw.len());
        self.ws();
    }

    /// The length of a name or a `"..."` string at the cursor.
    fn name_len(&self) -> usize {
        self.string_len().unwrap_or_else(|| name_len(self.rest()))
    }

    /// A name as an `IDENT` or `STRING` token; returns whether there was one.
    fn name(&mut self) -> bool {
        let n = self.name_len();
        if n == 0 {
            return false;
        }
        let kind = if self.rest().starts_with('"') {
            STRING
        } else {
            IDENT
        };
        self.token(kind, n);
        true
    }

    /// One token of `len_of` at the cursor as `kind`, or a reported `expected`.
    fn expect(
        &mut self,
        kind: crate::syntax::SyntaxKind,
        len_of: fn(&str) -> usize,
        expected: &str,
    ) -> bool {
        self.ws();
        let n = len_of(self.rest());
        if n == 0 {
            self.fail(&format!("expected {expected}"));
            return false;
        }
        self.token(kind, n);
        true
    }

    /// `[a, b]` (or `[a]` with `one`), each a number of `len_of`, as a `COORDS` node.
    fn coords(&mut self, len_of: fn(&str) -> usize, one: bool) -> bool {
        self.ws();
        if !self.rest().starts_with('[') {
            self.fail(if one {
                "expected `[evolution]`"
            } else {
                "expected `[visibility, evolution]`"
            });
            return false;
        }
        self.start(COORDS);
        self.token(L_BRACKET, 1);
        let mut ok = self.expect(NUMBER, len_of, "a number");
        if ok && !one {
            self.ws();
            if self.rest().starts_with(',') {
                self.token(COMMA, 1);
                ok = self.expect(NUMBER, len_of, "a number");
            } else {
                self.fail("expected `,`");
                ok = false;
            }
        }
        if ok {
            self.ws();
            if self.rest().starts_with(']') {
                self.token(R_BRACKET, 1);
            } else {
                self.fail("expected `]`");
                ok = false;
            }
        }
        self.finish();
        ok
    }

    /// `size [width, height]`.
    fn size(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("size");
        if self.coords(number_len, false) {
            self.eol();
        }
        self.finish();
    }

    /// `evolution A -> B@0.4 -> C / D -> ...`.
    fn evolution(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("evolution");
        let mut stages = 0;
        loop {
            self.ws();
            self.start(STAGE);
            if !self.name() {
                self.finish();
                self.fail("expected a stage name");
                self.finish();
                return;
            }
            // A boundary and a second name, in either order.
            let (mut at, mut slash) = (false, false);
            loop {
                self.ws();
                if !at && self.rest().starts_with('@') {
                    at = true;
                    self.token(AT, 1);
                    if !self.expect(NUMBER, number_len, "a stage boundary such as `0.25`") {
                        self.finish();
                        self.finish();
                        return;
                    }
                } else if !slash && self.rest().starts_with('/') {
                    slash = true;
                    self.token(TEXT, 1);
                    self.ws();
                    if !self.name() {
                        self.finish();
                        self.fail("expected a second stage name");
                        self.finish();
                        return;
                    }
                } else {
                    break;
                }
            }
            self.finish();
            stages += 1;
            self.ws();
            if self.rest().starts_with("->") {
                self.token(ARROW, 2);
            } else {
                break;
            }
        }
        if stages < 2 {
            self.fail("expected two or more stages separated by `->`");
        } else {
            self.eol();
        }
        self.finish();
    }

    /// `label [x, y]` with whole-number offsets, if present.
    fn label(&mut self) -> bool {
        self.start(LABEL);
        self.token(WARDLEY_KW, "label".len());
        let ok = self.coords(signed_int_len, false);
        self.finish();
        ok
    }

    /// `anchor name [v, e]`, or `component name [v, e]` with an optional label offset,
    /// sourcing decorator, and inertia.
    fn component(&mut self, kw: &str) {
        self.start(WARDLEY_STMT);
        self.kw(kw);
        if !self.name() {
            self.fail("expected a name");
            self.finish();
            return;
        }
        if !self.coords(number_len, false) {
            self.finish();
            return;
        }
        if kw == "component" {
            loop {
                self.ws();
                let r = self.rest();
                if r.starts_with("label") && word_len(r) == 5 {
                    if !self.label() {
                        self.finish();
                        return;
                    }
                } else if r.starts_with("inertia") && word_len(r) == 7 {
                    self.start(DECORATOR);
                    self.token(IDENT, 7);
                    self.finish();
                } else if let Some(body) = r.strip_prefix('(') {
                    let word = word_len(body);
                    let known =
                        ["build", "buy", "outsource", "market", "inertia"].contains(&&body[..word]);
                    if !known || !body[word..].starts_with(')') {
                        self.fail(
                            "expected `(build)`, `(buy)`, `(outsource)`, `(market)`, or \
                             `(inertia)`",
                        );
                        self.finish();
                        return;
                    }
                    self.start(DECORATOR);
                    self.token(L_PAREN, 1);
                    self.token(IDENT, word);
                    self.token(R_PAREN, 1);
                    self.finish();
                } else {
                    break;
                }
            }
        }
        self.eol();
        self.finish();
    }

    /// `evolve name 0.8`.
    fn evolve(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("evolve");
        if !self.name() {
            self.fail("expected a component name");
        } else if self.expect(NUMBER, number_len, "a target evolution such as `0.8`") {
            self.eol();
        }
        self.finish();
    }

    /// `pipeline name {`, then `component name [e]` lines, then `}`.
    fn pipeline(&mut self) {
        self.start(PIPELINE);
        self.kw("pipeline");
        if !self.name() {
            self.fail("expected a component name");
            self.finish();
            return;
        }
        self.ws();
        if !self.rest().starts_with('{') {
            self.fail("expected `{`");
            self.finish();
            return;
        }
        self.token(L_BRACE, 1);
        self.eol();
        loop {
            self.trivia();
            if self.at_eof() {
                let at = self.pos;
                self.error(at, at, "unclosed `pipeline`: expected `}`");
                break;
            }
            if self.rest().starts_with('}') {
                self.token(R_BRACE, 1);
                self.eol();
                break;
            }
            let r = self.rest();
            if !(r.starts_with("component") && word_len(r) == 9) {
                self.fail("expected `component name [evolution]` or `}`");
                continue;
            }
            self.start(WARDLEY_STMT);
            self.kw("component");
            if !self.name() {
                self.fail("expected a name");
            } else if self.coords(number_len, true) {
                self.ws();
                let r = self.rest();
                if !(r.starts_with("label") && word_len(r) == 5) || self.label() {
                    self.eol();
                }
            }
            self.finish();
        }
        self.finish();
    }

    /// `note "text" [v, e]`.
    fn note(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("note");
        match self.string_len() {
            Some(n) => {
                self.token(STRING, n);
                if self.coords(number_len, false) {
                    self.eol();
                }
            }
            None => self.fail("expected the note's text in quotes"),
        }
        self.finish();
    }

    /// `annotations [v, e]`.
    fn annotations(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("annotations");
        if self.coords(number_len, false) {
            self.eol();
        }
        self.finish();
    }

    /// `annotation 1,[v, e] "text"`.
    fn annotation(&mut self) {
        self.start(WARDLEY_STMT);
        self.kw("annotation");
        let int = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if !self.expect(NUMBER, int, "an annotation number") {
            self.finish();
            return;
        }
        self.ws();
        if !self.rest().starts_with(',') {
            self.fail("expected `,`");
            self.finish();
            return;
        }
        self.token(COMMA, 1);
        if self.coords(number_len, false) {
            self.ws();
            match self.string_len() {
                Some(n) => {
                    self.token(STRING, n);
                    self.eol();
                }
                None => self.fail("expected the annotation's text in quotes"),
            }
        }
        self.finish();
    }

    /// `accelerator name [v, e]` or `deaccelerator name [v, e]`.
    fn accelerator(&mut self, kw: &str) {
        self.start(WARDLEY_STMT);
        self.kw(kw);
        if !self.name() {
            self.fail("expected a name");
        } else if self.coords(number_len, false) {
            self.eol();
        }
        self.finish();
    }

    /// A port such as `+>` as a `PORT` token, if one is at the cursor.
    fn port(&mut self, ports_only: bool) -> bool {
        match port_len(self.rest()) {
            Some((n, port)) if port || !ports_only => {
                self.token(PORT, n);
                true
            }
            _ => false,
        }
    }

    /// `A -> B`, with an optional flow marker and `; label`.
    fn link(&mut self) {
        self.start(LINK_STMT);
        self.name();
        self.ws();
        let port = self.port(true);
        self.ws();
        let arrow = if self.port(false) {
            true
        } else {
            let n = arrow_len(self.rest());
            self.token(ARROW, n);
            n > 0
        };
        if !port && !arrow {
            self.fail("expected `->` or a flow such as `+>`");
            self.finish();
            return;
        }
        self.ws();
        if !self.name() {
            self.fail("expected the target's name");
            self.finish();
            return;
        }
        self.ws();
        self.port(true);
        self.ws();
        if self.rest().starts_with(';') {
            self.token(SEMI, 1);
            self.ws();
            self.rest_of_line(TEXT);
        }
        self.eol();
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxNode;

    fn tree(src: &str) -> (String, Vec<String>) {
        let (green, d) = parse(src);
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.to_string(), src);
        (
            format!("{root:#?}"),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    #[test]
    fn names() {
        assert_eq!(name_len("Cup of Tea [0.79, 0.61]"), 10);
        assert_eq!(name_len("real-time processing ->"), 20);
        assert_eq!(name_len("A->B"), 1);
        assert_eq!(name_len("byte pair encoding (BPE) [1]"), 24);
        assert_eq!(name_len("Layer 2"), 5);
        assert_eq!(name_len("Genesis@0.2"), 7);
        assert_eq!(name_len("2nd"), 0);
    }

    #[test]
    fn ports() {
        assert_eq!(port_len("+<> B"), Some((3, true)));
        assert_eq!(port_len("+'backup'> B"), Some((10, false)));
        assert_eq!(port_len("+'x'<> B"), Some((6, false)));
        assert_eq!(port_len("+'x B"), None);
        assert_eq!(arrow_len("-.-> B"), 4);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "wardley-beta\ntitle Tea Shop %% c\nsize [1100, 800]\n\
             evolution Genesis / Concept@0.2 -> Custom -> \"Product\" -> Commodity@1.0\n\
             anchor Business [0.95, 0.63]\n\
             component Cup of Tea [0.79, 0.61] label [-50, 10] (buy) (inertia)\n\
             component Kettle [0.43, 0.35] inertia %% old\n\
             Business -> Cup of Tea\nA -.-> B\nA +<> B\nA +'backup'> B\nA -> B; uses\n\
             evolve Kettle 0.62\n\
             pipeline Kettle {\n  component \"Gas\" [0.25]\n  component Electric [0.5] label [5, 5]\n}\n\
             note \"text\" [0.3, 0.49]\nannotations [0.1, 0.9]\nannotation 1,[0.6, 0.65] \"x\"\n\
             accelerator AI [0.5, 0.2]\ndeaccelerator \"Legacy\" [0.1, 0.7]\n\
             accTitle: t\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("LINK_STMT@").count(), 5);
        assert_eq!(t.matches("STAGE@").count(), 4);
        assert_eq!(t.matches("DECORATOR@").count(), 3);
        assert_eq!(t.matches("PORT@").count(), 2);
        assert!(t.contains("PIPELINE@"));
        assert!(t.contains("ACC_TITLE_STMT"));
    }

    #[test]
    fn errors() {
        let e = |src: &str| tree(&format!("wardley-beta\n{src}\n")).1;
        assert_eq!(tree("wardley\n").1, ["expected `wardley-beta` header"]);
        assert_eq!(e("component A"), ["expected `[visibility, evolution]`"]);
        assert_eq!(e("component A [0.5]"), ["expected `,`"]);
        assert_eq!(e("component A [0.5, x]"), ["expected a number"]);
        assert_eq!(
            e("component A [0.5, 0.5] (rent)"),
            ["expected `(build)`, `(buy)`, `(outsource)`, `(market)`, or `(inertia)`"]
        );
        assert_eq!(e("A B"), ["expected `->` or a flow such as `+>`"]);
        assert_eq!(e("A ->"), ["expected the target's name"]);
        assert_eq!(
            e("note text [0.1, 0.2]"),
            ["expected the note's text in quotes"]
        );
        assert_eq!(
            e("evolution Genesis"),
            ["expected two or more stages separated by `->`"]
        );
        assert_eq!(
            e("pipeline A {\nfoo\n}"),
            ["expected `component name [evolution]` or `}`"]
        );
        assert_eq!(e("pipeline A {\n"), ["unclosed `pipeline`: expected `}`"]);
        assert_eq!(e("evolve A"), ["expected a target evolution such as `0.8`"]);
        assert_eq!(
            e("[0.1]"),
            [
                "expected a component, an anchor, a link such as `A -> B`, or a statement such as \
             `evolve` or `note`"
            ]
        );
    }
}
