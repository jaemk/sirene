# Mermaid Sankey Diagrams

`sankey-beta`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/sankey.html>. Code: `crates/sirene-core/src/sankey/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/sankey.rs` and
the corpus under `crates/sirene-core/tests/fixtures/sankey/` (LANGS-6).

## MSANKEY-1 Syntax

- Header: `sankey-beta` or `sankey`.
- A row per line, `source,target,value`, as CSV: a name field is plain text (trimmed) or
  double-quoted, where `""` is a literal `"` and a quoted field may hold `,`; the value is a
  number, with an optional exponent (`1e3`), and the rest of the line.
- Blank lines, comments, directives, frontmatter `title` and `config.sankey`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A row with a
missing name, a missing `,`, more than three fields, a value that is not a number, or a quoted
field not closed on its line is an error. Tests: `snapshots`, `syntax_errors`, and the parser's
unit tests.

## MSANKEY-2 Model

`sankey::Sankey`: the frontmatter title, the nodes (names in order of first use), the links
(`Link`: source and target node indexes and the value), and from `config.sankey` the `width`
(600) and `height` (400), `link_color` (`source`, `target`, `gradient`, the default, or a color),
`align` (`nodeAlignment`: `justify`, the default, `left`, `right`, or `center`), `show_values`
(on unless `false`), `prefix` and `suffix` for values, `node_width` (10), and `node_padding`
(unset: 25 with values shown, else 10, as `Sankey::padding` gives). A bad config value is a
warning and ignored; an unknown key is ignored.

A negative value, a value over 1e30, a link from a node to itself, and a link that closes a cycle (named in the
message) are errors and the link is left out; a node that only such links named is not drawn.
`Sankey::format_value` rounds to two decimals and adds the prefix and suffix. Tests: `model`,
`errors`, and the module's unit tests.

## MSANKEY-3 Drawing

`Sankey::layout` follows d3-sankey as Mermaid configures it: a node's column is its depth
(`left`), its height from the right (`right`), or, for `justify`, its depth with sinks in the last
column, and `center` moves a source to just before its first target. Node heights are their value
(the larger of what flows in and out) on one scale chosen so the fullest column fits the height
with `node_padding` between nodes; six rounds of relaxation toward linked nodes, with collisions
resolved after each, place nodes vertically, and links stack on each node in the order of the
node at their other end. Each link is a band as wide as its value along a cubic between its nodes,
at half opacity in its source's color, its target's, a fixed color, or a gradient from one to the
other. Nodes are bars in the git branch palette. A node's label, its name and with `show_values`
its value, sits beside it toward the middle of the chart. The title is centered above. Every
element is owned by the diagram. Tests: `drawing` and the scene's unit tests.

## MSANKEY-4 ASCII

An outline in column order: each node's name and value, then a branch per outgoing link from top
to bottom with the target, the value, and a bar scaled to the largest link (eighth blocks in
Unicode, `#` in ASCII, at least a sliver). The title is centered above. Tests: `snapshots`,
`ascii_charset_snapshots`, and the renderer's unit tests.

## MSANKEY-5 Not drawn

Theme variables are ignored. Drawing edits are read-only (LANGS-5).
