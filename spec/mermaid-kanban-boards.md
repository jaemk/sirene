# Mermaid Kanban Boards

`kanban`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/kanban.html>. Code: `crates/sirene-core/src/kanban/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/kanban.rs` and
the corpus under `crates/sirene-core/tests/fixtures/kanban/` (LANGS-6).

## MKAN-1 Syntax

- Header: `kanban`.
- The mindmap grammar (MMIND-1): node lines (`id`, `id[text]`, `[text]`, the other shapes,
  strings, and markdown strings), `::icon(name)`, and `:::classes`.
- After a node, `@{ key: value, ... }` metadata, which may span lines. Mindmaps read it too and
  warn that it is not used there.
- Comments, directives, frontmatter `title` and `config.kanban.ticketBaseUrl`.

`kanban::parse` uses `mindmap::parser::parse_with` with the `kanban` header, so the tree is
lossless. An unclosed `@{` is an error. Tests: `snapshots`, `syntax_errors`, and the mindmap
parser's unit tests.

## MKAN-2 Model

`kanban::Kanban`: the title, the ticket base URL, and the columns in order. The first node's
indentation, counted in characters as for mindmaps, is the column indentation: a node there
starts a column, and a node indented more is a card in the last column. A node indented less is
an error and left out, as Mermaid rejects items without a section. A `Column` has its id, label,
icon, classes, and cards; a `Card` has its id, label, `assigned`, `ticket`, `priority`
(`Priority`: `Very High`, `High`, `Low`, or `Very Low` in any case), icon, and classes.

A card's metadata may also set its `label` and `icon`; a column's only its `label`. Other keys, an
unknown priority, and an entry without `:` are warnings and ignored. An unclosed `@{` is not read.
`::icon` and `:::` apply to the last column or card; before the first node they are errors, and an
icon is a warning that icons are not drawn. `Kanban::ticket_url` puts a ticket into the base URL
in place of `#TICKET#`. Tests: `columns_and_cards`, `errors`.

## MKAN-3 Drawing

Columns 220 pixels wide side by side, each a rounded gray panel with its bold title wrapped at
the top, and all as tall as the tallest. Cards stack down a column, each a rounded box in the
background color with a node-stroke border, its label wrapped and aligned left, and under it the
ticket at the left and the assignee at the right in a smaller size, each cut short with `...`
to fit (the ticket to half the width when there is an assignee). A card with a priority has a
colored stripe down its left edge: red for very high, orange for high, blue for low, light blue
for very low. The title is centered above. Every element is owned by the diagram. Tests:
`drawing`.

## MKAN-4 ASCII

A box per column side by side, its title in the top border, holding a rounded box per card. Card
text wraps at the widest label or details line, kept between 12 and 28 columns; a longer word
is broken. Under a card's
label, a details line holds its priority (`▲▲` very high, `▲` high, `▼` low, `▼▼` very low; `^^`,
`^`, `v`, `vv` in ASCII) and name, its ticket, and `@assignee`. The title is centered above.
Tests: `snapshots`, `ascii_charset_snapshots`, and the renderer's unit tests.

## MKAN-5 Not drawn

Icons, classes (Mermaid applies them only through CSS), and ticket links (the drawing has no
links) are kept in the model but not drawn. Drawing edits are read-only (LANGS-5).
