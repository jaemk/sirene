# Graphviz DOT

sirene draws Graphviz DOT graphs as images and as text, with its own layered layout:

```
$ sirene ascii classes.dot
┌─────────────────┐   ╭─────────────────╮
│       Dog       │   │       Cat       │
├─────────────────┤   ├─────────────────┤
│ + bark() : void │   │ + meow() : void │
└────────┬────────┘   ╰────────┬────────╯
         │                     │
         └───┐              ┌──┘
             │              │
             │              │
             └──┐       ┌───┘
                ▽       ▽
           ┌─────────────────┐
           │     Animal      │
           ├─────────────────┤
           │ + name : string │
           │ + age : int     │
           ├─────────────────┤
           │ + die() : void  │
           └─────────────────┘
```

```
digraph classes {
  node [shape=record]
  Animal [label="{Animal|+ name : string\l+ age : int\l|+ die() : void\l}"]
  Dog [label="{Dog|+ bark() : void\l}"]
  Cat [shape=Mrecord, label="{Cat|+ meow() : void}"]
  Dog -> Animal [arrowhead=empty]
  Cat -> Animal [arrowhead=empty]
}
```

A `.dot` or `.gv` file, a `dot` or `graphviz` block in Markdown, and any source given with
`--lang dot` is DOT; so is stdin or another file that starts with `graph`, `digraph`, or `strict`
and then `{` (after an optional ID). `--lang mermaid` reads a source as Mermaid instead.
Directories are scanned for `.dot` and `.gv` files. In the editor a DOT graph is read-only on the
drawing side. Only the first graph in a file is drawn.

## What is read

| Syntax | Drawn as |
|---|---|
| `digraph`, `graph`, `strict` | arrows at heads, no arrows, repeated edges merged |
| `a -> b -> c`, `a -> {b c}` | edge chains, and edges to every node in a subgraph |
| `node [...]`, `edge [...]` | defaults for later statements in the same braces |
| `subgraph cluster_x { ... }` | a cluster titled by its `label`, filled by `bgcolor`; other subgraphs only scope defaults |
| `rankdir`, `label`, `bgcolor` | the direction, the title, and the background |
| `shape` | the nearest sirene shape; `record` and `Mrecord` as compartments |
| `style` | `filled`, `rounded`, `dashed`, `dotted`, `bold`, `invis` |
| `color`, `fillcolor`, `fontcolor`, `penwidth` | border, fill, text color, line width |
| `label`, `headlabel`, `taillabel` | labels, with `\n`, `\l`, `\N`, `\G`, `\E`, `\T`, `\H` |
| `arrowhead`, `arrowtail`, `dir`, `minlen` | end markers, which ends have them, and rank span |
| `rank=same`, `min`, `max`, `source`, `sink` | nodes sharing a rank, or in the first or last |
| `constraint=false`, `group`, `ordering` | edges that do not rank, straight edges in a group, source order |
| `compound=true` with `lhead`, `ltail` | edges ending at a cluster's border |
| `a:p`, `a:e`, `tailport`, `headport` | record and compass ports |
| HTML `<TABLE>` node labels | compartments, with `<HR/>` sections and `PORT` cells |

Colors are X11 names, `#rrggbb`, `#rrggbbaa`, or `H,S,V` fractions. See
`spec/graphviz-dot.md` for the full list of shapes and arrows.

## Errors

Syntax errors, `->` in a `graph`, `--` in a `digraph`, and subgraphs nested more than 64 deep
are errors. An attribute that is not drawn, an unknown shape, color, arrow, or style, and a
second graph in a file are warnings, each once.

## Not drawn

The layout is sirene's, not Graphviz's. Record ports in top-down graphs (the edge joins the
node), images, node sizes, fonts, and other engines' layout attributes (`pos`, `overlap`,
`splines`) are warned about and ignored. HTML labels other than node tables are drawn as their
text.
