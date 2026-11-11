//! Diagram code blocks in Markdown (MARKDO-1).

use crate::diagram::Lang;

/// A fenced code block whose info string starts with `mermaid`, `dot`, or `graphviz`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The block's text, with the fence's indentation taken off each line.
    pub code: String,
    /// The language the info string names (DOT-5).
    pub lang: Lang,
    /// The line of the block's first code line, from 1.
    pub line: usize,
    /// The columns of indentation taken off each line.
    pub indent: usize,
    /// The byte offset just after the closing fence's line (the end of the text when the block
    /// is not closed).
    pub end: usize,
    /// Whether a closing fence ends the block; otherwise it runs to the end of the text.
    pub closed: bool,
    /// The bytes between the fences: from the start of the first code line to the start of the
    /// closing fence's line (or the end of the text).
    pub body: std::ops::Range<usize>,
}

impl Block {
    /// Where a position in [`Block::code`] (line and column from 1) is in the Markdown.
    pub fn position(&self, line: usize, col: usize) -> (usize, usize) {
        (self.line + line - 1, col + self.indent)
    }

    /// `src`, the Markdown this block was found in, with the block's code replaced by `code`:
    /// each line indented like the fence and ended like the Markdown's lines (`\r\n` or `\n`).
    /// The fences and everything outside the block are kept.
    pub fn replace_code(&self, src: &str, code: &str) -> String {
        let newline = if src.contains("\r\n") { "\r\n" } else { "\n" };
        let pad = " ".repeat(self.indent);
        let mut body = String::with_capacity(code.len() + 8);
        for line in code.lines() {
            if !line.is_empty() {
                body.push_str(&pad);
            }
            body.push_str(line);
            body.push_str(newline);
        }
        // An unclosed block at the end of a text without a final newline keeps it that way.
        if !self.closed && !src.ends_with('\n') && !code.ends_with('\n') {
            body.truncate(body.len() - newline.len());
        }
        let mut out = String::with_capacity(src.len() + body.len());
        out.push_str(&src[..self.body.start]);
        // The opening fence ends the text: the code starts a line of its own.
        if self.body.start == src.len() && !src.ends_with('\n') && !body.is_empty() {
            out.push_str(newline);
        }
        out.push_str(&body);
        out.push_str(&src[self.body.end..]);
        out
    }
}

/// An opening fence on `line`: its indent, character, and length, and the diagram language
/// the info string's first word names, if any: `mermaid`, or `dot` or `graphviz` (LANGS-1).
fn opening(line: &str) -> Option<(usize, char, usize, Option<Lang>)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
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
    let word = info.split_whitespace().next().unwrap_or("");
    Some((indent, c, len, Lang::of_info(word)))
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

/// The diagram blocks in `src` (`mermaid`, `dot`, and `graphviz`), in order. Other fenced
/// blocks are skipped whole, so a mermaid fence inside one is not a block.
pub fn mermaid_blocks(src: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    // (byte offset, line text without its newline) for each line.
    let mut lines = Vec::new();
    let mut at = 0;
    for raw in src.split_inclusive('\n') {
        lines.push((at, raw.trim_end_matches(['\n', '\r'])));
        at += raw.len();
    }
    let mut i = 0;
    while i < lines.len() {
        let Some((indent, c, len, lang)) = opening(lines[i].1) else {
            i += 1;
            continue;
        };
        let mut code = String::new();
        let mut j = i + 1;
        while j < lines.len() && !closes(lines[j].1, c, len) {
            let text = lines[j].1;
            let strip = text.len() - text.trim_start_matches(' ').len();
            code.push_str(&text[strip.min(indent)..]);
            code.push('\n');
            j += 1;
        }
        let end = lines.get(j + 1).map_or(src.len(), |(at, _)| *at);
        if let Some(lang) = lang {
            let start = lines.get(i + 1).map_or(src.len(), |(at, _)| *at);
            let stop = lines.get(j).map_or(src.len(), |(at, _)| *at);
            blocks.push(Block {
                code,
                lang,
                line: i + 2,
                indent,
                end,
                closed: j < lines.len(),
                body: start..stop.max(start),
            });
        }
        i = j + 1;
    }
    blocks
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
        let src = "```mermaid";
        let block = &mermaid_blocks(src)[0];
        assert_eq!(
            block.replace_code(src, "graph LR\n"),
            "```mermaid\ngraph LR\n"
        );
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
