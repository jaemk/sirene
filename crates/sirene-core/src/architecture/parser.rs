//! Line-oriented parser for Mermaid architecture diagrams, producing a lossless rowan tree
//! (MARCH-1): one statement per line, each a `group`, `service`, or `junction` declaration, an
//! edge between sides, or `align`.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "architecture-beta", word_len);
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        let before = p.pos;
        p.statement();
        if p.pos == before {
            p.fail(STATEMENT);
        }
    }
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["architecture-beta", "architecture"];

const STATEMENT: &str = "expected `group`, `service`, `junction`, `align`, `title`, or an edge \
                         such as `a:R --> L:b`";

/// The kinds of item a declaration makes.
pub(crate) const DECLARATIONS: &[&str] = &["group", "service", "junction"];

struct Parser<'a>(Base<'a>);

impl<'a> Deref for Parser<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.0
    }
}

impl DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

/// An id: letters, digits, and `_`.
pub(crate) fn id_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && word_len(s) == kw.len()
}

fn is_side(c: Option<char>) -> bool {
    matches!(c, Some('L' | 'R' | 'T' | 'B'))
}

impl Parser<'_> {
    fn statement(&mut self) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, 5);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if keyword(r, "align") {
            self.align();
        } else if let Some(kw) = DECLARATIONS.iter().find(|k| keyword(r, k)) {
            self.declaration(kw.len());
        } else if id_len(r) > 0 {
            self.edge_stmt();
        }
    }

    /// `group|service|junction id(icon)[label] in parent`.
    fn declaration(&mut self, kw: usize) {
        let junction = self.rest().starts_with("junction");
        self.start(ENTITY);
        self.token(REQ_KW, kw);
        self.ws();
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected an id");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        if !junction && self.rest().starts_with('(') {
            self.start(SHAPE);
            self.token(L_PAREN, 1);
            let r = self.rest();
            let line = scan::line_len(r);
            let Some(close) = r[..line].find(')') else {
                self.fail("expected `)` after the icon");
                self.finish();
                self.finish();
                return;
            };
            self.token(TEXT, close);
            self.token(R_PAREN, 1);
            self.finish();
        }
        if !junction && self.rest().starts_with('[') {
            self.start(LABEL);
            self.token(L_BRACKET, 1);
            let r = self.rest();
            let line = scan::line_len(r);
            let text = if r.starts_with('"') {
                r[1..line].find('"').map(|i| (STRING, i + 2))
            } else {
                r[..line].find(']').map(|i| (TEXT, i))
            };
            let Some((kind, len)) = text else {
                self.fail("expected `]` after the label");
                self.finish();
                self.finish();
                return;
            };
            self.token(kind, len);
            if self.rest().starts_with(']') {
                self.token(R_BRACKET, 1);
            } else {
                self.fail("expected `]` after the label");
                self.finish();
                self.finish();
                return;
            }
            self.finish();
        }
        let ws = scan::ws_len(self.rest());
        if keyword(&self.rest()[ws..], "in") {
            self.ws();
            self.token(OF_KW, 2);
            self.ws();
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected a group id after `in`");
                self.finish();
                return;
            }
            self.token(IDENT, n);
        }
        self.end();
        self.finish();
    }

    /// `align row|column a b ...`.
    fn align(&mut self) {
        self.start(SETTING_STMT);
        self.token(SETTING_KW, 5);
        self.ws();
        let r = self.rest();
        if keyword(r, "row") || keyword(r, "column") {
            self.token(IDENT, word_len(r));
        } else {
            self.fail("expected `row` or `column`");
            self.finish();
            return;
        }
        loop {
            let ws = scan::ws_len(self.rest());
            let n = id_len(&self.rest()[ws..]);
            if n == 0 {
                break;
            }
            self.ws();
            self.start(ENTITY_REF);
            self.token(IDENT, n);
            self.finish();
        }
        self.end();
        self.finish();
    }

    /// `a{group}:R <-[label]-> L:b{group}`.
    fn edge_stmt(&mut self) {
        self.start(RELATION_STMT);
        // The left end: id, `{group}`, `:`, side.
        self.start(ENTITY_REF);
        let n = id_len(self.rest());
        self.token(IDENT, n);
        self.group_mark();
        if !self.rest().starts_with(':') {
            self.fail("expected `:` and a side, such as `a:R`");
            self.finish();
            self.finish();
            return;
        }
        self.token(COLON, 1);
        if !is_side(self.rest().chars().next()) {
            self.fail("expected a side: `L`, `R`, `T`, or `B`");
            self.finish();
            self.finish();
            return;
        }
        self.token(SIDE, 1);
        self.finish();
        self.ws();
        if !self.link() {
            self.finish();
            return;
        }
        self.ws();
        // The right end: side, `:`, id, `{group}`.
        self.start(ENTITY_REF);
        let r = self.rest();
        if !(is_side(r.chars().next()) && r[1..].starts_with(':')) {
            self.fail("expected a side and the other end, such as `L:b`");
            self.finish();
            self.finish();
            return;
        }
        self.token(SIDE, 1);
        self.token(COLON, 1);
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected the id of the other end");
            self.finish();
            self.finish();
            return;
        }
        self.token(IDENT, n);
        self.group_mark();
        self.finish();
        self.end();
        self.finish();
    }

    /// `{group}` after an edge end's id.
    fn group_mark(&mut self) {
        if self.rest().starts_with("{group}") {
            self.start(METADATA);
            self.token(L_BRACE, 1);
            self.token(IDENT, 5);
            self.token(R_BRACE, 1);
            self.finish();
        }
    }

    /// `--`, `<--`, `-->`, `<-->`, or with a label: `-[label]-`. Returns whether it is whole.
    fn link(&mut self) -> bool {
        self.start(EDGE);
        if self.rest().starts_with('<') {
            self.token(ARROW, 1);
        }
        let r = self.rest();
        let ok = if r.starts_with("-[") {
            self.token(STROKE_OPEN, 2);
            self.start(EDGE_LABEL);
            let r = self.rest();
            let line = scan::line_len(r);
            match r[..line].find("]-") {
                Some(n) => {
                    self.token(TEXT, n);
                    self.finish();
                    self.token(STROKE, 2);
                    true
                }
                None => {
                    self.finish();
                    self.fail("expected `]-` to close the edge label");
                    false
                }
            }
        } else if r.starts_with("--") {
            self.token(STROKE, 2);
            true
        } else {
            self.fail("expected an edge: `--`, `-->`, `<--`, or `<-->`");
            false
        };
        if ok && self.rest().starts_with('>') {
            self.token(ARROW, 1);
        }
        self.finish();
        ok
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
    fn statements() {
        let (t, d) = tree(
            "architecture-beta\ntitle T\ngroup api(cloud)[API]\ngroup inner[In] in api\n\
             service db(database)[\"My DB\"] in inner\nservice s(logos:aws-ec2)[Server] in api\n\
             junction j in api\ndb:R --> L:s\ns{group}:B <-[uses]-> T:j\nj:L -- R:db\n\
             align row db s\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("ENTITY@").count(), 5);
        assert_eq!(t.matches("RELATION_STMT@").count(), 3);
        assert_eq!(t.matches("ARROW@").count(), 3);
        assert_eq!(t.matches("METADATA@").count(), 1);
        assert_eq!(t.matches("EDGE_LABEL@").count(), 1);
        assert_eq!(t.matches("SETTING_STMT@").count(), 1);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("architecture-beta\n{s}\n")).1;
        assert_eq!(e("service"), ["expected an id"]);
        assert_eq!(e("service a(cloud"), ["expected `)` after the icon"]);
        assert_eq!(e("service a[A"), ["expected `]` after the label"]);
        assert_eq!(e("service a in"), ["expected a group id after `in`"]);
        assert_eq!(e("service a b"), ["expected the end of the line"]);
        assert_eq!(e("a -- b"), ["expected `:` and a side, such as `a:R`"]);
        assert_eq!(e("a:X -- L:b"), ["expected a side: `L`, `R`, `T`, or `B`"]);
        assert_eq!(
            e("a:R => L:b"),
            ["expected an edge: `--`, `-->`, `<--`, or `<-->`"]
        );
        assert_eq!(
            e("a:R -- b"),
            ["expected a side and the other end, such as `L:b`"]
        );
        assert_eq!(e("a:R -[x L:b"), ["expected `]-` to close the edge label"]);
        assert_eq!(e("align diagonal a b"), ["expected `row` or `column`"]);
        assert_eq!(e("--"), [STATEMENT]);
    }
}
