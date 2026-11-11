//! Line-oriented parser for Mermaid block diagrams, producing a lossless rowan tree (MBLOCK-1).
//! A line holds any number of statements: blocks, `space`, edges between blocks, and `columns`;
//! `block:id` ... `end` nests a group.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan::{self, Stroke};
use crate::lines::{Base, end_block_len, too_deep};
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "block-beta", word_len);
    p.statements(0);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["block-beta", "block"];

const STATEMENT: &str = "expected a block such as `a` or `a[\"Label\"]`, `space`, `columns`, \
                         `block:id`, `end`, an edge such as `a --> b`, `classDef`, `class`, or \
                         `style`";

/// How deep groups nest (LANGS-8). Lower than the shared limit: sizing a group sizes each of
/// its groups twice, as Mermaid does, so the work doubles with every level.
const MAX_GROUPS: usize = 12;

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

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && id_len(s) == kw.len()
}

/// A block id, as Mermaid reads one: anything up to a shape, an edge, `:`, `,`, `;`, or space.
pub(crate) fn id_len(s: &str) -> usize {
    s.find(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '(' | '[' | ')' | ']' | '{' | '}' | '<' | '>' | ':' | '=' | '-' | ';' | ',' | '"'
            )
    })
    .unwrap_or(s.len())
}

/// A class name or an id in `class` and `style`: letters, digits, `_`, and `-`.
fn word_id_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(s.len())
}

/// The directions a block arrow may point.
const DIRECTIONS: &[&str] = &["right", "left", "up", "down", "x", "y"];

impl Parser<'_> {
    /// Statements up to the end of input, or at `depth` > 0 up to and including `end`.
    fn statements(&mut self, depth: usize) {
        loop {
            self.trivia();
            if self.at_eof() {
                if depth > 0 {
                    self.unclosed("unclosed `block`: expected `end`".into());
                }
                return;
            }
            if keyword(self.rest(), "end") {
                if depth > 0 {
                    self.token(END_KW, 3);
                    return;
                }
                let start = self.pos;
                self.start(ERROR);
                self.token(ERROR_TOKEN, 3);
                self.finish();
                self.error(start, start + 3, "`end` without a `block` to close");
                continue;
            }
            let before = self.pos;
            self.statement(depth);
            if self.pos == before {
                self.fail(STATEMENT);
            }
        }
    }

    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "classDef") {
            self.class_def(word_id_len);
        } else if keyword(r, "class") {
            self.class_stmt(word_id_len, "a block id");
        } else if keyword(r, "style") {
            self.style(word_id_len, "a block id");
        } else if keyword(r, "columns") {
            self.columns();
        } else if r.starts_with("block:") || keyword(r, "block") {
            self.group(depth);
        } else if id_len(r) > 0 {
            self.vertex();
        }
    }

    /// `columns N` or `columns auto`.
    fn columns(&mut self) {
        self.start(SETTING_STMT);
        self.token(SETTING_KW, "columns".len());
        self.ws();
        let n = scan::number_len(self.rest());
        if n > 0 {
            self.token(NUMBER, n);
        } else if keyword(self.rest(), "auto") {
            self.token(IDENT, 4);
        } else {
            self.fail("expected a number of columns such as `3`, or `auto`");
        }
        self.finish();
    }

    /// `block:id[shape][:width]` or `block`, its statements, and `end`.
    fn group(&mut self, depth: usize) {
        self.start(COMPOSITE);
        self.start(SUBGRAPH_HEADER);
        self.token(SUBGRAPH_KW, "block".len());
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            if id_len(self.rest()) > 0 {
                self.node();
            } else {
                self.fail("expected a group id after `block:`");
            }
        }
        self.finish();
        if depth < MAX_GROUPS {
            self.statements(depth + 1);
        } else {
            // LANGS-8: not read; its content is one `ERROR` node.
            let end = end_block_len(self.rest(), &["block:", "block"]).map(|n| self.pos + n);
            self.skip(end, too_deep("blocks", MAX_GROUPS));
            if end.is_some() {
                self.token(END_KW, 3);
            } else {
                self.unclosed("unclosed `block`: expected `end`".into());
            }
        }
        self.finish();
    }

    /// A block, or blocks joined by edges, on one line.
    fn vertex(&mut self) {
        self.start(VERTEX_STMT);
        self.node();
        loop {
            let ws = scan::ws_len(self.rest());
            if scan::stroke(&self.rest()[ws..]).is_none() {
                break;
            }
            self.ws();
            if !self.edge() {
                break;
            }
            self.ws();
            if id_len(self.rest()) == 0 {
                self.fail("expected a block after the edge");
                break;
            }
            self.node();
        }
        self.finish();
    }

    /// An id with an optional shape and label, then an optional `:width`.
    fn node(&mut self) {
        self.start(NODE);
        let n = id_len(self.rest());
        self.token(IDENT, n);
        let r = self.rest();
        if r.starts_with("<[") {
            self.block_arrow();
        } else if let Some((len, opener)) = scan::shape_open(r) {
            self.start(SHAPE);
            self.token(SHAPE_OPEN, len);
            self.start(LABEL);
            match self.rest().chars().next() {
                Some('"') => match scan::string_len(self.rest()) {
                    Some(n) if n <= scan::line_len(self.rest()) => {
                        let kind = if self.rest().starts_with("\"`") {
                            MD_STRING
                        } else {
                            STRING
                        };
                        self.token(kind, n);
                    }
                    _ => {
                        self.finish();
                        self.fail("unclosed `\"`: expected a closing `\"` on the same line");
                        self.finish();
                        self.finish();
                        return;
                    }
                },
                _ => {
                    let n =
                        scan::shape_text_len(self.rest(), opener).min(scan::line_len(self.rest()));
                    self.token(TEXT, n);
                }
            }
            self.finish();
            match scan::shape_close(self.rest(), opener) {
                Some(n) => self.token(SHAPE_CLOSE, n),
                None => {
                    let close = scan::closers(opener)[0];
                    self.fail(&format!("expected `{close}` to close the shape"));
                    self.finish();
                    self.finish();
                    return;
                }
            }
            self.finish();
        }
        if self.rest().starts_with(':') && scan::number_len(&self.rest()[1..]) > 0 {
            self.token(COLON, 1);
            let n = scan::number_len(self.rest());
            self.token(NUMBER, n);
        }
        self.finish();
    }

    /// `<["label"]>(right)`: a block drawn as an arrow, pointing in the listed directions.
    fn block_arrow(&mut self) {
        self.start(SHAPE);
        self.token(SHAPE_OPEN, 2);
        self.start(LABEL);
        let r = self.rest();
        let line = scan::line_len(r);
        if r.starts_with('"') {
            match scan::string_len(r).filter(|&n| n <= line) {
                Some(n) => self.token(STRING, n),
                None => {
                    self.finish();
                    self.fail("unclosed `\"`: expected a closing `\"` on the same line");
                    self.finish();
                    return;
                }
            }
        } else {
            let n = r[..line].find("]>").unwrap_or(line);
            self.token(TEXT, n);
        }
        self.finish();
        if !self.rest().starts_with("]>") {
            self.fail("expected `]>` to close the block arrow");
            self.finish();
            return;
        }
        self.token(SHAPE_CLOSE, 2);
        self.ws();
        if !self.rest().starts_with('(') {
            self.fail("expected the arrow's direction, such as `(right)`");
            self.finish();
            return;
        }
        self.token(L_PAREN, 1);
        loop {
            self.ws();
            let r = self.rest();
            match DIRECTIONS.iter().find(|d| keyword(r, d)) {
                Some(d) => self.token(DIR, d.len()),
                None => {
                    self.fail("expected `right`, `left`, `up`, `down`, `x`, or `y`");
                    self.finish();
                    return;
                }
            }
            self.ws();
            if self.rest().starts_with(',') {
                self.token(COMMA, 1);
            } else {
                break;
            }
        }
        if self.rest().starts_with(')') {
            self.token(R_PAREN, 1);
        } else {
            self.fail("expected `)` after the arrow's direction");
        }
        self.finish();
    }

    /// An edge stroke, with an optional label between its parts. Returns whether it is whole.
    fn edge(&mut self) -> bool {
        self.start(EDGE);
        let ok = match scan::stroke(self.rest()) {
            Some(Stroke::Full(n)) => {
                self.token(STROKE, n);
                true
            }
            Some(Stroke::Open(n, kind)) => {
                self.token(STROKE_OPEN, n);
                self.ws();
                self.start(EDGE_LABEL);
                let r = self.rest();
                let line = scan::line_len(r);
                if r.starts_with('"') {
                    match scan::string_len(r).filter(|&n| n <= line) {
                        Some(n) => self.token(STRING, n),
                        None => {
                            self.finish();
                            self.fail("unclosed `\"`: expected a closing `\"` on the same line");
                            self.finish();
                            return false;
                        }
                    }
                } else {
                    let n = scan::inline_edge_text_len(r, kind);
                    let n = scan::trim_end_len(r, n);
                    self.token(TEXT, n);
                }
                self.finish();
                self.ws();
                match scan::closing_stroke(self.rest(), kind) {
                    Some(n) => {
                        self.token(STROKE, n);
                        true
                    }
                    None => {
                        self.fail("expected the rest of the edge, such as `-->`");
                        false
                    }
                }
            }
            None => false,
        };
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
            "block-beta\ncolumns 3\na[\"A\"] b:2 space:2\nblock:g:2\n  columns auto\n  c((\"C\")) d\nend\n\
             e<[\"go\"]>(right, down)\na -- \"x\" --> b\nc --> d ==> e\nclassDef k fill:#f00\nclass a,b k\n\
             style c fill:#0f0\nblock\n  f\nend\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("COMPOSITE@").count(), 2);
        assert_eq!(t.matches("EDGE@").count(), 3);
        assert_eq!(t.matches("SETTING_STMT@").count(), 2);
        assert_eq!(t.matches("DIR@").count(), 2);
        assert!(
            t.contains("CLASSDEF_STMT@") && t.contains("CLASS_STMT@") && t.contains("STYLE_STMT@")
        );
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("block-beta\n{s}\n")).1;
        assert_eq!(
            e("a[\"A]"),
            ["unclosed `\"`: expected a closing `\"` on the same line"]
        );
        assert_eq!(e("a[\"A\""), ["expected `]` to close the shape"]);
        assert_eq!(e("a -->"), ["expected a block after the edge"]);
        assert_eq!(
            e("a<[\"x\"]>"),
            ["expected the arrow's direction, such as `(right)`"]
        );
        assert_eq!(
            e("a<[\"x\"]>(sideways)"),
            ["expected `right`, `left`, `up`, `down`, `x`, or `y`"]
        );
        assert_eq!(
            e("columns many"),
            ["expected a number of columns such as `3`, or `auto`"]
        );
        assert_eq!(e("end"), ["`end` without a `block` to close"]);
        assert_eq!(e("block:g\na"), ["unclosed `block`: expected `end`"]);
        assert_eq!(e("--> a"), [STATEMENT]);
        assert_eq!(tree("flow\n").1[0], "expected `block-beta` header");
    }
}
