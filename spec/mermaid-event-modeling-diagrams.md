# Mermaid Event Modeling Diagrams

`eventmodeling`, frames on a timeline in swimlanes (LANGS-2, LANGS-3, MCHART-1). Reference:
Mermaid 12.3.0's `docs/syntax/eventmodeling.md`, `diagrams/eventmodeling/`, and
`packages/parser/src/language/eventmodeling/`. Code: `crates/sirene-core/src/eventmodel/`,
`scene/eventmodel.rs`, and `ascii/eventmodel.rs`; `diagram::Kind::EventModel` picks it by the
header keyword. Tests: `crates/sirene-core/tests/eventmodel.rs` and the corpus under
`crates/sirene-core/tests/fixtures/eventmodel/` (LANGS-6).

## MEVM-1 Syntax

`eventmodeling` after any frontmatter, then statements:

- `title text`, `accTitle: text`, `accDescr: text`, `accDescr { text }`.
- `tf`/`timeframe` or `rf`/`resetframe`, a frame number (one to three digits), an entity type
  (`ui`, `pcr`/`processor`, `cmd`/`command`, `rmo`/`readmodel`, `evt`/`event`), a name
  (`Id` or `Ns.Id`, more dots allowed), any `->> NN`, an optional `[[Data]]`, and optional
  inline data (`{...}` to the line's last `}`, `"..."`, or `'...'`) after an optional
  `` `type` ``. One line.
- `data Name` and `note NN`, an optional `` `type` ``, and `{` ending its line; the block runs
  to a `}` line indented no deeper than the statement.
- `entity Name`.
- `gwt NN given items (when items)? then items`, each item `type Name`, across lines.

Data types are `json`, `jsobj`, `figma`, `salt`, `uri`, `md`, `html`, and `text`. Comments are
`%%`, `//`, and `/* */`. The parser is lossless. Tests: `statements`, `lengths`, `errors`,
`indented_blocks` in `eventmodel/parser.rs`; `snapshots`.

## MEVM-2 Model

- Frames in source order. A repeated frame number is an error and the later frame is dropped.
- A name `Ns.Id` has namespace `Ns`; a name with more dots has none.
- Data: a referenced data block's lines without their common indent, else the inline data's
  text as one line. An undefined data block is an error.
- Lanes: UI/Automation (UIs, processors), Command/Read Model, and Events, each group's own lane
  first, then a lane per namespace in order of first use, labeled `UI/A: Ns`, `C/RM: Ns`, or
  `Stream: Ns`. Only lanes with frames exist.
- Relations: a frame with `->>` sources takes them, deduplicated, in frame order; otherwise a
  time frame takes the latest earlier frame in another lane, and a reset frame takes none.
  Undefined frames are errors; a source of the wrong type (Mermaid's validator) is an error and
  the relation is kept; a frame's relation to itself is dropped with a warning.
- Notes and `gwt` scenarios report undefined frames; otherwise they are not used.
- The title comes from `title` or the frontmatter.

Tests: `model`, `lane_order`, `explicit_relations`, `diagnostics`.

## MEVM-3 Drawing

Lanes are rounded bands stacked top to bottom with a 10 px gap, each as tall as its tallest box
plus 15 px above and below (at least 100 px), with its bold label at the top left. Boxes are
filled with their type's Mermaid color and hold the bold name, wrapped at 430 px, and the data
lines below it. A box's text area is clamped to 80..450 by 80..750 px, then padded. The first
box starts at 250 px (more for long labels); a box in the previous frame's lane follows that
lane's last box, and any other starts 70 px before the previous box ends, but after its own
lane's last box. Relations are straight arrows from two thirds along the source's bottom (or top,
going up) to a third along the target's top (or bottom); in one lane, from side to side. The
title is above. Tests: `drawing`.

## MEVM-4 Text

The lanes as rows of a box split by dotted lines, each label at its lane's left. Each frame is a
box with ` NN type ` in its top border, the name centered, and a rule above the data lines. Boxes
follow the scene's order: after the lane's last box, or starting halfway along the previous
frame's box. A relation between lanes is a vertical arrow in a column inside both boxes and clear
of others; the rest are listed below as `NN Name → NN Name` (`->` in ASCII). Tests: `snapshots`,
and `wrapping`, `lanes_boxes_and_arrows`, `upward_and_listed` in `ascii/eventmodel.rs`.

## MEVM-5 Not drawn

Notes, `gwt` scenarios, `entity`, data types, and the `em*` theme variables. Mermaid does not
draw the title; sirene does. Drawing edits are read-only (LANGS-5).
