//! Line-oriented parser for Mermaid radar charts, producing a lossless rowan tree (MRADAR-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "radar-beta", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["radar-beta"];

/// The option keywords, each followed by one value.
pub(crate) const OPTIONS: &[&str] = &["showLegend", "max", "min", "graticule", "ticks"];

const STATEMENT: &str = "expected `axis`, `curve`, `title`, or an option such as `max 100`";

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

/// An axis or curve id: a letter or `_`, then letters, digits, and `_`.
fn id_len(s: &str) -> usize {
    if !s.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        return 0;
    }
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// A number such as `3`, `-2.5`, or `.5`.
pub(crate) fn number_len(s: &str) -> usize {
    let sign = usize::from(s.starts_with('-'));
    let body = &s[sign..];
    let int = body
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(body.len());
    let mut n = int;
    if body[n..].starts_with('.') {
        let frac = body[n + 1..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(body.len() - n - 1);
        if int + frac == 0 {
            return 0;
        }
        n += 1 + frac;
    }
    if n == 0 { 0 } else { sign + n }
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
        } else if keyword(r, "axis") {
            self.list(AXIS_STMT, AXIS_KW, "axis", false);
        } else if keyword(r, "curve") {
            self.list(SERIES_STMT, SERIES_KW, "curve", true);
        } else if let Some(kw) = OPTIONS.iter().find(|kw| keyword(r, kw)) {
            self.start(SETTING_STMT);
            self.token(SETTING_KW, kw.len());
            self.ws();
            let n = word_len(self.rest()).max(number_len(self.rest()));
            if n == 0 {
                self.fail(&format!("expected a value after `{kw}`"));
            } else {
                self.token(TEXT, n);
                self.end();
            }
            self.finish();
        }
    }

    /// `axis a["A"], b` or `curve a["A"]{1, 2}, b{x: 3}`.
    fn list(&mut self, node: SyntaxKind, kw: SyntaxKind, word: &str, values: bool) {
        self.start(node);
        self.token(kw, word.len());
        self.ws();
        loop {
            if !self.entry(word, values) {
                break;
            }
            self.ws();
            if !self.rest().starts_with(',') {
                self.end();
                break;
            }
            self.token(COMMA, 1);
            self.ws();
        }
        self.finish();
    }

    /// An id, an optional `["label"]`, and for a curve its `{...}` values. Returns whether it
    /// parsed whole.
    fn entry(&mut self, word: &str, values: bool) -> bool {
        let n = id_len(self.rest());
        if n == 0 {
            self.fail(&format!("expected an {word} id such as `a`"));
            return false;
        }
        self.start(ENTRY);
        self.token(IDENT, n);
        if self.rest().starts_with('[') {
            self.token(L_BRACKET, 1);
            self.ws();
            let Some(n) = self.string_len() else {
                self.fail("expected a quoted label such as `\"Math\"`");
                self.finish();
                return false;
            };
            self.token(STRING, n);
            self.ws();
            if !self.rest().starts_with(']') {
                self.fail("expected `]` after the label");
                self.finish();
                return false;
            }
            self.token(R_BRACKET, 1);
        }
        let ok = !values || self.values(word);
        self.finish();
        ok
    }

    /// Whitespace and newlines inside a curve's braces.
    fn gap(&mut self) {
        loop {
            let r = self.rest();
            let (ws, nl) = (scan::ws_len(r), scan::newline_len(r));
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else {
                return;
            }
        }
    }

    /// `{1, 2, 3}` or `{a: 1, b: 2}`, which may run over several lines.
    fn values(&mut self, word: &str) -> bool {
        self.ws();
        if !self.rest().starts_with('{') {
            self.fail(&format!("expected `{{` and the {word}'s values"));
            return false;
        }
        self.token(L_BRACE, 1);
        self.gap();
        if self.rest().starts_with('}') {
            self.token(R_BRACE, 1);
            return true;
        }
        loop {
            self.start(VALUE);
            let n = id_len(self.rest());
            if n > 0 {
                self.token(IDENT, n);
                self.ws();
                if self.rest().starts_with(':') {
                    self.token(COLON, 1);
                }
                self.gap();
            }
            let n = number_len(self.rest());
            if n == 0 {
                self.fail("expected a number, or an axis id and a number such as `a: 3`");
                self.finish();
                return false;
            }
            self.token(NUMBER, n);
            self.finish();
            self.gap();
            if self.rest().starts_with(',') {
                self.token(COMMA, 1);
                self.gap();
            } else if self.rest().starts_with('}') {
                self.token(R_BRACE, 1);
                return true;
            } else {
                self.fail("expected `,` or `}`");
                return false;
            }
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
    fn numbers() {
        assert_eq!(number_len("12,"), 2);
        assert_eq!(number_len("-2.5}"), 4);
        assert_eq!(number_len(".5"), 2);
        assert_eq!(number_len("3."), 2);
        assert_eq!(number_len("-"), 0);
        assert_eq!(number_len("."), 0);
        assert_eq!(number_len("a"), 0);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "radar-beta\n  title Grades\n  axis m[\"Math\"], s[\"Science\"]\n  axis e\n  curve a[\"Alice\"]{85, 90.5, -1}, b{ m: 1,\n    s: 2 }\n  max 100\n  graticule polygon\n  showLegend false\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("AXIS_STMT@").count(), 2);
        assert_eq!(t.matches("SERIES_STMT@").count(), 1);
        assert_eq!(t.matches("ENTRY@").count(), 5);
        assert_eq!(t.matches("VALUE@").count(), 5);
        assert_eq!(t.matches("SETTING_STMT@").count(), 3);
        assert!(t.contains("TITLE_STMT@"));
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("radar-beta\n  {s}\n")).1;
        assert_eq!(e("axis"), ["expected an axis id such as `a`"]);
        assert_eq!(
            e("axis a[Math]"),
            ["expected a quoted label such as `\"Math\"`"]
        );
        assert_eq!(e("axis a[\"Math\""), ["expected `]` after the label"]);
        assert_eq!(e("axis a b"), ["expected the end of the line"]);
        assert_eq!(e("curve a"), ["expected `{` and the curve's values"]);
        assert_eq!(
            e("curve a{x}"),
            ["expected a number, or an axis id and a number such as `a: 3`"]
        );
        assert_eq!(e("curve a{1 2}"), ["expected `,` or `}`"]);
        assert_eq!(e("max"), ["expected a value after `max`"]);
        assert_eq!(e("max 1 2"), ["expected the end of the line"]);
        assert_eq!(e("radius 3"), [STATEMENT]);
    }
}
