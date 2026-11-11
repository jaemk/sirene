# Graphviz DOT

DOT files lowered to the graph model (LANGS-2) and laid out by sirene's layered layout, which is
in the family of Graphviz's `dot` engine but not identical to it (`sirene_core::dot`). Reference:
<https://graphviz.org/doc/info/lang.html> and <https://graphviz.org/doc/info/attrs.html>. Tests:
unit tests in `crates/sirene-core/src/dot/`, the fixture corpus in
`crates/sirene-core/tests/fixtures/dot/` with `crates/sirene-core/tests/dot.rs`, and `dot_graphs`
in `crates/sirene/tests/cli.rs`.

## DOT-1 Grammar

The whole DOT grammar: `strict`, `graph` and `digraph`, an optional graph ID, node, edge, and
attribute statements, `a = b` graph attributes, attribute lists `[k=v, ...]` (`,` and `;`
separators, repeated lists), edge chains `a -> b -> c`, subgraphs as edge endpoints
(`a -> {b c}`), anonymous and named subgraphs, IDs (names, numerals, double-quoted strings with
`+` concatenation and `\` line continuation, HTML strings `<...>`), `//` and `/* */` comments, and
lines starting with `#`. Keywords are case-insensitive. `->` in a `graph` and `--` in a `digraph`
are errors. Only the first graph in a file is drawn; each later one is a warning.

## DOT-2 Model

- Nodes are nodes in first-mention order, with their `label` (default `\N`, the ID). `\n`, `\l`,
  and `\r` break lines; `\N`, `\G`, `\E`, `\T`, and `\H` are the node, graph, edge, tail, and head
  names.
- Edges are edges; a `digraph` edge has an arrow at its head, a `graph` edge none, and `dir`,
  `arrowhead`, and `arrowtail` change that. An edge to a subgraph joins every node in it.
- A subgraph whose name starts with `cluster` (any case) is a cluster titled by its `label`,
  nested as in the source; a node joins the first cluster it is mentioned in. Other subgraphs only
  scope defaults.
- `node [...]` and `edge [...]` defaults apply to later statements in their scope, as Graphviz
  does; a node takes the defaults in effect where it is first mentioned.
- `strict` merges repeated edges (either direction in a `graph`), combining their attributes.

## DOT-3 Attributes

- Graph: `rankdir` (direction), `label` (title), `bgcolor` (the drawing's background).
- Cluster: `label`, `color` and `pencolor` (border), `fillcolor` with `style=filled`, `bgcolor`,
  `fontcolor`, and `style`.
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
  field; `<port>` names are dropped.
- Edge: `label`, `headlabel`, `taillabel` (end labels), `style` (`dashed` and `dotted` dashed,
  `bold` thick, `invis`), `color`, `fontcolor`, `penwidth`, `dir`, `minlen` (1 to 10), and
  `arrowhead` and `arrowtail`: `normal`, `inv`, `vee`, `curve` an arrow; `empty`, `onormal` a
  hollow triangle; `diamond` and `odiamond`; `dot` and `odot` a circle; `box` and `tee` a bar;
  `crow` and `ocrow` crow's feet; `none`. The first shape of a stacked name such as `invdot` is
  drawn.
- Colors: X11 names, `#rrggbb`, `#rrggbbaa`, `H,S,V` fractions, the `/x11/` and `/svg/` schemes,
  and the first color of a list such as `red:blue`. An unknown color is a warning.

Any other attribute is a warning, once per name and kind per file (LANGS-6); layout attributes
from other engines (`pos`, `overlap`, `splines`), font attributes, and `width` and `height` are
among them. So is an unknown shape (drawn as an ellipse), arrow (drawn as `normal`), or style
word.

## DOT-4 Not drawn yet

These are parsed and warned about, then ignored: `rank=same|min|max|source|sink` subgraphs (they
need a constraint in LAYOUT-4), `constraint=false`, ports (`a:p:n`; the edge joins the node),
HTML labels (drawn as their text), `group`, `compound` with `lhead` and `ltail`, `ordering`, and
images.

## DOT-5 Not done yet

The language is detected from the source (LANGS-1); a `.dot` or `.gv` file, a `dot` or
`graphviz` Markdown block, and `--lang dot` do not force it yet, so such a source that does not
start with a DOT graph header is read as Mermaid. Drawing edits (LANGS-5) come later.
