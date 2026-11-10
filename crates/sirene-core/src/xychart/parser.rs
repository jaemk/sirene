//! Line-oriented parser for Mermaid XY charts, producing a lossless rowan tree (MXY-1).

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

pub(crate) const HEADERS: &[&str] = &["xychart-beta", "xychart"];

const STATEMENT: &str = "expected `line`, `bar`, `x-axis`, `y-axis`, or `title`";

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

/// The length of an unquoted name or title: up to whitespace or one of `[],"#;`.
fn name_len(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace() || "[],\"#;".contains(c))
        .unwrap_or(s.len())
}

/// The length of text at the start of `s` up to one of `stops` or the end of the line, without
/// trailing whitespace.
fn text_len(s: &str, stops: &[char]) -> usize {
    let line = &s[..scan::line_len(s)];
    let end = line.find(stops).unwrap_or(line.len());
    scan::trim_end_len(line, end)
}

/// The length of a number at the start of `s`: an optional sign, then digits with an optional
/// fraction, or a fraction alone (`.5`).
pub(crate) fn number_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'-' | b'+')));
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(i);
    i += int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        if frac > 0 {
            i += 1 + frac;
        }
    }
    if int == 0 && frac == 0 { 0 } else { i }
}

impl Parser<'_> {
    /// `xychart-beta` or `xychart`, optionally `horizontal` or `vertical`.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        let Some(kw) = HEADERS.iter().find(|kw| keyword(r, kw)) else {
            self.fail("expected `xychart-beta` header");
            return;
        };
        self.start(HEADER);
        self.token(GRAPH_KW, kw.len());
        self.ws();
        let r = self.rest();
        if keyword(r, "horizontal") || keyword(r, "vertical") {
            self.token(ORIENTATION, word_len(r));
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
            match self.string_len() {
                Some(n) => self.token(STRING, n),
                None => {
                    let n = text_len(self.rest(), &['#', ';']);
                    self.token(TEXT, n);
                }
            }
            self.end();
            self.finish();
        } else if keyword(r, "x-axis") || keyword(r, "y-axis") {
            self.axis();
        } else if keyword(r, "line") || keyword(r, "bar") {
            self.series();
        }
    }

    /// A `"string"` or an unquoted word, if either is here.
    fn name(&mut self) {
        match self.string_len() {
            Some(n) => self.token(STRING, n),
            None => {
                let n = name_len(self.rest());
                self.token(TEXT, n);
            }
        }
    }

    /// Whether a range `min --> max` starts here.
    fn at_range(&self) -> bool {
        let r = self.rest();
        let n = number_len(r);
        n > 0 && r[n..].trim_start_matches([' ', '\t']).starts_with("-->")
    }

    /// `x-axis [TITLE] [a, b]`, `x-axis [TITLE] MIN --> MAX`, or `y-axis [TITLE] [MIN --> MAX]`.
    fn axis(&mut self) {
        self.start(AXIS_STMT);
        self.token(AXIS_KW, "x-axis".len());
        self.ws();
        if !self.rest().starts_with('[') && !self.at_range() {
            self.name();
            self.ws();
        }
        let done = if self.rest().starts_with('[') {
            self.categories()
        } else if self.at_range() {
            self.range()
        } else {
            true
        };
        if done {
            self.end();
        }
        self.finish();
    }

    /// `MIN --> MAX`. Returns whether it is complete.
    fn range(&mut self) -> bool {
        let n = number_len(self.rest());
        self.token(NUMBER, n);
        self.ws();
        self.token(ARROW, 3);
        self.ws();
        let n = number_len(self.rest());
        if n == 0 {
            self.fail("expected a number after `-->`");
            return false;
        }
        self.token(NUMBER, n);
        true
    }

    /// `[a, "b c", d]`. Returns whether it is complete.
    fn categories(&mut self) -> bool {
        self.token(L_BRACKET, 1);
        loop {
            self.ws();
            match self.string_len() {
                Some(n) => self.token(STRING, n),
                None => {
                    let n = text_len(self.rest(), &[',', ']', '#', ';']);
                    if n == 0 {
                        self.fail("expected a category");
                        return false;
                    }
                    self.token(TEXT, n);
                }
            }
            match self.separator() {
                Some(true) => return true,
                Some(false) => {}
                None => return false,
            }
        }
    }

    /// After a list item: whitespace, then `,` or the closing `]`. Returns whether the list
    /// closed, or `None` when neither is here.
    fn separator(&mut self) -> Option<bool> {
        self.ws();
        if self.rest().starts_with(',') {
            self.token(COMMA, 1);
            Some(false)
        } else if self.rest().starts_with(']') {
            self.token(R_BRACKET, 1);
            Some(true)
        } else {
            self.fail("expected `,` or `]`");
            None
        }
    }

    /// `line [NAME] [1, 2.5 "label", -3]` or `bar [NAME] [...]`.
    fn series(&mut self) {
        self.start(SERIES_STMT);
        let n = word_len(self.rest());
        self.token(SERIES_KW, n);
        self.ws();
        if !self.rest().starts_with('[') {
            self.name();
            self.ws();
        }
        if !self.rest().starts_with('[') {
            self.fail("expected `[` and the values");
            self.finish();
            return;
        }
        self.token(L_BRACKET, 1);
        loop {
            self.ws();
            let n = number_len(self.rest());
            if n == 0 {
                self.fail("expected a number");
                self.finish();
                return;
            }
            self.token(NUMBER, n);
            self.ws();
            if let Some(n) = self.string_len() {
                self.token(STRING, n);
            }
            match self.separator() {
                Some(true) => break,
                Some(false) => {}
                None => {
                    self.finish();
                    return;
                }
            }
        }
        self.end();
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
    fn numbers() {
        for (s, n) in [
            ("12", 2),
            ("-3.4", 4),
            ("+1.3,", 4),
            (".98]", 3),
            ("5.", 1),
            ("-", 0),
            (".", 0),
            ("x", 0),
        ] {
            assert_eq!(number_len(s), n, "{s}");
        }
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "xychart-beta horizontal\n  title \"Sales\"\n  x-axis Month [jan, \"feb x\", mar]\n  \
             y-axis \"Revenue\" 0 --> 100\n  bar [1, 2.5, -3]\n  line \"avg\" [5 \"hi\", .5, +1]\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert!(t.contains("ORIENTATION@") && t.contains("\"horizontal\""));
        assert_eq!(t.matches("AXIS_STMT").count(), 2);
        assert_eq!(t.matches("SERIES_STMT").count(), 2);
        assert_eq!(t.matches("NUMBER@").count(), 8);
        assert!(t.contains("\"\\\"feb x\\\"\"") && t.contains("\"\\\"hi\\\"\""));
        let (_, d) = tree("xychart\n  x-axis 1 --> 5\n  y-axis T\n  y-axis -2 --> 2\n");
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("xychart\n  {s}\n")).1;
        assert_eq!(e("line"), ["expected `[` and the values"]);
        assert_eq!(e("line a b [1]"), ["expected `[` and the values"]);
        assert_eq!(e("bar [1, x]"), ["expected a number"]);
        assert_eq!(e("bar [1 2]"), ["expected `,` or `]`"]);
        assert_eq!(e("bar [1"), ["expected `,` or `]`"]);
        assert_eq!(e("x-axis [a,]"), ["expected a category"]);
        assert_eq!(e("bar [1,]"), ["expected a number"]);
        assert_eq!(e("x-axis 1 -->"), ["expected a number after `-->`"]);
        assert_eq!(e("x-axis a b"), ["expected the end of the line"]);
        assert_eq!(e("pie"), [STATEMENT]);
        assert_eq!(
            tree("xychart sideways\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(
            tree("xy\n  line [1]\n").1,
            ["expected `xychart-beta` header"]
        );
    }
}
