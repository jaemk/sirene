# Mermaid XY Charts

`xychart-beta` and `xychart`, lowered to their own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/xyChart.html>. Code: `crates/sirene-core/src/xychart/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/xychart.rs`
and the corpus under `crates/sirene-core/tests/fixtures/xychart/` (LANGS-6).

## MXY-1 Syntax

- Header: `xychart-beta` or `xychart`, optionally followed by `horizontal` or `vertical`.
- `title "TEXT"` or `title TEXT`, the text stopping at `#` or `;`.
- `x-axis [TITLE] [a, "b c", d]` (categories) or `x-axis [TITLE] MIN --> MAX` (numeric).
- `y-axis [TITLE] [MIN --> MAX]`.
- `line [NAME] [v, v "label", ...]` and `bar [NAME] [v, ...]`.
- A title or name is a `"string"` or one unquoted word. A number has an optional sign and may
  start with its fraction (`+1.3`, `.98`, `-3.4`).
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. An unclosed or
malformed list or range is an error. Tests: `snapshots`, `syntax_errors`, and the parser's unit
tests.

## MXY-2 Model

`xychart::XyChart`: the title (the `title` statement, else frontmatter `title`), the
accessibility title and description, `horizontal`, the x-axis title and `XAxis` (`Categories`
or `Range`), the y-axis title and range, and the series in order. A `Series` has its kind
(`Line` or `Bar`), an optional name, its values, and a label per value. A later axis statement
replaces an earlier one. A series that did not parse is left out.

`XyChart::positions` is the number of categories, or with none the longest series' length.
`x_labels` are the categories, else the positions numbered from 1, or on a numeric axis each
position's value, the values spread evenly from `min` to `max`. `y_bounds` is the y range, else
the data's least and greatest values, widened by 1 each way when they are equal. A range written
high to low is a warning and turned around; categories on the y-axis are an error; a series with
more values than categories is a warning, and the extra values are not drawn. `xychart::ticks`
picks round tick values as d3 does. Tests: `series`, `errors`, and the module's unit tests.

## MXY-3 Drawing

A vertical chart: categories along the bottom, each centered in an equal share of an axis at
least 560 pixels long (48 per position), and the value axis up the left, 340 pixels, with about
5 ticks. A horizontal chart turns this a quarter: categories down the left, wrapped at 160
pixels, and values along the bottom with about 6 ticks. On a numeric x-axis the positions and
about 8 ticks are spread across it, inset by half a share at each end. Bars go from the value
axis's low end to the value, 70% of a share wide and at most 80 pixels; lines are 2 pixels wide
through each position, with a value's label beside its point. Series take the branch colors in
order and draw in source order, later over earlier, as in Mermaid. Named series get a legend
under the title. The value axis's title sits above it (vertical) and the x-axis's below the
category labels; turned around when horizontal. Every element is owned by the diagram. Tests:
`drawing`.

## MXY-4 ASCII

A table with a row per position: the position's label, then for each series a 24-cell field
scaled across the value axis, a bar for a bar series (eighth blocks in Unicode, `#` in ASCII)
or a marker at the value for a line series (`●`, or `*`), and the value with its label. The
header row holds the x-axis title and each series' name, or `bar N` or `line N`. Under the
table, the value axis's title and span (`Revenue: 4000 to 11000`). The title is centered above.
Horizontal charts look the same. Tests: `snapshots`, `ascii_charset_snapshots`, and the
renderer's unit tests.

## MXY-5 Not drawn

Frontmatter `config` for `xyChart` (sizes, paddings, font sizes, `showDataLabel`,
`showLegend`, axis settings) and theme variables (`plotColorPalette` and the colors) are
ignored. Drawing edits are read-only (LANGS-5).
