# Mermaid Charts

Status: research. The Mermaid types that are not covered by the state, class, ER, and sequence
specs. Each needs its own model and scene builder (LANGS-2); some fit the graph model. Reference:
<https://mermaid.js.org/intro/syntax-reference.html>.

## MCHART-1 Grouping

| Type | Model | Notes |
|---|---|---|
| `requirementDiagram` | graph | Requirements and elements as compartment boxes, labeled relationships: `mermaid-requirement-diagrams.md`. |
| `C4Context` (and the other C4 types), `architecture-beta`, `block-beta` | graph | New shapes; C4 and architecture use icons, drawn as placeholders. `block-beta` places blocks in a grid, which needs a LAYOUT constraint. |
| `mindmap` | tree | A two-sided tree out from the root; ASCII is an outline: `mermaid-mindmaps.md`. |
| `pie` | chart | Slices with labels and a legend; ASCII is a table with percentage bars: `mermaid-pie-charts.md`. |
| `quadrantChart` | chart | Points in four labeled quadrants; ASCII plots them by name: `mermaid-quadrant-charts.md`. |
| `xychart-beta` | chart | Bars and lines over category or numeric axes; ASCII is a row per position: `mermaid-xy-charts.md`. |
| `radar-beta` | chart | Axes around a circle, and a closed curve per series; ASCII is a bar per axis and series: `mermaid-radar-charts.md`. |
| `sankey-beta` | chart | Flows between nodes in columns, each band as wide as its value; ASCII lists each node's outgoing flows: `mermaid-sankey-diagrams.md`. |
| `packet` | chart | Rows of bit fields; ASCII is the RFC header layout: `mermaid-packet-diagrams.md`. |
| `gantt` | chart | Dates, durations, dependencies (`after`), sections, and a time axis; ASCII is a bar per task: `mermaid-gantt-charts.md`. |
| `timeline`, `journey` | timeline | Sections along a line (`timeline`: `mermaid-timelines.md`); journey adds scores and actors: `mermaid-journeys.md`. |
| `gitGraph` | timeline | Commits on branch lanes, merges, and cherry-picks: `mermaid-git-graphs.md`. |
| `kanban` | board | Columns of cards, with the mindmap grammar; ASCII is a box per column: `mermaid-kanban-boards.md`. |

## MCHART-2 Open questions

- Which types to build. Usage suggests `gantt`, `pie`, `mindmap`, and `gitGraph` first.
- Whether `gantt` dates follow Mermaid's dayjs formats exactly, and which calendar features
  (`excludes`, `weekend`, `todayMarker`) matter.
- How charts look in ASCII, and whether `sirene ascii` refuses the ones with no useful text form.
- Whether the editor offers drawing edits for any of them (LANGS-5), or only the source pane.

The outcome is one `pending` spec per chosen type.
