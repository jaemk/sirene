//! Line-oriented parser for Mermaid state diagrams, producing a lossless rowan tree (LANGS-2).
//!
//! Every byte of input is emitted as exactly one token, so the tree's text always equals the
//! input. Errors are recorded as diagnostics and confined to the statement they occur in.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, is_end};
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "stateDiagram", id_len);
    Base::block(&mut p, 0, "composite state", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

/// The header keywords, longest first.
pub(crate) const HEADERS: &[&str] = &["stateDiagram-v2", "stateDiagram"];

const STATEMENT: &str = "expected a state, transition, or other statement";

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

impl Parser<'_> {
    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if r.starts_with("--") && is_end(r[2..].trim_start_matches([' ', '\t'])) {
            self.start(REGION_STMT);
            self.token(REGION_SEP, 2);
            self.finish();
        } else if keyword(r, "direction") {
            self.direction();
        } else if keyword(r, "classDef") {
            self.class_def(id_len);
        } else if keyword(r, "class") {
            self.class_stmt(id_len, "a state id");
        } else if keyword(r, "style") {
            self.style(id_len, "a state id");
        } else if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "click") {
            self.click();
        } else if keyword(r, "note") {
            self.note();
        } else if keyword(r, "state") {
            self.state(depth);
        } else {
            self.reference_stmt();
        }
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
                let end = self.pos;
                self.error(start, end, "unclosed note: expected `end note`");
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
            match self.string_len() {
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
        if self.rest().starts_with(":::") {
            self.token(TRIPLE_COLON, 3);
            let n = id_len(self.rest());
            self.token(IDENT, n);
        }
        self.ws();
        let r = self.rest();
        let close = if r.starts_with("<<") {
            Some(">>")
        } else if r.starts_with("[[") {
            Some("]]")
        } else {
            None
        };
        if let Some(close) = close {
            let len = r.find(close).map_or(scan::line_len(r), |i| i + 2);
            self.token(STEREOTYPE, len);
            self.builder.start_node_at(checkpoint, STATE_STMT.into());
            self.end();
            self.finish();
        } else if r.starts_with('{') {
            self.builder.start_node_at(checkpoint, COMPOSITE.into());
            self.token(L_BRACE, 1);
            Base::block(
                self,
                depth + 1,
                "composite state",
                STATEMENT,
                Parser::statement,
            );
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
