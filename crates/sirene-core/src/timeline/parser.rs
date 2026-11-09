//! Line-oriented parser for Mermaid timelines, producing a lossless rowan tree (TIMELI-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header();
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["timeline"];

const STATEMENT: &str = "expected a time period, `: event`, `section`, or `title`";

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

/// The length of text at the start of `s` up to the next separating `:` (one followed by
/// whitespace or the end of the line) or the end of the line, without trailing whitespace.
pub(crate) fn text_len(s: &str) -> usize {
    let line = &s[..scan::line_len(s)];
    let end = line
        .char_indices()
        .find(|&(i, c)| c == ':' && line[i + 1..].chars().next().is_none_or(char::is_whitespace))
        .map_or(line.len(), |(i, _)| i);
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    /// `timeline`, optionally `LR` or `TD`.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        if !keyword(self.rest(), "timeline") {
            self.fail("expected `timeline` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "timeline".len());
        self.ws();
        let r = self.rest();
        if ["LR", "TD"].iter().any(|d| keyword(r, d)) {
            self.token(DIR, 2);
        }
        self.end();
        self.finish();
    }

    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "title") {
            self.keyword_line(TITLE_STMT, TITLE_KW, "title");
        } else if keyword(r, "section") {
            self.keyword_line(SECTION_STMT, SECTION_KW, "section");
        } else if r.starts_with(':') {
            self.start(EVENTS_STMT);
            self.events();
            self.finish();
        } else {
            let n = text_len(r);
            if n == 0 {
                return;
            }
            self.start(PERIOD_STMT);
            self.token(PERIOD, n);
            self.events();
            self.finish();
        }
    }

    /// `title TEXT` or `section TEXT`.
    fn keyword_line(&mut self, node: SyntaxKind, kw: SyntaxKind, word: &str) {
        self.start(node);
        self.token(kw, word.len());
        self.ws();
        self.rest_of_line(TEXT);
        self.finish();
    }

    /// `: EVENT` pairs to the end of the line.
    fn events(&mut self) {
        loop {
            self.ws();
            if !self.rest().starts_with(':') {
                break;
            }
            self.token(COLON, 1);
            self.ws();
            let n = text_len(self.rest());
            self.token(EVENT, n);
        }
        self.end();
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
            "timeline TD\n  title T\n  section S\n  2002 : A : B\n       : C\n  10:30 : stand:up\n  x :\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("PERIOD_STMT").count(), 3);
        assert_eq!(t.matches("EVENTS_STMT").count(), 1);
        assert_eq!(t.matches("EVENT@").count(), 4);
        assert!(t.contains("PERIOD@") && t.contains("\"10:30\""));
        assert!(t.contains("\"stand:up\""));
    }

    #[test]
    fn headers() {
        assert!(tree("timeline\n").1.is_empty());
        assert!(tree("timeline LR\n").1.is_empty());
        assert_eq!(tree("timeline XY\n").1, ["expected the end of the line"]);
        assert_eq!(tree("timelines\n").1, ["expected `timeline` header"]);
    }

    #[test]
    fn text_lengths() {
        assert_eq!(text_len("2002 : x"), 4);
        assert_eq!(text_len("10:30 : x"), 5);
        assert_eq!(text_len("a:"), 1);
        assert_eq!(text_len("tail  \nnext"), 4);
    }
}
