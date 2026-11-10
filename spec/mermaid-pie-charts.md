# Mermaid Pie Charts

`pie`, lowered to its own chart model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/pie.html>. Code: `crates/sirene-core/src/pie/`; `diagram::lower`
picks it by the header keyword. Tests: `crates/sirene-core/tests/pie.rs` and the corpus under
`crates/sirene-core/tests/fixtures/pie/` (LANGS-6).

## MPIE-1 Syntax

- Header: `pie`, optionally followed by `showData`; a `title` may follow on the same line
  (`pie showData title Pets`).
- `title TEXT` to the end of the line.
- A slice: a label in double or single quotes, `:`, and a number (`"Dogs" : 386`, `'Cats': 85.5`).
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Keywords are
case-sensitive, as in Mermaid. Tests: `snapshots`, `syntax_errors`.

## MPIE-2 Model

`pie::Pie`: the title (the `title` statement, else frontmatter `title`), whether `showData` is
set, and the slices in order, each a label and a value. A negative value is an error and the
slice is skipped; a label given again is a warning and the later slice is skipped, as Mermaid
keeps the first. `Pie::ordered` gives the slices largest first (ties in source order), and
`pie::percent` a slice's share rounded to a whole percent. Tests: `slices`, `errors`.

## MPIE-3 Drawing

A circle cut into slices in `Pie::ordered` order, clockwise from twelve o'clock, each with the
next of eight colors and its percentage inside it. A slice that rounds to `0%` is not drawn. A
legend to the right lists every slice in the same order with its color, and with `showData` its
value in brackets (`Dogs [386]`). The title is centered above. When every value is zero the circle
is drawn empty. Every element is owned by the diagram. Tests: `drawing`.

## MPIE-4 ASCII

A table, one row per slice in `Pie::ordered` order: the label, a bar whose length is the slice's
share of 40 cells, the percentage, and with `showData` the value. Bars use `█` and the eighth
blocks (`▏` to `▉`) in Unicode and `#` in ASCII. The title is centered above. Tests: `snapshots`,
`ascii_charset_snapshots`.

## MPIE-5 Not drawn

Frontmatter `config` for `pie` (`textPosition`) and theme variables (`pie1` to `pie12`,
`pieStrokeColor`, and the rest) are ignored. Drawing edits are read-only (LANGS-5).
