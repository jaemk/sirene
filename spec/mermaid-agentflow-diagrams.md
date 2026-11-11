# Mermaid Agentflow Diagrams

`agentflow-beta`, a flowchart of `flow` containers with a reduced set of shapes and edges
(LANGS-2, LANGS-3, MCHART-1). Reference: Mermaid 12.3.0's `docs/syntax/agentflow.md` and
`diagrams/agentflow/`. Code: the flowchart parser and lowering
(`crates/sirene-core/src/flowchart/`, with the shape vocabulary in `flowchart/agentflow.rs`);
`diagram::Kind::Agentflow` picks it by the header keyword. Tests:
`crates/sirene-core/tests/agentflow.rs` and the corpus under
`crates/sirene-core/tests/fixtures/agentflow/` (LANGS-6).

## MAGF-1 Syntax

The flowchart grammar (FLOWCH-*) with `agentflow-beta` as the header keyword, followed by an
optional direction, and `Flowchart::agentflow` set. The parser is lossless. Tests: `snapshots`,
`header`.

## MAGF-2 Containers

- `flow id["Title"]`, `flow id`, and a bare `flow`, each optionally followed by `@{ ... }`
  metadata and closed by `end`, are subgraphs (FLOWCH-10). An untitled flow has no title.
  `@{ view: collapsed }` collapses it (FLOWMO-15). `subgraph` is an error.
- `global ... end` holds statements whose nodes belong to no flow; it draws nothing.
- `connector id["Title"]` declares a rounded node labeled by its title, or its id without one.
  Any other bracket is an error.

A flow is drawn unfilled with rounded corners (radius 10) and a 0.75 stroke; a collapsed flow
is drawn the same way around its title, rule, and dots. Tests: `containers`, `connectors`.

## MAGF-3 Edges

`-->` and `--x` (any length), `-.-` (any number of dots), the text forms `-- text -->` and
`-- text --x`, and `|text|` labels. Any other edge is an error: `` `==>` is not an agentflow
edge; use `-->`, `--x`, or `-.-` ``. Tests: `edges`.

## MAGF-4 Shapes

A node with no brackets is a rounded rectangle, as are `[...]` and `(...)`. `{...}`, `{{...}}`,
`[[...]]`, and `[/.../]` keep their flowchart shapes; any other bracket is an error and draws
rounded. `@{ shape }` takes agentflow's aliases:

| Names | Shape |
|-------|-------|
| `task`, `round`, `rect`, `squareRect`, `roundedRect`, `connector` | rounded rectangle |
| `tool`, `subroutine`, `subprocess`, `subproc`, `framed-rectangle` | framed rectangle |
| `input`, `lean_right`, `lean-right` | lean right |
| `decision`, `diamond` | diamond |
| `refdoc`, `lin-doc`, `lined-document` | lined document |
| `action`, `hexagon`, `hex` | hexagon |

A shape agentflow removed (`doc`, `stadium`, `circle`, `cylinder`, and the rest of Mermaid's
list) is an error, another Mermaid shape a warning, and both draw rounded. An unknown name is
an error. Tests: `shapes`.

## MAGF-5 Not drawn

Mermaid's agentflow theme colors and `neo` look. Drawing edits are read-only (LANGS-5).
