# Markdown

Diagram code blocks and formulas in Markdown files, for `sirene check` (CLI-11), rendering
(CLI-12), and the editor (WYSIWY-20). Tests: the unit tests in
`crates/sirene-core/src/markdown.rs` and `crates/sirene-core/tests/markdown_math.rs`.

## MARKDO-1 Diagram blocks

`sirene_core::markdown::mermaid_blocks` finds the fenced code blocks whose info string names a
diagram language (`info_lang`, then `Lang::of_info`): `mermaid`, `dot` or `graphviz` (DOT-1),
or `plantuml` or `puml` (PSEQ-7), in any case. The name is the info string's first word, up to
any `{` (`mermaid{caption=x}`), or for an info string in braces, its first class or else its
first bare word (Pandoc's `{.mermaid #fig}`, MyST's `{mermaid}`). They are found as CommonMark
fences them: an opening line of at least three backticks or tildes, indented at most three
spaces (a backtick fence's info string has no backticks), closed by a line of the same
character at least as long with nothing else on it, indented at most three spaces; an unclosed
block runs to the end of the text. Other fenced blocks are skipped whole, so a diagram fence
inside one is not a block.

Fences are found inside blockquotes and list items too. A blockquote line's markers (up to
three spaces, `>`, and an optional space, nested) come off first. A list item (`-`, `+`, `*`, or
one to nine digits and `.` or `)`, then one to four spaces, or one before indented code) opens
at its content column, and a fence in it may be indented up to three spaces past that column,
on the item's own line or a later one; a non-blank line indented less than the column ends the
item. A block inside one ends when its container does: a line with fewer blockquote markers, or
a non-blank line indented less than the item's column, ends it unclosed, and that line is read
again. Lazy continuation lines are not followed.

Each block has its code, with the blockquote markers and the opening fence's indentation (up to
that many spaces) taken off each line, the line of its first code line, the opening line's
markers (`Block::quote`), that indentation and the columns taken off each line (fewer for a
line indented less), the offset just after its closing fence's line (or where it ended), and
whether it was closed. `Block::position` maps a line and column in the code back to the
Markdown file, adding that line's own columns.

## MARKDO-2 Other diagram types

`Parse::other_diagram` gives the keyword of the other Mermaid diagram type a flowchart source
is (FLOWCH-16), e.g. `pie`. Every Mermaid 12.3 type is drawn, so the CLI skips no blocks.

## MARKDO-3 Replacing a block's code

`Block::body` is the byte range between a block's fences. `Block::replace_code` returns the
Markdown with that range replaced by new code: each non-blank line after the block's quote and
indented by its indentation, each blank line the quote's markers alone (`>`), every line ended
like the Markdown's (`\r\n` when it has any), and everything else
unchanged. The fences are kept too, unless a line of the new code would close the block: then
both are made one longer than the longest such line's run, so the code stays whole. Replacing a
block's code with itself returns the Markdown unchanged. The editor writes edits to a Markdown
file's block with it (WYSIWY-20).

## MARKDO-4 Adding a block

`append_block` returns the Markdown with a backtick-fenced block of new code added at the end,
after a blank line, named by an info string, its lines ended like the Markdown's. A fenced
block left open at the end of the Markdown is closed first, in its blockquote or list item, and the fences are made longer than
any fence line in the code, so the new block is found with exactly that code. The editor adds
the block of a Markdown file with none with it, and Save As to a Markdown file writes one
(WYSIWY-20).

## MARKDO-5 Formulas

`sirene_core::markdown::parts` returns the diagram blocks (MARKDO-1) and the formulas of a
Markdown text as `Part::Diagram(Block)` and `Part::Math(MathBlock)`, in source order, with byte
ranges (`Part::range`) that are in bounds, on character boundaries, ordered, and not
overlapping. `mermaid_blocks` still returns the diagram blocks alone. A `MathBlock` has its
`kind`, its `source`, the line of its first source line, and `position`, which maps a line and
column of `source` back to the Markdown, as `Block::position` does. `MathBlock::lower` lowers it
to an equation (LEMB-3):

| Kind | Found as | Style |
|---|---|---|
| `Fenced` | a fenced block whose info word is `math`, `latex`, or `tex`; the code is read as a TeX file (a wrapper is allowed) | display |
| `Display` | `$$...$$` with a non-blank body, in a paragraph (not across a blank line) | display |
| `Inline` | `$...$` on one line, by GitHub's rules (LEMB-3) | text |

Formulas are read in paragraphs at the top level, in blockquotes, and in list items, found as
MARKDO-1 finds fences. Not read: any other fenced block (a diagram fence too), an indented code
block, and a code span (a code span in the body does not close the formula). A backslash escapes
the ASCII punctuation after it, so `\$` is not a delimiter. `Lang::of_info("latex")` stays
`None`, so `mermaid_blocks` never returns these fences.

Numbering: a file's diagram blocks and formulas form one list in source order, and the CLI
numbers it from 1 for output names (`STEM-N.svg`), `ascii` block names, and `--link` lines
(CLI-12). A formula counts whether it is drawn or not. Without the `math` feature the CLI skips
the formulas with one warning per file. Tests: `crates/sirene-core/tests/markdown_math.rs`.
