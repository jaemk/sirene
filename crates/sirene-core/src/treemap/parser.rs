//! Line-oriented parser for Mermaid treemaps, producing a lossless rowan tree (MTREE-1). Each
//! item is a quoted name, with `: value` for a leaf; its indentation places it under a section.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "treemap-beta", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["treemap-beta", "treemap"];

const STATEMENT: &str =
    "expected an item such as `\"Section\"` or `\"Leaf\": 10`, `classDef`, or `title`";

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

/// A class name: a letter or `_`, then letters, digits, and `_`.
fn id_len(s: &str) -> usize {
    if !s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return 0;
    }
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// A value as Mermaid reads it: digits, `.`, `,`, and `_`.
fn value_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '_')))
        .unwrap_or(s.len())
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
            self.rest_of_line(TEXT);
            self.finish();
        } else if keyword(r, "classDef") {
            self.class_def(id_len);
        } else if r.starts_with(['"', '\'']) {
            self.item();
        } else {
            self.fail(STATEMENT);
        }
    }

    /// `"name"`, `"name": value`, or `"name", value`, then an optional `:::class`.
    fn item(&mut self) {
        self.start(NODE);
        let r = self.rest();
        let quote = &r[..1];
        let line = &r[..scan::line_len(r)];
        let Some(close) = line[1..].find(quote) else {
            self.fail(&format!(
                "unclosed `{quote}`: expected a closing `{quote}` on the same line"
            ));
            self.finish();
            return;
        };
        self.token(STRING, close + 2);
        self.ws();
        let r = self.rest();
        if (r.starts_with(':') && !r.starts_with(":::")) || r.starts_with(',') {
            let sep = if r.starts_with(':') { COLON } else { COMMA };
            self.token(sep, 1);
            self.ws();
            let n = value_len(self.rest());
            if n == 0 {
                self.fail("expected a value such as `12`");
                self.finish();
                return;
            }
            self.token(NUMBER, n);
            self.ws();
        }
        if self.rest().starts_with(":::") {
            self.start(CLASS_SHORTHAND);
            self.token(TRIPLE_COLON, 3);
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected a class name after `:::`");
                self.finish();
                self.finish();
                return;
            }
            self.token(IDENT, n);
            self.finish();
        }
        self.end();
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
    fn items() {
        let (t, d) = tree(
            "treemap-beta\ntitle Spend\n%% c\n\"A\"\n  \"B\": 10\n  'C' , 1,000.5:::big\n\"D\":::big\nclassDef big fill:#f00;\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 4);
        assert_eq!(t.matches("NUMBER@").count(), 2);
        assert_eq!(t.matches("CLASS_SHORTHAND@").count(), 2);
        assert!(t.contains("CLASSDEF_STMT@") && t.contains("TITLE_STMT@"));
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("treemap\n{s}\n")).1;
        assert_eq!(
            e("\"A"),
            ["unclosed `\"`: expected a closing `\"` on the same line"]
        );
        assert_eq!(e("\"A\":"), ["expected a value such as `12`"]);
        assert_eq!(e("\"A\": x"), ["expected a value such as `12`"]);
        assert_eq!(e("\"A\":::"), ["expected a class name after `:::`"]);
        assert_eq!(e("\"A\" 3"), ["expected the end of the line"]);
        assert_eq!(e("A: 3"), [STATEMENT]);
        assert_eq!(tree("flow\n").1[0], "expected `treemap-beta` header");
    }
}
