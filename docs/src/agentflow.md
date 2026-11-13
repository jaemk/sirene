# Agentflow diagrams

sirene draws Mermaid's agentflow diagrams (`agentflow-beta`) as images and as text: a flowchart
of `flow` containers with agentflow's shapes and edges.

```mermaid
agentflow-beta LR
  global
    user["User"]
  end

  flow plan["Planner"]
    goal@{ shape: input, label: "Read the goal" }
    split{"Split into tasks?"}
  end

  flow work["Workers"]
    search@{ shape: tool, label: "Web search" }
    notes@{ shape: refdoc, label: "Notes" }
    act@{ shape: action, label: "Draft answer" }
  end

  connector bus["Message bus"]

  user --> goal
  goal --> split
  split -- yes --> search
  split -->|no| act
  search --> notes
  notes -.- bus
  bus --> act
  act --x user
```

Everything else is flowchart syntax: see [Syntax](syntax.md). See
`crates/sirene-core/tests/fixtures/agentflow/` for more. In the editor an agentflow diagram is
read-only on the drawing side.

## Containers

- `flow id["Title"] ... end` groups nodes; `flow id` and a bare `flow` are untitled. Flows nest,
  and `flow id["Title"]@{ view: collapsed }` draws one as a single node.
- `global ... end` holds nodes that belong to no flow.
- `connector id["Title"]` declares a rounded node; without a title its label is its id.
- `subgraph` is an error.

## Shapes and edges

- A node is a rounded rectangle unless its brackets or `@{ shape }` say otherwise. `{}`, `{{}}`,
  `[[]]`, and `[/ /]` keep their flowchart shapes.
- `@{ shape }` takes `task`, `tool`, `input`, `decision`, `refdoc`, and `action`, and their
  flowchart names. A shape agentflow removed (e.g. `cylinder`) is an error and another Mermaid
  shape (e.g. `cloud`) a warning; both draw rounded.
- Edges are `-->`, `--x`, and `-.-`, with `-- text -->` or `|text|` labels. Any other edge is an
  error.

## Not drawn

Mermaid's agentflow theme colors and `neo` look.
