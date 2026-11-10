# Supported syntax

sirene reads Mermaid's flowchart syntax (`flowchart`, `graph`, and `flowchart-elk`). This page
lists what is read and where sirene differs from Mermaid. The other types sirene draws have pages
of their own; the rest (`quadrantChart`, `xychart-beta`, ...) are one error, and nothing is drawn.

## Header and direction

```
flowchart LR
```

The direction is `TB` (or `TD`), `BT`, `LR`, or `RL`, and defaults to `TB`. `direction` inside a
subgraph sets that subgraph's direction; outside one it has no effect and is a warning.

Frontmatter (`---` ... `---`) is read for `title:`, which is drawn above the diagram. Other keys
and `%%{init}%%` directives are ignored for now. Lines starting with `%%` are comments.

## Nodes and labels

```
A
B[Label]
C["Quoted: (any) [characters]"]
D["`**Bold** and *italic*`"]
E[Two<br>lines]
F:::important
```

- A node is created where it is first mentioned, and a node without a label shows its id.
- A later mention with a shape replaces the shape and label.
- `"..."` labels may hold any character but `"`; use `#quot;` for it. Entity codes such as
  `#amp;`, `#lt;`, and `#9829;` are decoded.
- `` "`...`" `` labels are markdown: `**bold**`, `__bold__`, `*italic*`, and `_italic_`.
- `<br>` is a line break.
- `:::name` adds a class.
- `end` is reserved and cannot be a node id.

See [Shapes](shapes.md) for the shape delimiters and `@{ shape: ... }`.

## Edges

```
A --> B
A --- B
A -.-> B
A ==> B
A ~~~ B
A <--> B
A --x B
A --o B
A -->|label| B
A -- label --> B
A ----> B
A & B --> C & D
A --> B --> C
A e1@--> B
```

| Stroke | Forms |
|---|---|
| normal | `---`, `-->` |
| thick | `===`, `==>` |
| dotted | `-.-`, `-.->` |
| invisible | `~~~` |

- `>`, `x`, and `o` are arrowheads; a start marker (`<`, `x`, `o`) makes the edge two-headed.
  Mermaid drops a start marker that does not match the end; sirene keeps it.
- Extra `-`, `=`, or `.` make an edge longer (up to 10), which puts its ends further apart.
- `A & B --> C & D` adds an edge from each node on the left to each on the right.
- An inline label (`-- text -->`) must be on one line. Mermaid allows a line break there; sirene
  does not, so a missing closing stroke cannot swallow the statements after it.
- `e1@` gives the edge an id, for `e1@{ ... }`, `class`, and `style`.

## Subgraphs

```
subgraph api [API]
    direction LR
    A --> B
end
subgraph "Data stores"
    db[(Postgres)]
end
api --> db
```

A subgraph's members are the nodes mentioned in its body. A node already in an earlier subgraph
stays there. Edges may start or end at a subgraph.

## Styling

```
classDef warm fill:#fdf2e3,stroke:#c77d1a,color:#5a3200
class A,B warm
C:::warm
style D fill:#e8f1fb,stroke-width:2px,stroke-dasharray:4 2
linkStyle 0 stroke:#d33,stroke-width:3px
linkStyle default stroke:#888
```

These properties are drawn: `fill`, `stroke`, `stroke-width`, `stroke-dasharray`, and `color`
(text). Others are ignored. Colors may be `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
`rgba()`, a CSS color name, `none`, or `transparent`.

Precedence, lowest first: `classDef default` (nodes) or `linkStyle default` (edges), then the
item's classes in order, then its own `style` or `linkStyle`. A filled node without a `color`
keeps a text color that reads on its fill.

`style` and `class` also accept an edge id, which Mermaid does not.

## Metadata

```
A@{ shape: cyl, label: "Orders" }
e1@{ animate: true }
```

An `@{ ... }` block is a flat mapping of `key: value` entries. Nodes read `shape` and `label`;
edges read `animate`, `animation`, and `curve`. Other keys are kept but have no effect.

## Click and accessibility

`click`, `accTitle`, and `accDescr` are parsed and checked; `click` has no effect in the editor.

## Errors

An error is confined to its statement: the rest of the diagram is still read and drawn. sirene
reports an error where Mermaid would fail to render, and a warning where Mermaid would silently
ignore or guess, such as a `class` naming a node that does not exist.
