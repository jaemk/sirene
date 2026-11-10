# Flowchart Parser

Parses Mermaid flowchart source into a lossless concrete syntax tree (CST). The semantic model
built from the tree is specified in `flowchart-model.md`.

Reference grammar: mermaid `packages/mermaid/src/diagrams/flowchart/parser/flow.jison` and the
preprocessing in `diagram-api/comments.ts` and `diagram-api/frontmatter.ts`.

## FLOWCH-1 Lossless round-trip

For every input string, including invalid input, the concatenated text of the tree's tokens is
byte-identical to the input. The parser never panics.

## FLOWCH-2 Error recovery

Invalid input produces a tree plus a list of diagnostics, each with a byte range and a message.
An error inside a statement is confined to that statement: the parser skips to the next
statement separator (newline or `;`), wraps the skipped text in an `ERROR` node, and continues.
Statements after the error parse normally.

## FLOWCH-3 Trivia

- A comment is a line whose first non-whitespace characters are `%%` and is not a directive. It
  runs to the end of the line. `%%` after other content on a line is not a comment (matching
  mermaid, which only strips whole-line comments).
- A directive starts with `%%{` and ends at the next `}%%`, and may span lines.
- Frontmatter is a block at byte 0 that opens with a `---` line and closes with the next `---`
  line. It is a single token; its YAML body is interpreted by the model.
- Whitespace, comments, and directives may appear between any two statements.

## FLOWCH-4 Header

The first statement is `flowchart`, `graph`, or `flowchart-elk`, optionally followed by
whitespace and a direction: `TB`, `TD`, `BT`, `RL`, `LR`, or the aliases `>` (LR), `<` (RL),
`^` (BT), `v` (TB). The header ends at a newline or `;`. Any other first statement is an error
(`expected flowchart or graph header`), except another Mermaid diagram type (FLOWCH-16).

## FLOWCH-5 Statements

Statements are separated by newlines or `;`. The statement forms are:

- vertex statement (FLOWCH-6)
- `subgraph` ... `end` (FLOWCH-10)
- `direction <dir>` (FLOWCH-11)
- `classDef`, `class`, `style`, `linkStyle` (FLOWCH-12)
- `click` (FLOWCH-13)
- `accTitle: ...`, `accDescr: ...`, `accDescr { ... }` (FLOWCH-14)

Keywords are recognized only as whole words at the start of a statement, so `classes` or
`endpoint` are node ids. This is more permissive than mermaid, which rejects some of these.

## FLOWCH-6 Vertex statements

A vertex statement is a chain: `group (edge group)*`, where a group is one or more nodes joined
by `&`, e.g. `A & B --> C & D -- text --> E`.

## FLOWCH-7 Nodes

A node is an id followed by optional parts, in order: a shape with label, a `:::class`
shorthand, and an `@{ ... }` metadata block.

- Id characters: Unicode letters and digits, `_`, `.`, `!`, `$`, `'`, `*`, `+`, `?`, `/`,
  `\`, backtick; `-` when not followed by `-`, `.`, or `>`; `=` when not followed by `=`.
- `end` as a node id is an error (reserved word in mermaid).

Shapes, by delimiter pair:

| Delimiters | Shape |
|---|---|
| `[ ]` | rect |
| `( )` | round |
| `([ ])` | stadium |
| `[[ ]]` | subroutine |
| `[( )]` | cylinder |
| `(( ))` | circle |
| `((( )))` | double circle |
| `(- -)` | ellipse |
| `> ]` | odd (asymmetric) |
| `{ }` | diamond |
| `{{ }}` | hexagon |
| `[/ /]` | lean right |
| `[\ \]` | lean left |
| `[/ \]` | trapezoid |
| `[\ /]` | inverted trapezoid |

## FLOWCH-8 Labels

A label is one of:

- unquoted text: any characters except `[ ] ( ) { } | "`, may span lines;
- a string: `"..."`, any characters except `"`, may span lines;
- a markdown string: `` "`...`" ``.

Entity codes (`#quot;`, `#9829;`) are kept verbatim in the tree and decoded by the model.

An unclosed shape whose closing delimiter does not appear before end of input ends at the end
of its line, with a diagnostic. This keeps a half-typed label from swallowing the rest of the
document.

## FLOWCH-9 Edges

An edge is an optional edge id (`id@`), a stroke, and an optional label.

| Stroke | Forms |
|---|---|
| normal | `---`, `-->`, `--x`, `--o`, extra `-` for length |
| thick | `===`, `==>`, `==x`, `==o`, extra `=` for length |
| dotted | `-.-`, `-.->`, `-.-x`, `-.-o`, extra `.` for length |
| invisible | `~~~`, extra `~` for length |

A start marker `<`, `x`, or `o` makes the edge bidirectional (`<-->`, `x--x`, `o--o`). An `x`
or `o` start marker must be separated from the preceding node by whitespace; otherwise it is part
of the node id.

Labels: `-->|text|`, or inline `-- text -->`, `== text ==>`, `-. text .->`. Inline label text
ends at the first closing stroke and must be on one line (mermaid allows newlines here; sirene
does not, so a missing closer cannot swallow later statements). Quoted inline labels may contain
stroke characters.

## FLOWCH-10 Subgraphs

`subgraph` followed by an optional header, statements, and `end`. Header forms:

- `subgraph` (no id, no title)
- `subgraph title text` (id and title from the same text)
- `subgraph "quoted title"`
- `subgraph id[title]` or `subgraph id [title]`

Subgraphs nest. A subgraph missing its `end` is closed at end of input with a diagnostic. An
`end` with no open subgraph is an error.

## FLOWCH-11 Direction

`direction` followed by `TB`, `TD`, `BT`, `RL`, or `LR`.

## FLOWCH-12 Styling statements

- `classDef <names> <styles>`: names are comma separated.
- `class <ids> <className>`: ids are comma separated.
- `style <id> <styles>`.
- `linkStyle <default | n,m,...> [interpolate <curve>] [<styles>]`.

`<styles>` is raw text to the end of the statement, split into comma-separated declarations by
the model (`\,` escapes a comma).

## FLOWCH-13 Click

`click <id>` followed by one of:

- `<callback>` or `call <callback>(<args>)`, optional `"tooltip"`;
- `href "<url>"` or `"<url>"`, optional `"tooltip"`, optional target (`_self`, `_blank`,
  `_parent`, `_top`).

## FLOWCH-14 Accessibility

`accTitle: <text>` and `accDescr: <text>` run to the end of the line. `accDescr { ... }` runs to
the next `}` and may span lines.

## FLOWCH-15 Metadata blocks

`<id>@{ ... }` attaches metadata to a node, edge, or subgraph. It is valid after a node in a
vertex statement and as a statement of its own (e.g. `e1@{ animate: true }`). The body runs to
the first `}` outside double-quoted strings (bodies do not nest) and may span lines. Its content
is interpreted by the model. An unclosed block ends at the end of its line, with a diagnostic.

## FLOWCH-16 Other diagram types

A document whose first statement starts with another Mermaid diagram keyword (`sequenceDiagram`,
`classDiagram`, `stateDiagram`, `erDiagram`, `gantt`, `pie`, `gitGraph`, `mindmap`, `timeline`,
and the rest of mermaid's types, with their `-v2` and `-beta` forms) is one `ERROR` node from that
keyword to the end, with one diagnostic on the keyword, e.g.
`` `venn-beta` diagrams are not supported; sirene draws flowchart, sequence, state, class, ER, git graph, timeline, pie, gantt, mindmap, journey, quadrant, XY, requirement, kanban, packet, radar, sankey, treemap, block, architecture, C4, ZenUML, and info diagrams `` (the list
is `diagram::DRAWN`). Nothing of it is drawn. A type sirene draws never reaches this parser:
`diagram::lower` sends it to its own (MSTATE-1, MCLASS-1, MER-1, MGIT-1, TIMELI-1, MPIE-1,
MGANTT-1, MMIND-1, MJOUR-1, MQUAD-1, MXY-1, MREQ-1, MKAN-1, MPKT-1, MRADAR-1, MSANKEY-1, MTREE-1, MINFO-1). Tests: `other_diagram_types_are_one_error` in
`crates/sirene-core/tests/flowchart_model.rs`.
