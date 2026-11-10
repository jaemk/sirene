//! Line-oriented parser for Mermaid venn diagrams, producing a lossless rowan tree (MVENN-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, number_len};
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["venn-beta"];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
        indent_mode: false,
    };
    p.start(ROOT);
    p.header(HEADERS, "venn-beta", word_len);
    if !p.diagnostics.is_empty() {
        // Without the header, the rest is not read.
        let len = p.rest().len();
        p.start(ERROR);
        p.token(ERROR_TOKEN, len);
        p.finish();
    }
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        p.statement();
    }
    p.finish();
    p.base.finish_tree()
}

/// The length of a word: a letter or `_`, then letters, digits, `-`, and `_`.
fn word_len(s: &str) -> usize {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return 0,
    }
    chars
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map_or(s.len(), |(i, _)| i)
}

/// The length of an identifier: a word, or a `"..."` string closed on the same line.
fn id_len(s: &str) -> usize {
    if let Some(body) = s.strip_prefix('"') {
        let line = &body[..scan::line_len(body)];
        return line.find('"').map_or(0, |i| i + 2);
    }
    word_len(s)
}

/// The length of a text node's id: an identifier or a number.
fn text_id_len(s: &str) -> usize {
    let n = id_len(s);
    if n > 0 { n } else { signed_number_len(s) }
}

/// A number, with an optional leading `+`.
fn signed_number_len(s: &str) -> usize {
    match s.strip_prefix('+') {
        Some(rest) if !rest.starts_with('-') => {
            let n = number_len(rest);
            if n > 0 { n + 1 } else { 0 }
        }
        _ => number_len(s),
    }
}

struct Parser<'a> {
    base: Base<'a>,
    /// Whether an indented `text` line names only its id, attaching to the last `set` or
    /// `union`: true after one, until a line starts at column 0.
    indent_mode: bool,
}

impl<'a> Deref for Parser<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.base
    }
}

impl DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Parser<'_> {
    /// Whether the cursor follows leading whitespace on its line.
    fn indented(&self) -> bool {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        !line.is_empty() && line.chars().all(|c| c == ' ' || c == '\t')
    }

    /// The keyword at the cursor, lowercased, when it is a whole word.
    fn keyword(&self) -> String {
        let r = self.rest();
        r[..word_len(r)].to_ascii_lowercase()
    }

    fn statement(&mut self) {
        let indented = self.indented();
        if !indented {
            self.indent_mode = false;
        }
        match self.keyword().as_str() {
            "title" => self.title(),
            "set" | "union" => {
                self.subset();
                self.indent_mode = true;
            }
            "text" => self.text(indented),
            "style" => {
                self.style(id_len, "a set or text id");
            }
            _ => self.fail("expected `set`, `union`, `text`, `style`, or `title`"),
        }
    }

    /// `title text`, up to a `#` or `;`.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        let r = self.rest();
        if !r.starts_with([' ', '\t']) {
            self.fail("expected a title");
            self.finish();
            return;
        }
        self.ws();
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let end = line.find(['#', ';']).unwrap_or(line.len());
        let len = scan::trim_end_len(r, end);
        self.token(TEXT, len);
        self.end();
        self.finish();
    }

    /// One identifier as an `IDENT` or `STRING` token; returns whether there was one.
    fn id(&mut self) -> bool {
        let n = id_len(self.rest());
        if n == 0 {
            return false;
        }
        let kind = if self.rest().starts_with('"') {
            STRING
        } else {
            IDENT
        };
        self.token(kind, n);
        true
    }

    /// Identifiers separated by commas.
    fn ids(&mut self) -> bool {
        self.start(ID_LIST);
        let mut any = false;
        loop {
            self.ws();
            if !self.id() {
                break;
            }
            any = true;
            self.ws();
            if !self.rest().starts_with(',') {
                break;
            }
            self.token(COMMA, 1);
        }
        self.finish();
        any
    }

    /// `["label"]` or `[label]`, if present; returns false after reporting a bad one.
    fn label(&mut self) -> bool {
        self.ws();
        let r = self.rest();
        if !r.starts_with('[') {
            return true;
        }
        let line = &r[..scan::line_len(r)];
        let (kind, len) = if let Some(body) = line.strip_prefix("[\"") {
            match body.find("\"]") {
                Some(i) => (STRING, i + 2),
                None => (TEXT, 0),
            }
        } else {
            match line.find(']') {
                Some(i) if !line[1..i].contains('"') => (TEXT, i - 1),
                _ => (TEXT, 0),
            }
        };
        if len == 0 {
            self.fail("expected a label such as `[\"Label\"]`");
            return false;
        }
        self.start(LABEL);
        self.token(L_BRACKET, 1);
        self.token(kind, len);
        self.token(R_BRACKET, 1);
        self.finish();
        true
    }

    /// `set id["label"]:size` or `union a,b["label"]:size`.
    fn subset(&mut self) {
        self.start(SET_STMT);
        let union = self.keyword() == "union";
        let kw = self.keyword().len();
        self.token(VENN_KW, kw);
        let ok = if union {
            self.ids()
        } else {
            self.start(ID_LIST);
            self.ws();
            let ok = self.id();
            self.finish();
            ok
        };
        if !ok {
            self.fail(if union {
                "expected set names"
            } else {
                "expected a set name"
            });
            self.finish();
            return;
        }
        if self.label() {
            self.ws();
            if self.rest().starts_with(':') {
                self.token(COLON, 1);
                self.ws();
                let n = signed_number_len(self.rest());
                if n > 0 {
                    self.token(NUMBER, n);
                    self.end();
                } else {
                    self.fail("expected a size");
                }
            } else {
                self.end();
            }
        }
        self.finish();
    }

    /// `text sets id["label"]`, or `text id["label"]` on a line indented under a `set` or
    /// `union`.
    fn text(&mut self, indented: bool) {
        self.start(TEXT_STMT);
        self.token(VENN_KW, "text".len());
        self.ws();
        // Look ahead: a lone id means the indented form.
        let r = self.rest();
        let first = text_id_len(r);
        let after = &r[first..];
        let after = &after[scan::ws_len(after)..];
        let lone = first > 0 && !after.starts_with(',') && id_len(after) == 0;
        if lone && !(indented && self.indent_mode) {
            self.fail("expected sets before the text id, or an indented line under a `set`");
            self.finish();
            return;
        }
        if !lone && !self.ids() {
            self.fail("expected sets and a text id");
            self.finish();
            return;
        }
        self.ws();
        let n = text_id_len(self.rest());
        if n == 0 {
            self.fail("expected a text id");
            self.finish();
            return;
        }
        self.start(NODE);
        let kind = if self.rest().starts_with('"') {
            STRING
        } else if id_len(self.rest()) > 0 {
            IDENT
        } else {
            NUMBER
        };
        self.token(kind, n);
        self.finish();
        if self.label() {
            self.end();
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
            "venn-beta\n  title Team overlap\n  set A[\"Alpha\"]:20\n  set \"B c\" [Beta] : 1.5\n  \
             union A,\"B c\"[\"AB\"]:+3\n    text x1[\"React\"]\n    text 42\n  text A,B y\n  \
             %% c\n  style A,B fill:#f00, color: red\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("SET_STMT@").count(), 3);
        assert_eq!(t.matches("TEXT_STMT@").count(), 3);
        assert_eq!(t.matches("LABEL@").count(), 4);
        assert!(t.contains("STYLE_STMT"));
        assert!(t.contains("TITLE_STMT"));
    }

    #[test]
    fn keywords_ignore_case() {
        let (_, d) = tree("venn-beta\nSET A\nUnion A,A\n");
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn indent_mode() {
        // A line at column 0 ends the indented form.
        let (_, d) = tree("venn-beta\nset A\n  text a\nstyle A fill:red\n  text b\n");
        assert_eq!(
            d,
            ["expected sets before the text id, or an indented line under a `set`"]
        );
        // So does starting without a set.
        let (_, d) = tree("venn-beta\n  text a\n");
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn errors() {
        assert_eq!(tree("venn\n").1, ["expected `venn-beta` header"]);
        assert_eq!(
            tree("venn-beta\nfoo\n").1,
            ["expected `set`, `union`, `text`, `style`, or `title`"]
        );
        assert_eq!(tree("venn-beta\nset\n").1, ["expected a set name"]);
        assert_eq!(tree("venn-beta\nset A:\n").1, ["expected a size"]);
        assert_eq!(
            tree("venn-beta\nset A[\"x]\n").1,
            ["expected a label such as `[\"Label\"]`"]
        );
        assert_eq!(
            tree("venn-beta\nset A B\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(tree("venn-beta\ntext A,B\n").1, ["expected a text id"]);
    }
}
