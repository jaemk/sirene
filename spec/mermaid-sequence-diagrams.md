# Mermaid Sequence Diagrams

`sequenceDiagram`, with its own model, layout, scene builder, and ASCII renderer (LANGS-2).
PlantUML sequence diagrams lower to the same model (PUML-3). Reference:
<https://mermaid.js.org/syntax/sequenceDiagram.html>.

## MSEQ-1 Syntax

- Participants: `participant A`, `actor A`, `participant A as Alice`, and the typed forms
  (`A@{ "type": "database" }`: boundary, control, entity, database, collections, queue);
  `create participant` and `destroy A`; `box Color Title ... end`, which holds only
  declarations (anything else is an error, as in Mermaid); a participant stays in the first
  `box` that declares it, and a declaration in another is an error. Test: `box_contents`.
- Messages `A->>B: text` with arrows `->`, `-->`, `->>`, `-->>`, `-x`, `--x`, `-)`, `--)`,
  `<<->>`, `<<-->>`; `+` and `-` after the arrow activate and deactivate the target.
- `activate A`, `deactivate A`; `autonumber`.
- Notes: `note left of A`, `note right of A`, `note over A,B`.
- Blocks: `loop`, `alt` / `else`, `opt`, `par` / `and`, `critical` / `option`, `break`, `rect`,
  each closed by `end`, nested.
- Comments, `accTitle`, `accDescr`; `link` and `links` are warnings (LANGS-6).

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

Drawing edits are read-only at first (LANGS-5). The planned edit module hit-tests participants,
messages, and notes, adds a participant, adds a message between two lifelines by dragging, edits
labels, and reorders messages.
