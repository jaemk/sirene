# sirene

sirene is a local WYSIWYG editor and viewer for [Mermaid](https://mermaid.js.org),
[PlantUML](plantuml.md), and [Graphviz DOT](dot.md) diagrams, with SVG, PNG, and terminal ASCII
output. It is one Rust binary: no browser, no Node, no Java, no network.

```
$ printf 'flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n' | sirene ascii
┌───────┐    ╱─────╲       ┌──────┐
│ Start ├───►│ Ok? ├──yes─►│ Done │
└───────┘    ╲─────╱       └──────┘
```

- `sirene FILE` opens an editor window: the source on the left, a live drawing on the right.
  Edits on either side are written to the source as ordinary text edits.
- `sirene svg`, `sirene png`, and `sirene ascii` render without opening a window, one file, a
  directory of diagrams, a Markdown file's diagram blocks, or stdin, and can watch for changes.
- The parser keeps every byte of the source, so the editor never reformats a file. Diagnostics
  point at a line and column, and a diagram with errors is still drawn.

## Diagrams

Mermaid:

- [Flowcharts](syntax.md) (`flowchart` and `graph`), [swimlane diagrams](swimlane.md)
  (`swimlane-beta`), and [agentflow diagrams](agentflow.md) (`agentflow-beta`)
- [Sequence diagrams](sequence.md) (`sequenceDiagram`) and [ZenUML diagrams](zenuml.md)
  (`zenuml`)
- [State diagrams](state.md) (`stateDiagram-v2`), [class diagrams](class.md) (`classDiagram`),
  [ER diagrams](er.md) (`erDiagram`), [requirement diagrams](requirement.md)
  (`requirementDiagram`), and [use case diagrams](usecase.md) (`usecase-beta`)
- [Block diagrams](block.md) (`block-beta`), [architecture diagrams](architecture.md)
  (`architecture-beta`), and [C4 diagrams](c4.md) (`C4Context` and the other C4 types)
- [Git graphs](git.md) (`gitGraph`), [timelines](timeline.md) (`timeline`),
  [gantt charts](gantt.md) (`gantt`), and [event modeling diagrams](eventmodel.md)
  (`eventmodeling`)
- [Pie charts](pie.md) (`pie`), [XY charts](xychart.md) (`xychart-beta`),
  [quadrant charts](quadrant.md) (`quadrantChart`), [radar charts](radar.md) (`radar-beta`),
  [sankey diagrams](sankey.md) (`sankey-beta`), [treemaps](treemap.md) (`treemap-beta`), and
  [venn diagrams](venn.md) (`venn-beta`)
- [Mindmaps](mindmap.md) (`mindmap`), [tree views](treeview.md) (`treeView-beta`),
  [ishikawa diagrams](ishikawa.md) (`ishikawa-beta`), [kanban boards](kanban.md) (`kanban`),
  [user journeys](journey.md) (`journey`), [Wardley maps](wardley.md) (`wardley-beta`), and
  [Cynefin diagrams](cynefin.md) (`cynefin-beta`)
- [Packet diagrams](packet.md) (`packet`), [railroad diagrams](railroad.md)
  (`railroad-ebnf-beta` and the other notations), and [info diagrams](info.md) (`info`)

Other Mermaid diagram types are reported as unsupported. See [Supported syntax](syntax.md) for
what is read and how it differs from Mermaid.

[PlantUML](plantuml.md) (`@startuml` and the other `@start` kinds): sequence, activity, class,
object, state, use case, component, deployment, timing, network, mindmap, WBS, gantt, JSON,
YAML, EBNF, regular expression, Chen ER, chronology, and file tree diagrams.

[Graphviz DOT](dot.md) (`digraph` and `graph`): records, HTML table labels, clusters, and ports.

Start with [Install](install.md) and the [Quickstart](quickstart.md).
