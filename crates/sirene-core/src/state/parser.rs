//! Line-oriented parser for Mermaid state diagrams, producing a lossless rowan tree (LANGS-2).
//!
//! Every byte of input is emitted as exactly one token, so the tree's text always equals the
//! input. Errors are recorded as diagnostics and confined to the statement they occur in.

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        src,
        pos: 0,
        builder: GreenNodeBuilder::new(),
        diagnostics: Vec::new(),
    };
    p.root();
    (p.builder.finish(), p.diagnostics)
}

/// The header keywords, longest first.
pub(crate) const HEADERS: &[&str] = &["stateDiagram-v2", "stateDiagram"];

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
}

fn offset(n: usize) -> TextSize {
    TextSize::try_from(n).expect("source larger than 4 GiB")
}

/// Length of a state id at the start of `s`: up to whitespace, punctuation that ends a
/// statement part, or an arrow.
fn id_len(s: &str) -> usize {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() || matches!(c, ':' | '{' | '}' | '"' | ',' | ';' | '[' | ']' | '<') {
            break;
        }
        if s[i..].starts_with("--") {
            break;
        }
        end = i + c.len_utf8();
    }
    end
}

/// Whether `s` starts with `kw` as a whole word.
fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && id_len(s) == kw.len()
}

/// Whether a statement may end here: end of input, a newline, `;`, or a closing `}`.
fn is_end(s: &str) -> bool {
    scan::is_stmt_end(s) || s.starts_with('}')
}

impl Parser<'_> {
    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    fn at_eof(&self) -> bool {
        self.pos == self.src.len()
    }

    fn token(&mut self, kind: SyntaxKind, len: usize) {
        if len == 0 {
            return;
        }
        let text = &self.src[self.pos..self.pos + len];
        self.builder.token(kind.into(), text);
        self.pos += len;
    }

    fn start(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into());
    }

    fn finish(&mut self) {
        self.builder.finish_node();
    }

    fn error(&mut self, start: usize, end: usize, message: impl Into<String>) {
        let range = TextRange::new(offset(start), offset(end));
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn ws(&mut self) {
        let n = scan::ws_len(self.rest());
        self.token(WHITESPACE, n);
    }

    /// Wraps the rest of the line in an `ERROR` node and reports `message`; a zero-width error
    /// when the line has ended.
    fn fail(&mut self, message: &str) {
        self.ws();
        let start = self.pos;
        let len = scan::line_len(self.rest());
        if len == 0 {
            self.error(start, start, message);
            return;
        }
        self.start(ERROR);
        self.token(ERROR_TOKEN, len);
        self.finish();
        self.error(start, start + len, message);
    }

    /// Requires the statement to end here.
    fn end(&mut self) {
        let n = scan::ws_len(self.rest());
        if !is_end(&self.rest()[n..]) {
            self.fail("expected the end of the line");
        }
    }

    /// The rest of the line as one `kind` token, without trailing whitespace.
    fn rest_of_line(&mut self, kind: SyntaxKind) {
        let len = scan::trim_end_len(self.rest(), scan::line_len(self.rest()));
        self.token(kind, len);
    }

    fn at_line_start(&self) -> bool {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        line.chars().all(char::is_whitespace)
    }

    // ---- document structure ----

    fn root(&mut self) {
        self.start(ROOT);
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if let Some(kw) = HEADERS.iter().find(|kw| keyword(r, kw)) {
            self.start(HEADER);
            self.token(GRAPH_KW, kw.len());
            self.end();
            self.finish();
        } else {
            let end = self.pos + scan::stmt_len(r);
            self.error(self.pos, end, "expected `stateDiagram` header");
        }
        self.block(0);
        self.finish();
    }

    fn frontmatter(&mut self) {
        let first = scan::line_len(self.src);
        if self.src[..first].trim_end() != "---" {
            return;
        }
        let mut line_start = first + scan::newline_len(&self.src[first..]);
        while line_start < self.src.len() {
            let rest = &self.src[line_start..];
            let len = scan::line_len(rest);
            if rest[..len].trim() == "---" {
                self.token(FRONTMATTER, line_start + len);
                return;
            }
            line_start += len + scan::newline_len(&rest[len..]);
        }
        self.start(ERROR);
        self.token(ERROR_TOKEN, first);
        self.finish();
        self.error(
            0,
            first,
            "unclosed frontmatter: expected a closing `---` line",
        );
    }

    fn trivia(&mut self) {
        loop {
            let r = self.rest();
            let ws = scan::ws_len(r);
            let nl = scan::newline_len(r);
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else if r.starts_with(';') {
                self.token(SEMI, 1);
            } else if let Some(body) = r.strip_prefix("%%{") {
                match body.find("}%%") {
                    Some(i) => self.token(DIRECTIVE, i + 6),
                    None => {
                        let start = self.pos;
                        self.token(DIRECTIVE, scan::line_len(r));
                        self.error(start, start + 3, "unclosed directive: expected `}%%`");
                    }
                }
            } else if r.starts_with("%%") && self.at_line_start() {
                self.token(COMMENT, scan::line_len(r));
            } else {
                return;
            }
        }
    }

    /// Statements up to the end of input, or at `depth` > 0 up to and including the `}` that
    /// closes the composite state.
    fn block(&mut self, depth: usize) {
        loop {
            self.trivia();
            if self.at_eof() {
                if depth > 0 {
                    self.error(self.pos, self.pos, "unclosed composite state: expected `}`");
                }
                return;
            }
            if self.rest().starts_with('}') {
                if depth > 0 {
                    self.token(R_BRACE, 1);
                    return;
                }
                let start = self.pos;
                self.start(ERROR);
                self.token(ERROR_TOKEN, 1);
                self.finish();
                self.error(start, start + 1, "unexpected `}`");
                continue;
            }
            let before = self.pos;
            self.statement(depth);
            if self.pos == before {
                // Never stall: report and skip the line.
                self.fail("expected a state, transition, or other statement");
            }
        }
    }

    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if r.starts_with("--") && is_end(r[2..].trim_start_matches([' ', '\t'])) {
            self.start(REGION_STMT);
            self.token(REGION_SEP, 2);
            self.finish();
        } else if keyword(r, "direction") {
            self.direction();
        } else if keyword(r, "classDef") {
            self.class_def();
        } else if keyword(r, "class") {
            self.class();
        } else if keyword(r, "style") {
            self.style();
        } else if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "click") {
            self.start(CLICK_STMT);
            self.token(CLICK_KW, 5);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if keyword(r, "note") {
            self.note();
        } else if keyword(r, "state") {
            self.state(depth);
        } else {
            self.reference_stmt();
        }
    }

    fn direction(&mut self) {
        self.start(DIRECTION_STMT);
        self.token(DIRECTION_KW, "direction".len());
        self.ws();
        let n = scan::dir_len(self.rest());
        if n > 0 {
            self.token(DIR, n);
            self.end();
        } else {
            self.fail("expected `TB`, `BT`, `LR`, or `RL`");
        }
        self.finish();
    }

    /// A comma-separated list of ids.
    fn id_list(&mut self) -> bool {
        self.start(ID_LIST);
        let mut any = false;
        loop {
            let n = id_len(self.rest());
            if n == 0 {
                break;
            }
            self.token(IDENT, n);
            any = true;
            if !self.rest().starts_with(',') {
                break;
            }
            self.token(COMMA, 1);
        }
        self.finish();
        any
    }

    fn class_def(&mut self) {
        self.start(CLASSDEF_STMT);
        self.token(CLASSDEF_KW, "classDef".len());
        self.ws();
        if self.id_list() {
            self.ws();
            self.rest_of_line(STYLE_TEXT);
        } else {
            self.fail("expected a class name");
        }
        self.finish();
    }

    fn class(&mut self) {
        self.start(CLASS_STMT);
        self.token(CLASS_KW, "class".len());
        self.ws();
        if self.id_list() {
            self.ws();
            let n = id_len(self.rest());
            if n > 0 {
                self.token(IDENT, n);
                self.end();
            } else {
                self.fail("expected a class name");
            }
        } else {
            self.fail("expected a state id");
        }
        self.finish();
    }

    fn style(&mut self) {
        self.start(STYLE_STMT);
        self.token(STYLE_KW, "style".len());
        self.ws();
        if self.id_list() {
            self.ws();
            self.rest_of_line(STYLE_TEXT);
        } else {
            self.fail("expected a state id");
        }
        self.finish();
    }

    fn acc(&mut self) {
        let title = self.rest().starts_with("accTitle");
        let (node, kw) = if title {
            (ACC_TITLE_STMT, ACC_TITLE_KW)
        } else {
            (ACC_DESCR_STMT, ACC_DESCR_KW)
        };
        self.start(node);
        self.token(kw, "accTitle".len());
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(ACC_TEXT);
        } else if !title && self.rest().starts_with('{') {
            self.token(L_BRACE, 1);
            match self.rest().find('}') {
                Some(i) => {
                    self.token(ACC_TEXT, i);
                    self.token(R_BRACE, 1);
                }
                None => {
                    let start = self.pos;
                    let len = self.rest().len();
                    self.token(ACC_TEXT, len);
                    self.error(start, start + len, "unclosed `accDescr {`: expected `}`");
                }
            }
        } else {
            self.fail("expected `:`");
        }
        self.finish();
    }

    /// `note left of S : text`, or a note whose text runs to an `end note` line.
    fn note(&mut self) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, 4);
        self.ws();
        let r = self.rest();
        let side = ["left", "right"].into_iter().find(|s| keyword(r, s));
        if let Some(side) = side {
            self.token(SIDE, side.len());
            self.ws();
            if keyword(self.rest(), "of") {
                self.token(OF_KW, 2);
                self.ws();
            } else {
                self.fail("expected `of`");
                self.finish();
                return;
            }
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected a state id");
                self.finish();
                return;
            }
            self.token(IDENT, n);
            self.ws();
        } else if !self.rest().starts_with('"') {
            self.fail("expected `left of` or `right of`");
            self.finish();
            return;
        }
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
        } else if side.is_some() && scan::is_stmt_end(self.rest()) {
            self.note_body();
        } else {
            // A floating note, `note "text" as N`; the lowering reports it.
            self.rest_of_line(TEXT);
        }
        self.finish();
    }

    /// The lines after `note left of S` up to `end note`.
    fn note_body(&mut self) {
        let start = self.pos;
        let mut at = start;
        loop {
            let rest = &self.src[at..];
            if rest.is_empty() {
                let len = self.src.len() - start;
                self.token(TEXT, len);
                self.error(start, self.pos, "unclosed note: expected `end note`");
                return;
            }
            let len = scan::line_len(rest);
            let line = rest[..len].trim();
            let words: Vec<&str> = line.split_whitespace().collect();
            if at > start && words == ["end", "note"] {
                let indent = scan::ws_len(rest);
                self.token(TEXT, at + indent - start);
                self.token(END_KW, 3);
                self.ws();
                self.token(NOTE_KW, 4);
                return;
            }
            at += len + scan::newline_len(&rest[len..]);
        }
    }

    /// `state ...`: a declaration, a pseudo-state, or a composite state.
    fn state(&mut self, depth: usize) {
        let checkpoint = self.builder.checkpoint();
        self.token(STATE_KW, 5);
        self.ws();
        if self.rest().starts_with('"') {
            let line = &self.rest()[..scan::line_len(self.rest())];
            match line[1..].find('"').map(|i| i + 2) {
                Some(n) => self.token(STRING, n),
                None => {
                    self.builder.start_node_at(checkpoint, STATE_STMT.into());
                    self.fail("unclosed string: expected `\"`");
                    self.finish();
                    return;
                }
            }
            self.ws();
            if keyword(self.rest(), "as") {
                self.token(AS_KW, 2);
                self.ws();
            } else {
                self.builder.start_node_at(checkpoint, STATE_STMT.into());
                self.fail("expected `as`");
                self.finish();
                return;
            }
        }
        let n = id_len(self.rest());
        if n == 0 {
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.fail("expected a state id");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        self.ws();
        let r = self.rest();
        if r.starts_with("<<") {
            let len = r.find(">>").map_or(scan::line_len(r), |i| i + 2);
            self.token(STEREOTYPE, len);
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.end();
            self.finish();
        } else if r.starts_with('{') {
            self.builder.start_node_at(checkpoint, COMPOSITE.into());
            self.token(L_BRACE, 1);
            self.block(depth + 1);
            self.finish();
        } else if r.starts_with(':') {
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else {
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.end();
            self.finish();
        }
    }

    /// A state reference: `[*]` or an id, with an optional `:::class`.
    fn reference(&mut self) -> bool {
        let star = self.rest().starts_with("[*]");
        let n = if star { 3 } else { id_len(self.rest()) };
        if n == 0 {
            return false;
        }
        self.start(STATE_REF);
        self.token(if star { STAR } else { IDENT }, n);
        if self.rest().starts_with(":::") {
            self.token(TRIPLE_COLON, 3);
            let n = id_len(self.rest());
            self.token(IDENT, n);
        }
        self.finish();
        true
    }

    /// A statement starting with a state: a transition, a description, or a bare state.
    fn reference_stmt(&mut self) {
        let checkpoint = self.builder.checkpoint();
        if !self.reference() {
            return;
        }
        let n = scan::ws_len(self.rest());
        let after = &self.rest()[n..];
        if after.starts_with("-->") {
            self.builder
                .start_node_at(checkpoint, TRANSITION_STMT.into());
            self.ws();
            self.token(STROKE, 3);
            self.ws();
            if !self.reference() {
                self.fail("expected a state after `-->`");
                self.finish();
                return;
            }
            self.ws();
            if self.rest().starts_with(':') {
                self.token(COLON, 1);
                self.ws();
                self.rest_of_line(TEXT);
            } else {
                self.end();
            }
            self.finish();
        } else if after.starts_with(':') {
            self.builder
                .start_node_at(checkpoint, DESCRIPTION_STMT.into());
            self.ws();
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else {
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.end();
            self.finish();
        }
    }
}
