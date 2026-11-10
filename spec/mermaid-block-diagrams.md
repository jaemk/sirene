# Mermaid Block Diagrams

`block-beta`, lowered to its own model over the flowchart model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/block.html>. Code: `crates/sirene-core/src/block/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/block.rs` and
the corpus under `crates/sirene-core/tests/fixtures/block/` (LANGS-6).

## MBLOCK-1 Syntax

- Header: `block-beta` or `block`.
- `columns N` or `columns auto`.
- Blocks: an id with an optional flowchart shape and label (`a["A"]`, `a(("A"))`, ...) and an
  optional width `:N`; several per line.
- `space` and `space:N`.
- Block arrows: `id<["label"]>(dir, ...)` with `right`, `left`, `up`, `down`, `x`, `y`.
- Groups: `block:id["label"]` or `block` alone, up to `end`, nested freely.
- Edges between blocks, with the flowchart strokes and labels (`a --> b`, `a -- "x" --- b`),
  chained.
- `classDef`, `class`, `style`, `accTitle`, `accDescr`, comments, directives, frontmatter `title`
  and `config.block.padding`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. An unclosed
`block`, an `end` with nothing to close, `columns` without a number or `auto`, an unclosed shape
or quote, a block arrow without directions or with an unknown one, an edge without a block after
it, `block:` without an id, and a line that is none of the above are errors. Tests: `snapshots`,
`syntax_errors`, and the parser's unit tests.

## MBLOCK-2 Model

`block::Block`: a `Flowchart` holding the blocks (nodes), groups (subgraphs), edges, classes, and
title; the grid as `groups` (`groups[0]` the diagram, each other a subgraph's), each with its
`columns` and its cells in order (`Cell`: a node, `space`, or a group, and its width); `arrows`,
each node's block arrow directions; and `padding` (`config.block.padding`, 8).

A block's first mention places it; a later one changes its shape and label. A label defaults to
the id; an anonymous group is `block-N`. `columns 0` and a width of 0 are errors and ignored or
drawn as 1; a width over the group's columns is a warning and narrowed. `class` and `style` on an
unknown id are warnings. A bad `padding` is a warning and ignored. Labels decode Mermaid's
`#name;` and HTML's `&name;` entities. Tests: `model`, `mentions_and_spaces`, `errors`.

## MBLOCK-3 Drawing

`block::layout` places the grid as Mermaid does: in each group every cell is as wide as the widest
block per column it spans and as tall as the tallest; a block `N` columns wide covers `N` cells
and the `padding` between them; rows wrap after `columns`; a group is its grid plus `padding`
around it and room for its title, and a group smaller than its siblings grows to their size.
Blocks draw with the flowchart shapes and styles, groups as clusters with their titles on top,
and edges as straight lines between block centers, clipped to the blocks, with labels at their
middle. A block arrow is an arrow outline with a head for each direction. Elements are owned by
their node, subgraph, or edge. Tests: `drawing` and the layout's and scene's unit tests.

## MBLOCK-4 ASCII

The same grid in cells (2 columns and 1 row between blocks): boxes with the flowchart frames,
groups as boxes with the title on the top border, and block arrows as boxes with `►◄▲▼` (`> < ^ v`
in ASCII) on the sides they point out of. Edges are listed below the grid as
`from --> to: label`. The title is centered above. Tests: `snapshots`, `ascii_charset_snapshots`,
and the renderer's unit tests.

## MBLOCK-5 Not drawn

Theme variables are ignored. Drawing edits are read-only (LANGS-5).
