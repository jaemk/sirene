# Mermaid Wardley Maps

`wardley-beta`: components placed by visibility (up) and evolution (across), with links,
evolve trends, pipelines, and notes. Reference: <https://mermaid.js.org/syntax/wardley.html> and
Mermaid 12.3.0's `diagrams/wardley` (grammar, `wardleyBuilder.ts`, `wardleyRenderer.ts`). Code:
`crates/sirene-core/src/wardley/`; `diagram::lower` picks it by the header keyword. Tests:
`crates/sirene-core/tests/wardley.rs` and the corpus under
`crates/sirene-core/tests/fixtures/wardley/` (LANGS-6).

## MWARD-1 Syntax

- Header: `wardley-beta`.
- `title text`, `size [width, height]`, `accTitle`, `accDescr`.
- `evolution A -> B -> C ...`: two or more stages, each a name with an optional `@position`
  and an optional `/ second name` (in either order).
- `anchor name [visibility, evolution]` and `component name [visibility, evolution]`, a
  component followed in any order by `label [x, y]` and the decorators `(build)`, `(buy)`,
  `(outsource)`, `(market)`, `(inertia)`, or a bare `inertia`.
- Links: `A -> B`, `A -.-> B`, flows `A +> B`, `A +< B`, `A +<> B`, labeled flows
  `A +'text'> B` (also `<` and `<>`), each with an optional `; label`.
- `evolve name evolution`, `pipeline name { component name [evolution] ... }`.
- `note "text" [visibility, evolution]`, `annotations [visibility, evolution]`,
  `annotation N,[visibility, evolution] "text"`, `accelerator "text" [...]`,
  `deaccelerator "text" [...]`.
- Names are words with spaces, `-`, `&`, and parentheses, or `"quoted strings"`. Coordinates
  from 0 to 1 are fractions; above 1, percentages up to 100.
- Mermaid comments (also after a statement) and directives.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests: the
parser's unit tests.

## MWARD-2 Model

`wardley::parse(src).lower()` gives a `wardley::Wardley` with nodes, links, trends, pipelines,
notes, annotations, and accelerators, built in Mermaid's order: anchors, components, notes,
pipelines, links, evolves, the first annotations box, annotations, accelerators,
deaccelerators. A node declared twice is merged. A pipeline component's id is
`parent_component` and takes its parent's visibility. Links resolve a name by id, then by
label. Tests: `model`, `pipelines`.

## MWARD-3 Problems

A coordinate over 100 is an error. A pipeline for a component not declared before it is an
error. A link or `evolve` naming an unknown component is a warning, and it is left out. Tests:
`problems`.

## MWARD-4 Drawing

As Mermaid's renderer draws it at 900 by 600 (or `size`) with 48 padding: the axes with
`Evolution` below and `Visibility` above the y axis (Mermaid turns it up the axis), the stages
spaced evenly or at their `@` positions with dashed lines between; components as circles of
radius 6, anchors as bold text, pipeline parents as squares above a rounded box around their
components joined by dashed lines; links stopping at the circles, dashed when `-.->`, with
arrowheads for flows and level labels (Mermaid turns them along the link); `evolve` as a red
dashed arrow; sourcing as a ring of radius 12 (`outsource` #666, `buy` #ccc, `build` #eee),
`market` as three linked dots; inertia as a bar right of the node; numbered annotation circles
and a box listing their texts; notes in bold, kept inside the chart; accelerators and
deaccelerators as arrows with their text below.

Mermaid's stylesheet fills every sourcing ring white, so build, buy, and outsource look the same
there; here they keep the renderer's greys. Tests: `drawing`, `snapshots`, and the unit tests in
`scene/wardley.rs`.

## MWARD-5 Text

The title, then the components plotted over the stages by name, with strategy and inertia in
parentheses, or by a key listed below where a name does not fit; anchors are `◆` (`@`),
pipeline parents `■` (`#`) a row above their components. Then the links, evolve trends,
pipelines, notes, annotations, and accelerators as lists. Tests: `snapshots`.

## MWARD-6 Not drawn

The hand-drawn look, Mermaid 12's `neo` look, and the `wardley` config and theme variables are
not read. Drawing edits are read-only (LANGS-5).
