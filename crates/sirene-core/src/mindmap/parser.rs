//! Line-oriented parser for Mermaid mindmaps, producing a lossless rowan tree (MMIND-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    parse_with(src, HEADERS)
}

/// Parses a document of indented nodes under one of `headers`, the first named in the error
/// for a missing one. Kanban boards share this grammar (MKAN-1).
pub(crate) fn parse_with(src: &str, headers: &[&str]) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src), Next::default(), Next::default());
    p.start(ROOT);
    p.header(headers, headers[0], word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["mindmap"];

const STATEMENT: &str = "expected a node, `::icon(...)`, or `:::class`";

/// Shape delimiters, longest first: the opening and closing text.
pub(crate) const SHAPES: [(&str, &str); 6] = [
    ("((", "))"),
    ("))", "(("),
    ("{{", "}}"),
    ("(", ")"),
    (")", "("),
    ("[", "]"),
];

/// The base parser, and the next `` `" `` and `}` for strings and metadata that may span lines.
struct Parser<'a>(Base<'a>, Next, Next);

/// The next match of a needle at or after a position. A later search from a position no further
/// than that match reuses it, so unclosed openers on many lines do not each rescan the rest.
#[derive(Default)]
struct Next {
    from: usize,
    at: Option<Option<usize>>,
}

impl Next {
    /// The absolute offset of the first `needle` in `rest`, which starts at `pos`.
    fn find(&mut self, rest: &str, pos: usize, needle: &str) -> Option<usize> {
        if let Some(at) = self.at
            && pos >= self.from
            && at.is_none_or(|a| pos <= a)
        {
            return at;
        }
        let at = rest.find(needle).map(|i| pos + i);
        self.from = pos;
        self.at = Some(at);
        at
    }
}

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

/// The length of a node id at the start of `s`: text up to a shape delimiter, `@{`, or the end
/// of the line, without trailing whitespace.
fn id_len(s: &str) -> usize {
    let line = &s[..scan::line_len(s)];
    let mut end = line.find(['(', '[', ')', '{', '}']).unwrap_or(line.len());
    if let Some(at) = line.find("@{") {
        end = end.min(at);
    }
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if r.starts_with(":::") {
            self.start(CLASS_SHORTHAND);
            self.token(TRIPLE_COLON, 3);
            self.ws();
            if scan::line_len(self.rest()) == 0 {
                self.fail("expected a class name");
            } else {
                self.rest_of_line(TEXT);
            }
            self.finish();
        } else if r.starts_with("::icon(") {
            self.icon();
        } else {
            self.node();
        }
    }

    /// `::icon(name)`.
    fn icon(&mut self) {
        self.start(ICON_STMT);
        self.token(ICON_KW, "::icon".len());
        self.token(L_PAREN, 1);
        let r = self.rest();
        match r[..scan::line_len(r)].find(')') {
            Some(i) => {
                self.token(TEXT, i);
                self.token(R_PAREN, 1);
                self.end();
            }
            None => self.fail("unclosed `::icon(`: expected `)`"),
        }
        self.finish();
    }

    /// `id`, `id` and a shape, or a shape alone.
    fn node(&mut self) {
        let n = id_len(self.rest());
        let r = &self.rest()[n..];
        let r = &r[scan::ws_len(r)..];
        let shape = SHAPES.iter().find(|(open, _)| r.starts_with(open));
        if n == 0 && shape.is_none() {
            return;
        }
        self.start(NODE);
        self.token(IDENT, n);
        self.ws();
        let closed = match shape {
            Some(&(open, close)) => self.shape(open, close),
            None => true,
        };
        if closed {
            self.ws();
            if !self.rest().starts_with("@{") || self.metadata() {
                self.end();
            }
        }
        self.finish();
    }

    /// `@{ key: value, ... }`, which may span lines. Returns whether it closed.
    fn metadata(&mut self) -> bool {
        self.start(METADATA);
        self.token(META_OPEN, 2);
        let pos = self.pos;
        let close = self.0.rest();
        let closed = match self.2.find(close, pos, "}").map(|at| at - pos) {
            Some(i) => {
                self.token(META_BODY, i);
                self.token(META_CLOSE, 1);
                true
            }
            None => {
                let start = self.pos;
                let n = scan::line_len(self.rest());
                self.token(META_BODY, n);
                self.error(start - 2, start + n, "unclosed `@{`: expected `}`");
                false
            }
        };
        self.finish();
        closed
    }

    /// A shape's delimiters around its text: a `"string"`, a markdown ``"`string`"``, or text.
    /// Returns whether it closed.
    fn shape(&mut self, open: &str, close: &str) -> bool {
        self.start(SHAPE);
        self.token(SHAPE_OPEN, open.len());
        let r = self.rest();
        let ws = scan::ws_len(r);
        let quoted = &r[ws..];
        let line = &r[..scan::line_len(r)];
        if let Some(body) = quoted.strip_prefix("\"`") {
            self.token(WHITESPACE, ws);
            // A markdown string may span lines.
            let from = self.pos + 2;
            match self.1.find(body, from, "`\"").map(|at| at - from) {
                Some(i) => self.token(MD_STRING, i + 4),
                None => {
                    self.fail("unclosed markdown string: expected `` `\" ``");
                    self.finish();
                    return false;
                }
            }
            self.ws();
        } else if quoted.starts_with('"') {
            self.token(WHITESPACE, ws);
            match self.string_len() {
                Some(n) => self.token(STRING, n),
                None => {
                    self.fail("unclosed string: expected `\"`");
                    self.finish();
                    return false;
                }
            }
            self.ws();
        } else {
            let n = line.find(close).unwrap_or(line.len());
            self.token(TEXT, n);
        }
        if self.rest().starts_with(close) {
            self.token(SHAPE_CLOSE, close.len());
            self.finish();
            true
        } else {
            self.fail(&format!("unclosed `{open}`: expected `{close}`"));
            self.finish();
            false
        }
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
            "mindmap\n  root((Root))\n    A[Square]\n    ::icon(fa fa-book)\n    :::urgent large\n    Plain text\n    (Rounded)\n    b))Bang((\n    c)Cloud(\n    d{{Hex}}\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 7);
        assert_eq!(t.matches("SHAPE@").count(), 6);
        assert!(t.contains("ICON_STMT") && t.contains("CLASS_SHORTHAND"));
        assert!(t.contains("IDENT@") && t.contains("\"Plain text\""));
    }

    #[test]
    fn strings() {
        let (t, d) = tree("mindmap\n  a[\"quoted (x)\"]\n  b[\"`**md**\n  next`\"]\n");
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("STRING@") && t.contains("MD_STRING@"));
    }

    #[test]
    fn errors() {
        assert_eq!(tree("mindmaps\n").1, ["expected `mindmap` header"]);
        assert_eq!(tree("mindmap\n  a[x\n").1, ["unclosed `[`: expected `]`"]);
        assert_eq!(
            tree("mindmap\n  a[\"x\n").1,
            ["unclosed string: expected `\"`"]
        );
        assert_eq!(
            tree("mindmap\n  a[\"`x\n").1,
            ["unclosed markdown string: expected `` `\" ``"]
        );
        assert_eq!(
            tree("mindmap\n  a[x] y\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(
            tree("mindmap\n  ::icon(x\n").1,
            ["unclosed `::icon(`: expected `)`"]
        );
        assert_eq!(tree("mindmap\n  :::\n").1, ["expected a class name"]);
        assert_eq!(
            tree("mindmap\n  a{x}\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("mindmap\n  {x\n").1, [STATEMENT]);
        assert_eq!(
            tree("mindmap\n  a[x]@{ k: v\n").1,
            ["unclosed `@{`: expected `}`"]
        );
        assert_eq!(
            tree("mindmap\n  a[x]@{ k: v } y\n").1,
            ["expected the end of the line"]
        );
    }

    #[test]
    fn metadata() {
        let (t, d) = tree("mindmap\n  a[x]@{ k: v }\n  b @{\n    k: v\n  }\n  mail@x.com\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("METADATA@").count(), 2);
        assert!(t.contains("\"mail@x.com\""));
    }

    #[test]
    fn ids() {
        assert_eq!(id_len("Long text here  \nx"), 14);
        assert_eq!(id_len("id[x]"), 2);
        assert_eq!(id_len("id [x]"), 2);
        assert_eq!(id_len("[x]"), 0);
    }
}
