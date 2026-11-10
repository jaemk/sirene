//! Line-oriented parser for Mermaid user journeys, producing a lossless rowan tree (MJOUR-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "journey", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["journey"];

const STATEMENT: &str = "expected a task (`name: score: actors`), `section`, or `title`";

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

/// The length of text at the start of `s` up to one of `stops` or the end of the line, without
/// trailing whitespace.
fn text_len(s: &str, stops: &[char]) -> usize {
    let line = &s[..scan::line_len(s)];
    let end = line.find(stops).unwrap_or(line.len());
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "title") {
            self.keyword_line(TITLE_STMT, TITLE_KW, "title", &['#', ';']);
        } else if keyword(r, "section") {
            self.keyword_line(SECTION_STMT, SECTION_KW, "section", &['#', ':', ';']);
        } else {
            self.task();
        }
    }

    /// `title TEXT` or `section TEXT`, the text stopping at any of `stops`.
    fn keyword_line(&mut self, node: SyntaxKind, kw: SyntaxKind, word: &str, stops: &[char]) {
        self.start(node);
        self.token(kw, word.len());
        self.ws();
        let n = text_len(self.rest(), stops);
        self.token(TEXT, n);
        self.end();
        self.finish();
    }

    /// `name: score: actor, actor`, kept as the name and the data after the first `:`.
    fn task(&mut self) {
        let n = text_len(self.rest(), &['#', ':', ';']);
        if n == 0 {
            return;
        }
        self.start(TASK_STMT);
        self.token(TASK_NAME, n);
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            let n = text_len(self.rest(), &['#', ';']);
            self.token(TASK_DATA, n);
            self.end();
        } else {
            self.fail("expected `:` and a score after the task name");
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
        let (t, d) = tree(
            "journey\n  title My day\n  accTitle: a\n  section Work\n    Make tea: 5: Me\n    Do work : 1 : Me, Cat\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("TASK_STMT").count(), 2);
        assert!(t.contains("TASK_DATA@") && t.contains("\"1 : Me, Cat\""));
        assert!(t.contains("SECTION_STMT") && t.contains("TITLE_STMT"));
    }

    #[test]
    fn errors() {
        // The line is still read as a statement after the header error.
        assert_eq!(
            tree("journeys\n").1,
            [
                "expected `journey` header",
                "expected `:` and a score after the task name"
            ]
        );
        assert_eq!(
            tree("journey\n  Make tea\n").1,
            ["expected `:` and a score after the task name"]
        );
        assert_eq!(
            tree("journey\n  a: 5 # x\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(
            tree("journey\n  section a: b\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("journey\n  : 5\n").1, [STATEMENT]);
    }
}
