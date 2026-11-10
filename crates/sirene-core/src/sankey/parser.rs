//! Line-oriented parser for Mermaid sankey diagrams, producing a lossless rowan tree
//! (MSANKEY-1). Each row is CSV: `source,target,value`, a field quoted when it holds a comma.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, number_len};
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "sankey", word_len);
    Base::block(&mut p, 0, "block", ROW, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["sankey", "sankey-beta"];

const ROW: &str = "expected a row such as `Source,Target,12.5`";

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

/// A `"..."` field at the start of `s`, with `""` for a quote inside, closed on the same line.
fn quoted_len(s: &str) -> Option<usize> {
    let line = &s[..scan::line_len(s)];
    let mut chars = line.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            if chars.peek().is_some_and(|&(_, c)| c == '"') {
                chars.next();
            } else {
                return Some(i + 1);
            }
        }
    }
    None
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        self.start(FLOW);
        for what in ["source", "target"] {
            if !self.name(what) {
                self.finish();
                return;
            }
            self.ws();
            if !self.rest().starts_with(',') {
                self.fail("expected `,` and the next field: a row is `source,target,value`");
                self.finish();
                return;
            }
            self.token(COMMA, 1);
            self.ws();
        }
        let r = self.rest();
        let n = number_len(r);
        let line = scan::line_len(r);
        if n == 0 || scan::trim_end_len(r, line) != n {
            let message = if r[..line].contains(',') {
                "expected 3 fields: a row is `source,target,value`"
            } else {
                "expected a number for the value, such as `12.5`"
            };
            self.fail(message);
        } else {
            self.token(NUMBER, n);
            self.ws();
        }
        self.finish();
    }

    /// One name field, quoted or not. Returns whether it holds one.
    fn name(&mut self, what: &str) -> bool {
        let r = self.rest();
        if r.starts_with('"') {
            match quoted_len(r) {
                Some(n) => {
                    self.token(STRING, n);
                    true
                }
                None => {
                    self.fail("unclosed `\"`: expected a closing `\"` on the same line");
                    false
                }
            }
        } else {
            let line = &r[..scan::line_len(r)];
            let end = line.find(',').unwrap_or(line.len());
            let n = scan::trim_end_len(line, end);
            if n == 0 {
                self.fail(&format!("expected the {what} name"));
                return false;
            }
            self.token(TEXT, n);
            true
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
    fn quoting() {
        assert_eq!(quoted_len("\"a, b\",x"), Some(6));
        assert_eq!(quoted_len("\"say \"\"hi\"\"\",x"), Some(12));
        assert_eq!(quoted_len("\"open\nx\""), None);
    }

    #[test]
    fn rows() {
        let (t, d) = tree(
            "sankey-beta\n\n%% source,target,value\nA,B,10\n\"C, D\",B ,2.5\n  E , \"F \"\"G\"\"\" , 3\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("FLOW@").count(), 3);
        assert_eq!(t.matches("STRING@").count(), 2);
        assert_eq!(t.matches("NUMBER@").count(), 3);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("sankey\n{s}\n")).1;
        assert_eq!(e(",B,1"), ["expected the source name"]);
        assert_eq!(e("A,,1"), ["expected the target name"]);
        assert_eq!(
            e("A"),
            ["expected `,` and the next field: a row is `source,target,value`"]
        );
        assert_eq!(
            e("A,B,x"),
            ["expected a number for the value, such as `12.5`"]
        );
        assert_eq!(
            e("A,B,"),
            ["expected a number for the value, such as `12.5`"]
        );
        assert_eq!(
            e("A,B,1,2"),
            ["expected 3 fields: a row is `source,target,value`"]
        );
        assert_eq!(
            e("\"A,B,1"),
            ["unclosed `\"`: expected a closing `\"` on the same line"]
        );
        assert_eq!(tree("flow\nA,B,1\n").1[0], "expected `sankey` header");
    }
}
