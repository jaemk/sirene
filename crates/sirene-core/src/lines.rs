//! The shared machinery of the line-oriented parsers (state, class, ER): a rowan builder over
//! the source, where every byte becomes exactly one token, so a tree's text always equals its
//! input (LANGS-2).

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::syntax::SyntaxKind::{self, *};

pub(crate) struct Base<'a> {
    pub src: &'a str,
    pub pos: usize,
    pub builder: GreenNodeBuilder<'static>,
    pub diagnostics: Vec<Diagnostic>,
}

fn offset(n: usize) -> TextSize {
    TextSize::try_from(n).expect("source larger than 4 GiB")
}

/// Whether a statement may end here: end of input, a newline, `;`, or a closing `}`.
pub(crate) fn is_end(s: &str) -> bool {
    scan::is_stmt_end(s) || s.starts_with('}')
}

impl<'a> Base<'a> {
    pub fn new(src: &'a str) -> Self {
        Base {
            src,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn finish_tree(self) -> (GreenNode, Vec<Diagnostic>) {
        (self.builder.finish(), self.diagnostics)
    }

    pub fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    pub fn at_eof(&self) -> bool {
        self.pos == self.src.len()
    }

    pub fn token(&mut self, kind: SyntaxKind, len: usize) {
        if len == 0 {
            return;
        }
        let text = &self.src[self.pos..self.pos + len];
        self.builder.token(kind.into(), text);
        self.pos += len;
    }

    pub fn start(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into());
    }

    pub fn finish(&mut self) {
        self.builder.finish_node();
    }

    pub fn error(&mut self, start: usize, end: usize, message: impl Into<String>) {
        let range = TextRange::new(offset(start), offset(end));
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    pub fn ws(&mut self) {
        let n = scan::ws_len(self.rest());
        self.token(WHITESPACE, n);
    }

    /// Wraps the rest of the line in an `ERROR` node and reports `message`; a zero-width error
    /// when the line has ended.
    pub fn fail(&mut self, message: &str) {
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
    pub fn end(&mut self) {
        let n = scan::ws_len(self.rest());
        if !is_end(&self.rest()[n..]) {
            self.fail("expected the end of the line");
        }
    }

    /// The rest of the line as one `kind` token, without trailing whitespace.
    pub fn rest_of_line(&mut self, kind: SyntaxKind) {
        let len = scan::trim_end_len(self.rest(), scan::line_len(self.rest()));
        self.token(kind, len);
    }

    /// The length of a `"..."` string at the cursor, closed on the same line.
    pub fn string_len(&self) -> Option<usize> {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        line.strip_prefix('"')?.find('"').map(|i| i + 2)
    }

    fn at_line_start(&self) -> bool {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        line.chars().all(char::is_whitespace)
    }

    /// Frontmatter, trivia, and a `HEADER` holding one of `headers` (each a whole word, per
    /// `id_len`), or an error naming `first`.
    pub fn header(&mut self, headers: &[&str], first: &str, id_len: fn(&str) -> usize) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if let Some(kw) = headers
            .iter()
            .find(|kw| r.starts_with(*kw) && id_len(r) == kw.len())
        {
            self.start(HEADER);
            self.token(GRAPH_KW, kw.len());
            self.end();
            self.finish();
        } else {
            let end = self.pos + scan::stmt_len(r);
            self.error(self.pos, end, format!("expected `{first}` header"));
        }
    }

    /// A frontmatter block at the start of the source, if any.
    pub fn frontmatter(&mut self) {
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

    /// Whitespace, newlines, `;`, comments, and directives between statements.
    pub fn trivia(&mut self) {
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

    /// `direction TB|BT|LR|RL`.
    pub fn direction(&mut self) {
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

    /// A comma-separated list of ids. Returns whether it holds one.
    pub fn id_list(&mut self, id_len: fn(&str) -> usize) -> bool {
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

    /// `classDef a,b css`.
    pub fn class_def(&mut self, id_len: fn(&str) -> usize) {
        self.start(CLASSDEF_STMT);
        self.token(CLASSDEF_KW, "classDef".len());
        self.ws();
        if self.id_list(id_len) {
            self.ws();
            self.rest_of_line(STYLE_TEXT);
        } else {
            self.fail("expected a class name");
        }
        self.finish();
    }

    /// `class a,b name`, applying a `classDef` to the ids; `what` names an id.
    pub fn class_stmt(&mut self, id_len: fn(&str) -> usize, what: &str) {
        self.start(CLASS_STMT);
        self.token(CLASS_KW, "class".len());
        self.ws();
        if self.id_list(id_len) {
            self.ws();
            let n = id_len(self.rest());
            if n > 0 {
                self.token(IDENT, n);
                self.end();
            } else {
                self.fail("expected a class name");
            }
        } else {
            self.fail(&format!("expected {what}"));
        }
        self.finish();
    }

    /// `style a,b css`; `what` names an id.
    pub fn style(&mut self, id_len: fn(&str) -> usize, what: &str) {
        self.start(STYLE_STMT);
        self.token(STYLE_KW, "style".len());
        self.ws();
        if self.id_list(id_len) {
            self.ws();
            self.rest_of_line(STYLE_TEXT);
        } else {
            self.fail(&format!("expected {what}"));
        }
        self.finish();
    }

    /// `accTitle: text`, `accDescr: text`, or `accDescr { text }`.
    pub fn acc(&mut self) {
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

    /// `click ...`, kept as text for the lowering to report.
    pub fn click(&mut self) {
        self.start(CLICK_STMT);
        self.token(CLICK_KW, 5);
        self.ws();
        self.rest_of_line(TEXT);
        self.finish();
    }

    /// Statements up to the end of input, or at `depth` > 0 up to and including the closing
    /// `}` (an error naming `what` when it is missing). `statement` parses one; when it consumes
    /// nothing the line is reported as `expected`.
    pub fn block<P: std::ops::DerefMut<Target = Self>>(
        p: &mut P,
        depth: usize,
        what: &str,
        expected: &str,
        statement: fn(&mut P, usize),
    ) {
        loop {
            p.trivia();
            if p.at_eof() {
                if depth > 0 {
                    let at = p.pos;
                    p.error(at, at, format!("unclosed {what}: expected `}}`"));
                }
                return;
            }
            if p.rest().starts_with('}') {
                if depth > 0 {
                    p.token(R_BRACE, 1);
                    return;
                }
                let start = p.pos;
                p.start(ERROR);
                p.token(ERROR_TOKEN, 1);
                p.finish();
                p.error(start, start + 1, "unexpected `}`");
                continue;
            }
            let before = p.pos;
            statement(p, depth);
            if p.pos == before {
                // Never stall: report and skip the line.
                p.fail(expected);
            }
        }
    }
}
