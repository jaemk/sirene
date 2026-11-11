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

/// How deep blocks nest before a parser stops reading into them (LANGS-8). Parsing, lowering,
/// layout, and drawing all recurse over the nesting, so the bound keeps each off the end of the
/// stack.
pub(crate) const MAX_DEPTH: usize = 64;

/// The error for a block nested deeper than `limit`; `what` is a plural noun.
pub(crate) fn too_deep(what: &str, limit: usize) -> String {
    format!("{what} nested more than {limit} deep")
}

fn offset(n: usize) -> TextSize {
    TextSize::try_from(n).expect("source larger than 4 GiB")
}

/// The length of a block's content up to its closing `}`, counting the braces nested in it and
/// passing over `"..."` strings closed on their line; `None` when the block is not closed.
pub(crate) fn brace_block_len(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'\n' => quoted = false,
            b'{' if !quoted => depth += 1,
            b'}' if !quoted => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// The length of a keyword block's content up to its closing `end`, where a statement starting
/// with one of `openers` (a whole word, or a prefix when it ends in `:`) opens a nested block;
/// `None` when the block is not closed.
pub(crate) fn end_block_len(s: &str, openers: &[&str]) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = 0;
    while at < s.len() {
        at += scan::ws_len(&s[at..]);
        let word = &s[at..];
        if scan::keyword(word, "end") {
            if depth == 0 {
                return Some(at);
            }
            depth -= 1;
        } else if openers.iter().any(|o| match o.strip_suffix(':') {
            Some(_) => word.starts_with(o),
            None => scan::keyword(word, o),
        }) {
            depth += 1;
        }
        at += scan::stmt_len(word);
        let rest = &s[at..];
        at += scan::newline_len(rest).max(usize::from(rest.starts_with(';')));
    }
    None
}

/// Whether a statement may end here: end of input, a newline, `;`, or a closing `}`.
pub(crate) fn is_end(s: &str) -> bool {
    scan::is_stmt_end(s) || s.starts_with('}')
}

/// The length of a number such as `3`, `-2.5`, or `.5` at the start of `s`, or 0.
pub(crate) fn number_len(s: &str) -> usize {
    let sign = usize::from(s.starts_with('-'));
    let body = &s[sign..];
    let int = body
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(body.len());
    let mut n = int;
    if body[n..].starts_with('.') {
        let frac = body[n + 1..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(body.len() - n - 1);
        if int + frac == 0 {
            return 0;
        }
        n += 1 + frac;
    }
    if n == 0 { 0 } else { sign + n }
}

#[cfg(test)]
#[test]
fn numbers() {
    assert_eq!(number_len("12,"), 2);
    assert_eq!(number_len("-2.5}"), 4);
    assert_eq!(number_len(".5"), 2);
    assert_eq!(number_len("3."), 2);
    assert_eq!(number_len("-"), 0);
    assert_eq!(number_len("."), 0);
    assert_eq!(number_len("a"), 0);
}

/// `s` split into lines at `<br>` (any case, with an optional `/`), each trimmed.
pub(crate) fn split_br(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        let lower = rest.to_ascii_lowercase();
        let found = ["<br>", "<br/>", "<br />"]
            .iter()
            .filter_map(|b| lower.find(b).map(|i| (i, b.len())))
            .min();
        match found {
            Some((i, n)) => {
                out.push(rest[..i].trim().to_owned());
                rest = &rest[i + n..];
            }
            None => {
                out.push(rest.trim().to_owned());
                return out;
            }
        }
    }
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

    /// Skips a block nested too deep: its content, up to the offset `end` when it is closed and
    /// the rest of the source when not, becomes one `ERROR` node after its leading whitespace,
    /// reported once as `message` on its first line.
    pub fn skip(&mut self, end: Option<usize>, message: String) {
        let end = end.map_or(self.src.len(), |n| n.max(self.pos));
        loop {
            let r = &self.src[self.pos..end];
            let (ws, nl) = (scan::ws_len(r), scan::newline_len(r));
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else {
                break;
            }
        }
        let start = self.pos;
        let len = end - start;
        let line = scan::line_len(self.rest()).min(len);
        self.error(start, start + line, message);
        if len > 0 {
            self.start(ERROR);
            self.token(ERROR_TOKEN, len);
            self.finish();
        }
    }

    /// Reports a block left open at the end of the source, once for the innermost one and not
    /// again for each block around it.
    pub fn unclosed(&mut self, message: String) {
        let range = TextRange::empty(offset(self.pos));
        let told = self.diagnostics.last();
        if told.is_none_or(|d| d.range != range || d.message != message) {
            self.diagnostics.push(Diagnostic::new(range, message));
        }
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
        if depth > MAX_DEPTH {
            let end = brace_block_len(p.rest()).map(|n| p.pos + n);
            let plural = match what.strip_suffix('y') {
                Some(stem) => format!("{stem}ies"),
                None => format!("{what}s"),
            };
            p.skip(end, too_deep(&plural, MAX_DEPTH));
            if end.is_some() {
                p.token(R_BRACE, 1);
            } else {
                p.unclosed(format!("unclosed {what}: expected `}}`"));
            }
            return;
        }
        loop {
            p.trivia();
            if p.at_eof() {
                if depth > 0 {
                    p.unclosed(format!("unclosed {what}: expected `}}`"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxNode;

    #[test]
    fn brace_blocks() {
        assert_eq!(brace_block_len("a } b"), Some(2));
        assert_eq!(brace_block_len("a { b } c } d }"), Some(10));
        // A brace in a string does not count; a string ends with its line.
        assert_eq!(brace_block_len("x \"}\" { \"{\" } }"), Some(14));
        assert_eq!(brace_block_len("x \"a\n} y"), Some(5));
        assert_eq!(brace_block_len("a { b }"), None);
        assert_eq!(brace_block_len(""), None);
    }

    #[test]
    fn end_blocks() {
        let openers = ["block:", "block"];
        assert_eq!(end_block_len("\n  a\n  end\n", &openers), Some(7));
        assert_eq!(end_block_len(" a; end", &openers), Some(4));
        // Nested blocks close first; `ending` and `blocker` are neither keyword.
        let s = "\nblock:x[\"X\"]\n  ending\n  blocker\nend\nblock\nend\nend";
        assert_eq!(end_block_len(s, &openers), Some(s.len() - 3));
        assert_eq!(end_block_len("\nblock\nend\n", &openers), None);
        assert_eq!(end_block_len("", &openers), None);
        assert_eq!(end_block_len("a\r\nend\r\n", &["subgraph"]), Some(3));
    }

    /// The tree and messages after `f` runs on `src`; the tree always holds `src` exactly.
    fn built(
        src: &str,
        f: impl FnOnce(&mut Base),
    ) -> (String, Vec<(std::ops::Range<usize>, String)>) {
        let mut p = Base::new(src);
        p.start(ROOT);
        f(&mut p);
        p.finish();
        let (green, diagnostics) = p.finish_tree();
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.to_string(), src);
        let kinds: Vec<String> = root
            .descendants_with_tokens()
            .skip(1)
            .map(|e| format!("{:?}", e.kind()))
            .collect();
        let messages = diagnostics
            .into_iter()
            .map(|d| (d.range.start().into()..d.range.end().into(), d.message))
            .collect();
        (kinds.join(" "), messages)
    }

    #[test]
    fn skips() {
        // Closed: the content after its leading whitespace is one node, reported on its line.
        let (tree, d) = built("\n  a {\n  }\n}", |p| {
            p.skip(Some(11), "too deep".into());
            p.token(R_BRACE, 1);
        });
        assert_eq!(tree, "NEWLINE WHITESPACE ERROR ERROR_TOKEN R_BRACE");
        assert_eq!(d, [(3..6, "too deep".to_owned())]);
        // Not closed: the rest of the source.
        let (tree, d) = built(" a\nb", |p| p.skip(None, "too deep".into()));
        assert_eq!(tree, "WHITESPACE ERROR ERROR_TOKEN");
        assert_eq!(d, [(1..2, "too deep".to_owned())]);
        // Nothing but whitespace, and an end before the cursor.
        let (tree, d) = built(" \n}", |p| {
            p.skip(Some(2), "too deep".into());
            p.token(R_BRACE, 1);
        });
        assert_eq!(tree, "WHITESPACE NEWLINE R_BRACE");
        assert_eq!(d, [(2..2, "too deep".to_owned())]);
        let (tree, d) = built("ab", |p| {
            p.token(TEXT, 1);
            p.skip(Some(0), "too deep".into());
            p.token(TEXT, 1);
        });
        assert_eq!(tree, "TEXT TEXT");
        assert_eq!(d, [(1..1, "too deep".to_owned())]);
    }

    #[test]
    fn unclosed_is_told_once() {
        let (_, d) = built("a", |p| {
            p.token(TEXT, 1);
            p.unclosed("unclosed x".into());
            p.unclosed("unclosed x".into());
            p.unclosed("unclosed y".into());
        });
        assert_eq!(
            d,
            [
                (1..1, "unclosed x".to_owned()),
                (1..1, "unclosed y".to_owned())
            ]
        );
    }
}
