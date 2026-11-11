# Mermaid Swimlane Diagrams

`swimlane-beta`, a flowchart whose top-level subgraphs are lanes (LANGS-2, LANGS-3, MCHART-1).
Reference: Mermaid 12.3.0's `docs/syntax/swimlanes.md`, `diagrams/swimlanes/`, and
`rendering-util/layout-algorithms/swimlanes/helpers.ts`. Code: the flowchart parser and lowering
(`crates/sirene-core/src/flowchart/`) and LAYOUT-12; `diagram::Kind::Swimlane` picks it by the
header keyword. Tests: `crates/sirene-core/tests/swimlane.rs` and the corpus under
`crates/sirene-core/tests/fixtures/swimlane/` (LANGS-6).

## MSWIM-1 Syntax

The flowchart grammar (FLOWCH-*) with `swimlane-beta` as the header keyword, followed by an
optional direction. The parser is lossless. Tests: `snapshots`, `header`.

## MSWIM-2 Lanes

Lowering is the flowchart's (FLOWMO-*), with `Flowchart::lanes` set. Then:

- A top-level subgraph is a lane, in declaration order.
- A lane is dropped when a node it listed belongs to an earlier subgraph, it has no members
  left, and no edge names it. A lane declared empty stays.
- A subgraph nested in a lane is a plain box, with a warning.
- Nodes outside every subgraph go in an untitled last lane, `__swimlane_default__`.

Tests: `lanes`, `nested`.

## MSWIM-3 Drawing

Laid out with lanes (LAYOUT-12) and drawn as a flowchart (DISPLA, ASCIIR): lanes are subgraph
boxes sharing borders, titled on their top border. An edge to or from a lane is not drawn.
Tests: `placement`, `edges_to_lanes`, `snapshots`.

## MSWIM-4 Not drawn

Mermaid's lane palette colors, `redux-color` theme, and `neo` look. Mermaid ranks each lane
without the edges between lanes and can reorder lanes (`automaticLaneOrdering`); sirene ranks by
every edge and keeps the declared order. Drawing edits are read-only (LANGS-5).
