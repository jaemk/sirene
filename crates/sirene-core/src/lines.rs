//! The shared machinery of the line-oriented parsers (state, class, ER): a rowan builder over
//! the source, where every byte becomes exactly one token, so a tree's text always equals its
//! input (LANGS-2).

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::syntax::SyntaxKind::{self, *};

/// The byte order mark a file may start with (LANGS-9). It reads as inline whitespace.
pub(crate) const BOM: char = '\u{feff}';

/// Whether `c` is whitespace or a byte order mark.
pub(crate) fn is_space(c: char) -> bool {
    c.is_whitespace() || c == BOM
}

/// Length of the byte order mark at the start of `s`, or 0.
pub(crate) fn bom_len(s: &str) -> usize {
    if s.starts_with(BOM) {
        BOM.len_utf8()
    } else {
        0
    }
}

/// Length of inline whitespace (not newlines) at the start of `s`.
pub(crate) fn ws_len(s: &str) -> usize {
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let newline = c == '\n' || (c == '\r' && chars.peek().is_some_and(|&(_, n)| n == '\n'));
        if newline || !is_space(c) {
            return i;
        }
    }
    s.len()
}

/// Length of the newline at the start of `s` (`\n` or `\r\n`), or 0.
pub(crate) fn newline_len(s: &str) -> usize {
    if s.starts_with("\r\n") {
        2
    } else if s.starts_with('\n') {
        1
    } else {
        0
    }
}

/// Length up to (not including) the next newline.
pub(crate) fn line_len(s: &str) -> usize {
    match s.find('\n') {
        Some(i) if i > 0 && s.as_bytes()[i - 1] == b'\r' => i - 1,
        Some(i) => i,
        None => s.len(),
    }
}

/// Whether `s` starts at a statement separator or end of input.
pub(crate) fn is_stmt_end(s: &str) -> bool {
    s.is_empty() || s.starts_with(';') || newline_len(s) > 0
}

/// Length up to (not including) the next statement separator.
pub(crate) fn stmt_len(s: &str) -> usize {
    let line = line_len(s);
    s[..line].find(';').unwrap_or(line)
}

/// `len` with trailing inline whitespace of `s[..len]` removed.
pub(crate) fn trim_end_len(s: &str, len: usize) -> usize {
    s[..len].trim_end().len()
}

/// Length of a metadata body after `@{`, up to (not including) the closing `}`, if closed.
/// Braces inside double-quoted strings are ignored.
pub(crate) fn meta_body_len(s: &str) -> Option<usize> {
    let mut in_string = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '}' if !in_string => return Some(i),
            _ => {}
        }
    }
    None
}

/// Length of a word made of Unicode alphanumerics, `-`, and `_`, in bytes.
pub(crate) fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

/// Whether `kw` is the complete leading word, case sensitively.
pub(crate) fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && word_len(s) == kw.len()
}

/// Length of a word made of Unicode alphanumerics and underscores, in bytes.
pub(crate) fn identifier_word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

/// Whether `kw` is the complete leading identifier word, case sensitively.
pub(crate) fn identifier_keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && identifier_word_len(s) == kw.len()
}

pub(crate) struct Base<'a> {
    pub src: &'a str,
    pub pos: usize,
    pub builder: GreenNodeBuilder<'static>,
    pub diagnostics: Vec<Diagnostic>,
    /// The next `}%%`, for directives.
    directive: Next,
}

/// Whether only spaces come between the start of `pos`'s line and `pos`. Reads back only over
/// the spaces, so it costs nothing on a long line.
pub(crate) fn only_space_before(src: &str, pos: usize) -> bool {
    src[..pos]
        .chars()
        .rev()
        .take_while(|&c| c != '\n')
        .all(is_space)
}

/// The next match of a needle at or after a position. A later search from a position no further
/// than that match reuses it, so unclosed openers on many lines do not each rescan the rest.
#[derive(Default)]
pub(crate) struct Next {
    from: usize,
    at: Option<Option<usize>>,
}

impl Next {
    /// The offset in `src` of the first `needle` at or after `pos`.
    pub fn find(&mut self, src: &str, pos: usize, needle: &str) -> Option<usize> {
        if let Some(at) = self.at
            && pos >= self.from
            && at.is_none_or(|a| pos <= a)
        {
            return at;
        }
        let at = src[pos..].find(needle).map(|i| pos + i);
        self.from = pos;
        self.at = Some(at);
        at
    }
}

/// The `}` closing a metadata body, as `meta_body_len` finds it, for searches from many
/// positions. Which quotes open strings depends on where a search starts, so the quotes are
/// counted from the start of the text, and a search reuses an earlier one that started with
/// the same count's parity.
#[derive(Default)]
pub(crate) struct MetaClose {
    /// How far quotes are counted, and whether an odd number come before there.
    counted: usize,
    odd: bool,
    /// By parity: where a search started, and what it found.
    found: [Option<(usize, Option<usize>)>; 2],
}

impl MetaClose {
    /// The offset in `src` of the `}` closing a body that starts at `pos`.
    pub fn find(&mut self, src: &str, pos: usize) -> Option<usize> {
        if pos < self.counted {
            (self.counted, self.odd) = (0, false);
        }
        let quotes = src.as_bytes()[self.counted..pos]
            .iter()
            .filter(|&&b| b == b'"')
            .count();
        self.odd ^= quotes % 2 == 1;
        self.counted = pos;
        let slot = &mut self.found[usize::from(self.odd)];
        if let Some((from, at)) = *slot
            && pos >= from
            && at.is_none_or(|a| pos <= a)
        {
            return at;
        }
        let at = meta_body_len(&src[pos..]).map(|i| pos + i);
        *slot = Some((pos, at));
        at
    }
}

/// How deep blocks nest before a parser stops reading into them (LANGS-8). Parsing, lowering,
/// layout, and drawing all recurse over the nesting, so the bound keeps each off the end of the
/// stack.
pub(crate) const MAX_DEPTH: usize = 64;

/// The error for a block nested deeper than `limit`; `what` is a plural noun.
pub(crate) fn too_deep(what: &str, limit: usize) -> String {
    format!("{what} nested more than {limit} deep")
}

/// `src` with each `\r` not followed by `\n` read as `\n`, as Mermaid reads it. Byte offsets are
/// unchanged, so ranges into the result are ranges into `src`.
pub(crate) fn lone_cr_as_lf(src: &str) -> std::borrow::Cow<'_, str> {
    if !src.contains('\r') {
        return src.into();
    }
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(if c == '\r' && chars.peek() != Some(&'\n') {
            '\n'
        } else {
            c
        });
    }
    out.into()
}

fn offset(n: usize) -> TextSize {
    TextSize::new(n.min(u32::MAX as usize) as u32)
}

/// The CST stores byte offsets in `u32`; reject larger sources before tokenization.
pub(crate) fn source_limit(src: &str, limit: usize) -> Option<Diagnostic> {
    (src.len() > limit).then(|| {
        Diagnostic::new(
            TextRange::empty(TextSize::new(0)),
            format!("the source is over the parser limit of {limit} bytes"),
        )
    })
}

pub(crate) fn oversized(src: &str) -> Option<(GreenNode, Vec<Diagnostic>)> {
    limited_tree(src, u32::MAX as usize)
}

pub(crate) fn limited_tree(src: &str, limit: usize) -> Option<(GreenNode, Vec<Diagnostic>)> {
    let diagnostic = source_limit(src, limit)?;
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(ROOT.into());
    builder.finish_node();
    Some((builder.finish(), vec![diagnostic]))
}

#[test]
fn parser_limit_rejects_before_building_a_cst() {
    assert!(limited_tree("é", 2).is_none());
    let (green, diagnostics) = limited_tree("éa", 2).unwrap();
    assert_eq!(green.text_len(), TextSize::new(0));
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("limit of 2 bytes"));
    assert_eq!(diagnostics[0].range, TextRange::empty(TextSize::new(0)));
    assert_eq!(offset(usize::MAX), TextSize::new(u32::MAX));
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
        at += ws_len(&s[at..]);
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
        at += stmt_len(word);
        let rest = &s[at..];
        at += newline_len(rest).max(usize::from(rest.starts_with(';')));
    }
    None
}

/// Whether a statement may end here: end of input, a newline, `;`, or a closing `}`.
pub(crate) fn is_end(s: &str) -> bool {
    is_stmt_end(s) || s.starts_with('}')
}

/// The length of a string in the langium diagrams' `STRING` terminal at the start of `s`:
/// `"..."` or `'...'` on one line, where a backslash escapes the next character.
pub(crate) fn quoted_len(s: &str) -> Option<usize> {
    let quote = s.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let mut chars = s.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if !matches!(chars.next(), Some((_, '\n' | '\r'))) => {}
            '\\' | '\n' | '\r' => return None,
            c if c == quote => return Some(i + 1),
            _ => {}
        }
    }
    None
}

/// A `quoted_len` string's text: without its quotes, and each escaped character as itself.
pub(crate) fn unquote_escaped(s: &str) -> String {
    let inner = s.get(1..s.len().saturating_sub(1)).unwrap_or_default();
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '\\' {
            chars.next().unwrap_or(c)
        } else {
            c
        });
    }
    out
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
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let n = if rest.len() >= 4 && rest[0] == b'<' && rest[1..3].eq_ignore_ascii_case(b"br") {
            if rest[3] == b'>' {
                4
            } else if rest[3..].starts_with(b"/>") {
                5
            } else if rest[3..].starts_with(b" />") {
                6
            } else {
                0
            }
        } else {
            0
        };
        if n > 0 {
            out.push(s[start..i].trim().to_owned());
            i += n;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(s[start..].trim().to_owned());
    out
}

#[cfg(test)]
#[test]
fn split_br_preserves_text_and_recognizes_supported_tags() {
    assert_eq!(
        split_br("  café <BR> <bR/>λ<Br /> <br  /> end<br>"),
        ["café", "", "λ", "<br  /> end", ""]
    );
    let many = "é<BR />".repeat(10_000);
    let parts = split_br(&many);
    assert_eq!(parts.len(), 10_001);
    assert!(parts[..10_000].iter().all(|s| s == "é"));
    assert_eq!(parts[10_000], "");
}

impl<'a> Base<'a> {
    pub fn new(src: &'a str) -> Self {
        let diagnostic = source_limit(src, u32::MAX as usize);
        let src = if diagnostic.is_some() { "" } else { src };
        Base {
            src,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            diagnostics: diagnostic.into_iter().collect(),
            directive: Next::default(),
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
        let n = ws_len(self.rest());
        self.token(WHITESPACE, n);
    }

    /// Wraps the rest of the line in an `ERROR` node and reports `message`; a zero-width error
    /// when the line has ended.
    pub fn fail(&mut self, message: &str) {
        self.ws();
        let start = self.pos;
        let len = line_len(self.rest());
        if len == 0 {
            self.error(start, start, message);
            return;
        }
        self.start(ERROR);
        self.token(ERROR_TOKEN, len);
        self.finish();
        self.error(start, start + len, message);
    }

    /// Requires the statement to end here, allowing a `%%` comment after whitespace.
    pub fn end(&mut self) {
        let n = ws_len(self.rest());
        let after = &self.rest()[n..];
        let spaced = n > 0 || self.src[..self.pos].ends_with([' ', '\t']);
        if spaced && after.starts_with("%%") && !after.starts_with("%%{") {
            self.token(WHITESPACE, n);
            self.token(COMMENT, line_len(self.rest()));
            return;
        }
        if !is_end(after) {
            self.fail("expected the end of the line");
        }
    }

    /// The rest of the line as one `kind` token, without trailing whitespace.
    pub fn rest_of_line(&mut self, kind: SyntaxKind) {
        let len = trim_end_len(self.rest(), line_len(self.rest()));
        self.token(kind, len);
    }

    /// A langium diagram's `title` text: the rest of the line up to a `%%` comment, without
    /// trailing whitespace, then the end of the statement.
    pub fn title_text(&mut self) {
        let r = self.rest();
        let line = &r[..line_len(r)];
        let end = line.find("%%").unwrap_or(line.len());
        let n = trim_end_len(line, end);
        self.token(TEXT, n);
        self.end();
    }

    /// A `title` whose text is the rest of its line, without trailing whitespace.
    pub fn title_line(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        self.rest_of_line(TEXT);
        self.finish();
    }

    /// A langium `title` statement, with the comment handling of `title_text`.
    pub fn langium_title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        self.title_text();
        self.finish();
    }

    /// The length of a `"..."` string at the cursor, closed on the same line.
    pub fn string_len(&self) -> Option<usize> {
        // Up to the closing quote or the line's end only, so items of a long line are not
        // each scanned to its end.
        let r = self.rest().strip_prefix('"')?;
        let end = r.find(['"', '\n'])?;
        (r.as_bytes()[end] == b'"').then_some(end + 2)
    }

    fn at_line_start(&self) -> bool {
        only_space_before(self.src, self.pos)
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
            let end = self.pos + stmt_len(r);
            self.error(self.pos, end, format!("expected `{first}` header"));
        }
    }

    /// A frontmatter block at the start of the source, if any, after a byte order mark
    /// (LANGS-9).
    pub fn frontmatter(&mut self) {
        self.token(WHITESPACE, bom_len(self.src));
        let (start, src) = (self.pos, self.rest());
        let first = line_len(src);
        if src[..first].trim_end() != "---" {
            return;
        }
        let mut line_start = first + newline_len(&src[first..]);
        while line_start < src.len() {
            let rest = &src[line_start..];
            let len = line_len(rest);
            if rest[..len].trim() == "---" {
                self.token(FRONTMATTER, line_start + len);
                return;
            }
            line_start += len + newline_len(&rest[len..]);
        }
        self.start(ERROR);
        self.token(ERROR_TOKEN, first);
        self.finish();
        self.error(
            start,
            start + first,
            "unclosed frontmatter: expected a closing `---` line",
        );
    }

    /// Whitespace, newlines, `;`, comments, and directives between statements.
    pub fn trivia(&mut self) {
        loop {
            let r = self.rest();
            let ws = ws_len(r);
            let nl = newline_len(r);
            if ws > 0 {
                self.token(WHITESPACE, ws);
            } else if nl > 0 {
                self.token(NEWLINE, nl);
            } else if r.starts_with(';') {
                self.token(SEMI, 1);
            } else if r.starts_with("%%{") {
                let from = self.pos + 3;
                match self.directive.find(self.src, from, "}%%") {
                    Some(at) => self.token(DIRECTIVE, at - from + 6),
                    None => {
                        let start = self.pos;
                        self.token(DIRECTIVE, line_len(r));
                        self.error(start, start + 3, "unclosed directive: expected `}%%`");
                    }
                }
            } else if r.starts_with("%%") && self.at_line_start() {
                self.token(COMMENT, line_len(r));
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
            let (ws, nl) = (ws_len(r), newline_len(r));
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
        let line = line_len(self.rest()).min(len);
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
                    let n = ws_len(p.rest());
                    let after = &p.rest()[n..];
                    if after.starts_with("%%") && !after.starts_with("%%{") {
                        p.end();
                    }
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
    fn shared_scanners_respect_boundaries() {
        assert_eq!(bom_len("\u{feff}é"), 3);
        assert_eq!(bom_len("é\u{feff}"), 0);
        assert_eq!(ws_len("\u{feff}\u{2003}\rX"), 7);
        assert_eq!(ws_len(" \r\nX"), 1);
        assert_eq!(newline_len("\rX"), 0);
        assert_eq!(newline_len("\r\nX"), 2);
        assert_eq!(line_len("é\r\nX"), 2);
        assert_eq!(line_len("é\rX"), 4);
        assert!(is_stmt_end(""));
        assert!(is_stmt_end(";x"));
        assert!(is_stmt_end("\r\nx"));
        assert!(!is_stmt_end("\rx"));
        assert_eq!(stmt_len("é;z\r\n"), 2);
        assert_eq!(stmt_len("é\r\n;"), 2);
        assert_eq!(stmt_len(""), 0);
        let text = "é\u{2003} \t;";
        assert_eq!(trim_end_len(text, text.len() - 1), "é".len());
        assert_eq!(trim_end_len("", 0), 0);
    }

    #[test]
    fn metadata_bodies() {
        assert_eq!(meta_body_len(" shape: rect }"), Some(13));
        assert_eq!(meta_body_len("label: \"a}b\" }x"), Some(13));
        assert_eq!(meta_body_len("label: x"), None);
        assert_eq!(meta_body_len("é \"x}y\" }"), Some(9));
        assert_eq!(meta_body_len("\"unclosed }"), None);
        // Unlike brace blocks, unquoted nested braces do not affect this scan.
        assert_eq!(meta_body_len("{a} }"), Some(2));
    }

    #[test]
    fn words_and_keywords() {
        assert_eq!(word_len("é-中_2: rest"), "é-中_2".len());
        assert_eq!(word_len("!title"), 0);
        assert!(keyword("title: x", "title"));
        assert!(keyword("title", "title"));
        for s in ["titles", "title_2", "title-é", "Title"] {
            assert!(!keyword(s, "title"), "{s:?}");
        }
        assert_eq!(identifier_word_len("é_中-2"), "é_中".len());
        assert!(identifier_keyword("title-name", "title"));
        assert!(!identifier_keyword("titleé", "title"));
        assert!(!identifier_keyword("title_2", "title"));
    }

    /// Searches from every position, forward and then backward, agree with a fresh scan.
    #[test]
    fn closers_match_a_fresh_scan() {
        for src in [
            "a } \"}\" } x",
            "\"a\" @{ \"}\n\" } \" }",
            "}}\"\"}\"}",
            "no closer \" here",
            "",
        ] {
            let mut meta = MetaClose::default();
            let mut next = Next::default();
            let fresh = |p: usize| meta_body_len(&src[p..]).map(|i| p + i);
            let plain = |p: usize| src[p..].find('}').map(|i| p + i);
            let order: Vec<usize> = (0..=src.len()).chain((0..=src.len()).rev()).collect();
            for p in order {
                assert_eq!(meta.find(src, p), fresh(p), "{src:?} at {p}");
                assert_eq!(next.find(src, p, "}"), plain(p), "{src:?} at {p}");
            }
        }
    }

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
    fn quoted() {
        assert_eq!(quoted_len("\"a b\" c"), Some(5));
        assert_eq!(quoted_len("'a \"b\"' c"), Some(7));
        assert_eq!(quoted_len(r#""a\"b" c"#), Some(6));
        assert_eq!(quoted_len("'é'"), Some(4));
        for bad in ["a", "\"a", "\"a\n\"", "\"a\\\n\"", "'a\\'", "\"a\\"] {
            assert_eq!(quoted_len(bad), None, "{bad:?}");
        }
        assert_eq!(unquote_escaped(r#""a\"b\\""#), r#"a"b\"#);
        assert_eq!(unquote_escaped("'é'"), "é");
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
    fn title_forms_keep_distinct_comment_rules() {
        let src = "title A; B %% comment";
        let (tree, d) = built(src, |p| p.title_line());
        assert_eq!(tree, "TITLE_STMT TITLE_KW WHITESPACE TEXT");
        assert!(d.is_empty());
        let (tree, d) = built(src, |p| p.langium_title());
        assert_eq!(
            tree,
            "TITLE_STMT TITLE_KW WHITESPACE TEXT WHITESPACE COMMENT"
        );
        assert!(d.is_empty());
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
