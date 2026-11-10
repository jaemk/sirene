# sirene

sirene is a local WYSIWYG editor and viewer for [Mermaid](https://mermaid.js.org) flowcharts,
with SVG, PNG, and terminal ASCII output. It is one Rust binary: no browser, no Node, no network.

```
$ printf 'flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n' | sirene ascii
┌───────┐    ╱─────╲       ┌──────┐
│ Start ├───►│ Ok? ├──yes─►│ Done │
└───────┘    ╲─────╱       └──────┘
```

- `sirene FILE` opens an editor window: the source on the left, a live drawing on the right.
  Edits on either side are written to the source as ordinary text edits.
- `sirene svg`, `sirene png`, and `sirene ascii` render without opening a window, one file, a
  directory of diagrams, or stdin, and can watch for changes.
- The parser keeps every byte of the source, so the editor never reformats a file. Diagnostics
  point at a line and column, and a diagram with errors is still drawn.

Flowcharts (`flowchart` and `graph`), [sequence diagrams](sequence.md) (`sequenceDiagram`),
[state diagrams](state.md) (`stateDiagram-v2`),
[class diagrams](class.md) (`classDiagram`), [ER diagrams](er.md) (`erDiagram`),
[git graphs](git.md) (`gitGraph`), and [timelines](timeline.md) (`timeline`) are drawn; other Mermaid diagram types are reported
as unsupported. See [Supported
syntax](syntax.md) for what is read and how it differs from Mermaid.

Start with [Install](install.md) and the [Quickstart](quickstart.md).
