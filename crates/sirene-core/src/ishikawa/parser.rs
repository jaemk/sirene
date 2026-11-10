//! Line-oriented parser for Mermaid ishikawa diagrams, producing a lossless rowan tree (MISH-1).

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
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        p.line();
    }
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["ishikawa", "ishikawa-beta"];

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
    /// `ishikawa-beta` or `ishikawa`; the effect may follow on the same line.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        let Some(kw) = HEADERS
            .iter()
            .rev()
            .find(|kw| r.starts_with(**kw) && scan::id_len(r) == kw.len())
        else {
            self.fail("expected `ishikawa-beta` header");
            return;
        };
        self.start(HEADER);
        self.token(GRAPH_KW, kw.len());
        self.finish();
    }

    /// One line: a cause, or the effect when it is the first.
    fn line(&mut self) {
        self.start(NODE);
        self.rest_of_line(TEXT);
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
    fn lines() {
        let (t, d) = tree("ishikawa-beta\n  Effect\n  %% c\n    A } ; b\n\n    B\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 3);
        assert!(t.contains("COMMENT"));
        let (t, d) = tree("ishikawa Effect\n  A");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 2);
    }

    #[test]
    fn errors() {
        assert_eq!(tree("fishbone\n").1, ["expected `ishikawa-beta` header"]);
    }
}
