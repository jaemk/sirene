# Block diagrams

sirene draws Mermaid block diagrams (`block-beta`) as images and as text. Blocks fill a grid of
columns in order; in text the edges are listed below the grid:

```
$ sirene ascii --charset ascii plain.mmd
+---+  +---+  +---+
| a |  | b |  | c |
+---+  +---+  +---+

+----------+  +---+
|    d     |  | e |
+----------+  +---+
```

```
block-beta
  columns 3
  a b c
  d:2 e
```

See `crates/sirene-core/tests/fixtures/block/valid/` for groups, block arrows, and styles. In the
editor a block diagram is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `columns 3`, `columns auto` | the columns of the diagram or group; `auto` (the default) is one row |
| `a`, `a["Label"]`, `a(("Label"))`, ... | a block, with the flowchart shapes |
| `a:2` | a block 2 columns wide |
| `space`, `space:2` | an empty column, or 2 |
| `a<["Label"]>(right)` | a block arrow pointing `right`, `left`, `up`, `down`, `x` (both across), or `y` (both up and down); several directions are allowed |
| `block:id["Title"]` ... `end`, `block` ... `end` | a group of blocks with its own columns |
| `a --> b`, `a -- "text" --- b`, `==>`, `-.->` | an edge, drawn straight from block to block |
| `classDef`, `class`, `style` | styles, on blocks and groups |
| frontmatter `title` | the title above |
| `config.block.padding` | the space between blocks (8) |
| `accTitle`, `accDescr` | read, not drawn |

Every block in a group is as large as the largest, per column it spans, as in Mermaid. Labels
decode entities in both forms, `&nbsp;` and `#nbsp;`.

## Errors

An unclosed `block`, an `end` with nothing to close, `columns` without a number, a block arrow
without directions or with an unknown one, an unclosed shape or quote, and an edge without a
block after it are errors. `columns 0` and a width of 0 are errors and drawn as 1. A block wider
than its group's columns and a `class` or `style` for an unknown id are warnings.

## Not drawn

Theme variables are ignored.
