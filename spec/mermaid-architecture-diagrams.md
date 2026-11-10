# Mermaid Architecture Diagrams

`architecture-beta`, lowered to its own model over the flowchart model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/architecture.html>. Code: `crates/sirene-core/src/architecture/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/architecture.rs`
and the corpus under `crates/sirene-core/tests/fixtures/architecture/` (LANGS-6).

## MARCH-1 Syntax

- Header: `architecture-beta` or `architecture`.
- `group id(icon)[label] in parent`, `service id(icon)[label] in parent`, and
  `junction id in parent`; the icon, label, and `in` are optional, and a label may be quoted.
- Edges: `a:R -- L:b`, with `<` and `>` for arrows at either end, `-[label]-` for a label, and
  `{group}` after an id to end at its group.
- `align row ...` and `align column ...`.
- `title`, `accTitle`, `accDescr`, comments, directives, and frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A missing id,
side, colon, or edge, an unclosed `(`, `[`, or `-[`, an `in` without a group, `align` without
`row` or `column`, extra text after a statement, and a line that is none of the above are errors.
Tests: `syntax_errors` and the parser's unit tests.

## MARCH-2 Model

`architecture::Architecture`: a `Flowchart` holding the services and junctions (nodes), groups
(subgraphs), edges, and title; `items`, each node's kind and icon; `node_parent`, `group_icons`,
and `group_parent`; `ports`, each edge's ends and sides and whether an end is its group; and
`aligns`.

An id declared twice, an unknown group after `in`, and an edge to an unknown id or to a group are
errors and skipped (a node with an unknown group is placed outside any group). `{group}` on a node
outside a group, or on both ends in one group, is a warning and the edge ends at the node. An
`align` member that is not a node, and an `align` with fewer than two, are warnings. Tests:
`model`, `errors`.

## MARCH-3 Drawing

`architecture::layout` places each group, and the diagram, as a grid: an edge `a:R -- L:b` puts
`b` one column right of `a`, `T`/`B` one row up or down, and an across side with an up-or-down
side one diagonal step; items are placed in breadth-first order and a taken cell pushes onward; a
group takes one cell of its parent's grid; `align` moves its members into one row or column.
Each column and row is as large as its largest item, items are centered, and the boxes of a row
share a middle line with labels below. Edges go straight or with one or two right-angle turns
from side to side; a `{group}` end is on the group's side, level with the node, and an edge
below a service starts under its label.

A service is an 80-pixel icon on Mermaid's blue with its label below; the icons `cloud`,
`database`, `disk`, `internet`, and `server` are drawn, others are a question mark. Groups are
dashed boxes with the title at the top left beside the group's icon. Junctions draw nothing.
Elements are owned by their node, subgraph, or edge. Tests: `drawing` and the layout's and
scene's unit tests.

## MARCH-4 ASCII

The same grid in cells (6 columns and 3 rows between items): a service is a box labeled
`label (icon)` (its id without a label), a junction is `●`, a group is a box titled on the top
border, and edges are the flowchart's lines and arrows. The title is centered above. Tests:
`snapshots`, `ascii_charset_snapshots`, and the renderer's unit tests.

## MARCH-5 Not drawn

Icon packs (`iconify` names) are not loaded; the layout is a grid, not Mermaid's force-directed
one. Theme variables are ignored. Drawing edits are read-only (LANGS-5).
