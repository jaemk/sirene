# Mermaid Mindmaps

`mindmap`, lowered to its own tree model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/mindmap.html>. Code: `crates/sirene-core/src/mindmap/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/mindmap.rs` and
the corpus under `crates/sirene-core/tests/fixtures/mindmap/` (LANGS-6).

## MMIND-1 Syntax

- Header: `mindmap`.
- A node line: an id, an id and a shape, or a shape alone. The id is the text up to a shape
  delimiter (`(`, `[`, `)`, `{`, `}`) or the end of the line, so it may hold spaces.
- Shapes: `[text]`, `(text)`, `((text))`, `)text(`, `))text((`, `{{text}}`. The text inside is
  raw text up to the closing delimiter, a `"string"`, or a markdown ``"`string`"``, which may span
  lines.
- `::icon(name)` and `:::class names` on lines of their own.
- `@{ ... }` metadata after a node, shared with kanban boards (MKAN-1); in a mindmap it is a
  warning and ignored.
- Comments, directives, frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests:
`snapshots`, `syntax_errors`, and the parser's unit tests.

## MMIND-2 Model

`mindmap::Mindmap`: the title (frontmatter `title`) and the nodes, the first being the root and
each after its parent. A node has its id (`None` for a shape alone), label (`flowchart::Label`;
the id when the shape is empty or missing), `Shape`, icon, classes, parent, children, depth, and
section: the index of the root's child it descends from (`None` for the root).

A node's parent is the nearest node above it indented less, counting leading whitespace
characters, as Mermaid does. A node with none after the root is an error and is left out, along
with the `::icon` and `:::` lines after it. `::icon` and `:::` apply to the last node; before the
first node they are errors. `::icon` is a warning that icons are not drawn. Tests: `tree`,
`nodes`, `errors`.

## MMIND-3 Drawing

The root in the middle. Its children go down the right side in order until that side holds half
their subtrees' height, and the rest down the left; each subtree spreads outward from there, a
node centered on its children's band. Each shape has its own outline: the default a box with
rounded top corners and a darker line along the bottom, a square, a rounded box, a circle, a cloud
of bumps, a bang of spikes, and a hexagon. Labels wrap at 200 pixels, with markdown bold and
italic. The root uses the theme's node stroke color; each section the next of the branch colors,
with text in black or white for contrast. Edges are curves in their section's color from the
parent's facing side to the child's, thinner with depth, drawn behind the nodes. The title is
centered above. Every element is owned by the diagram. Tests: `drawing`, `shapes`.

## MMIND-4 ASCII

An outline of the tree from the root, depth first, with `├── `, `└── `, and `│   ` (`+-- `,
`` `-- ``, `|   ` in ASCII). Each node shows its shape's delimiters (`[A]`, `((Root))`), and
continuation lines of a multi-line label line up under its first. The title is centered above.
Tests: `snapshots`, `ascii_charset_snapshots`.

## MMIND-5 Not drawn

Icons, class styles (Mermaid applies them only through CSS), Mermaid's `cose-bilkent` layout,
and frontmatter `config` for `mindmap` (`padding`, `maxNodeWidth`) are not used. Drawing edits are
read-only (LANGS-5).
