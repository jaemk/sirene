# Mermaid Charts

Status: research. The Mermaid types that are not covered by the state, class, ER, and sequence
specs. Each needs its own model and scene builder (LANGS-2); some fit the graph model. Reference:
<https://mermaid.js.org/intro/syntax-reference.html>.

## MCHART-1 Grouping

| Type | Model | Notes |
|---|---|---|
| `requirementDiagram`, `C4Context` (and the other C4 types), `architecture-beta`, `block-beta` | graph | New shapes; C4 and architecture use icons, drawn as placeholders. `block-beta` places blocks in a grid, which needs a LAYOUT constraint. |
| `mindmap` | tree | A radial or two-sided tree layout, not layered. |
| `pie` | chart | Slices with labels and a legend; ASCII is a table with percentage bars. |
| `xychart-beta`, `quadrantChart`, `radar-beta` | chart | Axes, scales, and points or bars. |
| `sankey-beta` | chart | Flows between ranked nodes; the ranking could reuse LAYOUT-4. |
| `packet-beta` | chart | Rows of bit fields. |
| `gantt` | timeline | Dates, durations, dependencies (`after`), sections, and a time axis; ASCII is a bar per task. |
| `timeline`, `journey` | timeline | Sections along a line (`timeline`: `mermaid-timelines.md`); journey adds scores and actors. |
| `gitGraph` | timeline | Commits on branch lanes, merges, and cherry-picks: `mermaid-git-graphs.md`. |
| `kanban` | board | Columns of cards. |

## MCHART-2 Open questions

- Which types to build. Usage suggests `gantt`, `pie`, `mindmap`, and `gitGraph` first.
- Whether `gantt` dates follow Mermaid's dayjs formats exactly, and which calendar features
  (`excludes`, `weekend`, `todayMarker`) matter.
- How charts look in ASCII, and whether `sirene ascii` refuses the ones with no useful text form.
- Whether the editor offers drawing edits for any of them (LANGS-5), or only the source pane.

The outcome is one `pending` spec per chosen type.
