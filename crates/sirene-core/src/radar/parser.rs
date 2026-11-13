//! Line-oriented parser for Mermaid radar charts, producing a lossless rowan tree (MRADAR-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, number_len, quoted_len};
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header();
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

/// An axis or curve id: letters, digits, and `_`, with `-` inside but not at either end.
fn id_len(s: &str) -> usize {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    if !s.starts_with(word) {
        return 0;
    }
    let n = word_len(s);
    s[..n].trim_end_matches('-').len()
}

impl Parser<'_> {
    /// `radar-beta`, optionally followed by `:`.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        if !keyword(self.rest(), "radar-beta") {
            self.fail("expected `radar-beta` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "radar-beta".len());
        self.ws();
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
        } else if keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, "title".len());
            self.ws();
            self.title_text();
            self.finish();
        } else if keyword(r, "axis") {
            self.list(AXIS_STMT, AXIS_KW, "axis", false);
        } else if keyword(r, "curve") {
            self.list(SERIES_STMT, SERIES_KW, "curve", true);
        } else if OPTIONS.iter().any(|kw| keyword(r, kw)) {
            self.options();
        }
    }

    /// Options separated by `,`: `max 100, min 0`.
    fn options(&mut self) {
        loop {
            let r = self.rest();
            let Some(kw) = OPTIONS.iter().find(|kw| keyword(r, kw)) else {
                self.fail(STATEMENT);
                return;
            };
            self.start(SETTING_STMT);
            self.token(SETTING_KW, kw.len());
            self.ws();
            let n = word_len(self.rest()).max(number_len(self.rest()));
            if n == 0 {
                self.fail(&format!("expected a value after `{kw}`"));
                self.finish();
                return;
            }
            self.token(TEXT, n);
            self.ws();
            if !self.rest().starts_with(',') {
                self.end();
                self.finish();
                return;
            }
            self.finish();
            self.token(COMMA, 1);
            self.ws();
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
            let Some(n) = quoted_len(self.rest()) else {
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
            // An id is one only before a `:`, as an id may start with a digit.
            let r = self.rest();
            let n = id_len(r);
            let keyed = n > 0 && r[n..].trim_start_matches([' ', '\t']).starts_with(':');
            if keyed || (n > 0 && number_len(r) == 0) {
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
        // A `:` after the header, options separated by `,`, `-` inside ids, single quotes,
        // and a title running past `#` and `;`.
        let (t, d) = tree(
            "radar-beta:\n  title Grades #1; 2 %% c\n  axis my-axis['Math'], 1b[\"a\\\"b\"]\n  curve c{my-axis: 1, 1b: 2}\n  max 100, min 0, ticks 4\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("SETTING_STMT@").count(), 3);
        assert!(t.contains("TEXT@") && t.contains("\"Grades #1; 2\""));
        assert!(t.contains("\"my-axis\""));
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
        assert_eq!(e("max 1, radius 3"), [STATEMENT]);
        assert_eq!(e("axis a-"), ["expected the end of the line"]);
        assert_eq!(e("radius 3"), [STATEMENT]);
    }
}
