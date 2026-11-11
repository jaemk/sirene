# Mermaid Use Case Diagrams

`usecase-beta`, lowered to the graph model (LANGS-2, LANGS-3, MCHART-1). Reference: Mermaid
12.3.0's `docs/syntax/usecase.md`. Code: `crates/sirene-core/src/usecase/`; `diagram::lower`
picks it by the header keyword. Tests: `crates/sirene-core/tests/usecase.rs` and the corpus under
`crates/sirene-core/tests/fixtures/usecase/` (LANGS-6).

## MUSE-1 Syntax

- Header: `usecase-beta`. Each statement is one line, except for Markdown strings, metadata
  blocks, JSON objects, `accDescr { ... }`, and `systemBoundary` blocks. `;` does not separate
  statements, and anything after a complete statement on its line is an error.
- Ids are `\w+` (ASCII). Names are an id, a `"..."` or `'...'` string on one line, or a
  `"`...`"` Markdown string. An id takes an optional `(label)` or `[label]`; unquoted label text
  stops at `()[]{}"'` and at `--`, `<--`, `..>`, `:::`, `@{`, and `<<`.
- After a name, in order and each optional: `@{ ... }` metadata, `<<stereotype>>`, and
  `:::a,b` classes (every name after a comma is a class).
- `actor a, b` declares actors; `actor a --> b` declares `a` and relates it.
- `systemBoundary name ... end` holds only actor and use case declarations, one level deep.
- `note for id text`, `json id@{ ... }:::classes`, `name@{ ... }` on its own line, `classDef`,
  `class a,b c,d`, `style id css`, `direction`, `accTitle`, `accDescr`, `%%` comments at the
  start of a line, and directives.
- `allowmixing`, `newpage`, `package`, `rectangle`, and `skinparam` are PlantUML statements and
  errors.

The parser is lossless. Tests: `snapshots` and the parser's unit tests.

## MUSE-2 Elements

An actor is declared only by `actor`. A standalone name declares a use case, as does an endpoint
with a shape, metadata, or a stereotype; an endpoint declared nowhere is an ellipse use case. A
quoted name's id is its text with each character outside `[A-Za-z0-9_]` replaced by `_`. Nodes
are ordered actors first, then use cases, then JSON tables, each by first mention. The default
direction is `LR`. Tests: `declarations`.

## MUSE-3 Metadata

An actor takes `type: normal|hollow|awesome`, `icon`, and `business`; a use case takes
`business`; a boundary takes `type: rect|package`; an edge id takes `animate` and
`animation: fast|slow` (which also animates). A stereotype is shown as `«X»` above the label. An
icon with a non-normal type, a business awesome or icon actor, a business rectangle, and any
other key are warnings. Tests: `metadata`, `edge_ids`, `errors`.

## MUSE-4 Relationships

| Operator | Drawn as |
|---|---|
| `-->`, `<--` | an arrowhead at the target, or at the source |
| `--` | no marker |
| `--o`, `o--`, `--x`, `x--` | a circle or cross at the target, or at the source |
| `-- label -->`, `-- label --`, `-- label --o`, `-- label --x`, `<-- label --`, `o-- label --`, `x-- label --` | the same, labelled |
| `..> : include`, `..> : extend` | dotted, an arrowhead, labelled `«include»` or `«extend»` |
| `--\|>` | a hollow triangle at the target |

Edges run from source to target. Each dash past two in `-->`, `<--`, and `--` (the right
operator in a labelled form) adds one to the edge's minimum length. `include` and `extend` join
two use cases, a generalization two actors or two use cases, and a JSON table takes only `-->`,
`<--`, and `--`; otherwise a warning. `:::` on an undeclared endpoint is a warning. Tests:
`relationships`, `errors`.

## MUSE-5 Notes and JSON tables

A note is a flowchart note beside its actor or use case; a note for anything else is a warning.
A JSON table is a rectangle titled by its id, with one two-column row per leaf of its strict JSON
object: nested keys as `a.b` and `items[0].name`, a list of values with the key on its first row
only, `{}` and `[]` for empty ones, JSON spelling for `null`, booleans, and numbers, and the last
value of a repeated key in its first place. Invalid JSON is a warning. Tests:
`notes_tables_boundaries`.

## MUSE-6 System boundaries

A boundary is a subgraph titled by its label, holding the elements declared in it; an element is
in at most one. `type: package` is kept as subgraph metadata and drawn with a tab around the
title. Tests: `notes_tables_boundaries`.

## MUSE-7 Styling

`:::`, `class`, and `style` apply to actors, use cases, boundaries, JSON tables, and edges with
ids; an unknown target is a warning. Tests: `styling`, `edge_ids`.

## MUSE-8 Drawing

Drawn, laid out, and rendered in ASCII as the graph model is (DISPLA, ASCIIR). Actors are
Mermaid's stick, hollow, and awesome figures with the label below; edges meet the figure.
Business elements get Mermaid's slash. In ASCII an actor is a box and a use case a rounded box.
Tests: `snapshots`, `ascii_charset_snapshots`.

## MUSE-9 Not drawn

Icon actors are stick figures. Mermaid 12's `redux-color` theme, `neo` look, ELK layout, the
`usecase` frontmatter config, and its color schemes are not used. Notes are placed beside their
target rather than as ordinary layout nodes. Drawing edits are read-only (LANGS-5).
