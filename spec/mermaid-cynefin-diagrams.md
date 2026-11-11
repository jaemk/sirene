# Mermaid Cynefin Diagrams

`cynefin-beta`, items in the five domains of the Cynefin framework (LANGS-2, LANGS-3,
MCHART-1). Reference: Mermaid 12.3.0's `docs/syntax/cynefin.md`, `diagrams/cynefin/`, and
`packages/parser/src/language/cynefin/cynefin.langium`. Code: `crates/sirene-core/src/cynefin/`,
`scene/cynefin.rs`, and `ascii/cynefin.rs`; `diagram::Kind::Cynefin` picks it by the header
keyword. Tests: `crates/sirene-core/tests/cynefin.rs` and the corpus under
`crates/sirene-core/tests/fixtures/cynefin/` (LANGS-6).

## MCYN-1 Syntax

`cynefin-beta` or `cynefin-beta:` after any frontmatter, then statements, one per line:

- `title text`, `accTitle: text`, `accDescr: text`, `accDescr { text }`.
- A domain line: `complex`, `complicated`, `clear`, `chaotic`, or `confusion`, then quoted items
  (`"..."` or `'...'`, `\` escapes a character) separated by whitespace.
- An item line: quoted items, valid only after a domain line or another item line.
- A transition: `from --> to`, optionally `: "label"`.

A `%%` comment may end a line. A string must close on its line. Keywords are lowercase. The
parser is lossless. Tests: `statements`, `errors` in `cynefin/parser.rs`; `snapshots`.

## MCYN-2 Model

- Items belong to the domain they follow, in source order. A domain declared again replaces its
  items, with a warning.
- A transition from a domain to itself is dropped, with a warning. An empty label is none.
- The title comes from `title` or the frontmatter. `showDomainDescriptions` under
  `config: cynefin:` (default true) shows each domain's decision model and practice.
- Confusion shows its first three items and a `+N more` count.

Tests: `model`, `warnings`, `confusion_cap`.

## MCYN-3 Drawing

Four tinted quadrants (Complex top left, Complicated top right, Chaotic bottom left, Clear
bottom right) split by dashed wavy boundaries, a heavy red cliff below the middle between
Chaotic and Clear, and a dashed confusion ellipse over the middle. Each domain centers its bold
name, its italic decision model and practice, and its item badges on its center; confusion's
overflow badge is dashed. The area starts at 800 by 600 and grows until each quadrant's text
fits inside it clear of the ellipse, which grows to hold confusion's text. Transitions are
curves between domain centers, bowed to the left of their direction, starting and ending
outside the domains' text, with an arrowhead and the label on a backing beside the curve's
middle. The title is above. Tests: `drawing`.

## MCYN-4 Text

The quadrants in a box split by dotted lines, with the cliff as a heavy line below the middle
and confusion boxed over the middle. Top domains' text is at the top of their quadrant and
bottom domains' at the bottom, centered, with items after a blank line. Transitions are listed
below as `From → To: label` (`->` in ASCII). Tests: `snapshots`, and `quadrants_and_confusion`
and `ascii_and_no_descriptions` in `ascii/cynefin.rs`.

## MCYN-5 Not drawn

Mermaid's `width`, `height`, `padding`, `boundaryAmplitude`, and `seed` options and the
`cynefin` theme variables; the boundaries' waviness is fixed. Drawing edits are read-only
(LANGS-5).
