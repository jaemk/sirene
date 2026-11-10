# Mermaid Journeys

`journey`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/userJourney.html>. Code: `crates/sirene-core/src/journey/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/journey.rs` and
the corpus under `crates/sirene-core/tests/fixtures/journey/` (LANGS-6).

## MJOUR-1 Syntax

- Header: `journey`.
- `title TEXT` and `section TEXT`, the text stopping at `#` or `;` (and for a section, `:`).
- A task: a name up to `:`, then the data up to `#`, `;`, or the end of the line
  (`Make tea: 5: Me, Cat`).
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A name without
`:` is an error. Tests: `snapshots`, `syntax_errors`, and the parser's unit tests.

## MJOUR-2 Model

`journey::Journey`: the title (the `title` statement, else frontmatter `title`), the
accessibility title and description, and the sections in order, each with an optional name
(`None` for the tasks before the first `section`) and its tasks. A task has a name, a score, and
its actors: the data split at `:` into the score and a comma-separated actor list, as Mermaid
does. A missing or non-numeric score is an error and the task is skipped; a score outside 1 to 5
is a warning and is clamped; parts after the actors are a warning and ignored. A `section` with
no name is an error. `Journey::actors` lists every actor once in order of first appearance.
Tests: `tasks`, `errors`.

## MJOUR-3 Drawing

The actors down the left, each a colored dot and its name. The tasks in a row of 150-pixel boxes,
their names wrapped, under a box per named section spanning its tasks; section `k` takes the
`k`th branch color, and its tasks a light tint of it. Each task shows a dot per actor along its
top edge. Below each task a dashed line runs to a face, placed lower for a lower score (30 pixels
per point); it smiles above 3, frowns below 3, and is level at 3. An arrow runs under the faces
across every task. The title is centered above. Every element is owned by the diagram. Tests:
`drawing`.

## MJOUR-4 ASCII

Each named section's name, then its tasks indented, one per row: the name padded to the longest,
a bar of five steps for the score rounded (`●`/`○`, or `*`/`.` in ASCII), the score, and the
actors. The title is centered above. Tests: `snapshots`, `ascii_charset_snapshots`.

## MJOUR-5 Not drawn

Frontmatter `config` for `journey` (box sizes, colors, `actorColours`) and theme variables are
ignored. Drawing edits are read-only (LANGS-5).
