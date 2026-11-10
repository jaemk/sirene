//! Line-oriented parser for Mermaid quadrant charts, producing a lossless rowan tree (MQUAD-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "quadrantChart", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["quadrantChart"];

const STATEMENT: &str = "expected a point (`name: [x, y]`), `title`, `x-axis`, `y-axis`, \
     `quadrant-1` to `quadrant-4`, or `classDef`";
const COORDS: &str = "expected `[x, y]` after the point's name";

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
    s.starts_with(kw) && word_len(s) == kw.len()
}

/// The length of text at the start of `s` up to one of `stops`, `-->` when `arrow`, or the end
/// of the line, without trailing whitespace.
fn text_len(s: &str, stops: &[char], arrow: bool) -> usize {
    let line = &s[..scan::line_len(s)];
    let mut end = line.find(stops).unwrap_or(line.len());
    if arrow && let Some(i) = line[..end].find("-->") {
        end = i;
    }
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, "title".len());
            self.ws();
            let n = text_len(self.rest(), &['#', ';'], false);
            self.token(TEXT, n);
            self.end();
            self.finish();
        } else if keyword(r, "x-axis") || keyword(r, "y-axis") {
            self.axis();
        } else if ["quadrant-1", "quadrant-2", "quadrant-3", "quadrant-4"]
            .iter()
            .any(|kw| keyword(r, kw))
        {
            self.start(QUADRANT_STMT);
            self.token(QUADRANT_KW, "quadrant-1".len());
            self.ws();
            self.text(false);
            self.end();
            self.finish();
        } else if keyword(r, "classDef") {
            self.class_def(word_len);
        } else {
            self.point();
        }
    }

    /// A `"string"`, or plain text up to `#`, `;`, or (when `arrow`) `-->`.
    fn text(&mut self, arrow: bool) {
        if let Some(n) = self.string_len() {
            self.token(STRING, n);
        } else {
            let n = text_len(self.rest(), &['#', ';'], arrow);
            self.token(TEXT, n);
        }
    }

    /// `x-axis LOW` or `x-axis LOW --> HIGH`, and the same for `y-axis`.
    fn axis(&mut self) {
        self.start(AXIS_STMT);
        self.token(AXIS_KW, "x-axis".len());
        self.ws();
        self.text(true);
        self.ws();
        if self.rest().starts_with("-->") {
            self.token(ARROW, 3);
            self.ws();
            self.text(true);
        }
        self.end();
        self.finish();
    }

    /// `name: [x, y] styles` or `name:::class: [x, y] styles`.
    fn point(&mut self) {
        let (kind, n) = match self.string_len() {
            Some(n) => (STRING, n),
            None => (TEXT, text_len(self.rest(), &['#', ':', ';'], false)),
        };
        if n == 0 {
            return;
        }
        self.start(POINT_STMT);
        self.token(kind, n);
        self.ws();
        if self.rest().starts_with(":::") {
            self.start(CLASS_SHORTHAND);
            self.token(TRIPLE_COLON, 3);
            let n = word_len(self.rest());
            if n == 0 {
                self.fail("expected a class name");
                self.finish();
                self.finish();
                return;
            }
            self.token(IDENT, n);
            self.finish();
            self.ws();
        }
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            if self.coords() {
                self.ws();
                self.rest_of_line(STYLE_TEXT);
            }
        } else {
            self.fail(COORDS);
        }
        self.finish();
    }

    /// `[x, y]`, each a number kept as written. Returns whether it is complete.
    fn coords(&mut self) -> bool {
        if !self.rest().starts_with('[') {
            self.fail(COORDS);
            return false;
        }
        self.token(L_BRACKET, 1);
        for (k, close) in [(0, ','), (1, ']')] {
            self.ws();
            let n = text_len(self.rest(), &[',', ']', '#', ';'], false);
            if n == 0 {
                self.fail(if k == 0 {
                    "expected the point's x value"
                } else {
                    "expected the point's y value"
                });
                return false;
            }
            self.token(NUMBER, n);
            self.ws();
            if !self.rest().starts_with(close) {
                self.fail(&format!("expected `{close}`"));
                return false;
            }
            self.token(if k == 0 { COMMA } else { R_BRACKET }, 1);
        }
        true
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
            "quadrantChart\n  title Reach\n  x-axis Low Reach --> High Reach\n  y-axis \"Low\"\n  \
             quadrant-1 Expand\n  Campaign A: [0.3, 0.6]\n  B:::hot: [ 1 , 0 ] radius: 9\n  \
             classDef hot color: #f00\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("POINT_STMT").count(), 2);
        assert!(t.contains("TEXT@") && t.contains("\"Low Reach\"") && t.contains("ARROW@"));
        assert!(t.contains("STRING@") && t.contains("QUADRANT_STMT"));
        assert!(t.contains("CLASS_SHORTHAND") && t.contains("CLASSDEF_STMT"));
        assert!(t.contains("STYLE_TEXT@") && t.contains("\"radius: 9\""));
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("quadrantChart\n  {s}\n")).1;
        assert_eq!(e("A"), [COORDS]);
        assert_eq!(e("A: 0.3, 0.6"), [COORDS]);
        assert_eq!(e("A: [, 1]"), ["expected the point's x value"]);
        assert_eq!(e("A: [1]"), ["expected `,`"]);
        assert_eq!(e("A: [1,]"), ["expected the point's y value"]);
        assert_eq!(e("A: [1, 2"), ["expected `]`"]);
        assert_eq!(e("A::: : [1, 2]"), ["expected a class name"]);
        assert_eq!(e("x-axis a --> b --> c"), ["expected the end of the line"]);
        assert_eq!(e(": [1, 2]"), [STATEMENT]);
        assert_eq!(
            tree("quadrant\n").1,
            ["expected `quadrantChart` header", COORDS]
        );
    }
}
