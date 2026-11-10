# Venn diagrams

sirene draws Mermaid's venn diagrams (`venn-beta`) as images and as text. Each `set` is a circle,
and each `union` is where circles overlap:

```
$ sirene ascii teams.mmd
       Team overlap

Frontend
  - React
  - Design Systems
Backend
  - API
Frontend & Backend: Shared
  - OpenAPI
```

```
venn-beta
  title "Team overlap"
  set A["Frontend"]:20
    text A1["React"]
    text A2["Design Systems"]
  set B["Backend"]:12
    text B1["API"]
  union A,B["Shared"]:3
    text AB1["OpenAPI"]
  style A fill:#ff6b6b
```

As an image, circles are sized and placed the way Mermaid places them. See
`crates/sirene-core/tests/fixtures/venn/valid/` for more. In the editor a venn diagram is
read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `set A`, `set A["Label"]` | a circle, with its label or id |
| `union A,B["Label"]` | the overlap of earlier sets, with its label |
| `:N` after a set or union | its size (a set's default is 10) |
| `text A,B id["Label"]`, or `text id` indented under a set or union | text inside the region |
| `style ids fill:..., color:..., stroke:..., stroke-width:..., fill-opacity:...` | those styles |
| `title`, `%% comment`, frontmatter `title` | read; the title is drawn above |

A union of three or more sets also overlaps each pair of them. A union naming a set not declared
before it is an error.

## Not drawn

The hand-drawn look, Mermaid 12's newer default look and theme, and the `venn` config options are
not read.
