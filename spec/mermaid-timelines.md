# Mermaid Timelines

`timeline`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/timeline.html>. Code: `crates/sirene-core/src/timeline/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/timeline.rs` and
the corpus under `crates/sirene-core/tests/fixtures/timeline/` (LANGS-6).

## TIMELI-1 Syntax

- Header: `timeline`, optionally followed by `LR` (the default) or `TD`.
- `title TEXT` and `section TEXT`, each to the end of the line.
- A time period and its events: `PERIOD : EVENT : EVENT`. A line that starts with `:` adds
  events to the period before it. A `:` separates only when followed by whitespace or the end of
  the line, so `10:30 : standup` is the period `10:30`.
- `<br>` in any text breaks the line.
- Comments (`%%`, and a line starting with `#` as in Mermaid's lexer), directives, frontmatter
  `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests:
`snapshots`, `syntax_errors`.

## TIMELI-2 Model

`timeline::Timeline`: the title (the `title` statement, else frontmatter `title`), the
direction, and sections in order, each with an optional name and its periods in order; periods
before the first `section` are in an unnamed section. A period has its label and its events in
order. Events before any period and a `section` without a name are errors, and an event with no
text is a warning; each is skipped. A section without periods draws nothing. Tests:
`periods_and_events`, `sections`, `errors`.

## TIMELI-3 Drawing

`LR`: the periods in a row, left to right, each a filled box with its events stacked below it
and joined to it by a dashed line, across a horizontal arrow under the periods. A named section
is a box above its periods, spanning them. Each section takes the next of eight colors; when
there are no named sections, each period does instead. Columns are as wide as their text, up to
200 pixels, past which text wraps at spaces. `TD`: the periods in a column, top to bottom, each
with its events in a row to its right, across a vertical arrow beside them; a section's box is
to the left of its periods, spanning them. Every element is owned by the diagram. Tests:
`drawing`.

## TIMELI-4 ASCII

The same boxes drawn with box characters, with text wrapped at 24 columns: for `LR` the periods
in a row of boxes, an arrow (`►`, `>` in ASCII) under them with `┼` where each period meets it,
and each period's events in boxes stacked below; section boxes above. For `TD` the periods in a
column with an arrow (`▼`, `v`) down their left, and each period's events in boxes to its right;
section boxes to the left. Tests: `snapshots`, `ascii_charset_snapshots`.

## TIMELI-5 Not drawn

Frontmatter `config` for `timeline` (`disableMulticolor`) and theme colors (`cScale0` and the
rest) are ignored.
