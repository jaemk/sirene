# Diagram Languages

How languages other than Mermaid flowcharts fit the pipeline. Each language has its own spec:
Mermaid state (`mermaid-state-diagrams.md`), class (`mermaid-class-diagrams.md`), ER
(`mermaid-er-diagrams.md`), sequence (`mermaid-sequence-diagrams.md`), pie
(`mermaid-pie-charts.md`), gantt (`mermaid-gantt-charts.md`), mindmap
(`mermaid-mindmaps.md`), journey (`mermaid-journeys.md`), quadrant chart (`mermaid-quadrant-charts.md`), XY chart (`mermaid-xy-charts.md`), requirement (`mermaid-requirement-diagrams.md`), kanban (`mermaid-kanban-boards.md`), packet (`mermaid-packet-diagrams.md`), radar chart (`mermaid-radar-charts.md`), sankey (`mermaid-sankey-diagrams.md`), treemap (`mermaid-treemaps.md`), block (`mermaid-block-diagrams.md`), and the remaining Mermaid types (`mermaid-charts.md`); Graphviz DOT (`graphviz-dot.md`); D2 (`d2.md`); PlantUML
(`plantuml.md`).

## LANGS-1 Detecting the language

A file's extension names its language: `.mmd` and `.mermaid` Mermaid, `.dot` and `.gv` DOT, `.d2`
D2, `.puml`, `.plantuml`, `.pu`, and `.iuml` PlantUML. Stdin, and a file with another extension,
is detected from its first statement: `@startuml` (or another `@start...`) is PlantUML; `graph`,
`digraph`, or `strict` followed by an optional ID and `{` is DOT; anything else is Mermaid. D2 has
no header, so it needs its extension or `--lang d2`. Every command takes `--lang
mermaid|dot|d2|plantuml` to override detection. Within Mermaid, the header keyword picks the
diagram type (FLOWCH-4).

In Markdown (MARKDO-1), a block's info string picks the language: `mermaid`; `dot` or
`graphviz`; `d2`; `plantuml` or `puml`. Blocks of other languages are left alone.

## LANGS-2 Pipeline

Each language has a parser to a lossless syntax tree with diagnostics (the `syntax` and
`diagnostic` modules, as FLOWCH-1 to FLOWCH-3), and lowers to one of three models:

- the graph model: the flowchart model (FLOWMO) with the additions in LANGS-3, for flowcharts,
  state, class, ER, requirement, use case, DOT, D2, and PlantUML's graph-shaped diagrams. It reuses the layered layout
  (LAYOUT), the scene builder (DISPLA), the ASCII renderer, and hit testing.
- the sequence model (MSEQ-2), for Mermaid and PlantUML sequence diagrams, with its own layout,
  scene builder, and ASCII renderer.
- chart models (MCHART), each with its own layout, scene builder, and ASCII renderer.

SVG, PNG, themes, and text metrics (DISPLA-4 to DISPLA-12) are shared by all of them. A
diagnostic's position is in the source the user wrote, as now.

`diagram::lower` returns a `diagram::Diagram`: `Graph` (a `Flowchart`) or one of the other models
(`Sequence`, MSEQ-2; `Git`, MGIT-2; `Timeline`, TIMELI-2; `Pie`, MPIE-2; `Gantt`, MGANTT-3; `Mindmap`, MMIND-2; `Journey`, MJOUR-2; `Quadrant`, MQUAD-2; `XyChart`, MXY-2; `Kanban`, MKAN-2; `Packet`, MPKT-2; `Radar`, MRADAR-2; `Sankey`, MSANKEY-2; `Treemap`, MTREE-2; `Block`, MBLOCK-2; `Architecture`, MARCH-2; `C4`, MC4-2; `Info`, MINFO-2; `Ishikawa`, MISH-2; `Venn`, MVENN-2; `Wardley`, MWARD-2; `TreeView`, MTREEV-2; ZenUML lowers to `Sequence`, MZEN-2). `Diagram::scene` and `Diagram::ascii`
draw any of them, and the CLI, Markdown rendering, and the editor's drawing and export go through
them. Only a graph is hit-tested and edited; the others draw read-only (LANGS-5) with every
element owned by the diagram.

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

`diagram::Kind::of` is `Kind::Dot` for a source that starts with a DOT graph header (DOT-1), and
otherwise reads the Mermaid header keyword after frontmatter, comments, and directives;
`diagram::lower` sends the source to its parser, and the CLI and the editor go through it.
`Kind::of_lang` and `diagram::lower_as` take a `diagram::Lang` (Mermaid or DOT) that overrides
detection: `--lang`, else `Lang::of_path` (the extension) or `Lang::of_info` (the Markdown
block's info word), as DOT-5. `--lang` takes `mermaid` and `dot` so far.

## LANGS-4 Commands

`check`, `ascii`, `svg`, `png`, `--watch`, directories, and Markdown files work for every
language. A directory is scanned for every extension in LANGS-1. Every Mermaid 12.3 type is
drawn.

## LANGS-5 Editor

Every language opens in the editor with the source pane, the live drawing, selection, zoom,
export, and copy. Drawing edits (WYSIWY) need a per-language edit module that writes source text,
as `flowchart::edit` does; a language without one has a read-only drawing, and the edit items
are disabled with the reason (WYSIWY-19), e.g. `Add Node (drawing edits are not supported for
DOT yet)`. Edit modules come after a language renders, in the order of LANGS-7.

Sequence, state, class, ER, requirement, and use case diagrams, git graphs, timelines, pie and gantt charts,
mindmaps, user journeys, quadrant, XY, and radar charts, kanban boards, packet, sankey, block, architecture, C4, ZenUML, info, ishikawa, venn, Wardley, tree view, use case, railroad, swimlane, Cynefin, event modeling, and agentflow diagrams, and treemaps are read-only this way: `Document::read_only` gives the reason, the drawing's menu holds only the
disabled Add Node, Select All, and Fit Window, a press on a node pans instead of connecting, and
every edit action says `Can't ...: drawing edits are not supported for state diagrams yet` (or
class diagrams, ER diagrams, requirement diagrams, use case diagrams, git graphs, timelines, sequence diagrams, pie charts, gantt
charts, mindmaps, user journeys, quadrant charts, XY charts, kanban boards, packet diagrams,
radar charts, sankey diagrams, treemaps, block diagrams, architecture diagrams, C4 diagrams, ZenUML diagrams, ishikawa diagrams, venn diagrams, Wardley maps, tree views, or railroad diagrams; an info diagram says it has
nothing to edit).
A sequence diagram, git graph, timeline, pie, gantt, quadrant, XY, or radar chart, mindmap, user
journey, kanban board, packet, sankey, block, architecture, C4, ZenUML, info, ishikawa, or venn diagram, Wardley map, tree view, railroad diagram, or treemap is drawn from `Document::other` and has nothing to select. Tests: `state_diagrams_are_read_only` in `crates/sirene-gui/tests/refusals.rs`.

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

## LANGS-8 Limits

No source overflows the stack or hangs. Nesting is read `lines::MAX_DEPTH` (64) deep; `lines::too_deep` words the error, `<things> nested
more than 64 deep`, which is reported once per diagram (once per rule in a railroad diagram):

- Blocks with a closer (flowchart subgraphs, composite states, class namespaces, C4 boundaries,
  ZenUML blocks, DOT subgraphs): a deeper block is not read. Its
  text through the matching closer is one `ERROR` node, so the tree still holds every byte.
  Blocks left open at the end of the source are one `unclosed` error per kind of block in the
  shared line parser (`lines::Base`).
- Block diagrams nest 12 deep, since sizing a block visits its children twice per level.
- Indented levels (mindmap nodes, treemap sections, tree view entries, ishikawa causes): deeper
  lines are left out.
- Sequence diagrams: a deeper block is left out with the statements inside it, up to its `end`.
  A participant draws 64 activations at once; deeper ones are tracked but not drawn.
- Architecture diagrams: a deeper group is drawn outside any group.
- Railroad diagrams: a rule with a deeper group or expression fails at that point, as any other
  syntax error in a rule does.
- JSON (use case sources, `%%{init}%%` directives): a deeper value is invalid JSON. A directive
  that is not valid JSON is ignored, as Mermaid ignores it.

A diagram reports at most `diagram::MAX_DIAGNOSTICS` (500) problems; the rest are one more,
`<n> more problems not shown`, at the first one left out, an error when any of them is.

A gantt chart dates each task once, in dependency order; `excludes` that leaves no working day is
an error and is ignored (MGANTT-3). An ASCII drawing is cut off at 2^22 cells (ASCIIR-1).

Tests: `crates/sirene-core/tests/limits.rs` draws a source at each limit and 2,000-statement
chains, and checks the diagnostics; `skipped_blocks_keep_their_text` checks the trees. The
`scale` bench in `crates/sirene-core/benches/scale.rs` times reading, scene building, and text
drawing of large inputs at two sizes ten times apart.

## LANGS-9 Source text

A source draws the same with `\n` or `\r\n` line endings, with or without a final newline, and
after a byte order mark (U+FEFF), which reads as whitespace and may come before frontmatter or a
Markdown fence. The tree keeps it. Tests: `crates/sirene-core/tests/text.rs` draws every
fixture each way.
