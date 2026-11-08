//! Mermaid code blocks in Markdown (MARKDO-1).

/// A fenced code block whose info string starts with `mermaid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The block's text, with the fence's indentation taken off each line.
    pub code: String,
    /// The line of the block's first code line, from 1.
    pub line: usize,
    /// The columns of indentation taken off each line.
    pub indent: usize,
    /// The byte offset just after the closing fence's line (the end of the text when the block
    /// is not closed).
    pub end: usize,
    /// Whether a closing fence ends the block; otherwise it runs to the end of the text.
    pub closed: bool,
}

impl Block {
    /// Where a position in [`Block::code`] (line and column from 1) is in the Markdown.
    pub fn position(&self, line: usize, col: usize) -> (usize, usize) {
        (self.line + line - 1, col + self.indent)
    }
}

/// An opening fence on `line`: its indent, character, and length, when the info string's first
/// word is `mermaid`.
fn opening(line: &str) -> Option<(usize, char, usize, bool)> {
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
    Some((indent, c, len, word == "mermaid"))
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

/// The mermaid blocks in `src`, in order. Other fenced blocks are skipped whole, so a mermaid
/// fence inside one is not a block.
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
        let Some((indent, c, len, mermaid)) = opening(lines[i].1) else {
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
        if mermaid {
            blocks.push(Block {
                code,
                line: i + 2,
                indent,
                end,
                closed: j < lines.len(),
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
        assert_eq!(opening("```mermaid"), Some((0, '`', 3, true)));
        assert_eq!(opening("   ~~~ js"), Some((3, '~', 3, false)));
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
