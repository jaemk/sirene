//! Scannerless recursive-descent parser producing a lossless rowan tree.
//!
//! Every byte of input is emitted as exactly one token, so the tree's text always equals the
//! input (FLOWCH-1). Errors are recorded as diagnostics and confined to the statement they occur
//! in (FLOWCH-2).

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use super::scan::{self, Opener, Stroke};
use crate::diagnostic::Diagnostic;
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

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
}

/// Mermaid's other diagram types, by the keyword that starts them.
const OTHER_DIAGRAMS: &[&str] = &[
    "architecture",
    "architecture-beta",
    "block",
    "block-beta",
    "C4Component",
    "C4Container",
    "C4Context",
    "C4Deployment",
    "C4Dynamic",
    "classDiagram",
    "classDiagram-v2",
    "erDiagram",
    "gantt",
    "gitGraph",
    "info",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "packet-beta",
    "pie",
    "quadrantChart",
    "radar-beta",
    "requirementDiagram",
    "sankey",
    "sankey-beta",
    "sequenceDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "timeline",
    "treemap-beta",
    "xychart",
    "xychart-beta",
    "zenuml",
];

/// The other diagram type `r` starts with, if any (FLOWCH-16).
pub(super) fn other_diagram(r: &str) -> Option<&str> {
    let end = r
        .find(|c: char| c.is_whitespace() || c == ';' || c == ':')
        .unwrap_or(r.len());
    let word = &r[..end];
    OTHER_DIAGRAMS.contains(&word).then_some(word)
}

fn offset(n: usize) -> TextSize {
    TextSize::try_from(n).expect("source larger than 4 GiB")
}

impl<'a> Parser<'a> {
    // ---- primitives ----

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn at_eof(&self) -> bool {
        self.pos == self.src.len()
    }

    fn token(&mut self, kind: SyntaxKind, len: usize) {
        debug_assert!(len > 0, "empty {kind:?} token");
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

    /// Eats inline whitespace.
    fn ws(&mut self) {
        let n = scan::ws_len(self.rest());
        if n > 0 {
            self.token(WHITESPACE, n);
        }
    }

    /// Eats inline whitespace only if `pred` holds for the text after it. Returns whether
    /// `pred` held.
    fn ws_then(&mut self, pred: impl FnOnce(&str) -> bool) -> bool {
        let n = scan::ws_len(self.rest());
        if !pred(&self.rest()[n..]) {
            return false;
        }
        if n > 0 {
            self.token(WHITESPACE, n);
        }
        true
    }

    /// Whether only inline whitespace precedes the cursor on its line.
    fn at_line_start(&self) -> bool {
        let before = &self.src[..self.pos];
        let line = before.rfind('\n').map_or(before, |i| &before[i + 1..]);
        line.chars().all(char::is_whitespace)
    }

    /// Wraps the rest of the statement in an `ERROR` node and reports `message`.
    fn error_rest(&mut self, message: &str) {
        let start = self.pos;
        let len = scan::stmt_len(self.rest());
        self.start(ERROR);
        if len > 0 {
            self.token(ERROR_TOKEN, len);
        }
        self.finish();
        self.error(start, start + len, message);
    }

    /// Reports `message` for whatever follows: a zero-width error at a statement end, or the
    /// remaining statement text wrapped in an `ERROR` node.
    fn fail(&mut self, message: &str) {
        if self.ws_then(|r| !scan::is_stmt_end(r)) {
            self.error_rest(message);
        } else {
            self.error(self.pos, self.pos, message);
        }
    }

    /// Requires the statement to end here (after optional whitespace, which is left in place).
    fn end_of_statement(&mut self) {
        let n = scan::ws_len(self.rest());
        if !scan::is_stmt_end(&self.rest()[n..]) {
            self.fail("expected newline or `;`");
        }
    }

    // ---- document structure ----

    fn root(&mut self) {
        self.start(ROOT);
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if ["flowchart-elk", "flowchart", "graph"]
            .iter()
            .any(|kw| scan::keyword(r, kw))
        {
            self.header();
        } else if let Some(kind) = other_diagram(r) {
            // FLOWCH-16: the whole document is another diagram type; draw nothing of it.
            let start = self.pos;
            self.start(ERROR);
            self.token(ERROR_TOKEN, r.len());
            self.finish();
            self.error(
                start,
                start + kind.len(),
                format!("`{kind}` diagrams are not supported; sirene draws flowcharts"),
            );
            self.finish();
            return;
        } else {
            let end = self.pos + scan::stmt_len(r);
            self.error(self.pos, end, "expected `flowchart` or `graph` header");
        }
        self.statements(0);
        self.finish();
    }

    /// FLOWCH-3: frontmatter block at byte 0.
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

    /// Whitespace, newlines, `;`, comments, and directives between statements (FLOWCH-3).
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
            } else if r.starts_with("%%{") {
                self.directive();
            } else if r.starts_with("%%") && self.at_line_start() {
                self.token(COMMENT, scan::line_len(r));
            } else {
                return;
            }
        }
    }

    fn directive(&mut self) {
        let r = self.rest();
        match r[3..].find("}%%") {
            Some(i) => self.token(DIRECTIVE, i + 6),
            None => {
                let start = self.pos;
                self.token(DIRECTIVE, scan::line_len(r));
                self.error(start, start + 3, "unclosed directive: expected `}%%`");
            }
        }
    }

    /// FLOWCH-4.
    fn header(&mut self) {
        self.start(HEADER);
        self.token(GRAPH_KW, scan::id_len(self.rest()));
        if self.ws_then(|r| !scan::is_stmt_end(r)) {
            let n = scan::header_dir_len(self.rest());
            if n > 0 {
                self.token(DIR, n);
            } else {
                self.error_rest("expected a direction: TB, TD, BT, RL, or LR");
            }
        }
        self.finish();
        self.end_of_statement();
    }

    /// Parses statements until end of input or, when nested, an `end` keyword. Returns whether
    /// it stopped at `end`.
    fn statements(&mut self, depth: usize) -> bool {
        loop {
            self.trivia();
            if self.at_eof() {
                return false;
            }
            if depth > 0 && scan::keyword(self.rest(), "end") {
                return true;
            }
            self.statement(depth);
        }
    }

    /// FLOWCH-5.
    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        let after_kw = |kw: &str| {
            let tail = &r[kw.len()..];
            tail[scan::ws_len(tail)..].chars().next()
        };
        if scan::keyword(r, "subgraph") {
            self.subgraph(depth);
        } else if scan::keyword(r, "end") {
            let start = self.pos;
            self.start(ERROR);
            self.token(END_KW, 3);
            self.finish();
            self.error(start, start + 3, "`end` without a matching `subgraph`");
        } else if scan::keyword(r, "direction") {
            self.direction_stmt();
        } else if scan::keyword(r, "classDef") {
            self.class_def_stmt();
        } else if scan::keyword(r, "class") {
            self.class_stmt();
        } else if scan::keyword(r, "style") {
            self.style_stmt();
        } else if scan::keyword(r, "linkStyle") {
            self.link_style_stmt();
        } else if scan::keyword(r, "click") {
            self.click_stmt();
        } else if scan::keyword(r, "accTitle") && after_kw("accTitle") == Some(':') {
            self.acc_stmt(ACC_TITLE_STMT, ACC_TITLE_KW);
        } else if scan::keyword(r, "accDescr") && matches!(after_kw("accDescr"), Some(':' | '{')) {
            self.acc_stmt(ACC_DESCR_STMT, ACC_DESCR_KW);
        } else if scan::id_len(r) > 0 {
            self.vertex_stmt();
        } else {
            self.error_rest("expected a statement");
            return;
        }
        self.end_of_statement();
    }

    // ---- vertex statements ----

    /// FLOWCH-6.
    fn vertex_stmt(&mut self) {
        self.start(VERTEX_STMT);
        self.group();
        while self.ws_then(scan::edge_start) {
            self.edge();
            if self.ws_then(|r| scan::id_len(r) > 0) {
                self.group();
            } else {
                self.fail("expected a node after the edge");
                break;
            }
        }
        self.finish();
    }

    fn group(&mut self) {
        self.start(NODE_GROUP);
        self.node();
        while self.ws_then(|r| r.starts_with('&')) {
            self.token(AMP, 1);
            if self.ws_then(|r| scan::id_len(r) > 0) {
                self.node();
            } else {
                self.fail("expected a node after `&`");
                break;
            }
        }
        self.finish();
    }

    /// FLOWCH-7.
    fn node(&mut self) {
        self.start(NODE);
        let start = self.pos;
        let n = scan::id_len(self.rest());
        self.token(IDENT, n);
        if &self.src[start..self.pos] == "end" {
            self.error(start, self.pos, "`end` is reserved and cannot be a node id");
        }
        if let Some((len, opener)) = scan::shape_open(self.rest()) {
            self.shape(len, opener);
        }
        if self.rest().starts_with(":::") {
            self.start(CLASS_SHORTHAND);
            self.token(TRIPLE_COLON, 3);
            let n = scan::id_len(self.rest());
            if n > 0 {
                self.token(IDENT, n);
            } else {
                self.error(self.pos, self.pos, "expected a class name after `:::`");
            }
            self.finish();
        }
        if self.rest().starts_with("@{") {
            self.metadata();
        }
        self.finish();
    }

    fn shape(&mut self, open_len: usize, opener: Opener) {
        self.start(SHAPE);
        let open = self.pos;
        self.token(SHAPE_OPEN, open_len);
        self.label(
            |r| scan::shape_text_len(r, opener),
            |r| scan::shape_close(r, opener).is_some(),
        );
        match scan::shape_close(self.rest(), opener) {
            Some(n) => self.token(SHAPE_CLOSE, n),
            None => {
                let message = format!(
                    "unclosed `{}`: expected `{}`",
                    &self.src[open..open + open_len],
                    scan::closers(opener)[0]
                );
                self.error(open, open + open_len, message);
            }
        }
        self.finish();
    }

    /// FLOWCH-8. A quoted label is a string with optional surrounding whitespace; an unquoted
    /// label is a single `TEXT` token of `text_len` bytes, cut at the end of its line when
    /// `closes` does not hold where it stops.
    fn label(&mut self, text_len: impl Fn(&str) -> usize, closes: impl Fn(&str) -> bool) {
        self.start(LABEL);
        let r = self.rest();
        if r[scan::ws_len(r)..].starts_with('"') {
            self.ws();
            self.string();
            self.ws();
        } else {
            let mut len = text_len(r);
            if !closes(&r[len..]) {
                len = len.min(scan::line_len(r));
            }
            if len > 0 {
                self.token(TEXT, len);
            }
        }
        self.finish();
    }

    /// A `"..."` or `` "`...`" `` string at the cursor; unclosed strings end at end of line.
    fn string(&mut self) {
        let r = self.rest();
        let kind = if r.starts_with("\"`") {
            MD_STRING
        } else {
            STRING
        };
        match scan::string_len(r) {
            Some(n) => self.token(kind, n),
            None => {
                let start = self.pos;
                self.token(kind, scan::line_len(r));
                self.error(start, start + 1, "unclosed string: expected `\"`");
            }
        }
    }

    /// FLOWCH-15.
    fn metadata(&mut self) {
        self.start(METADATA);
        let open = self.pos;
        self.token(META_OPEN, 2);
        match scan::meta_body_len(self.rest()) {
            Some(n) => {
                if n > 0 {
                    self.token(META_BODY, n);
                }
                self.token(META_CLOSE, 1);
            }
            None => {
                let n = scan::line_len(self.rest());
                if n > 0 {
                    self.token(META_BODY, n);
                }
                self.error(open, open + 2, "unclosed `@{`: expected `}`");
            }
        }
        self.finish();
    }

    /// FLOWCH-9.
    fn edge(&mut self) {
        self.start(EDGE);
        if let Some(n) = scan::edge_id_len(self.rest()) {
            self.start(EDGE_ID);
            self.token(IDENT, n);
            self.token(AT, 1);
            self.finish();
        }
        match scan::stroke(self.rest()) {
            Some(Stroke::Full(n)) => {
                self.token(STROKE, n);
                if self.ws_then(|r| r.starts_with('|')) {
                    self.start(EDGE_LABEL);
                    let open = self.pos;
                    self.token(PIPE, 1);
                    self.label(|r| scan::text_len(r, &[]), |r| r.starts_with('|'));
                    if self.rest().starts_with('|') {
                        self.token(PIPE, 1);
                    } else {
                        self.error(open, open + 1, "unclosed `|`: expected `|`");
                    }
                    self.finish();
                }
            }
            Some(Stroke::Open(n, kind)) => {
                let open = self.pos;
                self.token(STROKE_OPEN, n);
                self.start(EDGE_LABEL);
                self.label(
                    |r| scan::inline_edge_text_len(r, kind),
                    |r| scan::closing_stroke(r, kind).is_some(),
                );
                self.finish();
                match scan::closing_stroke(self.rest(), kind) {
                    Some(m) => self.token(STROKE, m),
                    None => self.error(
                        open,
                        open + n,
                        "unclosed edge label: expected a closing stroke such as `-->`",
                    ),
                }
            }
            None => unreachable!("edge() called without an edge at the cursor"),
        }
        self.finish();
    }

    // ---- other statements ----

    /// FLOWCH-10.
    fn subgraph(&mut self, depth: usize) {
        let kw_start = self.pos;
        self.start(SUBGRAPH);
        self.start(SUBGRAPH_HEADER);
        self.token(SUBGRAPH_KW, "subgraph".len());
        if self.ws_then(|r| !scan::is_stmt_end(r)) {
            let r = self.rest();
            let n = scan::id_len(r);
            let after = &r[n..];
            let after = &after[scan::ws_len(after)..];
            if r.starts_with('"') {
                self.start(LABEL);
                self.string();
                self.finish();
            } else if n > 0 && after.starts_with('[') {
                self.token(IDENT, n);
                self.ws();
                self.shape(1, Opener::Rect);
            } else if n > 0 && scan::is_stmt_end(after) {
                self.token(IDENT, n);
            } else {
                let len = scan::trim_end_len(r, scan::stmt_len(r));
                self.start(LABEL);
                self.token(TEXT, len);
                self.finish();
            }
        }
        self.finish();
        self.end_of_statement();
        if self.statements(depth + 1) {
            self.token(END_KW, "end".len());
        } else {
            self.error(
                kw_start,
                kw_start + "subgraph".len(),
                "unclosed `subgraph`: expected `end`",
            );
        }
        self.finish();
    }

    /// FLOWCH-11.
    fn direction_stmt(&mut self) {
        self.start(DIRECTION_STMT);
        self.token(DIRECTION_KW, "direction".len());
        if self.ws_then(|r| scan::dir_len(r) > 0) {
            self.token(DIR, 2);
        } else {
            self.fail("expected a direction: TB, TD, BT, RL, or LR");
        }
        self.finish();
    }

    /// Comma-separated ids (or numbers when `numbers` is set) as an `ID_LIST` node.
    fn id_list(&mut self, numbers: bool) {
        let (kind, len): (SyntaxKind, fn(&str) -> usize) = if numbers {
            (NUMBER, scan::number_len)
        } else {
            (IDENT, scan::id_len)
        };
        self.start(ID_LIST);
        self.token(kind, len(self.rest()));
        while self.rest().starts_with(',') && len(&self.rest()[1..]) > 0 {
            self.token(COMMA, 1);
            self.token(kind, len(self.rest()));
        }
        self.finish();
    }

    /// Raw style declarations to the end of the statement (FLOWCH-12).
    fn style_text(&mut self) {
        let r = self.rest();
        self.token(STYLE_TEXT, scan::trim_end_len(r, scan::stmt_len(r)));
    }

    /// FLOWCH-12: `classDef <names> <styles>`.
    fn class_def_stmt(&mut self) {
        self.start(CLASSDEF_STMT);
        self.token(CLASSDEF_KW, "classDef".len());
        if !self.ws_then(|r| scan::id_len(r) > 0) {
            self.fail("expected a class name");
        } else {
            self.id_list(false);
            if self.ws_then(|r| !scan::is_stmt_end(r)) {
                self.style_text();
            } else {
                self.fail("expected style declarations");
            }
        }
        self.finish();
    }

    /// FLOWCH-12: `class <ids> <className>`.
    fn class_stmt(&mut self) {
        self.start(CLASS_STMT);
        self.token(CLASS_KW, "class".len());
        if !self.ws_then(|r| scan::id_len(r) > 0) {
            self.fail("expected a node id");
        } else {
            self.id_list(false);
            if self.ws_then(|r| scan::id_len(r) > 0) {
                self.token(IDENT, scan::id_len(self.rest()));
            } else {
                self.fail("expected a class name");
            }
        }
        self.finish();
    }

    /// FLOWCH-12: `style <id> <styles>`.
    fn style_stmt(&mut self) {
        self.start(STYLE_STMT);
        self.token(STYLE_KW, "style".len());
        if !self.ws_then(|r| scan::id_len(r) > 0) {
            self.fail("expected a node id");
        } else {
            self.token(IDENT, scan::id_len(self.rest()));
            if self.ws_then(|r| !scan::is_stmt_end(r)) {
                self.style_text();
            } else {
                self.fail("expected style declarations");
            }
        }
        self.finish();
    }

    /// FLOWCH-12: `linkStyle <default | n,m,...> [interpolate <curve>] [<styles>]`.
    fn link_style_stmt(&mut self) {
        self.start(LINKSTYLE_STMT);
        self.token(LINKSTYLE_KW, "linkStyle".len());
        if self.ws_then(|r| scan::keyword(r, "default")) {
            self.token(DEFAULT_KW, "default".len());
        } else if self.ws_then(|r| scan::number_len(r) > 0) {
            self.id_list(true);
        } else {
            self.fail("expected `default` or link indexes");
            self.finish();
            return;
        }
        if self.ws_then(|r| scan::keyword(r, "interpolate")) {
            self.token(INTERPOLATE_KW, "interpolate".len());
            if self.ws_then(|r| scan::id_len(r) > 0) {
                self.token(IDENT, scan::id_len(self.rest()));
            } else {
                self.fail("expected a curve name");
                self.finish();
                return;
            }
        }
        if self.ws_then(|r| !scan::is_stmt_end(r)) {
            self.style_text();
        }
        self.finish();
    }

    /// FLOWCH-13.
    fn click_stmt(&mut self) {
        self.start(CLICK_STMT);
        self.token(CLICK_KW, "click".len());
        if !self.ws_then(|r| scan::id_len(r) > 0) {
            self.fail("expected a node id");
            self.finish();
            return;
        }
        self.token(IDENT, scan::id_len(self.rest()));
        if !self.ws_then(|r| !scan::is_stmt_end(r)) {
            self.fail("expected a callback, `call`, `href`, or a URL string");
            self.finish();
            return;
        }
        let r = self.rest();
        if scan::keyword(r, "href") {
            self.token(HREF_KW, "href".len());
            if self.ws_then(|r| r.starts_with('"')) {
                self.string();
                self.click_tail(true);
            } else {
                self.fail("expected a URL string");
            }
        } else if scan::keyword(r, "call") {
            self.token(CALL_KW, "call".len());
            if self.ws_then(|r| scan::id_len(r) > 0) {
                self.token(IDENT, scan::id_len(self.rest()));
                if self.rest().starts_with('(') {
                    self.callback_args();
                }
                self.click_tail(false);
            } else {
                self.fail("expected a callback name");
            }
        } else if r.starts_with('"') {
            self.string();
            self.click_tail(true);
        } else if scan::id_len(r) > 0 {
            self.token(IDENT, scan::id_len(r));
            self.click_tail(false);
        } else {
            self.error_rest("expected a callback, `call`, `href`, or a URL string");
        }
        self.finish();
    }

    fn callback_args(&mut self) {
        let open = self.pos;
        self.token(L_PAREN, 1);
        let r = self.rest();
        let line = scan::line_len(r);
        match r[..line].find(')') {
            Some(n) => {
                if n > 0 {
                    self.token(CALLBACK_ARGS, n);
                }
                self.token(R_PAREN, 1);
            }
            None => {
                if line > 0 {
                    self.token(CALLBACK_ARGS, line);
                }
                self.error(open, open + 1, "unclosed `(`: expected `)`");
            }
        }
    }

    /// Optional tooltip string and, for links, an optional target.
    fn click_tail(&mut self, link: bool) {
        if self.ws_then(|r| r.starts_with('"')) {
            self.string();
        }
        if link && self.ws_then(|r| scan::link_target_len(r) > 0) {
            self.token(LINK_TARGET, scan::link_target_len(self.rest()));
        }
    }

    /// FLOWCH-14.
    fn acc_stmt(&mut self, node: SyntaxKind, kw: SyntaxKind) {
        self.start(node);
        self.token(kw, "accTitle".len());
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            let r = self.rest();
            let n = scan::trim_end_len(r, scan::line_len(r));
            if n > 0 {
                self.token(ACC_TEXT, n);
            }
        } else {
            let open = self.pos;
            self.token(L_BRACE, 1);
            let r = self.rest();
            match r.find('}') {
                Some(n) => {
                    if n > 0 {
                        self.token(ACC_TEXT, n);
                    }
                    self.token(R_BRACE, 1);
                }
                None => {
                    let n = scan::line_len(r);
                    if n > 0 {
                        self.token(ACC_TEXT, n);
                    }
                    self.error(open, open + 1, "unclosed `{`: expected `}`");
                }
            }
        }
        self.finish();
    }
}
