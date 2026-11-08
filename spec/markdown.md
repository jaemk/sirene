# Markdown

Mermaid code blocks in Markdown files, for `sirene check` (CLI-11) and rendering (CLI-12).
Tests: the unit tests in `crates/sirene-core/src/markdown.rs`.

## MARKDO-1 Mermaid blocks

`sirene_core::markdown::mermaid_blocks` finds the fenced code blocks whose info string's first
word is `mermaid`, as CommonMark fences them: an opening line of at least three backticks or
tildes, indented at most three spaces (a backtick fence's info string has no backticks), closed
by a line of the same character at least as long with nothing else on it, indented at most
three spaces; an unclosed block runs to the end of the text. Other fenced blocks are skipped
whole, so a mermaid fence inside one is not a block. Each block has its code, with the opening
fence's indentation (up to that many spaces) taken off each line, the line of its first code
line, that indentation, the offset just after its closing fence's line, and whether it was
closed. `Block::position` maps a line and column in the code back to the Markdown file.

## MARKDO-2 Other diagram types

`Parse::other_diagram` gives the keyword of the other Mermaid diagram type a source is (FLOWCH-16),
e.g. `pie`, so callers can skip such blocks.

## MARKDO-3 Replacing a block's code

`Block::body` is the byte range between a block's fences. `Block::replace_code` returns the
Markdown with that range replaced by new code: each non-blank line indented by the block's
indentation, every line ended like the Markdown's (`\r\n` when it has any), and everything else,
fences included, unchanged. Replacing a block's code with itself returns the Markdown unchanged.
The editor writes edits to a Markdown file's block with it (WYSIWY-20).
