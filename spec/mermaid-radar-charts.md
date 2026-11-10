# Mermaid Radar Charts

`radar-beta`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/radar.html>. Code: `crates/sirene-core/src/radar/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/radar.rs` and
the corpus under `crates/sirene-core/tests/fixtures/radar/` (LANGS-6).

## MRADAR-1 Syntax

- Header: `radar-beta`.
- `axis ID["label"], ID, ...`: one or more axes, each an id (a letter or `_`, then letters,
  digits, and `_`) with an optional quoted label in brackets.
- `curve ID["label"]{VALUES}, ...`: one or more curves, each with listed values (`{1, 2.5, -3}`)
  or keyed values (`{a: 1, b: 2}`). Inside the braces values may span lines.
- Options, one value each: `showLegend true|false`, `max N`, `min N`, `graticule circle|polygon`,
  and `ticks N`.
- `title TEXT`, the text stopping at `#` or `;`.
- Comments, directives, frontmatter `title` and `config.radar`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A missing id,
an unquoted or unclosed label, a curve without braces, a value that is not a number, a missing
`,` or `}`, and an option without a value are errors. Tests: `snapshots`, `syntax_errors`, and
the parser's unit tests.

## MRADAR-2 Model

`radar::Radar`: the title (the `title` statement, else frontmatter `title`), the accessibility
title and description, the axes (`Axis`: id and label, the label defaulting to the id), the
curves (`Curve`: id, label, and one value per axis in axis order), `show_legend` (on unless
`false`), `max` (unset unless given), `min` (0 unless given), `graticule` (`Circle` unless
`polygon`), `ticks` (5 unless set, 1 to 100), and from `config.radar` the `width` and `height`
(600), the margins (50 each), `axis_scale_factor` (1), `axis_label_factor` (1.05), and
`curve_tension` (0.17). A bad config value is a warning and ignored.

All axes are read before any curve, so a curve may come first. A repeated axis id is an error and
ignored. Listed values map to the axes in order; fewer values than axes is an error and the rest
sit at `min`, and more is an error and the extras are ignored. Keyed values map by axis id; a key
that is not an axis, a repeated key (the first wins), a value without a key among keyed ones, and
an axis with no value (sitting at `min`) are errors. A bad option value is an error and ignored,
as is a `max` at or under `min`. `Radar::bounds` gives the center and rim values (`min`, and
`max` or the largest value, the rim always above the center), and `Radar::share` a value's place
between them, clamped. Tests: `model`, `errors`, and the module's unit tests.

## MRADAR-3 Drawing

As in Mermaid: the radius is half the smaller of `width` and `height`; axis `i` of `n` points at
angle `2 pi i / n - pi / 2` (the first up, the rest clockwise), its line reaching
`axis_scale_factor` of the radius and its label `axis_label_factor` of it, pushed out by half the
label's size so a label beside the chart clears its axis. `ticks` rings, round or with a corner
on each axis, are drawn at even steps out to the radius with a faint fill. Each curve passes
through its values' points as a closed curve of cubics with Mermaid's `closedRoundCurve`
control points (`curve_tension`), filled at half opacity and stroked in its color (the git branch
palette). The legend lists the curves toward the top right, the title is centered above, and the
canvas grows to hold the labels and legend. Every element is owned by the diagram. Tests:
`drawing` and the scene's unit tests.

## MRADAR-4 ASCII

A group of rows per axis: the axis label on its first row, then for each curve its label, a bar of
24 cells for its share (eighth blocks in Unicode, `#` in ASCII), and its value. A line under the
groups gives the range, e.g. `0 to 100`. The title is centered above. Tests: `snapshots`,
`ascii_charset_snapshots`, and the renderer's unit tests.

## MRADAR-5 Not drawn

Theme variables (`cScale0` and up, `themeVariables.radar`) are ignored. Drawing edits are
read-only (LANGS-5).
