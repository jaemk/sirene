//! Line-oriented parser for Mermaid packet diagrams, producing a lossless rowan tree (MPKT-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "packet", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["packet", "packet-beta"];

const STATEMENT: &str = "expected a field (`0-15: \"label\"` or `+8: \"label\"`) or `title`";

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

fn digits(s: &str) -> usize {
    s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len())
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
            let line = &self.rest()[..scan::line_len(self.rest())];
            let end = line.find(['#', ';']).unwrap_or(line.len());
            let n = scan::trim_end_len(line, end);
            self.token(TEXT, n);
            self.end();
            self.finish();
        } else if r.starts_with('+') || digits(r) > 0 {
            self.field();
        }
    }

    /// `START: "label"`, `START-END: "label"`, or `+BITS: "label"`.
    fn field(&mut self) {
        self.start(FIELD);
        if self.rest().starts_with('+') {
            self.token(PLUS, 1);
        }
        let n = digits(self.rest());
        if n == 0 {
            self.fail("expected a bit count after `+`");
            self.finish();
            return;
        }
        self.token(NUMBER, n);
        self.ws();
        if self.rest().starts_with('-') {
            self.token(DASH, 1);
            self.ws();
            let n = digits(self.rest());
            if n == 0 {
                self.fail("expected the last bit after `-`");
                self.finish();
                return;
            }
            self.token(NUMBER, n);
            self.ws();
        }
        if !self.rest().starts_with(':') {
            self.fail("expected `:` and a quoted label");
            self.finish();
            return;
        }
        self.token(COLON, 1);
        self.ws();
        match self.string_len() {
            Some(n) => {
                self.token(STRING, n);
                self.end();
            }
            None => self.fail("expected a quoted label such as `\"Source Port\"`"),
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
            "packet-beta\n  title UDP\n  0-15: \"Source Port\"\n  16 - 31 : \"Dest\"\n  +8: \"x\"\n  40: \"flag\"\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("FIELD@").count(), 4);
        assert_eq!(t.matches("NUMBER@").count(), 6);
        assert!(t.contains("PLUS@") && t.contains("DASH@") && t.contains("TITLE_STMT@"));
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("packet\n  {s}\n")).1;
        assert_eq!(e("+: \"x\""), ["expected a bit count after `+`"]);
        assert_eq!(e("0-: \"x\""), ["expected the last bit after `-`"]);
        assert_eq!(e("0 \"x\""), ["expected `:` and a quoted label"]);
        assert_eq!(
            e("0: x"),
            ["expected a quoted label such as `\"Source Port\"`"]
        );
        assert_eq!(e("0: \"x\" y"), ["expected the end of the line"]);
        assert_eq!(e("bits"), [STATEMENT]);
    }
}
