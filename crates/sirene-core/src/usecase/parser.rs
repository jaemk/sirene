//! Line-oriented parser for Mermaid use case diagrams, producing a lossless rowan tree
//! (MUSE-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["usecase-beta"];

/// PlantUML statements Mermaid rejects by name.
const PLANTUML: &[&str] = &[
    "allowmixing",
    "newpage",
    "package",
    "rectangle",
    "skinparam",
];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
    };
    p.start(ROOT);
    p.header(HEADERS, "usecase-beta", header_len);
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
        let before = p.pos;
        p.statement();
        if p.pos == before {
            p.fail("expected a statement");
        }
    }
    p.finish();
    p.base.finish_tree()
}

fn header_len(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace()).unwrap_or(s.len())
}

/// The length of an id: ASCII letters, digits, and `_`, as Mermaid's `\w+`.
pub(crate) fn id_len(s: &str) -> usize {
    s.bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count()
}

fn word(s: &str) -> &str {
    &s[..id_len(s)]
}

/// The length of a `"`...`"` Markdown string, which may span lines, if closed.
fn markdown_len(s: &str) -> Option<usize> {
    s.strip_prefix("\"`")?.find("`\"").map(|i| i + 4)
}

/// The length of a `"..."` or `'...'` string closed on its line.
fn plain_len(s: &str) -> Option<usize> {
    let q = s.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let line = &s[1..scan::line_len(s)];
    line.find(q).map(|i| i + 2)
}

/// The kind of a relationship operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    /// `-->`, with any number of dashes.
    Forward,
    /// `<--`, with any number of dashes.
    Backward,
    /// `--`, with any number of dashes.
    Line,
    ForwardCircle,
    BackwardCircle,
    ForwardCross,
    BackwardCross,
    /// `--|>`.
    Generalization,
    /// `..>`, before `: include` or `: extend`.
    Dependency,
}

/// The relationship operator at the start of `s` and its length, matched in the order
/// Mermaid's lexer tries them.
pub(crate) fn op(s: &str) -> Option<(Op, usize)> {
    let dashes = |s: &str| s.bytes().take_while(|b| *b == b'-').count();
    if s.starts_with("--|>") {
        return Some((Op::Generalization, 4));
    }
    if s.starts_with("..>") {
        return Some((Op::Dependency, 3));
    }
    let d = dashes(s);
    if d >= 2 && s[d..].starts_with('>') {
        return Some((Op::Forward, d + 1));
    }
    if let Some(r) = s.strip_prefix('<') {
        let d = dashes(r);
        if d >= 2 {
            return Some((Op::Backward, d + 1));
        }
    }
    for (text, op) in [
        ("--o", Op::ForwardCircle),
        ("o--", Op::BackwardCircle),
        ("--x", Op::ForwardCross),
        ("x--", Op::BackwardCross),
    ] {
        if s.starts_with(text) {
            return Some((op, 3));
        }
    }
    (d >= 2).then_some((Op::Line, d))
}

/// The length of unquoted label text: up to a delimiter, a quote, an operator, a suffix
/// marker, or the end of the line, without trailing whitespace.
fn text_len(s: &str) -> usize {
    let line = &s[..scan::line_len(s)];
    let mut end = line.len();
    for (i, c) in line.char_indices() {
        let r = &line[i..];
        if matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'')
            || ["--", "<--", "..>", ":::", "@{", "<<"]
                .iter()
                .any(|m| r.starts_with(m))
        {
            end = i;
            break;
        }
    }
    scan::trim_end_len(s, end)
}

/// Whether `s` starts a `json name@{` declaration.
fn json_follows(s: &str) -> bool {
    let Some(r) = s.strip_prefix("json") else {
        return false;
    };
    let ws = scan::ws_len(r);
    if ws == 0 {
        return false;
    }
    let r = &r[ws..];
    let n = id_len(r);
    if n == 0 {
        return false;
    }
    let r = &r[n..];
    let r = r[scan::ws_len(r)..].strip_prefix('@');
    r.is_some_and(|r| r[scan::ws_len(r)..].starts_with('{'))
}

/// The length of a name at `s`: an id, a plain string, or a Markdown string.
fn name_len(s: &str) -> Option<usize> {
    if s.starts_with("\"`") {
        markdown_len(s)
    } else if s.starts_with(['"', '\'']) {
        plain_len(s)
    } else {
        Some(id_len(s)).filter(|&n| n > 0)
    }
}

/// Whether `s` starts a standalone `name@{ ... }` statement.
fn meta_follows(s: &str) -> bool {
    name_len(s).is_some_and(|n| {
        let r = &s[n..];
        r[scan::ws_len(r)..].starts_with("@{")
    })
}

/// The length of the JSON object at `s`, which starts with `{`, honoring strings.
fn json_len(s: &str) -> Option<usize> {
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for (i, c) in s.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
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

impl<'a> Parser<'a> {
    /// Whitespace, newlines, `%%` comments on their own lines, and directives. Unlike other
    /// diagrams, `;` does not separate statements.
    fn trivia(&mut self) {
        loop {
            let r = self.rest();
            let ws = scan::ws_len(r);
            let nl = scan::newline_len(r);
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else if let Some(body) = r.strip_prefix("%%{") {
                match body.find("}%%") {
                    Some(i) => self.token(DIRECTIVE, i + 6),
                    None => {
                        let start = self.pos;
                        self.token(DIRECTIVE, scan::line_len(r));
                        self.error(start, start + 3, "unclosed directive: expected `}%%`");
                    }
                }
            } else if r.starts_with("%%") && self.at_line_start() {
                self.token(COMMENT, scan::line_len(r));
            } else {
                return;
            }
        }
    }

    fn at_line_start(&self) -> bool {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        line.chars().all(scan::is_space)
    }

    /// Requires the statement to end with its line.
    fn eol(&mut self) {
        let n = scan::ws_len(self.rest());
        let r = &self.rest()[n..];
        if !(r.is_empty() || scan::newline_len(r) > 0) {
            self.fail("expected the end of the line: put each statement on its own line");
        }
    }

    fn statement(&mut self) {
        let r = self.rest();
        let w = word(r);
        let after = &r[w.len()..];
        let acc = |c: &[char]| after[scan::ws_len(after)..].starts_with(c);
        match w {
            "accTitle" if acc(&[':']) => self.acc(),
            "accDescr" if acc(&[':', '{']) => self.acc(),
            "direction" => self.direction(),
            "actor" => self.actor(false),
            "systemBoundary" => self.boundary(),
            "note" => self.note(),
            "json" if json_follows(r) => self.json(),
            "classDef" => self.class_def(id_len),
            "class" => self.class_stmt(),
            "style" => self.style(id_len, "an element or edge id"),
            "end" => self.fail("unexpected `end`: no system boundary is open"),
            _ if PLANTUML.contains(&w.to_ascii_lowercase().as_str()) => self.fail(&format!(
                "`{w}` is PlantUML syntax that Mermaid use case diagrams do not support"
            )),
            _ if meta_follows(r) => self.meta_stmt(),
            _ => self.entity_stmt(false),
        }
    }

    /// A name: an id with an optional `(label)` (or `[label]` when `rect` is allowed), a plain
    /// string, or a Markdown string. Returns whether there was one.
    fn name(&mut self, rect: bool) -> bool {
        let r = self.rest();
        if r.starts_with("\"`") {
            match markdown_len(r) {
                Some(n) => self.token(MD_STRING, n),
                None => {
                    self.fail("unclosed Markdown string: expected `` `\" ``");
                    return true;
                }
            }
            return true;
        }
        if r.starts_with(['"', '\'']) {
            match plain_len(r) {
                Some(n) => self.token(STRING, n),
                None => self.fail("unclosed string: expected a closing quote on the same line"),
            }
            return true;
        }
        let n = id_len(r);
        if n == 0 {
            return false;
        }
        self.token(IDENT, n);
        let r = self.rest();
        let ws = scan::ws_len(r);
        let open = r[ws..].chars().next();
        if open == Some('(') || (rect && open == Some('[')) {
            self.ws();
            self.shape();
        }
        true
    }

    /// `(label)` or `[label]`.
    fn shape(&mut self) {
        let close = if self.rest().starts_with('(') {
            ')'
        } else {
            ']'
        };
        self.start(SHAPE);
        self.token(SHAPE_OPEN, 1);
        self.ws();
        if !self.label() {
            self.fail("expected a label");
            self.finish();
            return;
        }
        self.ws();
        if self.rest().starts_with(close) {
            self.token(SHAPE_CLOSE, 1);
        } else {
            self.fail(&format!("expected `{close}`"));
        }
        self.finish();
    }

    /// A label: a plain string, a Markdown string, or unquoted text. Returns whether there was
    /// one.
    fn label(&mut self) -> bool {
        let r = self.rest();
        if r.starts_with("\"`") || r.starts_with(['"', '\'']) {
            return self.name(false);
        }
        let n = text_len(r);
        self.token(TEXT, n);
        n > 0
    }

    /// `@{ ... }`, `<<stereotype>>`, and `:::classes`, in that order, each optional.
    fn suffixes(&mut self, stereotype: bool) {
        if self.peek().starts_with("@{") {
            self.ws();
            self.metadata();
        }
        if stereotype && self.peek().starts_with("<<") {
            self.ws();
            let r = self.rest();
            let line = &r[..scan::line_len(r)];
            match line[2..].find(">>") {
                Some(i) if !line[2..2 + i].trim().is_empty() => self.token(STEREOTYPE, i + 4),
                Some(_) => self.fail("expected a stereotype between `<<` and `>>`"),
                None => self.fail("unclosed stereotype: expected `>>`"),
            }
        }
        if self.peek().starts_with(":::") {
            self.ws();
            self.classes();
        }
    }

    /// The rest of the line after whitespace.
    fn peek(&self) -> &'a str {
        let r = self.rest();
        &r[scan::ws_len(r)..]
    }

    /// `@{ key: value, ... }`, which may span lines.
    fn metadata(&mut self) {
        self.start(METADATA);
        self.token(META_OPEN, 2);
        match scan::meta_body_len(self.rest()) {
            Some(n) => {
                self.token(META_BODY, n);
                self.token(META_CLOSE, 1);
            }
            None => self.fail("unclosed `@{`: expected `}`"),
        }
        self.finish();
    }

    /// `:::a,b`.
    fn classes(&mut self) {
        self.start(CLASS_SHORTHAND);
        self.token(TRIPLE_COLON, 3);
        self.ws();
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected a class name after `:::`");
            self.finish();
            return;
        }
        // As in Mermaid, every name after a comma is another class, even with spaces between.
        self.token(IDENT, n);
        while self.peek().starts_with(',') {
            self.ws();
            self.token(COMMA, 1);
            self.ws();
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected a class name after `,`");
                break;
            }
            self.token(IDENT, n);
        }
        self.finish();
    }

    /// An actor or use case occurrence: a name and its suffixes.
    fn node(&mut self, rect: bool) -> bool {
        if name_len(self.rest()).is_none() && !self.rest().starts_with(['"', '\'']) {
            return false;
        }
        self.start(NODE);
        self.name(rect);
        self.suffixes(true);
        self.finish();
        true
    }

    /// The relationship at `s` (after whitespace): its optional `id@` length and operator.
    fn edge_follows(&self) -> Option<(usize, Op, usize)> {
        let r = self.peek();
        let n = id_len(r);
        if n > 0
            && let Some(after) = r[n..].strip_prefix('@')
            && !after.starts_with('{')
        {
            let ws = scan::ws_len(after);
            if let Some((o, len)) = op(&after[ws..]) {
                return Some((n, o, len));
            }
        }
        op(r).map(|(o, len)| (0, o, len))
    }

    /// A relationship and its target, after its source.
    fn relation(&mut self) {
        let Some((id, kind, len)) = self.edge_follows() else {
            return;
        };
        self.ws();
        self.start(EDGE);
        if id > 0 {
            self.start(EDGE_ID);
            self.token(IDENT, id);
            self.token(AT, 1);
            self.finish();
            self.ws();
        }
        let errors = self.diagnostics.len();
        self.token(STROKE, len);
        match kind {
            Op::Dependency => {
                self.ws();
                if self.rest().starts_with(':') {
                    self.token(COLON, 1);
                    self.ws();
                    let w = word(self.rest());
                    if w.eq_ignore_ascii_case("include") || w.eq_ignore_ascii_case("extend") {
                        self.token(RELATION, w.len());
                    } else {
                        self.fail("expected `include` or `extend`");
                    }
                } else {
                    self.fail("expected `: include` or `: extend` after `..>`");
                }
            }
            Op::Line => {
                self.edge_label(&[Op::Forward, Op::Line, Op::ForwardCircle, Op::ForwardCross])
            }
            Op::Backward => self.edge_label(&[Op::Line]),
            Op::BackwardCircle | Op::BackwardCross => self.edge_label(&[Op::Line]),
            _ => {}
        }
        self.finish();
        if self.diagnostics.len() > errors {
            return;
        }
        self.ws();
        if !self.node(true) {
            self.fail("expected an actor or use case after the relationship");
        }
    }

    /// A label between an operator and a closing one of `closers`, if there is one.
    fn edge_label(&mut self, closers: &[Op]) {
        let r = self.peek();
        let n = if r.starts_with("\"`") {
            markdown_len(r)
        } else if r.starts_with(['"', '\'']) {
            plain_len(r)
        } else {
            Some(text_len(r)).filter(|&n| n > 0)
        };
        let Some(n) = n else { return };
        let after = &r[n..];
        let ws = scan::ws_len(after);
        let Some((_, len)) = op(&after[ws..]).filter(|(o, _)| closers.contains(o)) else {
            return;
        };
        self.ws();
        self.start(EDGE_LABEL);
        let kind = if r.starts_with("\"`") {
            MD_STRING
        } else if r.starts_with(['"', '\'']) {
            STRING
        } else {
            TEXT
        };
        self.token(kind, n);
        self.finish();
        self.ws();
        self.token(STROKE, len);
    }

    /// `name`, `name(label)`, or `name[label]` with suffixes, then an optional relationship.
    fn entity_stmt(&mut self, in_boundary: bool) {
        let at = self.pos;
        self.start(VERTEX_STMT);
        if !self.node(true) {
            self.finish();
            if self.pos == at {
                self.fail("expected an actor, a use case, or a statement");
            }
            return;
        }
        if self.edge_follows().is_some() {
            if in_boundary {
                self.fail("relationships go after the system boundary's `end`");
            } else {
                self.relation();
            }
        }
        self.eol();
        self.finish();
    }

    /// `actor a, b` or `actor a --> b`.
    fn actor(&mut self, in_boundary: bool) {
        self.start(VERTEX_STMT);
        self.token(USECASE_KW, "actor".len());
        self.ws();
        if !self.node(false) {
            self.fail("expected an actor name");
            self.finish();
            return;
        }
        if self.peek().starts_with(',') {
            while self.peek().starts_with(',') {
                self.ws();
                self.token(COMMA, 1);
                self.ws();
                if !self.node(false) {
                    self.fail("expected an actor name after `,`");
                    break;
                }
            }
        } else if self.edge_follows().is_some() {
            if in_boundary {
                self.fail("relationships go after the system boundary's `end`");
            } else {
                self.relation();
            }
        }
        self.eol();
        self.finish();
    }

    /// `systemBoundary name ... end`.
    fn boundary(&mut self) {
        self.start(SUBGRAPH);
        self.start(SUBGRAPH_HEADER);
        self.token(USECASE_KW, "systemBoundary".len());
        self.ws();
        self.start(NODE);
        if self.name(true) {
            self.suffixes(false);
        } else {
            self.fail("expected the system boundary's id or title");
        }
        self.finish();
        self.eol();
        self.finish();
        loop {
            self.trivia();
            if self.at_eof() {
                let at = self.pos;
                self.error(at, at, "unclosed `systemBoundary`: expected `end`");
                break;
            }
            let r = self.rest();
            match word(r) {
                "end" => {
                    self.token(END_KW, 3);
                    self.eol();
                    break;
                }
                "actor" => self.actor(true),
                "systemBoundary" => self.fail("system boundaries cannot be nested"),
                "note" | "json" | "classDef" | "class" | "style" | "direction" | "accTitle"
                | "accDescr" => self.fail(
                    "a system boundary holds only actor and use case declarations; move this \
                     after its `end`",
                ),
                _ if meta_follows(r) => self.fail(
                    "a system boundary holds only actor and use case declarations; move this \
                     after its `end`",
                ),
                _ => {
                    let before = self.pos;
                    self.entity_stmt(true);
                    if self.pos == before {
                        self.fail("expected an actor, a use case, or `end`");
                    }
                }
            }
        }
        self.finish();
    }

    /// `note for id label`.
    fn note(&mut self) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, 4);
        self.ws();
        if word(self.rest()) != "for" {
            self.fail("expected `for`");
            self.finish();
            return;
        }
        self.token(FOR_KW, 3);
        self.ws();
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected the id of the actor or use case the note is for");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        self.ws();
        if !self.label() {
            self.fail("expected the note's text");
        } else {
            self.eol();
        }
        self.finish();
    }

    /// `json name@{ ... }:::classes`.
    fn json(&mut self) {
        self.start(JSON_STMT);
        self.token(USECASE_KW, 4);
        self.ws();
        let n = id_len(self.rest());
        self.token(IDENT, n);
        self.ws();
        self.token(AT, 1);
        self.ws();
        match json_len(self.rest()) {
            Some(n) => {
                self.token(JSON_BODY, n);
                if self.peek().starts_with(":::") {
                    self.ws();
                    self.classes();
                }
                self.eol();
            }
            None => {
                let start = self.pos;
                let len = self.rest().len();
                self.token(JSON_BODY, len);
                self.error(start, start + 1, "unclosed JSON object: expected `}`");
            }
        }
        self.finish();
    }

    /// `name@{ ... }` on its own line.
    fn meta_stmt(&mut self) {
        self.start(META_STMT);
        let n = name_len(self.rest()).unwrap_or(0);
        let kind = if self.rest().starts_with("\"`") {
            MD_STRING
        } else if self.rest().starts_with(['"', '\'']) {
            STRING
        } else {
            IDENT
        };
        self.token(kind, n);
        self.ws();
        self.metadata();
        self.eol();
        self.finish();
    }

    /// `class a,b c,d`: classes `c` and `d` for elements `a` and `b`.
    fn class_stmt(&mut self) {
        self.start(CLASS_STMT);
        self.token(CLASS_KW, "class".len());
        self.ws();
        if !self.id_list(id_len) {
            self.fail("expected an element or edge id");
        } else {
            self.ws();
            if self.id_list(id_len) {
                self.eol();
            } else {
                self.fail("expected a class name");
            }
        }
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
    fn operators() {
        assert_eq!(op("-->"), Some((Op::Forward, 3)));
        assert_eq!(op("---->"), Some((Op::Forward, 5)));
        assert_eq!(op("<---"), Some((Op::Backward, 4)));
        assert_eq!(op("--"), Some((Op::Line, 2)));
        assert_eq!(op("--o"), Some((Op::ForwardCircle, 3)));
        assert_eq!(op("x--"), Some((Op::BackwardCross, 3)));
        assert_eq!(op("--|>"), Some((Op::Generalization, 4)));
        assert_eq!(op("..>"), Some((Op::Dependency, 3)));
        assert_eq!(op("-"), None);
        assert_eq!(op("a"), None);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "usecase-beta\ndirection LR\nactor A(\"Main\")@{ type: hollow } <<Staff>>, B:::x,y\n\
             systemBoundary sb[\"Shop\"]@{ type: package }\n  %% c\n  Buy(Buy it)\n  actor C\nend\n\
             A opens@-- \"starts\" ---> Buy\nBuy ..> : include Pay\nA --|> B\n\
             note for Buy \"`**x**`\"\njson P@{ \"a\": {\"b\": 1} }:::d\nP@{ type: package }\n\
             class A,B x,y\nclassDef x stroke:#f00\nstyle A fill:#fff\naccTitle: T\n",
        );
        assert!(d.is_empty(), "{d:?}");
        for kind in [
            "DIRECTION_STMT",
            "SUBGRAPH_HEADER",
            "END_KW",
            "EDGE_ID",
            "EDGE_LABEL",
            "RELATION",
            "NOTE_STMT",
            "JSON_STMT",
            "META_STMT",
            "CLASS_STMT",
            "CLASSDEF_STMT",
            "STYLE_STMT",
            "ACC_TITLE_STMT",
            "STEREOTYPE",
        ] {
            assert!(t.contains(kind), "{kind}: {t}");
        }
        assert_eq!(t.matches("EDGE@").count(), 3, "{t}");
    }

    #[test]
    fn labels() {
        let (t, d) = tree("usecase-beta\nA -- files R&D report --> B\nC(Costs 19.95 EUR!)\n");
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("\"files R&D report\""), "{t}");
        assert!(t.contains("\"Costs 19.95 EUR!\""), "{t}");
        // `A -- B` has no label: there is no closing operator.
        let (t, d) = tree("usecase-beta\nA -- B\n");
        assert!(d.is_empty(), "{d:?}");
        assert!(!t.contains("EDGE_LABEL"), "{t}");
    }

    #[test]
    fn errors() {
        let e = |src: &str| tree(&format!("usecase-beta\n{src}\n")).1;
        assert_eq!(tree("usecase\n").1, ["expected `usecase-beta` header"]);
        assert_eq!(
            e("A --> B C"),
            ["expected the end of the line: put each statement on its own line"]
        );
        assert_eq!(
            e("systemBoundary s\n  A --> B\nend"),
            ["relationships go after the system boundary's `end`"]
        );
        assert_eq!(
            e("systemBoundary s\n  A"),
            ["unclosed `systemBoundary`: expected `end`"]
        );
        assert_eq!(e("end"), ["unexpected `end`: no system boundary is open"]);
        assert_eq!(
            e("skinparam x"),
            ["`skinparam` is PlantUML syntax that Mermaid use case diagrams do not support"]
        );
        assert_eq!(
            e("A ..> B"),
            ["expected `: include` or `: extend` after `..>`"]
        );
        assert_eq!(e("A(\"x\""), ["expected `)`"]);
        assert_eq!(e("A<<x"), ["unclosed stereotype: expected `>>`"]);
        assert_eq!(e("note A x"), ["expected `for`"]);
        assert_eq!(
            e("json P@{ \"a\": 1"),
            ["unclosed JSON object: expected `}`"]
        );
        assert_eq!(
            e("A; B"),
            ["expected the end of the line: put each statement on its own line"]
        );
    }
}
