# Mermaid Charts

Status: research. The Mermaid types that are not covered by the state, class, ER, and sequence
specs. Each needs its own model and scene builder (LANGS-2); some fit the graph model. Reference:
<https://mermaid.js.org/intro/syntax-reference.html>.

## MCHART-1 Grouping

| Type | Model | Notes |
|---|---|---|
| `requirementDiagram` | graph | Requirements and elements as compartment boxes, labeled relationships: `mermaid-requirement-diagrams.md`. |
| `C4Context` (and the other C4 types) | graph | People, systems, containers, and components in nested boundaries, placed in rows: `mermaid-c4-diagrams.md`. |
| `architecture-beta` | graph | Services with icons in groups, placed on a grid by their edge sides: `mermaid-architecture-diagrams.md`. |
| `block-beta` | graph | Blocks in a grid of columns, drawn with the flowchart shapes: `mermaid-block-diagrams.md`. |
| `mindmap` | tree | A two-sided tree out from the root; ASCII is an outline: `mermaid-mindmaps.md`. |
| `pie` | chart | Slices with labels and a legend; ASCII is a table with percentage bars: `mermaid-pie-charts.md`. |
| `quadrantChart` | chart | Points in four labeled quadrants; ASCII plots them by name: `mermaid-quadrant-charts.md`. |
| `xychart-beta` | chart | Bars and lines over category or numeric axes; ASCII is a row per position: `mermaid-xy-charts.md`. |
| `radar-beta` | chart | Axes around a circle, and a closed curve per series; ASCII is a bar per axis and series: `mermaid-radar-charts.md`. |
| `sankey-beta` | chart | Flows between nodes in columns, each band as wide as its value; ASCII lists each node's outgoing flows: `mermaid-sankey-diagrams.md`. |
| `treemap-beta` | chart | Nested rectangles, each as large as its value; ASCII is an outline with a bar per item: `mermaid-treemaps.md`. |
| `packet` | chart | Rows of bit fields; ASCII is the RFC header layout: `mermaid-packet-diagrams.md`. |
| `gantt` | chart | Dates, durations, dependencies (`after`), sections, and a time axis; ASCII is a bar per task: `mermaid-gantt-charts.md`. |
| `timeline`, `journey` | timeline | Sections along a line (`timeline`: `mermaid-timelines.md`); journey adds scores and actors: `mermaid-journeys.md`. |
| `gitGraph` | timeline | Commits on branch lanes, merges, and cherry-picks: `mermaid-git-graphs.md`. |
| `zenuml` | sequence | Calls, replies, and fragments in ZenUML's syntax, lowered to the sequence model: `mermaid-zenuml-diagrams.md`. |
| `info` | text | sirene's version, where Mermaid draws its own: `mermaid-info-diagrams.md`. |
| `ishikawa-beta` | tree | A fishbone: the effect at the end of a spine, causes on bones; ASCII is an outline: `mermaid-ishikawa-diagrams.md`. |
| `venn-beta` | circles | Sets as circles sized and overlapped by their sizes; ASCII lists each set and overlap: `mermaid-venn-diagrams.md`. |
| `kanban` | board | Columns of cards, with the mindmap grammar; ASCII is a box per column: `mermaid-kanban-boards.md`. |

Mermaid 12 also adds `wardley-beta`, `treeView-beta`, `usecase-beta`,
`railroad-beta` (and its `-ebnf`, `-abnf`, and `-peg` forms), `swimlane-beta`, `eventmodeling`,
`cynefin-beta`, and `agentflow-beta`. Until each is built it is FLOWCH-16's "not supported"
error.

## MCHART-2 Open questions

- Which types to build. Usage suggests `gantt`, `pie`, `mindmap`, and `gitGraph` first.
- Whether `gantt` dates follow Mermaid's dayjs formats exactly, and which calendar features
  (`excludes`, `weekend`, `todayMarker`) matter.
- How charts look in ASCII, and whether `sirene ascii` refuses the ones with no useful text form.
- Whether the editor offers drawing edits for any of them (LANGS-5), or only the source pane.

The outcome is one `pending` spec per chosen type.
