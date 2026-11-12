//! Line-oriented parser for PlantUML sequence diagrams, producing a lossless rowan tree
//! (PSEQ-1 to PSEQ-4). Each line is one statement; a note, `ref`, title, legend, header,
//! footer, or `skinparam` block holds its lines up to its closer.

use std::ops::{Deref, DerefMut};

use rowan::{GreenNode, TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

use super::Kind;

pub(crate) fn parse(src: &str, kind: Kind) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
        kind,
    };
    p.start(ROOT);
    p.document();
    p.finish();
    p.base.finish_tree()
}

pub(crate) const STATEMENT: &str = "expected a message such as `A -> B : text`, or a statement \
                                    such as `participant`, `note`, or `alt`";

/// The participant keywords (PSEQ-2).
pub(crate) const PARTICIPANTS: &[&str] = &[
    "participant",
    "actor",
    "boundary",
    "control",
    "entity",
    "database",
    "collections",
    "queue",
];

/// The other statement keywords, matched without regard to case.
const KEYWORDS: &[&str] = &[
    "activate",
    "deactivate",
    "destroy",
    "return",
    "autonumber",
    "alt",
    "else",
    "opt",
    "loop",
    "par",
    "break",
    "critical",
    "group",
    "end",
    "box",
    "hide",
    "show",
    "newpage",
    "autoactivate",
    "caption",
    "mainframe",
];

struct Parser<'a> {
    base: Base<'a>,
    /// The document's kind (PUML-1); every kind but sequence is read a line at a time.
    kind: Kind,
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

/// Length of a word at the start of `s`: letters, digits, and `_`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// The keyword `s` starts with, lowercased, if it is one of `words`.
fn keyword_in(s: &str, words: &[&'static str]) -> Option<&'static str> {
    let word = &s[..word_len(s)];
    words.iter().find(|k| k.eq_ignore_ascii_case(word)).copied()
}

/// The length of a participant name at the start of `s`: a `"..."` string on one line, or a
/// run of characters other than whitespace and the arrow and statement punctuation.
pub(crate) fn name_len(s: &str) -> usize {
    if let Some(body) = s.strip_prefix('"') {
        let line = &body[..scan::line_len(body)];
        return line.find('"').map_or(0, |i| i + 2);
    }
    s.find(|c: char| c.is_whitespace() || "-<>:[]?+*!\"#\\/&,=".contains(c))
        .unwrap_or(s.len())
}

/// `s` lowercased with its runs of whitespace as one space, to match closers like `end note`.
fn squeeze(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// The length of a head at the start of `s` (`>>`, `>`, `\\`, `\`, `//`, `/`), or with `left`,
/// the length of one ending at `s.len()` (`<<`, `<`, ...).
fn head_len(s: &str, left: bool) -> usize {
    let heads: &[&str] = if left {
        &["<<", "<", "\\\\", "\\", "//", "/"]
    } else {
        &[">>", ">", "\\\\", "\\", "//", "/"]
    };
    heads
        .iter()
        .find(|h| {
            if left {
                s.ends_with(*h)
            } else {
                s.starts_with(*h)
            }
        })
        .map_or(0, |h| h.len())
}

/// Whether an `x` or `o` arrow end may stand next to `c`: at a line's ends, whitespace, an
/// edge marker, or the modifiers and text after the target.
fn decor_boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| c.is_whitespace() || "[]?:+-*!".contains(c))
}

/// Where the message arrow in `line` starts, and its length (PSEQ-3): `-` or `--` with an
/// optional `[style]`, a head at either or both ends, and `x` or `o` beside a head.
pub(crate) fn find_arrow(line: &str) -> Option<(usize, usize)> {
    let mut quoted = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < line.len() {
        let c = bytes[i];
        if c == b'"' {
            quoted = !quoted;
        } else if !quoted && c == b':' {
            return None;
        } else if !quoted && c == b'-' {
            if let Some(found) = arrow_at(line, i) {
                return Some(found);
            }
            // Past this run of dashes.
            while i < line.len() && bytes[i] == b'-' {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    None
}

/// The arrow whose first dash is at `dash`, if one is there.
fn arrow_at(line: &str, dash: usize) -> Option<(usize, usize)> {
    let left_head = head_len(&line[..dash], true);
    let mut start = dash - left_head;
    // `x` or `o` before the head or the dashes: `[x->`, `?o->`, or `A x<- B`.
    let mut left_cross = false;
    if let Some('x' | 'o') = line[..start].chars().next_back() {
        let at = start - 1;
        let prev = line[..at].chars().next_back();
        if matches!(prev, Some('[' | '?'))
            || prev.is_some_and(char::is_whitespace) && !line[..at].trim().is_empty()
        {
            start = at;
            left_cross = true;
        }
    }
    // The dashes, with an optional `[style]` after the first.
    let mut end = dash + 1;
    if line[end..].starts_with('[') {
        end += line[end..].find(']')? + 1;
    }
    end += line[end..].len() - line[end..].trim_start_matches('-').len();
    let right_head = head_len(&line[end..], false);
    end += right_head;
    let mut right_cross = false;
    if let Some(c @ ('x' | 'o')) = line[end..].chars().next()
        && decor_boundary(line[end + 1..].chars().next())
    {
        end += c.len_utf8();
        right_cross = true;
    }
    (left_head > 0 || right_head > 0 || left_cross || right_cross).then_some((start, end - start))
}

impl Parser<'_> {
    fn warn(&mut self, start: usize, end: usize, message: impl Into<String>) {
        let range = TextRange::new(
            TextSize::try_from(start).expect("source fits"),
            TextSize::try_from(end).expect("source fits"),
        );
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    /// The text before `@startuml` (ignored), the header, the statements, and `@enduml`.
    /// Without any `@start` line, as in Markdown blocks, the whole source is the statements.
    fn document(&mut self) {
        let bom = scan::bom_len(self.src);
        self.token(WHITESPACE, bom);
        if !super::is_plantuml(self.src) {
            self.diagram(false);
            return;
        }
        loop {
            let r = self.rest();
            if self.at_eof() {
                // `is_plantuml` and the scanner disagree on a line break or a space.
                let at = self.pos;
                self.error(at, at, "expected `@startuml`");
                return;
            }
            if r[scan::ws_len(r)..].starts_with("@start") {
                break;
            }
            self.token(COMMENT, scan::line_len(r));
            let nl = scan::newline_len(self.rest());
            self.token(NEWLINE, nl);
        }
        self.ws();
        let r = self.rest();
        let n = 1 + word_len(&r[1..]);
        let kw = &r[..n];
        if !super::supported(self.kind) && !kw.eq_ignore_ascii_case("@startuml") {
            let message = format!("`{kw}` diagrams are not supported yet");
            let start = self.pos;
            self.start(HEADER);
            self.token(GRAPH_KW, n);
            self.finish();
            self.error(start, start + n, message);
            let rest = self.rest().len();
            self.token(COMMENT, rest);
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, n);
        self.ws();
        self.rest_of_line(TEXT);
        self.ws();
        self.finish();
        self.diagram(true);
    }

    /// The statements of the document's kind, or an error for a kind not drawn yet (PUML-2).
    fn diagram(&mut self, started: bool) {
        self.trivia();
        if !super::supported(self.kind) {
            let start = self.pos;
            let len = scan::line_len(self.rest());
            let message = format!(
                "PlantUML {} diagrams are not supported yet",
                self.kind.name()
            );
            self.error(start, start + len, message);
            let rest = self.rest().len();
            self.start(ERROR);
            self.token(ERROR_TOKEN, rest);
            self.finish();
            return;
        }
        if self.kind == Kind::Sequence {
            self.statements(started);
        } else {
            self.lines(started, super::raw(self.kind));
        }
    }

    /// Whether the line here is an `@end...` line, and its length.
    fn end_len(&self) -> Option<usize> {
        let r = self.rest();
        r.get(..4)
            .is_some_and(|p| p.eq_ignore_ascii_case("@end"))
            .then(|| 4 + word_len(&r[4..]))
    }

    /// The end of the statements: the missing `@enduml` warning, or the `@end...` line and the
    /// text after it. Returns whether the statements end here.
    fn at_end(&mut self, started: bool) -> bool {
        if self.at_eof() {
            if started {
                let at = self.pos;
                self.warn(
                    at,
                    at,
                    "missing `@enduml`; the diagram runs to the end of the file",
                );
            }
            return true;
        }
        let Some(n) = self.end_len() else {
            return false;
        };
        self.start(HEADER);
        self.token(END_KW, n);
        self.finish();
        self.after_end();
        true
    }

    /// One `TEXT_STMT` per line up to `@end...`, for the kinds whose lowering reads lines. In
    /// `raw` kinds, another language (DOT, EBNF, ...), `'` starts no comment.
    fn lines(&mut self, started: bool, raw: bool) {
        loop {
            if raw {
                self.blank();
            } else {
                self.trivia();
            }
            if self.at_end(started) {
                return;
            }
            let r = self.rest();
            let len = scan::trim_end_len(r, scan::line_len(r));
            self.start(TEXT_STMT);
            self.token(TEXT, len);
            self.finish();
        }
    }

    /// Statements up to `@enduml` or the end of input, where a missing `@enduml` is a warning
    /// after `@startuml`.
    fn statements(&mut self, started: bool) {
        loop {
            self.trivia();
            if self.at_end(started) {
                return;
            }
            let before = self.pos;
            self.statement();
            if self.pos == before {
                self.fail(STATEMENT);
            }
        }
    }

    /// The text after `@enduml`, ignored, with a warning at a second diagram.
    fn after_end(&mut self) {
        let start = self.pos;
        let rest = self.rest();
        let mut at = 0;
        while at < rest.len() {
            let len = scan::line_len(&rest[at..]);
            let line = &rest[at..at + len];
            let ws = scan::ws_len(line);
            if line[ws..].starts_with("@start") {
                let s = start + at + ws;
                self.warn(
                    s,
                    start + at + len,
                    "only the first diagram in a file is drawn",
                );
                break;
            }
            at += len + scan::newline_len(&rest[at + len..]);
        }
        let n = rest.len();
        self.token(COMMENT, n);
    }

    /// Whitespace and newlines.
    fn blank(&mut self) {
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

    /// Whitespace, newlines, `'` comments, and `/' ... '/` comments.
    fn trivia(&mut self) {
        loop {
            let r = self.rest();
            let ws = scan::ws_len(r);
            let nl = scan::newline_len(r);
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else if let Some(body) = r.strip_prefix("/'") {
                match body.find("'/") {
                    Some(i) => self.token(COMMENT, i + 4),
                    None => {
                        let start = self.pos;
                        self.token(COMMENT, r.len());
                        self.error(start, start + 2, "unclosed comment: expected `'/`");
                    }
                }
            } else if r.starts_with('\'') {
                self.token(COMMENT, scan::line_len(r));
            } else {
                return;
            }
        }
    }

    /// Requires the line to end here.
    fn eol(&mut self) {
        self.ws();
        let r = self.rest();
        if !(r.is_empty() || scan::newline_len(r) > 0) {
            self.fail("expected the end of the line");
        }
    }

    fn name_token(&mut self) -> bool {
        let r = self.rest();
        let n = name_len(r);
        if n == 0 {
            return false;
        }
        let kind = if r.starts_with('"') { STRING } else { IDENT };
        self.token(kind, n);
        true
    }

    fn statement(&mut self) {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        if r.starts_with('!') {
            self.directive();
        } else if line.starts_with("==") {
            self.bracketed("=", "==");
        } else if line.starts_with("...") {
            self.bracketed(".", "...");
        } else if line.starts_with("||") {
            self.space();
        } else if keyword_in(r, PARTICIPANTS).is_some() || keyword_in(r, &["create"]).is_some() {
            self.participant();
        } else if let Some(kw) = keyword_in(r, &["note", "hnote", "rnote"]) {
            self.note(kw.len());
        } else if keyword_in(r, &["ref"]).is_some()
            && keyword_in(r[3..].trim_start(), &["over"]).is_some()
        {
            self.reference();
        } else if keyword_in(r, &["title"]).is_some() {
            self.title();
        } else if let Some(kw) = keyword_in(r, &["legend", "header", "footer", "skinparam"]) {
            self.setting(kw);
        } else if let Some(kw) = keyword_in(r, KEYWORDS)
            && find_arrow(line).is_none()
        {
            self.keyword_stmt(kw);
        } else {
            self.message();
        }
    }

    /// `!include file`, `!define`, `!theme`, and the other preprocessor lines, kept whole.
    fn directive(&mut self) {
        self.start(SEQ_STMT);
        let n = 1 + word_len(&self.rest()[1..]);
        self.token(SEQ_KW, n);
        self.ws();
        self.rest_of_line(TEXT);
        self.finish();
    }

    /// `== text ==` or `... text ...`: the opening run of `c`, the text, and a closing run.
    fn bracketed(&mut self, c: &str, min: &str) {
        self.start(SEQ_STMT);
        let r = self.rest();
        let open = r.len() - r.trim_start_matches(c).len();
        self.token(SEQ_KW, open);
        self.ws();
        let r = self.rest();
        let line = scan::trim_end_len(r, scan::line_len(r));
        let body = &r[..line];
        let close = body.len() - body.trim_end_matches(c).len();
        if line == 0 {
            if min == "==" {
                self.fail("expected the separator's text and `==`");
            }
            self.finish();
            return;
        }
        if close < min.len() {
            self.fail(&format!("expected `{min}` after the text"));
            self.finish();
            return;
        }
        let text = scan::trim_end_len(body, line - close);
        self.token(TEXT, text);
        self.ws();
        let r = self.rest();
        let n = r.len() - r.trim_start_matches(c).len();
        self.token(SEQ_KW, n);
        self.eol();
        self.finish();
    }

    /// `|||` or `||N||`.
    fn space(&mut self) {
        self.start(SEQ_STMT);
        let r = self.rest();
        if r.starts_with("|||") {
            self.token(SEQ_KW, 3);
            self.eol();
            self.finish();
            return;
        }
        self.token(SEQ_KW, 2);
        self.ws();
        let n = self
            .rest()
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(self.rest().len());
        if n == 0 {
            self.fail("expected `|||` or a number of pixels such as `||45||`");
            self.finish();
            return;
        }
        self.token(NUMBER, n);
        self.ws();
        if !self.rest().starts_with("||") {
            self.fail("expected `||` after the number");
            self.finish();
            return;
        }
        self.token(SEQ_KW, 2);
        self.eol();
        self.finish();
    }

    /// `[create] KIND NAME [as NAME] [<<stereotype>>] [#color] [order N]`, or `create NAME`.
    fn participant(&mut self) {
        self.start(PARTICIPANT_STMT);
        if keyword_in(self.rest(), &["create"]).is_some() {
            self.token(SEQ_KW, 6);
            self.ws();
        }
        if let Some(kw) = keyword_in(self.rest(), PARTICIPANTS) {
            self.token(SEQ_KW, kw.len());
            self.ws();
        }
        if !self.name_token() {
            self.fail("expected a participant name");
            self.finish();
            return;
        }
        loop {
            self.ws();
            let r = self.rest();
            if r.is_empty() || scan::newline_len(r) > 0 {
                break;
            }
            if keyword_in(r, &["as"]).is_some() {
                self.token(AS_KW, 2);
                self.ws();
                if !self.name_token() {
                    self.fail("expected a name after `as`");
                    break;
                }
            } else if r.starts_with("<<") {
                match r[..scan::line_len(r)].find(">>") {
                    Some(i) => self.token(STEREOTYPE, i + 2),
                    None => {
                        self.fail("unclosed `<<`: expected `>>`");
                        break;
                    }
                }
            } else if r.starts_with('#') {
                let n = r.find(char::is_whitespace).unwrap_or(r.len());
                self.token(STYLE_TEXT, n);
            } else if keyword_in(r, &["order"]).is_some() {
                self.token(SEQ_KW, 5);
                self.ws();
                let n = crate::lines::number_len(self.rest());
                if n == 0 {
                    self.fail("expected a number after `order`");
                    break;
                }
                self.token(NUMBER, n);
            } else {
                self.fail("expected the end of the line");
                break;
            }
        }
        self.finish();
    }

    /// The names after `over`, or after `left of`: comma-separated.
    fn names(&mut self) -> bool {
        self.start(ID_LIST);
        let mut any = false;
        loop {
            if !self.name_token() {
                break;
            }
            any = true;
            self.ws();
            if !self.rest().starts_with(',') {
                break;
            }
            self.token(COMMA, 1);
            self.ws();
        }
        self.finish();
        any
    }

    /// `note left|right [of A]`, `note over A[, B]`, or `note across`, with an optional
    /// `#color`, then `: text` or lines up to `end note`.
    fn note(&mut self, kw: usize) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, kw);
        self.ws();
        let side = keyword_in(self.rest(), &["left", "right", "over", "across"]);
        let Some(side) = side else {
            self.fail("expected `left`, `right`, `over`, or `across`");
            self.finish();
            return;
        };
        self.token(SIDE, side.len());
        self.ws();
        match side {
            "left" | "right" => {
                if keyword_in(self.rest(), &["of"]).is_some() {
                    self.token(OF_KW, 2);
                    self.ws();
                    if !self.names() {
                        self.fail("expected a participant name");
                        self.finish();
                        return;
                    }
                }
            }
            "over" if !self.names() => {
                self.fail("expected a participant name");
                self.finish();
                return;
            }
            _ => {}
        }
        self.ws();
        let r = self.rest();
        if r.starts_with('#') {
            let n = r
                .find(|c: char| c.is_whitespace() || c == ':')
                .unwrap_or(r.len());
            self.token(STYLE_TEXT, n);
            self.ws();
        }
        self.text_or_body(&["end note", "endnote"], "note");
        self.finish();
    }

    /// `ref over A[, B] : text`, or lines up to `end ref`.
    fn reference(&mut self) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, 3);
        self.ws();
        self.token(SIDE, 4);
        self.ws();
        if !self.names() {
            self.fail("expected a participant name");
            self.finish();
            return;
        }
        self.ws();
        self.text_or_body(&["end ref", "endref"], "ref");
        self.finish();
    }

    /// `: text` to the end of the line, or the following lines up to one of `closers`.
    fn text_or_body(&mut self, closers: &[&str], what: &str) {
        let r = self.rest();
        if r.starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
        } else if r.is_empty() || scan::newline_len(r) > 0 {
            self.body(closers, what);
        } else {
            self.fail("expected `:` and the text, or the end of the line");
        }
    }

    /// The lines after this one up to one of `closers` (lowercase, words separated by one
    /// space), each a `TEXT`, and the closer an `END_KW`.
    fn body(&mut self, closers: &[&str], what: &str) {
        loop {
            let nl = scan::newline_len(self.rest());
            if nl == 0 {
                self.unclosed(format!("unclosed `{what}`: expected `{}`", closers[0]));
                return;
            }
            self.token(NEWLINE, nl);
            self.ws();
            let r = self.rest();
            let len = scan::trim_end_len(r, scan::line_len(r));
            if self.end_len().is_some() {
                self.unclosed(format!("unclosed `{what}`: expected `{}`", closers[0]));
                return;
            }
            if closers.contains(&squeeze(&r[..len]).as_str()) {
                self.token(END_KW, len);
                self.ws();
                return;
            }
            self.token(TEXT, len);
            self.ws();
        }
    }

    /// `title text`, or `title` and lines up to `end title`.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, 5);
        self.ws();
        let r = self.rest();
        if r.is_empty() || scan::newline_len(r) > 0 {
            self.body(&["end title", "endtitle"], "title");
        } else {
            if r.starts_with(':') {
                self.token(COLON, 1);
                self.ws();
            }
            self.rest_of_line(TEXT);
        }
        self.finish();
    }

    /// `legend` ... `endlegend`, `header` and `footer` (one line, or lines up to their
    /// closer), and `skinparam` (one line, or a `{` block): kept for the lowering to report.
    fn setting(&mut self, kw: &str) {
        self.start(SEQ_STMT);
        self.token(SEQ_KW, kw.len());
        self.ws();
        let r = self.rest();
        let line = scan::trim_end_len(r, scan::line_len(r));
        let block = match kw {
            "legend" => true,
            "header" | "footer" => line == 0,
            _ => r[..line].ends_with('{'),
        };
        self.rest_of_line(TEXT);
        self.ws();
        if block {
            match kw {
                "legend" => self.body(&["endlegend", "end legend"], "legend"),
                "header" => self.body(&["endheader", "end header"], "header"),
                "footer" => self.body(&["endfooter", "end footer"], "footer"),
                _ => self.body(&["}"], "skinparam"),
            }
        }
        self.finish();
    }

    /// Any other keyword statement: its keyword, then a participant name or text.
    fn keyword_stmt(&mut self, kw: &str) {
        self.start(SEQ_STMT);
        self.token(SEQ_KW, kw.len());
        self.ws();
        match kw {
            "activate" | "deactivate" | "destroy" => {
                if !self.name_token() {
                    self.fail("expected a participant name");
                    self.finish();
                    return;
                }
                self.ws();
                let r = self.rest();
                if r.starts_with('#') {
                    let n = r.find(char::is_whitespace).unwrap_or(r.len());
                    self.token(STYLE_TEXT, n);
                }
                self.eol();
            }
            "end" => {
                if let Some(what) = keyword_in(self.rest(), &["box"]) {
                    self.token(SEQ_KW, what.len());
                }
                self.eol();
            }
            _ => self.rest_of_line(TEXT),
        }
        self.finish();
    }

    /// `FROM ARROW TO [MODIFIERS] [: TEXT]`, where an end may be `[`, `]`, or `?`. Consumes
    /// nothing when the line has no arrow.
    fn message(&mut self) {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        let Some((at, len)) = find_arrow(line) else {
            return;
        };
        self.start(MESSAGE_STMT);
        let from = &line[..scan::trim_end_len(line, at)];
        match from {
            "[" => self.token(L_BRACKET, 1),
            "?" => self.token(IDENT, 1),
            _ if !from.is_empty() && name_len(from) == from.len() => {
                self.name_token_len(from.len())
            }
            _ => {
                self.fail("expected a participant before the arrow");
                self.finish();
                return;
            }
        }
        self.ws();
        self.token(ARROW, len);
        self.ws();
        let r = self.rest();
        if r.starts_with(']') {
            self.token(R_BRACKET, 1);
        } else if r.starts_with('?') {
            self.token(IDENT, 1);
        } else if !self.name_token() {
            self.fail("expected a participant after the arrow");
            self.finish();
            return;
        }
        self.ws();
        let r = self.rest();
        let n = r.len() - r.trim_start_matches(['+', '-', '*', '!']).len();
        self.token(ACTIVATION, n);
        self.ws();
        let r = self.rest();
        if r.starts_with('#') {
            let n = r
                .find(|c: char| c.is_whitespace() || c == ':')
                .unwrap_or(r.len());
            self.token(STYLE_TEXT, n);
            self.ws();
        }
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
        }
        self.eol();
        self.finish();
    }

    fn name_token_len(&mut self, n: usize) {
        let kind = if self.rest().starts_with('"') {
            STRING
        } else {
            IDENT
        };
        self.token(kind, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxNode;

    fn tree(src: &str) -> (String, Vec<String>) {
        let (green, d) = parse(src, super::super::kind(src));
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.to_string(), src);
        (
            format!("{root:#?}"),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    fn doc(body: &str) -> String {
        format!("@startuml\n{body}\n@enduml\n")
    }

    #[test]
    fn arrows() {
        fn a(s: &str) -> Option<&str> {
            find_arrow(s).map(|(at, n)| &s[at..at + n])
        }
        assert_eq!(a("A -> B"), Some("->"));
        assert_eq!(a("A --> B: x -> y"), Some("-->"));
        assert_eq!(a("A->>B"), Some("->>"));
        assert_eq!(a("A <- B"), Some("<-"));
        assert_eq!(a("A <-> B"), Some("<->"));
        assert_eq!(a("A ->x B"), Some("->x"));
        assert_eq!(a("A x<- B"), Some("x<-"));
        assert_eq!(a("[-> B"), Some("->"));
        assert_eq!(a("[o-> B"), Some("o->"));
        assert_eq!(a("[x-> B"), Some("x->"));
        assert_eq!(a("A ->o]"), Some("->o"));
        assert_eq!(a("A -\\ B"), Some("-\\"));
        assert_eq!(a("A -[#red]> B"), Some("-[#red]>"));
        assert_eq!(a("A -[#red]-> B"), Some("-[#red]->"));
        assert_eq!(a("Max->Bob"), Some("->"));
        assert_eq!(a("x->y"), Some("->"));
        assert_eq!(a("A - B"), None);
        assert_eq!(a("A: x -> y"), None);
        assert_eq!(a("\"a->b\" -> c"), Some("->"));
    }

    #[test]
    fn documents() {
        assert!(tree("@startuml\n@enduml\n").1.is_empty());
        assert!(
            tree("prose\n@startuml name\nA -> B\n@enduml\nafter\n")
                .1
                .is_empty()
        );
        // Without `@startuml`, the whole source is the diagram.
        assert!(tree("A -> B\n").1.is_empty());
        assert_eq!(
            tree("robust W\n").1,
            ["PlantUML timing diagrams are not supported yet"]
        );
        assert_eq!(
            tree("@startuml\nA -> B\n").1,
            ["missing `@enduml`; the diagram runs to the end of the file"]
        );
        assert_eq!(
            tree("@startuml\n@enduml\n@startuml\n@enduml\n").1,
            ["only the first diagram in a file is drawn"]
        );
        assert_eq!(
            tree("@startsalt\n{}\n@endsalt\n").1,
            ["`@startsalt` diagrams are not supported yet"]
        );
        // Line-read kinds hold one statement per line and end at any `@end...` line.
        let (t, d) = tree("@startuml\nstart\n:a;\n@enduml\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("TEXT_STMT@").count(), 2);
        assert_eq!(
            tree(&doc("' c\n/' a\nb '/\nA -> B")).1,
            Vec::<String>::new()
        );
        assert_eq!(
            tree(&doc("/' a")).1,
            [
                "unclosed comment: expected `'/`",
                "missing `@enduml`; the diagram runs to the end of the file"
            ]
        );
    }

    #[test]
    fn statements() {
        let (t, d) = tree(&doc(
            "participant \"Long\" as L #red <<svc>> order 2\nactor Bob\ncreate control C\n\
             L -> Bob ++ : hi\nBob --> L --: ok\n[-> L\nL ->] : out\n?-> Bob\n\
             note left of L : n\nnote over L, Bob\n  two\n  lines\nend note\nnote across: all\n\
             ref over L : r\nalt yes\nelse no\nend\ngroup g [x]\nend\n== sep ==\n...\n\
             ... later ...\n|||\n||45||\nactivate Bob #blue\nreturn done\nautonumber 10 5\n\
             title T\nlegend\nx\nendlegend\nskinparam a {\n b c\n}\n!include x.puml\nbox \"B\" #ccc\nend box\nhide footbox",
        ));
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("PARTICIPANT_STMT@").count(), 3);
        assert_eq!(t.matches("MESSAGE_STMT@").count(), 5);
        assert_eq!(t.matches("NOTE_STMT@").count(), 4);
        assert_eq!(t.matches("TITLE_STMT@").count(), 1);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&doc(s)).1;
        assert_eq!(e("participant"), ["expected a participant name"]);
        assert_eq!(e("participant A as"), ["expected a name after `as`"]);
        assert_eq!(e("participant A <<x"), ["unclosed `<<`: expected `>>`"]);
        assert_eq!(
            e("participant A order"),
            ["expected a number after `order`"]
        );
        assert_eq!(e("participant A B"), ["expected the end of the line"]);
        assert_eq!(
            e("note above A"),
            ["expected `left`, `right`, `over`, or `across`"]
        );
        assert_eq!(e("note over : x"), ["expected a participant name"]);
        assert_eq!(
            e("note left of A x"),
            ["expected `:` and the text, or the end of the line"]
        );
        assert_eq!(
            e("note over A\nx"),
            ["unclosed `note`: expected `end note`"]
        );
        assert_eq!(e("== x"), ["expected `==` after the text"]);
        assert_eq!(e("=="), ["expected the separator's text and `==`"]);
        assert_eq!(e("... x"), ["expected `...` after the text"]);
        assert_eq!(
            e("||x||"),
            ["expected `|||` or a number of pixels such as `||45||`"]
        );
        assert_eq!(e("||4"), ["expected `||` after the number"]);
        assert_eq!(e("activate"), ["expected a participant name"]);
        assert_eq!(e("-> B"), ["expected a participant before the arrow"]);
        assert_eq!(e("A ->"), ["expected a participant after the arrow"]);
        assert_eq!(e("A -> B C"), ["expected the end of the line"]);
        assert_eq!(e("hello"), [STATEMENT]);
    }

    #[test]
    fn words() {
        assert_eq!(name_len("Alice -> B"), 5);
        assert_eq!(name_len("\"A b\" -> c"), 5);
        assert_eq!(name_len("B++"), 1);
        assert_eq!(squeeze("End   Note"), "end note");
    }
}
