# Mermaid Quadrant Charts

`quadrantChart`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/quadrantChart.html>. Code: `crates/sirene-core/src/quadrant/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/quadrant.rs`
and the corpus under `crates/sirene-core/tests/fixtures/quadrant/` (LANGS-6).

## MQUAD-1 Syntax

- Header: `quadrantChart`.
- `title TEXT`, the text stopping at `#` or `;`.
- `x-axis LOW` or `x-axis LOW --> HIGH`, and the same for `y-axis`; each text plain or a
  `"string"`.
- `quadrant-1 TEXT` to `quadrant-4 TEXT`.
- A point: a name (plain up to `:`, or a `"string"`), an optional `:::class`, `:`, then
  `[x, y]` and optional styles to the end of the line (`radius: 10, color: #f00`).
- `classDef a,b styles`.
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A point without
a complete `[x, y]` is an error. Tests: `snapshots`, `syntax_errors`, and the parser's unit
tests.

## MQUAD-2 Model

`quadrant::QuadrantChart`: the title (the `title` statement, else frontmatter `title`), the
accessibility title and description, the x and y axes (`Axis`: a low and a high label), the four
quadrant labels, and the points in order. A later statement for the same axis or quadrant
replaces an earlier one. A point has a name, `x` and `y` from 0 to 1, its class, and its
`PointStyle`: `radius`, `color`, `stroke-color`, and `stroke-width`, from its class's `classDef`
first (wherever it appears) and then its own styles. A coordinate that is not a number is an
error and the point is skipped; one outside 0 to 1 is a warning and is clamped. An unknown style,
a bad value, a style without `:`, and a class with no `classDef` are warnings and ignored.
`Axis::placed` puts each label at the middle of its half, or a low label alone at the middle of
the axis. Tests: `points`, `styles`, `errors`.

## MQUAD-3 Drawing

A 400-pixel square split into four quadrants, each a tint of the theme's node stroke color
(`quadrant-1` darkest, `quadrant-3` lightest), with a 1-pixel line between them and a 2-pixel
border. Quadrant labels wrap to their quadrant and sit at its top, or in its middle when there are
no points. The x-axis labels are below the square and the y-axis labels left of it. Each point is
a circle at its coordinates (radius 5 and the node stroke color unless styled; a stroke only with
`stroke-width`, in `stroke-color` or the fill), named below in a smaller size. The title is
centered above. The drawing grows to hold points and names past the square's edges. Every element
is owned by the diagram. Tests: `drawing`.

## MQUAD-4 ASCII

A box split in four by lines, each quadrant at least 20 columns and 5 rows, its label centered on
the top row (the middle row when there are no points). The x-axis labels go under the box and
the y-axis labels left of it. Each point is `●` (`*` in ASCII) at its coordinates, with its name
beside it on the right, else the left, where it stays inside one quadrant, off the middle line,
and a column clear of other text and points. A point whose name does not fit shows a key instead
(`1` to `9`, then `a` to `z`, then `#`), and the keys and names are listed under the box. The
title is centered above. Tests: `snapshots`, `ascii_charset_snapshots`, and the renderer's unit
tests.

## MQUAD-5 Not drawn

Frontmatter `config` for `quadrantChart` (sizes, paddings, font sizes, `xAxisPosition`,
`yAxisPosition`) and theme variables are ignored: the x-axis labels are always below and the
y-axis labels always left, horizontal. Drawing edits are read-only (LANGS-5).
