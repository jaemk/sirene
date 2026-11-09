# Mermaid Sequence Diagrams

`sequenceDiagram`, with its own model, layout, scene builder, and ASCII renderer (LANGS-2).
PlantUML sequence diagrams lower to the same model (PUML-3). Reference:
<https://mermaid.js.org/syntax/sequenceDiagram.html>.

## MSEQ-1 Syntax

- Participants: `participant A`, `actor A`, `participant A as Alice`, and the typed forms
  (`A@{ "type": "database" }`: boundary, control, entity, database, collections, queue);
  `create participant` and `destroy A`; `box Color Title ... end`.
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
Activations are spans on a participant from activation to deactivation; an unbalanced one is a
diagnostic and closes at the end.

## MSEQ-3 Layout

Participants are columns. Each column is as wide as its header; the gap between two neighbors
fits the longest message label between them. Every message, note, and block edge takes a row, in
event order; a self-message loops to the right over two rows. Activation bars are offset
sideways when nested. A block is a frame spanning the columns its events touch, with its kind and
label in a tab and dashed lines at its dividers. Headers are drawn at the top and again at the
bottom; a created participant's header sits at its creation row, and a destroyed one's lifeline
ends at a cross.

## MSEQ-4 Drawing

Solid and dashed lines, filled and open arrowheads, crosses, and async half-arrows as the arrow
names. Actors are stick figures, and the typed participants use their shapes. In ASCII, lifelines
are `│` (`|`), messages `────►` and `- - ►` (`--->`, `- ->`), and frames use the box characters.
Elements hit-test as participants, messages, and notes.

## MSEQ-5 Editor

Drawing edits are read-only at first (LANGS-5). The planned edit module adds a participant, adds
a message between two lifelines by dragging, edits labels, and reorders messages.
