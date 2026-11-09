# Mermaid State Diagrams

`stateDiagram` and `stateDiagram-v2` (the same language), lowered to the graph model (LANGS-2).
Reference: <https://mermaid.js.org/syntax/stateDiagram.html>.

## MSTATE-1 Syntax

- States: `s1`, `state "Description" as s1`, `s1 : Description`; a description over several
  statements adds lines.
- Transitions: `s1 --> s2`, with an optional label `s1 --> s2 : label`.
- `[*]`: the start state as a transition's source, the end state as its target, one of each per
  composite state.
- Composite states: `state Name { ... }`, nested; `--` inside one separates concurrent regions.
- Pseudo-states: `state f <<fork>>`, `<<join>>`, `<<choice>>`.
- Notes: `note left of S : text`, `note right of S` ... `end note`.
- `direction TB|BT|LR|RL` at the top or in a composite state.
- Styling: `classDef`, `class s1,s2 name`, `s1:::name`, `style` (as FLOWCH-12).
- Comments (`%%`), `accTitle`, `accDescr`.

## MSTATE-2 Model

States are rounded rectangles labeled with their description, or their ID. `[*]` becomes a start
or end node scoped to its composite state; fork and join are bars across the direction of flow;
choice is a small diamond. A composite state is a cluster titled with its name; its regions are
sub-clusters separated by dashed lines (LANGS-3). Transitions are edges with an arrow at the
target and their label in the middle. Notes are note nodes beside their state (LANGS-3).

## MSTATE-3 Layout and drawing

Layered layout (LAYOUT), top to bottom unless `direction` says otherwise. A transition from a
composite state's border (`Composite --> s`) connects to the cluster, as flowchart edges to
subgraphs do. In ASCII the start state is `●` (`*`), the end state `◉` (`@`), and a bar is a row
of `━` (`=`).

## MSTATE-4 Unsupported

`click`, and transitions to states in another composite state's region, are warnings
(LANGS-6).
