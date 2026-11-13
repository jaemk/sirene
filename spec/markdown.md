# Markdown

Diagram code blocks in Markdown files, for `sirene check` (CLI-11), rendering (CLI-12), and the
editor (WYSIWY-20). Tests: the unit tests in `crates/sirene-core/src/markdown.rs`.

## MARKDO-1 Diagram blocks

`sirene_core::markdown::mermaid_blocks` finds the fenced code blocks whose info string's first
word names a diagram language (`Lang::of_info`): `mermaid`, `dot` or `graphviz` (DOT-1), or
`plantuml` or `puml` (PSEQ-7). They are found as CommonMark fences them: an opening line of at
least three backticks or tildes, indented at most three spaces (a backtick fence's info string
has no backticks), closed by a line of the same character at least as long with nothing else on
it, indented at most three spaces; an unclosed block runs to the end of the text. Other fenced
blocks are skipped whole, so a diagram fence inside one is not a block. Each block has its code, with the opening
fence's indentation (up to that many spaces) taken off each line, the line of its first code
line, that indentation and the columns taken off each line (fewer for a line indented less),
the offset just after its closing fence's line, and whether it was closed. `Block::position`
maps a line and column in the code back to the Markdown file, adding that line's own columns.

## MARKDO-2 Other diagram types

`Parse::other_diagram` gives the keyword of the other Mermaid diagram type a flowchart source
is (FLOWCH-16), e.g. `pie`. Every Mermaid 12.3 type is drawn, so the CLI skips no blocks.

## MARKDO-3 Replacing a block's code

`Block::body` is the byte range between a block's fences. `Block::replace_code` returns the
Markdown with that range replaced by new code: each non-blank line indented by the block's
indentation, every line ended like the Markdown's (`\r\n` when it has any), and everything else
unchanged. The fences are kept too, unless a line of the new code would close the block: then
both are made one longer than the longest such line's run, so the code stays whole. Replacing a
block's code with itself returns the Markdown unchanged. The editor writes edits to a Markdown
file's block with it (WYSIWY-20).

## MARKDO-4 Adding a block

`append_block` returns the Markdown with a backtick-fenced block of new code added at the end,
after a blank line, named by an info string, its lines ended like the Markdown's. A fenced
block left open at the end of the Markdown is closed first, and the fences are made longer than
any fence line in the code, so the new block is found with exactly that code. The editor adds
the block of a Markdown file with none with it, and Save As to a Markdown file writes one
(WYSIWY-20).
