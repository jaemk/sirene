//! Parser for Mermaid event modeling diagrams, producing a lossless rowan tree (MEVM-1).
//! Statements are one per line, except data blocks and notes, whose `{ ... }` spans lines, and
//! `gwt`, whose clauses may.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["eventmodeling"];

/// The entity types, in compact and relaxed spellings.
pub(crate) const TYPES: &[&str] = &[
    "ui",
    "pcr",
    "processor",
    "cmd",
    "command",
    "rmo",
    "readmodel",
    "evt",
    "event",
];

/// The data types allowed in backticks before data.
pub(crate) const DATA_TYPES: &[&str] = &[
    "json", "jsobj", "figma", "salt", "uri", "md", "html", "text",
];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
    };
    p.start(ROOT);
    p.header(HEADERS, HEADERS[0], word_len);
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        let before = p.pos;
        p.statement();
        if p.pos == before {
            p.fail("expected a statement");
        }
    }
    p.finish();
    p.base.finish_tree()
}

/// The length of a word: letters, digits, and `_`.
pub(crate) fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// The length of an identifier: a letter or `_`, then letters, digits, and `_`.
fn id_len(s: &str) -> usize {
    if s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        word_len(s)
    } else {
        0
    }
}

/// The length of a qualified name such as `Cart.ItemAdded`.
fn qualified_len(s: &str) -> usize {
    let mut n = id_len(s);
    if n == 0 {
        return 0;
    }
    while s[n..].starts_with('.') {
        let m = id_len(&s[n + 1..]);
        if m == 0 {
            break;
        }
        n += 1 + m;
    }
    n
}

/// The length of a frame number (one to three digits), or 0.
fn frame_len(s: &str) -> usize {
    let n = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if (1..=3).contains(&n) && word_len(s) == n {
        n
    } else {
        0
    }
}

/// The length of inline data: `{...}` up to the line's last `}`, or `"..."` or `'...'` up to
/// the line's last matching quote.
fn inline_len(s: &str) -> Option<usize> {
    let line = &s[..scan::line_len(s)];
    let close = match line.chars().next()? {
        '{' => '}',
        q @ ('"' | '\'') => q,
        _ => return None,
    };
    let i = line[1..].rfind(close)?;
    Some(i + 2)
}

/// The length of a data block: `{`, the rest of its line blank, then lines up to a `}` line
/// indented no deeper than `indent`. `Err` holds the length to the end of input when the
/// block is unclosed.
fn block_len(s: &str, indent: usize) -> Result<usize, usize> {
    let first = scan::line_len(s);
    let mut at = first + scan::newline_len(&s[first..]);
    while at < s.len() {
        let rest = &s[at..];
        let len = scan::line_len(rest);
        let line = &rest[..len];
        let lead = line.len() - line.trim_start().len();
        if line.trim() == "}" && lead <= indent {
            return Ok(at + lead + 1);
        }
        at += len + scan::newline_len(&rest[len..]);
    }
    Err(s.len())
}

struct Parser<'a> {
    base: Base<'a>,
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
    /// Trivia, with `//` and `/* */` comments as well as `%%`.
    fn trivia(&mut self) {
        loop {
            self.base.trivia();
            if !self.comment() {
                return;
            }
        }
    }

    /// A `%%` or `//` comment to the end of the line, or a `/* */` comment; returns whether
    /// there was one.
    fn comment(&mut self) -> bool {
        let r = self.rest();
        if r.starts_with("%%") || r.starts_with("//") {
            let n = scan::line_len(r);
            self.token(COMMENT, n);
        } else if let Some(body) = r.strip_prefix("/*") {
            match body.find("*/") {
                Some(i) => self.token(COMMENT, i + 4),
                None => {
                    let start = self.pos;
                    self.token(COMMENT, r.len());
                    self.error(start, start + 2, "unclosed comment: expected `*/`");
                }
            }
        } else {
            return false;
        }
        true
    }

    /// Requires the statement to end here, allowing a trailing comment.
    fn eol(&mut self) {
        loop {
            self.ws();
            let r = self.rest();
            if r.starts_with("/*") && !r[2..scan::line_len(r)].contains("*/") {
                // A comment running onto later lines ends the statement.
                self.comment();
                return;
            }
            if !self.comment() {
                break;
            }
        }
        self.end();
    }

    /// The column of the cursor's line's first non-blank character.
    fn indent(&self) -> usize {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        line.len() - line.trim_start().len()
    }

    fn statement(&mut self) {
        let r = self.rest();
        let word = &r[..word_len(r)];
        match word {
            "accTitle" | "accDescr" => self.acc(),
            "title" => self.title(),
            "tf" | "timeframe" | "rf" | "resetframe" => self.frame(word.len()),
            "entity" => self.entity(),
            "data" => self.data(),
            "note" => self.note(),
            "gwt" => self.gwt(),
            _ => self.fail(
                "expected a frame such as `tf 01 ui CartUI`, `rf`, `data`, `note`, `gwt`, \
                 `entity`, or `title`",
            ),
        }
    }

    /// `title text`, up to a comment.
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

    /// An entity type such as `cmd`; returns whether it was there.
    fn entity_type(&mut self) -> bool {
        let n = word_len(self.rest());
        if TYPES.contains(&&self.rest()[..n]) {
            self.token(EM_TYPE, n);
            true
        } else {
            false
        }
    }

    /// `` `json` `` before data; returns false after reporting a bad one.
    fn data_type(&mut self) -> bool {
        let r = self.rest();
        if !r.starts_with('`') {
            return true;
        }
        let line = &r[..scan::line_len(r)];
        match line[1..].find('`') {
            Some(i) if DATA_TYPES.contains(&&line[1..=i]) => {
                self.token(EM_DATA_TYPE, i + 2);
                self.ws();
                true
            }
            _ => {
                self.fail(
                    "expected a data type in backticks: `json`, `jsobj`, `figma`, `salt`, \
                     `uri`, `md`, `html`, or `text`",
                );
                false
            }
        }
    }

    /// `tf|timeframe|rf|resetframe 01 type Name (->> 02)* ([[data]])? data?`.
    fn frame(&mut self, kw: usize) {
        self.start(FRAME_STMT);
        self.token(EM_KW, kw);
        self.ws();
        self.frame_body();
        self.finish();
    }

    fn frame_body(&mut self) {
        let n = frame_len(self.rest());
        if n == 0 {
            return self.fail("expected a frame number of one to three digits, such as `01`");
        }
        self.token(NUMBER, n);
        self.ws();
        if !self.entity_type() {
            return self.fail(
                "expected an entity type: `ui`, `pcr`, `cmd`, `rmo`, or `evt` (or `processor`, \
                 `command`, `readmodel`, or `event`)",
            );
        }
        self.ws();
        let n = qualified_len(self.rest());
        if n == 0 {
            return self.fail("expected an entity name such as `AddItem` or `Cart.ItemAdded`");
        }
        self.token(IDENT, n);
        self.ws();
        while self.rest().starts_with("->>") {
            self.token(ARROW, 3);
            self.ws();
            let n = frame_len(self.rest());
            if n == 0 {
                return self.fail("expected a frame number after `->>`");
            }
            self.token(NUMBER, n);
            self.ws();
        }
        if self.rest().starts_with("[[") {
            self.token(L_BRACKET, 2);
            let n = id_len(self.rest());
            if n == 0 || !self.rest()[n..].starts_with("]]") {
                return self.fail("expected a data block name and `]]`, such as `[[AddItem01]]`");
            }
            self.token(IDENT, n);
            self.token(R_BRACKET, 2);
            self.ws();
        }
        if !self.data_type() {
            return;
        }
        if let Some(n) = inline_len(self.rest()) {
            self.token(EM_DATA, n);
        } else if self.rest().starts_with(['{', '"', '\'']) {
            return self.fail("unclosed data: expected a closing `}` or quote on the same line");
        }
        self.eol();
    }

    /// `entity Name`.
    fn entity(&mut self) {
        self.start(ENTITY);
        self.token(EM_KW, "entity".len());
        self.ws();
        let n = qualified_len(self.rest());
        if n == 0 {
            self.fail("expected an entity name");
        } else {
            self.token(IDENT, n);
            self.eol();
        }
        self.finish();
    }

    /// A `{` block on the lines below, after an optional data type.
    fn block(&mut self, indent: usize) {
        if !self.data_type() {
            return;
        }
        let r = self.rest();
        if !r.starts_with('{') {
            return self.fail("expected `{` and the data on the lines below it");
        }
        let line = &r[..scan::line_len(r)];
        if !line[1..].trim().is_empty() {
            return self.fail("expected a newline after `{`; a data block's lines go below it");
        }
        match block_len(r, indent) {
            Ok(n) => {
                self.token(EM_DATA, n);
                self.eol();
            }
            Err(n) => {
                let start = self.pos;
                self.token(EM_DATA, n);
                self.error(start, start + 1, "unclosed data block: expected a `}` line");
            }
        }
    }

    /// `data Name { ... }`.
    fn data(&mut self) {
        let indent = self.indent();
        self.start(DATA_STMT);
        self.token(EM_KW, "data".len());
        self.ws();
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected a data block name");
        } else {
            self.token(IDENT, n);
            self.ws();
            self.block(indent);
        }
        self.finish();
    }

    /// `note 01 { ... }`.
    fn note(&mut self) {
        let indent = self.indent();
        self.start(NOTE_STMT);
        self.token(EM_KW, "note".len());
        self.ws();
        let n = frame_len(self.rest());
        if n == 0 {
            self.fail("expected the number of the frame the note is on");
        } else {
            self.token(NUMBER, n);
            self.ws();
            self.block(indent);
        }
        self.finish();
    }

    /// The next word after whitespace and newlines, and the length of that gap.
    fn peek(&self) -> (usize, &str) {
        let r = self.rest();
        let gap = r.len() - r.trim_start().len();
        let s = &r[gap..];
        (gap, &s[..word_len(s)])
    }

    /// Whitespace and newlines up to the next word.
    fn skip_gap(&mut self) {
        loop {
            let r = self.rest();
            let ws = scan::ws_len(r);
            let nl = scan::newline_len(r);
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else {
                return;
            }
        }
    }

    /// `type Name` items after a `given`, `when`, or `then`; returns whether there was one.
    fn gwt_items(&mut self) -> bool {
        let mut any = false;
        while TYPES.contains(&self.peek().1) {
            self.skip_gap();
            self.entity_type();
            self.ws();
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected an entity name such as `ItemAdded`");
                return false;
            }
            self.token(IDENT, n);
            any = true;
        }
        if !any {
            self.fail("expected an entity type and name such as `evt ItemAdded`");
        }
        any
    }

    /// `gwt 01 given items (when items)? then items`, across lines.
    fn gwt(&mut self) {
        self.start(GWT_STMT);
        self.token(EM_KW, "gwt".len());
        self.ws();
        self.gwt_body();
        self.finish();
    }

    fn gwt_body(&mut self) {
        let n = frame_len(self.rest());
        if n == 0 {
            return self.fail("expected the number of the frame the scenario is on");
        }
        self.token(NUMBER, n);
        for (kw, required) in [("given", true), ("when", false), ("then", true)] {
            if self.peek().1 != kw {
                if required {
                    self.ws();
                    return self.fail(&format!("expected `{kw}`"));
                }
                continue;
            }
            self.skip_gap();
            self.token(EM_KW, kw.len());
            if !self.gwt_items() {
                return;
            }
        }
        self.eol();
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
            "%% c\neventmodeling\n  title Cart %% note\n  accTitle: A\n\
             tf 01 ui CartUI // why\n\
             timeframe 02 command AddItem [[AddItem01]]\n\
             tf 03 evt Cart.ItemAdded `json`{ \"a\": 1 } /* x */\n\
             rf 04 rmo Items ->> 03 ->> 1\n\
             tf 005 pcr P 'quoted'\n\
             entity ItemAdded\n\
             data AddItem01 {\n  description: 'john'\n  nested: {\n  }\n}\n\
             note 02 `md` {\nhello\n}\n\
             /* block\ncomment */\n\
             gwt 03\n  given\n    evt ItemAdded\n  when cmd AddItem\n  then\n    evt ItemAdded\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("FRAME_STMT@").count(), 5);
        assert_eq!(t.matches("ARROW@").count(), 2);
        assert_eq!(t.matches("EM_DATA@").count(), 4);
        assert_eq!(t.matches("EM_DATA_TYPE@").count(), 2);
        assert!(t.contains("DATA_STMT") && t.contains("NOTE_STMT") && t.contains("GWT_STMT"));
        assert!(t.contains("ENTITY@") && t.contains("TITLE_STMT"));
    }

    #[test]
    fn lengths() {
        assert_eq!(frame_len("01 "), 2);
        assert_eq!(frame_len("1234"), 0);
        assert_eq!(frame_len("01a"), 0);
        assert_eq!(qualified_len("A.b_c.D x"), 7);
        assert_eq!(qualified_len("A. x"), 1);
        assert_eq!(qualified_len("1A"), 0);
        assert_eq!(inline_len("{ a: {b} } x"), Some(10));
        assert_eq!(inline_len("'a' 'b'"), Some(7));
        assert_eq!(inline_len("{ a"), None);
        assert_eq!(block_len("{\n a\n }\n}\n", 0), Ok(9));
        assert_eq!(block_len("{\n a\n }\n", 2), Ok(7));
        assert_eq!(block_len("{\n a\n", 0), Err(5));
    }

    #[test]
    fn errors() {
        assert_eq!(tree("flowchart\n").1[0], "expected `eventmodeling` header");
        let one = |src: &str| {
            let d = tree(&format!("eventmodeling\n{src}\n")).1;
            assert_eq!(d.len(), 1, "{src}: {d:?}");
            d[0].clone()
        };
        assert!(one("tf 1234 ui A").starts_with("expected a frame number"));
        assert!(one("tf 01 view A").starts_with("expected an entity type"));
        assert!(one("tf 01 ui").starts_with("expected an entity name"));
        assert!(one("tf 01 ui A ->> x").starts_with("expected a frame number after"));
        assert!(one("tf 01 ui A [[x").starts_with("expected a data block name"));
        assert!(one("tf 01 ui A `yaml`{a}").starts_with("expected a data type"));
        assert!(one("tf 01 ui A { a").starts_with("unclosed data"));
        assert_eq!(one("tf 01 ui A B"), "expected the end of the line");
        assert!(one("data D { a }").starts_with("expected a newline after `{`"));
        assert!(one("data D").starts_with("expected `{`"));
        assert_eq!(
            one("data D {\na\n"),
            "unclosed data block: expected a `}` line"
        );
        assert!(one("note x").starts_with("expected the number"));
        assert_eq!(one("gwt 01 then evt A"), "expected `given`");
        assert_eq!(one("gwt 01 given evt A"), "expected `then`");
        assert!(one("gwt 01 given then evt A").starts_with("expected an entity type and name"));
        assert!(one("frame 01").starts_with("expected a frame such as"));
        assert_eq!(one("/* a"), "unclosed comment: expected `*/`");
    }

    #[test]
    fn indented_blocks() {
        // A `}` indented deeper than `data` belongs to the data.
        let (t, d) = tree("eventmodeling\n  data D {\n    a: {\n    }\n  }\n  tf 01 ui A [[D]]\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("EM_DATA@").count(), 1);
        assert_eq!(t.matches("FRAME_STMT@").count(), 1);
    }
}
