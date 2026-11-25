# Mermaid Sequence Diagrams

`sequenceDiagram`, with its own model, layout, scene builder, and ASCII renderer (LANGS-2).
PlantUML sequence diagrams lower to the same model (PUML-3). Reference:
<https://mermaid.js.org/syntax/sequenceDiagram.html>.

## MSEQ-1 Syntax

- Participants: `participant A`, `actor A`, `participant A as Alice`, and the typed forms
  (`A@{ "type": "database" }`: boundary, control, entity, database, collections, queue).
  Braces in quoted metadata values are literal. The parser also accepts `create participant`
  and `destroy A`. A `box Color Title ... end` block holds only declarations. A participant
  stays in the first `box` that declares it, and a declaration in another is an error.
  Tests include `box_contents` and `quoted_participant_metadata_delimiters`.
- Messages `A->>B: text` with arrows `->`, `-->`, `->>`, `-->>`, `-x`, `--x`, `-)`, `--)`,
  `<<->>`, `<<-->>`; `+` and `-` after the arrow activate and deactivate the target.
- `activate A`, `deactivate A`; `autonumber`, `autonumber START`, `autonumber START STEP`,
  and `autonumber off`. Every message advances the counter, even while numbering is off.
  Bare `autonumber` resumes the current number and step; a nonzero `START` sets the next
  number, and `autonumber START` resets the step to 1. A supplied zero leaves the current
  number or step unchanged (except the single-argument form still resets the step to 1).
- Notes: `note left of A`, `note right of A`, `note over A,B`.
- Blocks: `loop`, `alt` / `else`, `opt`, `par` / `and`, `critical` / `option`, `break`, `rect`,
  each closed by `end`, nested.
- Comments, `accTitle`, `accDescr`; `link` and `links` are warnings (LANGS-6).
- `$$...$$` in a participant alias, message text, note, block label (`loop`, `alt`, `opt`,
  `par`, `critical`, `break`), or section title (`else`, `and`, `option`) is a formula (LEMB-1).
  `box` and diagram titles stay text. `\\` is kept as written. The model keeps the source in
  `Text`; `Sequence::math` is set by this lowering.

## MSEQ-2 Model

Participants in order of declaration or first mention, each with a kind and label. Events in
order: messages (from, to, arrow, label, activation change), notes (side and participants),
block openings, dividers (`else`, `and`, `option`), and closings, creations, and destructions.
PlantUML adds separators, delays, spaces, `ref` frames, `group` blocks, and a hidden footbox
(PSEQ-5). Activations are spans on a participant from activation to deactivation. Deactivating a
participant that is not active is an error; one left active is a warning and closes at the end.
Blocks are balanced: a divider outside its block (`else` in `alt`, `and` in `par`, `option` in
`critical`) and an `end` with nothing to close are errors, and a block left open is an error and
closes at the end. Keywords match in any case. Model: `sequence::Sequence`; tests:
`crates/sirene-core/tests/sequence.rs` and its fixtures.

## MSEQ-3 Layout

Participants are columns. Each column is as wide as its header; the gap between two neighbors
fits the longest message label between them. Every message, note, and block edge takes a row, in
event order; a self-message loops to the right over two rows. Activation bars are offset
sideways when nested. A block is a frame spanning the columns its events touch, with its kind and
label in a tab and dashed lines at its dividers. Headers are drawn at the top and again at the
bottom; a created participant's header sits at its creation row, and a destroyed one's lifeline
ends at a cross at its next message. One layout (`sequence::layout`) serves the scene and the
text renderer, in pixels or cells.

## MSEQ-4 Drawing

Solid and dashed lines, filled and open arrowheads, crosses, and async half-arrows as the arrow
names. Actors are stick figures, and the typed participants use their shapes; `autonumber` puts a
numbered badge at each message's start. In ASCII, lifelines are `│` (`|`) and heavy while active,
messages `────►` and `┄┄┄►` (`--->`, `...>`), with `>` for an open head and `×` (`x`) for a
cross; actors are stick figures and the other participants boxes; frames use the box characters,
`rect` and `box` dotted ones; numbers prefix the label (`1. `). Every element is owned by the
diagram (LANGS-2).

## MSEQ-5 Editor

`sequence::edit` takes drawing edits (WYSIWY-21): participants and messages are selected,
participants are added, renamed, reordered, and deleted, a message is added by dragging between
two lifelines, and message text and arrows are edited.
