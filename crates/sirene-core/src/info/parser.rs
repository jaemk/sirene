//! Line-oriented parser for Mermaid info diagrams, producing a lossless rowan tree (MINFO-1).

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

pub(crate) const HEADERS: &[&str] = &["info"];

const STATEMENT: &str = "expected `showInfo`, `title`, `accTitle`, or `accDescr`";

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

impl Parser<'_> {
    /// `info`, optionally `showInfo` on the same line.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        if !scan::keyword(self.rest(), "info") {
            self.fail("expected `info` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "info".len());
        self.ws();
        if scan::keyword(self.rest(), "showInfo") {
            self.token(SHOW_INFO_KW, "showInfo".len());
        }
        self.end();
        self.finish();
    }

    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if scan::keyword(r, "accTitle") || scan::keyword(r, "accDescr") {
            self.acc();
        } else if scan::keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, "title".len());
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if scan::keyword(r, "showInfo") {
            self.start(SHOW_INFO_STMT);
            self.token(SHOW_INFO_KW, "showInfo".len());
            self.end();
            self.finish();
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
        let (t, d) = tree("info showInfo\n");
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("SHOW_INFO_KW"));
        let (t, d) = tree("%% c\ninfo\n  showInfo\n  title T\n  accTitle: a\n");
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("SHOW_INFO_STMT") && t.contains("TITLE_STMT"));
        assert!(t.contains("ACC_TITLE_STMT"));
    }

    #[test]
    fn errors() {
        assert_eq!(tree("infos\n").1, ["expected `info` header"]);
        assert_eq!(tree("info x\n").1, ["expected the end of the line"]);
        assert_eq!(
            tree("info\n  showInfo x\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("info\n  A --> B\n").1, [STATEMENT]);
    }
}
