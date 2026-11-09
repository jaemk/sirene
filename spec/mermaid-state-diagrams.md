# Mermaid State Diagrams

`stateDiagram` and `stateDiagram-v2` (the same language), lowered to the graph model (LANGS-2).
Reference: <https://mermaid.js.org/syntax/stateDiagram.html>. Code: `crates/sirene-core/src/state/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/state.rs` and the
corpus under `crates/sirene-core/tests/fixtures/state/` (LANGS-6).

## MSTATE-1 Syntax

- States: `s1`, `state "Description" as s1`, `s1 : Description`, `state s1 : Description`; a
  description over several statements adds lines.
- Transitions: `s1 --> s2`, with an optional label `s1 --> s2 : label`.
- `[*]`: the start state as a transition's source, the end state as its target, one of each per
  composite state (and per region).
- Composite states: `state Name { ... }` and `state "Title" as Name { ... }`, nested; `--` inside
  one separates concurrent regions.
- Pseudo-states: `state f <<fork>>`, `<<join>>`, `<<choice>>`.
- Notes: `note left of S : text`, `note right of S` ... `end note`.
- `direction TB|BT|LR|RL` at the top or in a composite state.
- Styling: `classDef`, `class s1,s2 name`, `s1:::name`, `style` (as FLOWCH-12), applied once every
  state exists, so they may come before the state.
- Comments (`%%`), directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser is line-oriented and lossless: the tree's text is the source. A malformed statement is
an `ERROR` node and an error diagnostic, confined to its line. Tests: `snapshots`,
`syntax_errors`.

## MSTATE-2 Model

States are rounded rectangles labeled with their description, or their ID. `[*]` becomes a start
(small filled circle) or end (ringed circle) node scoped to its composite state or region; fork
and join are bars across the direction of flow; choice is a small diamond. A composite state is a
cluster titled with its name; its regions are untitled sub-clusters drawn with dashed borders and
no fill (LANGS-3). Transitions are edges with an arrow at the target and their label in the
middle. Notes are note nodes beside their state (LANGS-3). Tests: `states_and_transitions`,
`start_and_end_states`, `pseudo_states`, `regions`, `notes`, `direction_and_styling`, `metadata`.

## MSTATE-3 Layout and drawing

Layered layout (LAYOUT), top to bottom unless `direction` says otherwise. A transition from a
composite state's border (`Composite --> s`) connects to the cluster, as flowchart edges to
subgraphs do. In ASCII the start state is `●` (`*`), the end state `◉` (`@`), a choice `◇` (`?`),
and a bar is a row of `━` (`=`), or a column of `┃` (`|`) across a horizontal flow. A note is a box
with a `╮` corner joined by `┄`. Tests: `fork_bars_turn_with_the_direction`, `snapshots`,
`ascii_charset_snapshots`.

## MSTATE-4 Unsupported

`click`, floating notes, notes on composite states, other stereotypes (such as `<<history>>`,
drawn as a plain state), `--` outside a composite state, and transitions between states in two
regions of one composite state are warnings on their span (LANGS-6). Tests:
`unsupported_is_a_warning`.
