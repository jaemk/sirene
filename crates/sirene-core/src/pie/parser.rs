//! Line-oriented parser for Mermaid pie charts, producing a lossless rowan tree (MPIE-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header();
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["pie"];

const STATEMENT: &str = "expected a slice (`\"label\" : value`) or `title`";

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

/// The length of a number at the start of `s`: an optional `-`, digits, and an optional
/// fraction.
pub(crate) fn number_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = usize::from(b.first() == Some(&b'-'));
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(i);
    if int == 0 {
        return 0;
    }
    i += int;
    if b.get(i) == Some(&b'.') {
        let frac = digits(i + 1);
        if frac > 0 {
            i += 1 + frac;
        }
    }
    i
}

impl Parser<'_> {
    /// `pie`, optionally `showData`; a statement may follow on the same line.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        if !keyword(self.rest(), "pie") {
            self.fail("expected `pie` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "pie".len());
        self.ws();
        if keyword(self.rest(), "showData") {
            self.token(SHOW_DATA_KW, "showData".len());
        }
        self.finish();
    }

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
        } else if r.starts_with(['"', '\'']) {
            self.slice();
        }
    }

    /// `"label" : value`.
    fn slice(&mut self) {
        self.start(SLICE_STMT);
        let r = self.rest();
        let quote = &r[..1];
        let line = &r[..scan::line_len(r)];
        match line[1..].find(quote) {
            Some(i) => self.token(STRING, i + 2),
            None => {
                self.fail(&format!("unclosed string: expected `{quote}`"));
                self.finish();
                return;
            }
        }
        self.ws();
        if !self.rest().starts_with(':') {
            self.fail("expected `:`");
            self.finish();
            return;
        }
        self.token(COLON, 1);
        self.ws();
        let n = number_len(self.rest());
        if n == 0 {
            self.fail("expected a number");
        } else {
            self.token(NUMBER, n);
            self.end();
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
    fn statements() {
        let (t, d) = tree("pie showData title T\n  \"a\" : 1\n  'b':2.5\n  \"c\" : -3\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("SLICE_STMT").count(), 3);
        assert_eq!(t.matches("NUMBER@").count(), 3);
        assert!(t.contains("SHOW_DATA_KW") && t.contains("TITLE_STMT"));
    }

    #[test]
    fn errors() {
        assert_eq!(tree("pies\n").1, ["expected `pie` header"]);
        assert_eq!(tree("pie\n  \"a\n").1, ["unclosed string: expected `\"`"]);
        assert_eq!(tree("pie\n  \"a\" 1\n").1, ["expected `:`"]);
        assert_eq!(tree("pie\n  \"a\" : x\n").1, ["expected a number"]);
        assert_eq!(
            tree("pie\n  \"a\" : 1 2\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("pie\n  a : 1\n").1, [STATEMENT]);
    }

    #[test]
    fn numbers() {
        assert_eq!(number_len("42"), 2);
        assert_eq!(number_len("-4.25 x"), 5);
        assert_eq!(number_len("3."), 1);
        assert_eq!(number_len("-"), 0);
        assert_eq!(number_len(".5"), 0);
    }
}
