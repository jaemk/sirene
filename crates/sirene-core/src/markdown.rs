//! Diagram code blocks in Markdown (MARKDO-1).

use crate::diagram::Lang;

/// A fenced code block whose info string names a diagram language (`mermaid`, `dot`,
/// `graphviz`, `plantuml`, or `puml`; see [`info_lang`]), at the top level or inside
/// blockquotes and list items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The block's text, with the blockquote markers and the fence's indentation taken off
    /// each line.
    pub code: String,
    /// The language the info string names (DOT-5).
    pub lang: Lang,
    /// The line of the block's first code line, from 1.
    pub line: usize,
    /// The blockquote markers before the opening fence (`> `), empty outside a blockquote.
    pub quote: String,
    /// The fence's indentation after `quote`, including any list item's content column: the
    /// most taken off each code line after its blockquote markers.
    pub indent: usize,
    /// The columns taken off each code line, in order: its blockquote markers and `indent`, or
    /// less for a line indented less than the fence.
    pub strips: Vec<usize>,
    /// The byte offset just after the closing fence's line (where the block's container ends,
    /// or the end of the text, when the block is not closed).
    pub end: usize,
    /// Whether a closing fence ends the block; otherwise it runs to the end of its blockquote
    /// or list item, or of the text.
    pub closed: bool,
    /// The bytes between the fences: from the start of the first code line to the start of the
    /// closing fence's line (or where the block ends).
    pub body: std::ops::Range<usize>,
    /// The byte offsets of the opening fence's run and of the closing one's, its character,
    /// and the opening run's length.
    fences: (usize, Option<usize>, char, usize),
}

impl Block {
    /// Where a position in [`Block::code`] (line and column from 1) is in the Markdown.
    pub fn position(&self, line: usize, col: usize) -> (usize, usize) {
        let strip = self.strips.get(line - 1).copied().unwrap_or(self.indent);
        (self.line + line - 1, col + strip)
    }

    /// `src`, the Markdown this block was found in, with the block's code replaced by `code`:
    /// each line after the fence's blockquote markers, indented like the fence, and ended like
    /// the Markdown's lines (`\r\n` or `\n`). Everything outside the block is kept, and so are
    /// the fences, except that both are made longer when a line of `code` would otherwise
    /// close the block.
    pub fn replace_code(&self, src: &str, code: &str) -> String {
        let newline = newline(src);
        let pad = format!("{}{}", self.quote, " ".repeat(self.indent));
        let mut body = String::with_capacity(code.len() + 8);
        for line in code.lines() {
            if line.is_empty() {
                body.push_str(self.quote.trim_end());
            } else {
                body.push_str(&pad);
                body.push_str(line);
            }
            body.push_str(newline);
        }
        // An unclosed block at the end of a text without a final newline keeps it that way.
        if !self.closed
            && self.body.end == src.len()
            && !src.ends_with('\n')
            && !code.ends_with('\n')
            && !body.is_empty()
        {
            body.truncate(body.len() - newline.len());
        }
        // The fences' length around `body`: longer than any run in it that would close them.
        let (open, close, c, len) = self.fences;
        let need = fence_len(code, c, len);
        let mut out = String::with_capacity(src.len() + body.len());
        out.push_str(&src[..open]);
        out.extend(std::iter::repeat_n(c, need));
        out.push_str(&src[open + len..self.body.start]);
        // The opening fence ends the text: the code starts a line of its own.
        if self.body.start == src.len() && !src.ends_with('\n') && !body.is_empty() {
            out.push_str(newline);
        }
        out.push_str(&body);
        match close {
            Some(at) => {
                let rest = &src[at..];
                let run = rest.len() - rest.trim_start_matches(c).len();
                out.push_str(&src[self.body.end..at]);
                out.extend(std::iter::repeat_n(c, need.max(run)));
                out.push_str(&rest[run..]);
            }
            None => out.push_str(&src[self.body.end..]),
        }
        out
    }
}

/// The line ending of `src`'s lines: `\r\n` when it has one, else `\n`.
fn newline(src: &str) -> &'static str {
    if src.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The length a fence of `c`, at least `len` long, needs so that no line of `code` closes it,
/// however far the fence is indented.
fn fence_len(code: &str, c: char, len: usize) -> usize {
    code.lines()
        .map(|l| l.trim_start_matches(' '))
        .filter(|l| closes(l, c, len))
        .map(|l| l.len() - l.trim_start_matches(c).len() + 1)
        .fold(len, usize::max)
}

/// `src` with a block of `code` added at the end, fenced with backticks and named `info`
/// (`mermaid`, `dot`, `plantuml`), after a blank line. A fenced block left open at the end of
/// `src` is closed first, so the new block is not read as its code. Lines end like `src`'s.
pub fn append_block(src: &str, info: &str, code: &str) -> String {
    let newline = newline(src);
    let mut out = src.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(newline);
    }
    if let Some(fence) = open_fence(src) {
        out.push_str(&fence);
        out.push_str(newline);
    }
    if !out.is_empty() {
        out.push_str(newline);
    }
    let fence = "`".repeat(fence_len(code, '`', 3));
    out.push_str(&fence);
    out.push_str(info);
    out.push_str(newline);
    for line in code.lines() {
        out.push_str(line);
        out.push_str(newline);
    }
    out.push_str(&fence);
    out.push_str(newline);
    out
}

/// The line that closes a fenced block (of any language) left open at the end of `src`, in
/// its blockquote or list item.
fn open_fence(src: &str) -> Option<String> {
    fenced(src)
        .pop()
        .filter(|b| b.close.is_none() && b.stop == usize::MAX)
        .map(|b| {
            let fence: String = std::iter::repeat_n(b.c, b.len).collect();
            format!("{}{}{fence}", b.quote, " ".repeat(b.indent))
        })
}

/// (byte offset, text without its line ending) for each line of `src`, with a byte order mark
/// before the first line taken off it and its offset (LANGS-9).
fn lines(src: &str) -> Vec<(usize, &str)> {
    let mut lines = Vec::new();
    let mut at = 0;
    for raw in src.split_inclusive('\n') {
        let text = if at == 0 {
            raw.trim_start_matches(crate::flowchart::scan::BOM)
        } else {
            raw
        };
        let bom = raw.len() - text.len();
        lines.push((at + bom, text.trim_end_matches(['\n', '\r'])));
        at += raw.len();
    }
    lines
}

/// The number of spaces `s` starts with.
fn spaces(s: &str) -> usize {
    s.len() - s.trim_start_matches(' ').len()
}

/// The blockquote markers (up to 3 spaces, `>`, and an optional space) at the start of `line`,
/// at most `max` of them: how many, and their length.
fn quotes(line: &str, max: usize) -> (usize, usize) {
    let (mut n, mut at) = (0, 0);
    while n < max {
        let sp = spaces(&line[at..]);
        if sp > 3 || !line[at + sp..].starts_with('>') {
            break;
        }
        at += sp + 1;
        if line[at..].starts_with(' ') {
            at += 1;
        }
        n += 1;
    }
    (n, at)
}

/// The content column of a list item whose marker (`-`, `+`, `*`, or 1 to 9 digits and `.`
/// or `)`) starts `s`, relative to `s`, when the item has content on this line.
fn list_item(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let marker = match b.first()? {
        b'-' | b'+' | b'*' => 1,
        b'0'..=b'9' => {
            let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 9 || !matches!(b.get(digits), Some(b'.' | b')')) {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    let sp = spaces(&s[marker..]);
    if sp == 0 || s[marker + sp..].is_empty() {
        return None;
    }
    // Five or more spaces start indented code one column after the marker.
    Some(marker + if sp > 4 { 1 } else { sp })
}

/// An opening fence: its indent, character, and length, and the diagram language it names.
type Opening = (usize, char, usize, Option<Lang>);

/// An opening fence on `line`: its indent, character, and length, and the diagram language
/// its info string names, if any (LANGS-1).
fn opening(line: &str) -> Option<Opening> {
    let indent = spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let c = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = rest.len() - rest.trim_start_matches(c).len();
    if len < 3 {
        return None;
    }
    let info = rest[len..].trim();
    // A backtick fence's info string has no backticks.
    if c == '`' && info.contains('`') {
        return None;
    }
    Some((indent, c, len, info_lang(info)))
}

/// The diagram language an info string names: its first word (GitHub, `mermaid title`), up to
/// any `{` (`mermaid{...}`), or the first class or bare word in braces (Pandoc's `{.mermaid}`,
/// MyST's `{mermaid}`); in any case.
pub fn info_lang(info: &str) -> Option<Lang> {
    let word = match info.strip_prefix('{') {
        Some(attrs) => {
            let attrs = attrs.split('}').next().unwrap_or("");
            let mut words = attrs.split_whitespace();
            let class = words.clone().find_map(|w| w.strip_prefix('.'));
            class.or_else(|| words.find(|w| !w.starts_with('#') && !w.contains('=')))?
        }
        None => {
            let word = info.split_whitespace().next().unwrap_or("");
            word.split('{').next().unwrap_or("")
        }
    };
    Lang::of_info(&word.to_ascii_lowercase())
}

/// A fenced block of any language, found by [`fenced`].
struct Fenced {
    lang: Option<Lang>,
    /// The index of the opening fence's line.
    open: usize,
    /// The index of the closing fence's line, if any.
    close: Option<usize>,
    /// The index of the line that ended the block's blockquote or list item, or `usize::MAX`
    /// when the block is closed or runs to the end of the text.
    stop: usize,
    c: char,
    len: usize,
    /// The byte offsets of the opening fence's run and the closing one's.
    runs: (usize, Option<usize>),
    quote: String,
    indent: usize,
    code: String,
    strips: Vec<usize>,
}

/// The fenced blocks in `src`, in order, at the top level or inside blockquotes and list items
/// (MARKDO-1). A block in a container ends with it when not closed first.
fn fenced(src: &str) -> Vec<Fenced> {
    let lines = lines(src);
    let mut blocks = Vec::new();
    // The content columns of the list items open at the previous line, after its blockquote
    // markers, and how many of those markers it had.
    let mut items: Vec<usize> = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    while i < lines.len() {
        let (at, line) = lines[i];
        let (q, qlen) = quotes(line, usize::MAX);
        if q != depth {
            items.clear();
            depth = q;
        }
        let rest = &line[qlen..];
        let Some((base, (rel, c, len, lang))) = item_opening(rest, &mut items) else {
            i += 1;
            continue;
        };
        let indent = base + rel;
        let mut code = String::new();
        let mut strips = Vec::new();
        let mut j = i + 1;
        let mut close = None;
        let mut stop = usize::MAX;
        while j < lines.len() {
            let (line_at, line) = lines[j];
            let (n, qlen) = quotes(line, q);
            let rest = &line[qlen..];
            let lead = spaces(rest);
            if n < q || (lead < base && !rest.trim().is_empty()) {
                stop = j;
                break;
            }
            if lead >= base && closes(&rest[base..], c, len) {
                close = Some(line_at + qlen + lead);
                break;
            }
            let strip = lead.min(indent);
            code.push_str(&rest[strip..]);
            code.push('\n');
            strips.push(qlen + strip);
            j += 1;
        }
        blocks.push(Fenced {
            lang,
            open: i,
            close: close.map(|_| j),
            stop,
            c,
            len,
            runs: (at + qlen + indent, close),
            quote: line[..qlen].to_owned(),
            indent,
            code,
            strips,
        });
        i = if stop == usize::MAX { j + 1 } else { j };
    }
    blocks
}

/// An opening fence on `rest`, a line after its blockquote markers, in the innermost of
/// `items` (the content columns of open list items) that holds it, or in a list item it
/// starts: the item's content column, and the fence as [`opening`] gives it. Updates `items`.
fn item_opening(rest: &str, items: &mut Vec<usize>) -> Option<(usize, Opening)> {
    if rest.trim().is_empty() {
        return None;
    }
    let mut pos = spaces(rest);
    while items.last().is_some_and(|col| pos < *col) {
        items.pop();
    }
    let mut base = items.last().copied().unwrap_or(0);
    while pos - base <= 3
        && let Some(col) = list_item(&rest[pos..])
    {
        base = pos + col;
        items.push(base);
        pos = base + spaces(&rest[base..]);
    }
    Some((base, opening(&rest[base..])?))
}

/// Whether `line` closes a fence of `c` at least `len` long.
fn closes(line: &str, c: char, len: usize) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let run = rest.len() - rest.trim_start_matches(c).len();
    run >= len && rest[run..].trim().is_empty()
}

/// The diagram blocks in `src` (see [`Block`]), in order, despite the name. Other fenced blocks
/// are skipped whole, so a diagram fence inside one is not a block.
pub fn mermaid_blocks(src: &str) -> Vec<Block> {
    let lines = lines(src);
    let offset = |i: usize| lines.get(i).map_or(src.len(), |(at, _)| *at);
    fenced(src)
        .into_iter()
        .filter_map(|b| {
            let lang = b.lang?;
            let start = offset(b.open + 1);
            let (stop, end) = match b.close {
                Some(j) => (offset(j), offset(j + 1)),
                None => (offset(b.stop), offset(b.stop)),
            };
            Some(Block {
                code: b.code,
                lang,
                line: b.open + 2,
                quote: b.quote,
                indent: b.indent,
                strips: b.strips,
                end,
                closed: b.close.is_some(),
                body: start..stop.max(start),
                fences: (b.runs.0, b.runs.1, b.c, b.len),
            })
        })
        .collect()
}

/// Whether `path` names a Markdown file (`.md` or `.markdown`).
pub fn is_markdown(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("md") || x.eq_ignore_ascii_case("markdown"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_mermaid_blocks() {
        let src = "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\ntext\n\n```rust\n```mermaid\n```\n\n  ~~~~ mermaid title\n  graph TD\n    X\n  ~~~~\nafter\n```mermaid\nflowchart TD\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].code, "flowchart LR\n  A --> B\n");
        assert_eq!(blocks[0].line, 4);
        assert_eq!(blocks[0].indent, 0);
        assert_eq!(&src[blocks[0].end..blocks[0].end + 1], "\n");
        // Indented fences lose their indent; positions add it back.
        assert_eq!(blocks[1].code, "graph TD\n  X\n");
        assert_eq!(blocks[1].line, 15);
        assert_eq!(blocks[1].position(2, 3), (16, 5));
        // An unclosed block runs to the end.
        assert_eq!(blocks[2].code, "flowchart TD\n");
        assert_eq!(blocks[2].end, src.len());
        assert!(blocks[0].closed && blocks[1].closed && !blocks[2].closed);
    }

    #[test]
    fn replacing_code() {
        let src = "# Doc\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\ntext\n";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(&src[block.body.clone()], "flowchart LR\n  A --> B\n");
        let out = block.replace_code(src, "flowchart TD\n  X --> Y\n  Y --> Z\n");
        assert_eq!(
            out,
            "# Doc\n\n```mermaid\nflowchart TD\n  X --> Y\n  Y --> Z\n```\n\ntext\n"
        );
        // Round trip, and the block is found again with the new code.
        assert_eq!(block.replace_code(src, &block.code), src);
        assert_eq!(
            mermaid_blocks(&out)[0].code,
            "flowchart TD\n  X --> Y\n  Y --> Z\n"
        );

        // An indented fence indents the code (not blank lines); CRLF stays CRLF.
        let src = "- item\n\n  ```mermaid\r\n  graph TD\r\n    A\r\n  ```\r\n";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(block.code, "graph TD\n  A\n");
        assert_eq!(
            block.replace_code(src, "graph TD\n\n  B\n"),
            "- item\n\n  ```mermaid\r\n  graph TD\r\n\r\n    B\r\n  ```\r\n"
        );
        // An empty block, an unclosed one, and one whose fence ends the text.
        let src = "```mermaid\n```\n";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(block.body, 11..11);
        assert_eq!(
            block.replace_code(src, "graph TD"),
            "```mermaid\ngraph TD\n```\n"
        );
        let src = "```mermaid\ngraph TD";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(block.replace_code(src, "graph LR"), "```mermaid\ngraph LR");
        assert_eq!(block.replace_code(src, ""), "```mermaid\n");
        let src = "```mermaid";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(
            block.replace_code(src, "graph LR\n"),
            "```mermaid\ngraph LR\n"
        );
    }

    #[test]
    fn positions_in_lines_indented_less_than_the_fence() {
        let src = "   ```mermaid\n   graph TD\n A --> B\n```\n";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(block.code, "graph TD\nA --> B\n");
        assert_eq!(block.strips, [3, 1]);
        assert_eq!(block.position(1, 1), (2, 4));
        assert_eq!(block.position(2, 3), (3, 4));
    }

    #[test]
    fn code_with_a_fence_lengthens_the_fences() {
        let src = "a\n```mermaid\ngraph TD\n```\nb\n";
        let block = &mermaid_blocks(src)[0];
        let code = "graph TD\n```\n  A\n";
        let out = block.replace_code(src, code);
        assert_eq!(out, "a\n````mermaid\ngraph TD\n```\n  A\n````\nb\n");
        assert_eq!(mermaid_blocks(&out)[0].code, code);
        // Again with a longer run, from the longer fences; a fence that is not closed by the
        // code is kept as it is.
        let block = &mermaid_blocks(&out)[0];
        let out = block.replace_code(&out, "graph TD\n`````\n");
        assert_eq!(out, "a\n``````mermaid\ngraph TD\n`````\n``````\nb\n");
        let block = &mermaid_blocks(&out)[0];
        assert_eq!(
            block.replace_code(&out, "graph TD\n"),
            "a\n``````mermaid\ngraph TD\n``````\nb\n"
        );
        // Indented tildes, CRLF, and an unclosed block.
        let src = "  ~~~ dot\r\n  digraph {}\r\n  ~~~\r\n";
        let block = &mermaid_blocks(src)[0];
        let out = block.replace_code(src, "digraph {}\n~~~\n");
        assert_eq!(out, "  ~~~~ dot\r\n  digraph {}\r\n  ~~~\r\n  ~~~~\r\n");
        assert_eq!(mermaid_blocks(&out)[0].code, "digraph {}\n~~~\n");
        let src = "```mermaid\ngraph TD\n";
        let block = &mermaid_blocks(src)[0];
        let out = block.replace_code(src, "graph TD\n```\n");
        assert_eq!(out, "````mermaid\ngraph TD\n```\n");
        assert_eq!(mermaid_blocks(&out)[0].code, "graph TD\n```\n");
    }

    #[test]
    fn appending_blocks() {
        assert_eq!(
            append_block("", "mermaid", "graph TD"),
            "```mermaid\ngraph TD\n```\n"
        );
        assert_eq!(
            append_block("# Doc", "dot", "digraph {}\n"),
            "# Doc\n\n```dot\ndigraph {}\n```\n"
        );
        // A block left open is closed first, so the new block is found.
        let out = append_block("# Doc\n\n````python\nx = 1\n", "mermaid", "graph TD\n");
        assert_eq!(
            out,
            "# Doc\n\n````python\nx = 1\n````\n\n```mermaid\ngraph TD\n```\n"
        );
        assert_eq!(mermaid_blocks(&out)[0].code, "graph TD\n");
        // Code with a fence, and CRLF.
        let out = append_block("a\r\n", "mermaid", "graph TD\n```\n");
        assert_eq!(out, "a\r\n\r\n````mermaid\r\ngraph TD\r\n```\r\n````\r\n");
        assert_eq!(mermaid_blocks(&out)[0].code, "graph TD\n```\n");
    }

    #[test]
    fn other_diagrams() {
        use crate::flowchart::parse;
        assert_eq!(
            parse("%% c\nsequenceDiagram\n  A->>B: hi\n")
                .other_diagram()
                .as_deref(),
            Some("sequenceDiagram")
        );
        assert_eq!(parse("flowchart TD\n  pie\n").other_diagram(), None);
        assert_eq!(parse("nonsense here\n").other_diagram(), None);
    }

    #[test]
    fn blocks_in_blockquotes() {
        let src =
            "> text\n>\n> ```mermaid\n> graph TD\n>   A\n>\n> ```\n\n> > ~~~dot\n> > digraph {}\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].code, "graph TD\n  A\n\n");
        assert_eq!(blocks[0].quote, "> ");
        assert_eq!(blocks[0].line, 4);
        assert_eq!(blocks[0].position(2, 3), (5, 5));
        assert!(blocks[0].closed);
        assert_eq!(&src[blocks[0].end..], "\n> > ~~~dot\n> > digraph {}\n");
        assert_eq!(blocks[1].quote, "> > ");
        assert_eq!(blocks[1].code, "digraph {}\n");
        assert!(!blocks[1].closed);
        // Replacing keeps the markers, with a bare `>` on blank lines, and the fences grow.
        let out = blocks[0].replace_code(src, "graph LR\n\n```\n");
        assert_eq!(
            out,
            "> text\n>\n> ````mermaid\n> graph LR\n>\n> ```\n> ````\n\n> > ~~~dot\n> > digraph {}\n"
        );
        assert_eq!(mermaid_blocks(&out)[0].code, "graph LR\n\n```\n");
        assert_eq!(blocks[0].replace_code(src, &blocks[0].code), src);
        // A block ends with its blockquote; the line after is read again.
        let src = "> ```mermaid\n> graph TD\n```mermaid\npie\n```\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].code, "graph TD\n");
        assert!(!blocks[0].closed);
        assert_eq!(&src[blocks[0].end..], "```mermaid\npie\n```\n");
        assert_eq!(
            blocks[0].replace_code(src, "graph LR\n"),
            "> ```mermaid\n> graph LR\n```mermaid\npie\n```\n"
        );
        assert_eq!(blocks[1].code, "pie\n");
        // Code that only looks like a quote marker stays code.
        let src = "```mermaid\n> x\n```\n";
        assert_eq!(mermaid_blocks(src)[0].code, "> x\n");
    }

    #[test]
    fn blocks_in_list_items() {
        // An ordered item's content column is past its marker; a nested item's past both.
        let src = "10. step\n\n    ```mermaid\n    graph TD\n      A\n    ```\n- a\n  - b\n\n    ```dot\n    digraph {}\n    ```\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].code, "graph TD\n  A\n");
        assert_eq!(blocks[0].indent, 4);
        assert_eq!(blocks[0].position(2, 3), (5, 7));
        assert_eq!(blocks[1].code, "digraph {}\n");
        assert!(blocks[0].closed && blocks[1].closed);
        // A fence on the item's own line, and one inside a blockquote's item.
        let src = "- ```mermaid\n  pie\n  ```\n> 1) ```dot\n>    a\n>    ```\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!((blocks[0].indent, blocks[0].code.as_str()), (2, "pie\n"));
        assert_eq!((blocks[1].quote.as_str(), blocks[1].indent), ("> ", 3));
        assert_eq!(blocks[1].code, "a\n");
        assert_eq!(
            blocks[1].replace_code(src, "b\n\nc\n"),
            "- ```mermaid\n  pie\n  ```\n> 1) ```dot\n>    b\n>\n>    c\n>    ```\n"
        );
        // A line less indented than the item ends it, and the block with it.
        let src = "- ```mermaid\n  graph TD\nafter\n";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(block.code, "graph TD\n");
        assert!(!block.closed);
        assert_eq!(&src[block.end..], "after\n");
        // Four columns past the item's content column is indented code, not a fence.
        assert!(mermaid_blocks("- a\n\n      ```mermaid\n      x\n      ```\n").is_empty());
        // Not list items: emphasis, a thematic break, and a marker with nothing after it.
        assert!(list_item("*em*").is_none());
        assert!(list_item("---").is_none());
        assert!(list_item("1.").is_none());
        assert!(list_item("1234567890. x").is_none());
        assert_eq!(list_item("-      x"), Some(2));
        assert_eq!(list_item("2) x"), Some(3));
        // A block left open in an item is closed in the item before a block is appended.
        let out = append_block("- ```mermaid\n  graph TD\n", "dot", "digraph {}\n");
        assert_eq!(
            out,
            "- ```mermaid\n  graph TD\n  ```\n\n```dot\ndigraph {}\n```\n"
        );
        assert_eq!(mermaid_blocks(&out).len(), 2);
    }

    #[test]
    fn info_strings() {
        assert_eq!(info_lang("mermaid"), Some(Lang::Mermaid));
        assert_eq!(info_lang("Mermaid title=x"), Some(Lang::Mermaid));
        assert_eq!(info_lang("{.mermaid}"), Some(Lang::Mermaid));
        assert_eq!(info_lang("{#fig .mermaid width=3}"), Some(Lang::Mermaid));
        assert_eq!(info_lang("{mermaid}"), Some(Lang::Mermaid));
        assert_eq!(info_lang("{ .dot }"), Some(Lang::Dot));
        assert_eq!(info_lang("mermaid{caption=x}"), Some(Lang::Mermaid));
        assert_eq!(info_lang("PlantUML"), Some(Lang::PlantUml));
        assert_eq!(info_lang("{#fig}"), None);
        assert_eq!(info_lang("{.python}"), None);
        assert_eq!(info_lang(""), None);
        let blocks = mermaid_blocks("```{.mermaid}\npie\n```\n~~~ {mermaid}\npie\n~~~\n");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn byte_order_mark() {
        let src = "\u{feff}```mermaid\r\nflowchart LR\r\n  A\r\n```\r\n";
        let blocks = mermaid_blocks(src);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, "flowchart LR\n  A\n");
        assert_eq!(blocks[0].line, 2);
        assert!(blocks[0].closed);
    }

    #[test]
    fn fences() {
        assert_eq!(
            opening("```mermaid"),
            Some((0, '`', 3, Some(Lang::Mermaid)))
        );
        assert_eq!(opening("```dot"), Some((0, '`', 3, Some(Lang::Dot))));
        assert_eq!(
            opening("~~~ graphviz {.x}"),
            Some((0, '~', 3, Some(Lang::Dot)))
        );
        assert_eq!(
            opening("```plantuml"),
            Some((0, '`', 3, Some(Lang::PlantUml)))
        );
        assert_eq!(opening("```puml"), Some((0, '`', 3, Some(Lang::PlantUml))));
        assert_eq!(opening("```dotnet"), Some((0, '`', 3, None)));
        assert_eq!(opening("   ~~~ js"), Some((3, '~', 3, None)));
        assert_eq!(opening("    ```mermaid"), None);
        assert_eq!(opening("``mermaid"), None);
        assert_eq!(opening("``` a`b"), None);
        assert_eq!(opening("text"), None);
        assert!(closes("````", '`', 3));
        assert!(!closes("``", '`', 3));
        assert!(!closes("``` x", '`', 3));
        assert!(!closes("    ```", '`', 3));
        assert!(is_markdown(std::path::Path::new("a/b.MD")));
        assert!(is_markdown(std::path::Path::new("b.markdown")));
        assert!(!is_markdown(std::path::Path::new("b.mmd")));
    }
}
