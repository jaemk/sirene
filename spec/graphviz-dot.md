# Graphviz DOT

DOT files lowered to the graph model (LANGS-2) and laid out by sirene's layered layout, which is
in the family of Graphviz's `dot` engine but not identical to it. Reference:
<https://graphviz.org/doc/info/lang.html> and <https://graphviz.org/doc/info/attrs.html>.

## DOT-1 Grammar

The whole DOT grammar: `strict`, `graph` and `digraph`, an optional graph ID, node, edge, and
attribute statements, `a = b` graph attributes, attribute lists `[k=v, ...]` (`,` and `;`
separators, repeated lists), edge chains `a -> b -> c`, subgraphs as edge endpoints
(`a -> {b c}`), anonymous and named subgraphs, IDs (names, numbers, double-quoted strings with
`+` concatenation, HTML strings `<...>`), and `//`, `/* */`, and `#` comments. Keywords are
case-insensitive. `->` in a `graph` and `--` in a `digraph` are errors.

## DOT-2 Model

- Nodes are nodes with their `label` (default the ID; `\n`, `\l`, `\r` break lines), and `\N` and
  `\G` substitutions.
- Edges are edges; a `digraph` edge has an arrow at its head, a `graph` edge none, and `dir`,
  `arrowhead`, and `arrowtail` change that.
- A subgraph whose name starts with `cluster` is a cluster titled by its `label`; other subgraphs
  only scope defaults.
- `node [...]` and `edge [...]` defaults apply to later statements in their scope, as Graphviz
  does.
- `strict` merges repeated edges.

## DOT-3 Attributes

- Graph: `rankdir` (direction), `label` (title), `bgcolor`.
- Node: `shape` (`box`, `rect`, `square`, `ellipse`, `oval`, `circle`, `doublecircle`, `point`,
  `diamond`, `plaintext`, `plain`, `none`, `cylinder`, `hexagon`, `octagon`, `parallelogram`,
  `trapezoid`, `invtrapezoid`, `triangle`, `note`, `tab`, `folder`, `box3d`, `component`, `record`
  and `Mrecord` as compartments (LANGS-3)), `style` (`filled`, `rounded`, `dashed`, `dotted`,
  `bold`, `invis`), `color`, `fillcolor`, `fontcolor`, `penwidth`, `width`, `height`.
- Edge: `label`, `headlabel`, `taillabel` (end labels), `style`, `color`, `fontcolor`,
  `penwidth`, `arrowhead` and `arrowtail` (`normal`, `vee`, `empty`, `onormal`, `diamond`,
  `odiamond`, `dot`, `odot`, `box`, `tee`, `none`), `dir`, `minlen`.
- Colors: X11 names, `#rrggbb`, `#rrggbbaa`.

Any other attribute is a warning, once per name per file (LANGS-6); layout attributes from other
engines (`pos`, `overlap`, `splines`) are among them.

## DOT-4 Not drawn yet

These are parsed and warned about, then ignored: `rank=same|min|max|source|sink` subgraphs (they
need a constraint in LAYOUT-4), `constraint=false`, ports (`a:p:n`), HTML labels (drawn as their
text), `group`, `compound` with `lhead` and `ltail`, `ordering`, and images.
