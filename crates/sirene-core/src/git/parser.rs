//! Line-oriented parser for Mermaid git graphs, producing a lossless rowan tree (MGIT-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{self, Base};
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header();
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["gitGraph"];

const STATEMENT: &str = "expected `commit`, `branch`, `checkout`, `merge`, or `cherry-pick`";

/// The statement keywords, longest first where one starts another.
pub(crate) const KEYWORDS: &[&str] = &[
    "commit",
    "branch",
    "checkout",
    "switch",
    "merge",
    "cherry-pick",
];

/// The attribute names, as `name:`.
pub(crate) const ATTRIBUTES: &[&str] = &["id", "type", "tag", "msg", "order", "parent"];

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

/// Length of a word at the start of `s`: letters, digits, `-`, and `_`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && word_len(s) == kw.len()
}

/// Length of an unquoted branch name or value at the start of `s`: up to whitespace or `;`.
pub(crate) fn name_len(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(s.len())
}

impl Parser<'_> {
    /// `gitGraph`, an optional `LR`, `TB`, or `BT`, and an optional `:`.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if !keyword(r, "gitGraph") {
            // The line is the error, not also a statement.
            self.fail("expected `gitGraph` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "gitGraph".len());
        self.ws();
        let r = self.rest();
        if ["LR", "TB", "BT"].iter().any(|d| keyword(r, d)) {
            self.token(DIR, 2);
            self.ws();
        }
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
        }
        self.end();
        self.finish();
    }

    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if let Some(kw) = KEYWORDS.iter().find(|kw| keyword(r, kw)) {
            self.git_stmt(kw);
        }
    }

    /// A statement: its keyword, a branch name for those that take one, then attributes.
    fn git_stmt(&mut self, kw: &str) {
        self.start(GIT_STMT);
        self.token(GIT_KW, kw.len());
        if matches!(kw, "branch" | "checkout" | "switch" | "merge") && !self.name() {
            self.fail("expected a branch name");
            self.finish();
            return;
        }
        self.attributes(kw == "commit");
        self.finish();
    }

    /// A branch name, quoted or not. Returns whether there was one.
    fn name(&mut self) -> bool {
        self.ws();
        let r = self.rest();
        if r.starts_with('"') {
            match self.string_len() {
                Some(n) => {
                    self.token(STRING, n);
                    true
                }
                None => false,
            }
        } else {
            let n = name_len(r);
            self.token(IDENT, n);
            n > 0
        }
    }

    /// `name: value` pairs up to the end of the line. With `message`, a bare `"..."` is an
    /// attribute without a key: a commit's `msg:` is optional.
    fn attributes(&mut self, message: bool) {
        loop {
            let n = scan::ws_len(self.rest());
            if lines::is_end(&self.rest()[n..]) {
                return;
            }
            self.ws();
            let r = self.rest();
            if message && r.starts_with('"') {
                let Some(n) = self.string_len() else {
                    self.fail("unclosed string: expected `\"`");
                    return;
                };
                self.start(ATTR);
                self.token(STRING, n);
                self.finish();
                continue;
            }
            let key = word_len(r);
            let colon = key + scan::ws_len(&r[key..]);
            if !ATTRIBUTES.contains(&&r[..key]) || !r[colon..].starts_with(':') {
                self.fail("expected an attribute such as `id: \"...\"`");
                return;
            }
            self.start(ATTR);
            self.token(ATTR_KEY, key);
            self.ws();
            self.token(COLON, 1);
            self.ws();
            let r = self.rest();
            if r.starts_with('"') {
                match self.string_len() {
                    Some(n) => self.token(STRING, n),
                    None => {
                        self.fail("unclosed string: expected `\"`");
                        self.finish();
                        return;
                    }
                }
            } else {
                let n = name_len(r);
                if n == 0 {
                    self.fail("expected a value");
                    self.finish();
                    return;
                }
                let kind = if r[..n].bytes().all(|b| b.is_ascii_digit()) {
                    NUMBER
                } else {
                    IDENT
                };
                self.token(kind, n);
            }
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
    fn headers() {
        for src in [
            "gitGraph\n",
            "gitGraph:\n",
            "gitGraph TB:\n",
            "gitGraph BT\n",
        ] {
            assert!(tree(src).1.is_empty(), "{src}");
        }
        assert_eq!(tree("gitGraph XY:\n").1, ["expected the end of the line"]);
        assert_eq!(tree("gitGraphs\n").1, ["expected `gitGraph` header"]);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "gitGraph\n  commit id: \"a\" type: REVERSE tag:\"v1\"\n  branch \"cherry-pick\" order: 2\n  \
             switch main\n  merge develop\n  cherry-pick id: \"a\" parent:\"b\"\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("GIT_STMT").count(), 5);
        assert_eq!(t.matches("ATTR@").count(), 6);
        assert!(t.contains("NUMBER"));
    }

    #[test]
    fn errors() {
        let (_, d) =
            tree("gitGraph\n  branch\n  commit id: \"x\n  commit foo\n  commit id:\n  push\n");
        assert_eq!(
            d,
            [
                "expected a branch name",
                "unclosed string: expected `\"`",
                "expected an attribute such as `id: \"...\"`",
                "expected a value",
                STATEMENT,
            ]
        );
    }

    #[test]
    fn words() {
        assert_eq!(word_len("cherry-pick id"), 11);
        assert_eq!(name_len("feature/x;"), 9);
        assert!(keyword("merge x", "merge"));
        assert!(!keyword("merged", "merge"));
    }
}
