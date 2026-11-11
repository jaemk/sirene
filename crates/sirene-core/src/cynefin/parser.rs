//! Line-oriented parser for Mermaid Cynefin diagrams, producing a lossless rowan tree
//! (MCYN-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["cynefin-beta"];

/// The domain names, as the source spells them.
pub(crate) const DOMAINS: &[&str] = &["complex", "complicated", "clear", "chaotic", "confusion"];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
        in_domain: false,
    };
    p.start(ROOT);
    if !p.header() {
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

/// The length of a word: letters, digits, `-`, and `_`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

/// The domain name at the start of `s`, when it is a whole word (`-->` may follow at once).
fn domain_len(s: &str) -> usize {
    let n = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    if DOMAINS.contains(&&s[..n]) { n } else { 0 }
}

/// The length of a `"..."` or `'...'` string closed on the same line; `\` escapes a character.
pub(crate) fn string_len(s: &str) -> Option<usize> {
    let quote = s.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let line = &s[..scan::line_len(s)];
    let mut chars = line.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == quote {
            return Some(i + 1);
        }
    }
    None
}

struct Parser<'a> {
    base: Base<'a>,
    /// Whether a line of quoted items belongs to a domain: true after a domain line, until
    /// another statement.
    in_domain: bool,
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
    /// Frontmatter, trivia, and `cynefin-beta` or `cynefin-beta:`; returns whether it was there.
    fn header(&mut self) -> bool {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        let n = word_len(r);
        if &r[..n] != HEADERS[0] {
            let start = self.pos;
            self.error(
                start,
                start + scan::stmt_len(r),
                "expected `cynefin-beta` header",
            );
            return false;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, n);
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
        }
        self.eol();
        self.finish();
        true
    }

    fn statement(&mut self) {
        let r = self.rest();
        if r.starts_with("accTitle") || r.starts_with("accDescr") {
            self.in_domain = false;
            self.acc();
            return;
        }
        if r.starts_with("title") && word_len(r) == 5 {
            self.in_domain = false;
            self.title();
            return;
        }
        let n = domain_len(r);
        if n > 0 {
            let after = &r[n..];
            if after[scan::ws_len(after)..].starts_with("-->") {
                self.in_domain = false;
                self.transition(n);
            } else {
                self.domain(n);
                self.in_domain = true;
            }
            return;
        }
        if string_len(r).is_some() {
            self.start(ITEM_STMT);
            if self.in_domain {
                self.items();
                self.eol();
            } else {
                self.fail("expected a domain such as `complex` before its items");
            }
            self.finish();
            return;
        }
        self.in_domain = false;
        self.fail(
            "expected a domain (`complex`, `complicated`, `clear`, `chaotic`, or `confusion`), \
             a quoted item, a transition such as `complex --> clear`, or `title`",
        );
    }

    /// Requires the statement to end here, allowing a trailing `%%` comment.
    fn eol(&mut self) {
        self.ws();
        if self.rest().starts_with("%%") {
            let n = scan::line_len(self.rest());
            self.token(COMMENT, n);
        }
        self.end();
    }

    /// `title text`, up to a `%%` comment.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let end = line.find("%%").unwrap_or(line.len());
        let len = scan::trim_end_len(r, end);
        self.token(TEXT, len);
        self.eol();
        self.finish();
    }

    /// Quoted items separated by whitespace, up to the end of the line or a bad one.
    fn items(&mut self) {
        while let Some(n) = string_len(self.rest()) {
            self.token(STRING, n);
            self.ws();
        }
        let r = self.rest();
        if r.starts_with(['"', '\'']) {
            self.fail("unclosed string: expected a closing quote on the same line");
        }
    }

    /// A domain name, then any items on the same line.
    fn domain(&mut self, n: usize) {
        self.start(DOMAIN_STMT);
        self.token(CYNEFIN_KW, n);
        self.ws();
        self.items();
        self.eol();
        self.finish();
    }

    /// `from --> to` with an optional `: "label"`.
    fn transition(&mut self, n: usize) {
        self.start(TRANSITION_STMT);
        self.token(CYNEFIN_KW, n);
        self.ws();
        self.token(ARROW, 3);
        self.ws();
        let m = domain_len(self.rest());
        if m == 0 {
            self.fail("expected a domain after `-->`");
            self.finish();
            return;
        }
        self.token(CYNEFIN_KW, m);
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            match string_len(self.rest()) {
                Some(len) => {
                    self.token(STRING, len);
                    self.eol();
                }
                None => self.fail("expected a quoted label such as `\"Stabilized\"`"),
            }
        } else {
            self.eol();
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
            "%% c\ncynefin-beta: %% header\n  title Incident %% note\n  accTitle: A\n  \
             complex \"a\" 'b'\n    \"c\"\n    \"d \\\" e\" \"f\"\n  confusion\n  \
             complex-->clear\n  chaotic --> complex : \"Stabilized\" %% why\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("DOMAIN_STMT@").count(), 2);
        assert_eq!(t.matches("ITEM_STMT@").count(), 2);
        assert_eq!(t.matches("TRANSITION_STMT@").count(), 2);
        assert_eq!(t.matches("STRING@").count(), 6);
        assert!(t.contains("TITLE_STMT"));
        assert!(t.contains("ACC_TITLE_STMT"));
    }

    #[test]
    fn string_lengths() {
        assert_eq!(string_len("\"ab\" x"), Some(4));
        assert_eq!(string_len("'a\"b'"), Some(5));
        assert_eq!(string_len("\"a\\\"b\""), Some(6));
        assert_eq!(string_len("\"ab\nc\""), None);
        assert_eq!(string_len("ab"), None);
    }

    #[test]
    fn errors() {
        assert_eq!(tree("cynefin\n").1, ["expected `cynefin-beta` header"]);
        assert_eq!(
            tree("cynefin-beta\n\"a\"\n").1,
            ["expected a domain such as `complex` before its items"]
        );
        // A title ends the domain block.
        assert_eq!(
            tree("cynefin-beta\nclear\ntitle T\n\"a\"\n").1,
            ["expected a domain such as `complex` before its items"]
        );
        assert_eq!(
            tree("cynefin-beta\ncomplexity\n").1.len(),
            1,
            "a domain name must be a whole word"
        );
        assert_eq!(
            tree("cynefin-beta\nclear --> simple\n").1,
            ["expected a domain after `-->`"]
        );
        assert_eq!(
            tree("cynefin-beta\nclear --> chaotic : oops\n").1,
            ["expected a quoted label such as `\"Stabilized\"`"]
        );
        assert_eq!(
            tree("cynefin-beta\nclear \"a\" b\n").1,
            ["expected the end of the line"]
        );
        assert_eq!(
            tree("cynefin-beta\nclear \"a\n").1,
            ["unclosed string: expected a closing quote on the same line"]
        );
        assert_eq!(
            tree("cynefin-beta\nComplex\n").1.len(),
            1,
            "domain names are lowercase"
        );
    }
}
