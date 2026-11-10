//! Line-oriented parser for Mermaid gantt charts, producing a lossless rowan tree (MGANTT-1).

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

pub(crate) const HEADERS: &[&str] = &["gantt"];

const STATEMENT: &str = "expected a task (`name : dates`) or a setting";

/// Settings followed by a value, lowercased; the value ends at `#` or `;`.
pub(crate) const VALUED: &[&str] = &[
    "dateformat",
    "axisformat",
    "tickinterval",
    "excludes",
    "includes",
    "weekday",
    "weekend",
];

/// Settings that stand alone, lowercased.
pub(crate) const FLAGS: &[&str] = &["inclusiveenddates", "topaxis"];

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

/// The length of the text at the start of `s` before any of `stops` or the end of the line,
/// without trailing whitespace.
fn value_len(s: &str, stops: &[char]) -> usize {
    let line = &s[..scan::line_len(s)];
    let end = line.find(stops).unwrap_or(line.len());
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if !r[..word_len(r)].eq_ignore_ascii_case("gantt") {
            self.fail("expected `gantt` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "gantt".len());
        self.end_or_comment();
        self.finish();
    }

    /// A `#` comment, then the end of the statement.
    fn end_or_comment(&mut self) {
        self.ws();
        if self.rest().starts_with('#') {
            let n = scan::line_len(self.rest());
            self.token(COMMENT, n);
        }
        self.end();
    }

    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if r.starts_with('#') {
            let n = scan::line_len(r);
            self.token(COMMENT, n);
            return;
        }
        let n = word_len(r);
        let word = r[..n].to_ascii_lowercase();
        let followed = r[n..].starts_with([' ', '\t']);
        match word.as_str() {
            "acctitle" | "accdescr" if r.starts_with("accTitle") || r.starts_with("accDescr") => {
                self.acc()
            }
            "title" | "section" if followed => {
                let (node, kw) = if word == "title" {
                    (TITLE_STMT, TITLE_KW)
                } else {
                    (SECTION_STMT, SECTION_KW)
                };
                self.start(node);
                self.token(kw, n);
                self.ws();
                self.rest_of_line(TEXT);
                self.finish();
            }
            "click" if followed => self.click(),
            "todaymarker" if followed => {
                self.start(SETTING_STMT);
                self.token(SETTING_KW, n);
                self.ws();
                let v = value_len(self.rest(), &[';']);
                self.token(TEXT, v);
                self.end();
                self.finish();
            }
            w if VALUED.contains(&w) && followed => {
                self.start(SETTING_STMT);
                self.token(SETTING_KW, n);
                self.ws();
                let v = value_len(self.rest(), &['#', ';']);
                if v == 0 {
                    self.fail("expected a value");
                } else {
                    self.token(TEXT, v);
                    self.end_or_comment();
                }
                self.finish();
            }
            w if FLAGS.contains(&w) && !r[n..].trim_start().starts_with(':') => {
                self.start(SETTING_STMT);
                self.token(SETTING_KW, n);
                self.end_or_comment();
                self.finish();
            }
            _ => self.task(),
        }
    }

    /// `NAME : DATA`.
    fn task(&mut self) {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let Some(colon) = line.find(':') else {
            return;
        };
        let name = scan::trim_end_len(line, colon);
        if name == 0 {
            return;
        }
        self.start(TASK_STMT);
        self.token(TASK_NAME, name);
        self.ws();
        self.token(COLON, 1);
        self.ws();
        let v = value_len(self.rest(), &['#', ';']);
        self.token(TASK_DATA, v);
        self.end_or_comment();
        self.finish();
    }
}

/// The text of the first `kind` token of `node`, trimmed.
pub(crate) fn text_of(node: &crate::syntax::SyntaxNode, kind: SyntaxKind) -> Option<String> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
        .map(|t| t.text().trim().to_owned())
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
            "GANTT # c\n  dateFormat YYYY-MM-DD # x\n  todayMarker stroke:#f00\n  topAxis\n  \
             title A plan\n  Section One\n  A task : a1, 2014-01-01, 3d # note\n  # whole line\n  \
             click a1 href \"x\"\n  inclusiveEndDates; Other : 1d\n  topAxis : 2d\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("SETTING_STMT").count(), 4);
        assert_eq!(t.matches("TASK_STMT").count(), 3);
        assert!(t.contains("\"stroke:#f00\""));
        assert!(t.contains("TASK_DATA@") && t.contains("\"a1, 2014-01-01, 3d\""));
        assert!(t.contains("SECTION_STMT") && t.contains("CLICK_STMT"));
    }

    #[test]
    fn errors() {
        assert_eq!(tree("gant\n").1, ["expected `gantt` header"]);
        assert_eq!(tree("gantt\n  dateFormat # x\n").1, ["expected a value"]);
        assert_eq!(tree("gantt\n  just words\n").1, [STATEMENT]);
        assert_eq!(
            tree("gantt\n  topAxis now\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("gantt\n  : 1d\n").1, [STATEMENT]);
    }
}
