# Diagram Languages

How languages other than Mermaid flowcharts fit the pipeline. Each language has its own spec:
Mermaid state (`mermaid-state-diagrams.md`), class (`mermaid-class-diagrams.md`), ER
(`mermaid-er-diagrams.md`), sequence (`mermaid-sequence-diagrams.md`), pie
(`mermaid-pie-charts.md`), gantt (`mermaid-gantt-charts.md`), and the remaining Mermaid types (`mermaid-charts.md`); Graphviz DOT (`graphviz-dot.md`); D2 (`d2.md`); PlantUML
(`plantuml.md`).

## LANGS-1 Detecting the language

A file's extension names its language: `.mmd` and `.mermaid` Mermaid, `.dot` and `.gv` DOT, `.d2`
D2, `.puml`, `.plantuml`, `.pu`, and `.iuml` PlantUML. Stdin, and a file with another extension,
is detected from its first statement: `@startuml` (or another `@start...`) is PlantUML; `graph`,
`digraph`, or `strict` followed by an optional ID and `{` is DOT; anything else is Mermaid. D2 has
no header, so it needs its extension or `--lang d2`. Every command takes `--lang
mermaid|dot|d2|plantuml` to override detection. Within Mermaid, the header keyword picks the
diagram type, as today (FLOWCH-4, FLOWCH-16).

In Markdown (MARKDO-1), a block's info string picks the language: `mermaid`; `dot` or
`graphviz`; `d2`; `plantuml` or `puml`. Blocks of other languages are left alone.

## LANGS-2 Pipeline

Each language has a parser to a lossless syntax tree with diagnostics (the `syntax` and
`diagnostic` modules, as FLOWCH-1 to FLOWCH-3), and lowers to one of three models:

- the graph model: the flowchart model (FLOWMO) with the additions in LANGS-3, for flowcharts,
  state, class, ER, DOT, D2, and PlantUML's graph-shaped diagrams. It reuses the layered layout
  (LAYOUT), the scene builder (DISPLA), the ASCII renderer, and hit testing.
- the sequence model (MSEQ-2), for Mermaid and PlantUML sequence diagrams, with its own layout,
  scene builder, and ASCII renderer.
- chart models (MCHART), each with its own layout, scene builder, and ASCII renderer.

SVG, PNG, themes, and text metrics (DISPLA-4 to DISPLA-12) are shared by all of them. A
diagnostic's position is in the source the user wrote, as now.

`diagram::lower` returns a `diagram::Diagram`: `Graph` (a `Flowchart`) or one of the other models
(`Sequence`, MSEQ-2; `Git`, MGIT-2; `Timeline`, TIMELI-2; `Pie`, MPIE-2; `Gantt`, MGANTT-3). `Diagram::scene` and `Diagram::ascii` draw any of them, and the CLI, Markdown
rendering, and the editor's drawing and export go through them. Only a graph is hit-tested and
edited; the others draw read-only (LANGS-5) with every element owned by the diagram.

## LANGS-3 Graph model additions

The flowchart model gains what the other graph languages need. Each is drawn in the scene and in
ASCII, and hit-tests as its node or edge:

- compartment nodes: a title row and sections of text rows separated by rules (class members, ER
  attributes, DOT records);
- edge end markers at either end: arrow, open arrow, hollow triangle, filled and hollow diamond,
  circle, cross, and crow's-foot cardinalities (`|o`, `||`, `}o`, `}|`);
- labels at an edge's ends as well as its middle (multiplicities);
- state shapes: start (filled circle), end (ringed circle), fork and join bars, choice diamond,
  history;
- notes: a folded-corner node linked to its target by a dashed edge with no marker, ranked beside
  the target;
- region separators: a cluster split into concurrent regions by dashed lines.

Done so far: notes (`Flowchart::notes`, laid out beside their node in its rank by
`layout::NoteSpec`), regions (`Subgraph::region`: untitled sub-clusters with dashed borders), and
the state shapes, which reuse the flowchart shapes `sm-circ`, `fr-circ`, `fork`, and an unlabeled
`diam`. A fork bar turns across a horizontal flow, and in ASCII the round ones and an unlabeled
diamond are one character. Compartments (`Node::sections`, rows of cells aligned in columns),
end markers (`Arrow::Triangle`, `Diamond`, `HollowDiamond`, and the crow's feet `ZeroOne`, `One`,
`ZeroMany`, `OneMany`), and end labels (`Edge::start_label`, `Edge::end_label`) came with class
diagrams. End labels take no room in the layout. In ASCII the crow's feet are `?` (zero or one),
`1`, `*` (zero or more), and `+` (one or more).

Within Mermaid, `diagram::Kind::of` reads the header keyword after frontmatter, comments, and
directives, and `diagram::lower` sends the source to its parser; the CLI and the editor go through
it. Languages other than Mermaid, and `--lang`, come with DOT.

## LANGS-4 Commands

`check`, `ascii`, `svg`, `png`, `--watch`, directories, and Markdown files work for every
language. A directory is scanned for every extension in LANGS-1. FLOWCH-16's "not supported"
error narrows to the Mermaid types not yet built, and says which are.

## LANGS-5 Editor

Every language opens in the editor with the source pane, the live drawing, selection, zoom,
export, and copy. Drawing edits (WYSIWY) need a per-language edit module that writes source text,
as `flowchart::edit` does; a language without one has a read-only drawing, and the edit items
are disabled with the reason (WYSIWY-19), e.g. `Add Node (drawing edits are not supported for
DOT yet)`. Edit modules come after a language renders, in the order of LANGS-7.

Sequence, state, class, and ER diagrams, git graphs, timelines, and pie and gantt charts are
read-only this way: `Document::read_only` gives the reason, the drawing's menu holds only the
disabled Add Node, Select All, and Fit Window, a press on a node pans instead of connecting, and
every edit action says `Can't ...: drawing edits are not supported for state diagrams yet` (or
class diagrams, ER diagrams, git graphs, timelines, sequence diagrams, pie charts, or gantt
charts). A sequence diagram, git graph, timeline, or pie or gantt chart is drawn from
`Document::other` and has nothing to select. Tests: `state_diagrams_are_read_only` in `crates/sirene-gui/tests/refusals.rs`.

## LANGS-6 Unsupported syntax

A construct a language allows but sirene does not draw is a warning naming it, on its span, and
is skipped; it is never dropped silently. Each language has a fixture corpus under
`crates/sirene-core/tests/fixtures/<language>/`, with expected diagnostics and ASCII, as the
flowchart corpus has.

## LANGS-7 Order

Every Mermaid type comes first, so that any Mermaid file draws:

1. Graph model additions (LANGS-3) and language detection (LANGS-1).
2. Mermaid state, then class, then ER: the most-used Mermaid types after flowcharts and sequence,
   and all graph-model.
3. Mermaid git graphs (MGIT) and timelines, with the `diagram::Diagram` split (LANGS-2).
4. Mermaid sequence: the sequence model and its layout.
5. The rest of the Mermaid charts (MCHART-1), one spec each.
6. Graphviz DOT: a small, well-defined grammar on the same layout.
7. PlantUML sequence and activity, on the sequence and graph models.
8. Edit modules for state and class.
9. D2 and the rest of PlantUML, after their research.
