# Mermaid Treemaps

`treemap-beta`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/treemap.html>. Code: `crates/sirene-core/src/treemap/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/treemap.rs` and
the corpus under `crates/sirene-core/tests/fixtures/treemap/` (LANGS-6).

## MTREE-1 Syntax

- Header: `treemap-beta` or `treemap`.
- An item per line: `"name"` (a section) or `"name": value` / `"name", value` (a leaf), in double
  or single quotes, with an optional `:::class`. A value is digits with `.`, `,`, and `_`.
- `classDef`, `title`, `accTitle`, `accDescr`, blank lines, comments, directives, frontmatter
  `title` and `config.treemap`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A name not closed
on its line, a missing value after `:` or `,`, a missing class after `:::`, and a line that is none
of the above are errors. Tests: `snapshots`, `syntax_errors`, and the parser's unit tests.

## MTREE-2 Model

`treemap::Treemap`: the title, `acc_title`, `acc_descr`, and the items (`Item`: name, value,
class, the `classDef` styles, parent, and children). An item nests under the nearest item above it
with less indentation that is a section; a leaf holds nothing. Several top-level items are allowed.
From `config.treemap`: `padding` (10), `diagram_padding` (8), `show_values` (on unless `false`),
`width` and `height` (`nodeWidth` and `nodeHeight` times ten: 1000 by 400), and `value_format`
(`valueFormat`, a d3-format subset in `treemap::Format`; default `,`). A bad config value is a
warning and ignored.

A value that is not a number is an error and its item is left out. A section with no items and a
class with no `classDef` are warnings. `Treemap::values` gives each item's value, a section's being
the sum of its items. Tests: `model`, `errors`, and the module's unit tests.

## MTREE-3 Drawing

`Treemap::layout` follows d3's squarified treemap as Mermaid configures it: children sorted by
value, largest first, with `padding` between them, and each section inset by `padding` with a
25-unit header on top. Sections are filled in the git branch palette by name, with their name
(bold, truncated to fit) and value (italic) in the header. A leaf takes its section's color, lighter,
with its name and value centered and sized to fit; a top-level leaf is outlined only. `classDef`
styles apply on top. The title is centered above. Every element is owned by the diagram. Tests:
`drawing` and the scene's unit tests.

## MTREE-4 ASCII

An outline, largest first: each item's name, formatted value, and a bar for its share of the total
(eighth blocks in Unicode, `#` in ASCII), with a blank line between top-level items. The title is
centered above. Tests: `snapshots`, `ascii_charset_snapshots`, and the renderer's unit tests.

## MTREE-5 Not drawn

Theme variables are ignored. Drawing edits are read-only (LANGS-5).
