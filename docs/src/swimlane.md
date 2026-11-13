# Swimlane diagrams

sirene draws Mermaid's swimlane diagrams (`swimlane-beta`) as images and as text: a flowchart
whose top-level subgraphs are lanes, side by side across the flow.

```mermaid
swimlane-beta LR
  subgraph buyer [Buyer]
    choose([Choose product])
    pay[Pay invoice]
  end

  subgraph store [Store]
    reserve[Reserve stock]
    ship((Ship))
  end

  choose --> reserve
  reserve -.->|Invoice ready| pay
  pay ==> ship
  ship --> audit[Audit]
```

Everything else is flowchart syntax: see [Syntax](syntax.md). See
`crates/sirene-core/tests/fixtures/swimlane/` for more. In the editor a swimlane diagram is
read-only on the drawing side.

## Lanes

- The header direction (`TB` when there is none) is the flow; lanes run along it, in the order
  they are declared.
- Nodes outside every lane share an untitled last lane.
- A node listed in two lanes stays in the first, with a warning; the second lane is dropped if
  that leaves it empty and no edge names it.
- A subgraph inside a lane is drawn as a plain box, with a warning.

## Not drawn

An edge to or from a lane itself. Mermaid's lane colors and its `redux-color` theme and `neo`
look are not used. Ranks follow every edge, so a diagram can be longer along the flow than
Mermaid's, which ranks each lane apart from the edges between lanes.
