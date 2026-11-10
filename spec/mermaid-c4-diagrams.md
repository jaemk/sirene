# Mermaid C4 Diagrams

`C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, and `C4Deployment`, lowered to their own
model over the flowchart model (LANGS-2). Reference: <https://mermaid.js.org/syntax/c4.html>.
Code: `crates/sirene-core/src/c4/`; `diagram::lower` picks it by the header keyword. Tests:
`crates/sirene-core/tests/c4.rs` and the corpus under `crates/sirene-core/tests/fixtures/c4/`
(LANGS-6).

## MC4-1 Syntax

- Headers: `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment`.
- Each statement is a macro call `Name(arg, ...)`; an argument is quoted or bare text, and may be
  named as `$key=value`. A call followed by `{` on the same line opens a block closed by `}`.
- `title`, `accTitle`, `accDescr`, comments, directives, and frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. An unclosed
string or argument list, a missing `,` between arguments, a `$key` without `=`, an unclosed or
stray brace, and a line that is none of the above are errors. Tests: `syntax_errors` and the
parser's unit tests.

## MC4-2 Model

`c4::C4`: a `Flowchart` holding the elements (nodes), boundaries (subgraphs), relations (edges),
and title; `elements`, each element's base (`Person`, `System`, `Container`, `Component`), body
(box, `Db`, `Queue`), `_Ext`, technology, description, and colors; `node_parent`; `boundaries`,
each boundary's kind and whether it is a deployment node; `boundary_parent`; `rels`, each
relation's technology, description, and colors; and `shapes_in_row` and `boundaries_in_row`.

Arguments are read by position or name per macro as Mermaid reads them. `Boundary` takes its
kind from its `type` argument; `Enterprise_Boundary`, `System_Boundary`, and
`Container_Boundary` are `ENTERPRISE`, `SYSTEM`, and `CONTAINER`; `Deployment_Node`, `Node`,
`Node_L`, and `Node_R` are deployment nodes. `Rel_Back` swaps its ends, `BiRel` has arrows at
both, and `Rel_U`/`Rel_D`/`Rel_L`/`Rel_R`/`RelIndex` are `Rel`. Relations and style updates
resolve after every declaration, so they may come first.

An id declared twice, a macro without an alias, and a relation or style to an unknown id are
errors and skipped. A block after an element is a warning and its contents are placed outside.
Unsupported macros (tags, `Lay_*`, and the like), a row count that is not a whole number of at
least 1, a style on a boundary, and a relation style with no such relation are warnings. Tests:
`model`, `errors`.

## MC4-3 Drawing

`c4::layout` places each boundary, and the diagram, as Mermaid does: elements in rows of
`shapes_in_row` from the left, then child boundaries below them in rows of `boundaries_in_row`,
rows top-aligned; a boundary is its content with padding and its title. The gap between columns
is 50 pixels, or wide enough for the widest label of a relation between two elements in one row.

An element is a 216-pixel-wide box with `<<type>>`, a person icon for people, the label in bold,
`[technology]` in italics, and the wrapped description, in Mermaid's colors for its base and
`_Ext`; `Db` is an upright cylinder and `Queue` a lying one. A boundary is a box with no fill,
dashed unless it is a deployment node, titled at its top left with its label, `[kind]`, and
description. Relations are straight lines between the boxes with their label and
`[technology]`, drawn over the elements. `UpdateElementStyle` and `UpdateRelStyle` colors override the defaults. Elements
are owned by their node, subgraph, or edge. Tests: `drawing` and the layout's unit tests.

## MC4-4 ASCII

The same rows in cells: an element is a rounded box of `<<type>>`, label, `[technology]`, and
the description wrapped at 30 columns; a cylinder for `Db`; a boundary is a box titled
`label [KIND]` on its top border. The relations are listed below the drawing as
`a --> b: label [technology]` (`<-->` for `BiRel`). The title is centered above. Tests:
`snapshots`, `ascii_charset_snapshots`, and the renderer's unit tests.

## MC4-5 Not drawn

Tags, sprites, links, relation offsets, and relation directions are ignored; boundary styles are
not drawn. Theme variables are ignored. Drawing edits are read-only (LANGS-5).
