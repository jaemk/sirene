# Graphviz DOT

DOT files lowered to the graph model (LANGS-2) and laid out by sirene's layered layout, which is
in the family of Graphviz's `dot` engine but not identical to it (`sirene_core::dot`). Reference:
<https://graphviz.org/doc/info/lang.html> and <https://graphviz.org/doc/info/attrs.html>. Tests:
unit tests in `crates/sirene-core/src/dot/`, the fixture corpus in
`crates/sirene-core/tests/fixtures/dot/` with `crates/sirene-core/tests/dot.rs`, the property
tests in `crates/sirene-core/tests/dot_props.rs`, and `dot_graphs` in
`crates/sirene/tests/cli.rs`.

## DOT-1 Grammar

The whole DOT grammar: `strict`, `graph` and `digraph`, an optional graph ID, node, edge, and
attribute statements, `a = b` graph attributes, attribute lists `[k=v, ...]` (`,` and `;`
separators, repeated lists), edge chains `a -> b -> c`, subgraphs as edge endpoints
(`a -> {b c}`), anonymous and named subgraphs, IDs (names, numerals, double-quoted strings with
`+` concatenation and `\` line continuation, HTML strings `<...>`), `//` and `/* */` comments, and
lines starting with `#`. Keywords are case-insensitive. `->` in a `graph` and `--` in a `digraph`
are errors. Only the first graph in a file is drawn; each later one is a warning. Subgraphs nest
64 deep (LANGS-8); a deeper one is an error and is skipped up to its closing brace, so no source
overflows the stack. An unclosed block is one error, however many blocks are open.

## DOT-2 Model

- Nodes are nodes in first-mention order, with their `label` (default `\N`, the ID). `\n`, `\l`,
  and `\r` break lines; `\N`, `\G`, `\E`, `\T`, and `\H` are the node, graph, edge, tail, and head
  names, whose own escapes are read too (the node `"a\nb"` is labeled on two lines); `\\` is a
  backslash. HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`, `&#NNN;`, and
  `&#xHH;`) are decoded after the names are put in and before the line breaks are read; an
  unknown one is text.
- Edges are edges; a `digraph` edge has an arrow at its head, a `graph` edge none, and `dir`,
  `arrowhead`, and `arrowtail` change that. An edge to a subgraph joins every node in it.
- A subgraph whose name starts with `cluster` (any case) is a cluster titled by its `label`,
  nested as in the source; a node is a member of the innermost cluster it is mentioned in, and of
  the first of two clusters that do not nest. Other subgraphs only scope defaults. A cluster named
  as a node is renamed with a trailing `_`, since the graph model names both from one set. A
  cluster starts with the graph attributes (`a=b` or `graph [...]`) set in the scopes around it
  before it, as Graphviz does: a root `label` before a cluster also titles the cluster.
- `node [...]` and `edge [...]` defaults apply to later statements in their scope, as Graphviz
  does; a node takes the defaults in effect where it is first mentioned.
- `strict` merges repeated edges (either direction in a `graph`), combining their attributes.

## DOT-3 Attributes

- Graph: `rankdir` (direction), `label` (title; a blank one is none), `bgcolor` (the drawing's
  background).
- Cluster: `label`, `color` and `pencolor` (border), `bgcolor` (fill), `style=filled` (fills
  with `fillcolor`, else `color`, else `bgcolor`, else light gray), `fontcolor`, and `style`.
- Node: `shape`, `style` (`filled`, `rounded`, `dashed`, `dotted`, `bold`, `invis`, `solid`),
  `color` (border, and fill when filled without `fillcolor`), `fillcolor` (with `filled`; light
  gray by default), `fontcolor`, `penwidth`. Shapes are drawn as the nearest sirene shape: `box`,
  `rect`, `rectangle`, `square`, `Msquare` a box; `ellipse`, `oval`, `egg` an ellipse (the
  default); `circle`, `Mcircle`, `doublecircle`; `point` a dot without a label; `diamond`,
  `Mdiamond`; `plaintext`, `plain`, `none`, `underline` text alone; `cylinder`; `hexagon` (also
  `pentagon`, `septagon`, and the octagons); `parallelogram`; `trapezium`, `invtrapezium`;
  `triangle`, `invtriangle`; `note`; `tab` and `folder` a folder; `box3d` stacked boxes;
  `component` a lined box; `cds` a flag; `star` a bang. `record` and `Mrecord` are compartments
  (LANGS-3): the first field is the title, and each other field a section with a row per nested
  field; `<port>` names are dropped, and braces nest 16 deep (a deeper one is text).
- Edge: `label`, `headlabel`, `taillabel` (end labels), `style` (`dashed` and `dotted` dashed,
  `bold` thick, `invis`), `color`, `fontcolor`, `penwidth` (drawn up to 32px, DISPLA-6), `dir`,
  `minlen` (rounded, 1 to 10), and
  `arrowhead` and `arrowtail`: `normal`, `inv`, `vee`, `curve` an arrow; `empty`, `onormal` a
  hollow triangle; `diamond` and `odiamond`; `dot` and `odot` a circle; `box` and `tee` a bar;
  `crow` and `ocrow` crow's feet; `none`. The first shape of a stacked name such as `invdot` is
  drawn.
- Colors: X11 names, `#rrggbb`, `#rrggbbaa`, `H,S,V` fractions, the `/x11/` and `/svg/` schemes,
  and the first color of a list such as `red:blue`. An unknown color is a warning, once for the
  attribute however many nodes or edges it applies to.
- HTML labels are drawn as their text: tags are dropped, `<br/>` and each table row start a
  line, a row's cells are a space apart, and entities are decoded as in other labels (DOT-2).
  `&nbsp;` padding is kept; source indentation is not.

Any other attribute is a warning, once per name and kind per file (LANGS-6); layout attributes
from other engines (`pos`, `overlap`, `splines`), font attributes, and `width` and `height` are
among them. So is an unknown shape (drawn as an ellipse), arrow (drawn as `normal`), or style
word.

## DOT-4 Not drawn yet

These are parsed and warned about, then ignored: `rank=same|min|max|source|sink` subgraphs (they
need a constraint in LAYOUT-4), `constraint=false`, ports (`a:p:n`; the edge joins the node),
HTML labels (drawn as their text), `group`, `compound` with `lhead` and `ltail`, `ordering`, and
images.

## DOT-5 Choosing the language

A source is read as DOT when `--lang dot` is given, else when its file ends in `.dot` or `.gv`
(any case) or its Markdown block is named `dot` or `graphviz`, else when it starts with a DOT graph
header (LANGS-1). `--lang mermaid`, a `.mmd` or `.mermaid` file, and a `mermaid` block are read
as Mermaid even when they start like a DOT graph. A source forced to DOT without a graph header is
a syntax error, not a flowchart. The editor reads a file by its extension or block name the same
way (`Lang::of_path`, `Lang::of_info`, `diagram::lower_as`). Tests: `forced_languages` in
`diagram.rs` and in `crates/sirene/tests/cli.rs`, and `refusals.rs` in `sirene-gui`. Drawing
edits (LANGS-5) come later.
