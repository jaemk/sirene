# PlantUML Sequence Diagrams

PlantUML sequence diagrams (PUML-3), lowered to the sequence model (MSEQ-2) and drawn by its
layout, scene builder, and ASCII renderer (`sirene_core::plantuml`). Reference: the PlantUML
language reference guide and <https://plantuml.com/sequence-diagram>; the fixtures are written
for sirene, not copied from it. Tests: unit tests in `crates/sirene-core/src/plantuml/`, the
fixture corpus in `crates/sirene-core/tests/fixtures/plantuml/` with
`crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PSEQ-1 Documents

The diagram is the first `@startuml` ... `@enduml` document (PUML-1); text before it is
ignored, a missing `@enduml` is a warning and the document runs to the end, and a second
document is a warning ("only the first diagram in a file is drawn"). A source with no `@start`
line (a Markdown block, or `--lang plantuml`) is the diagram's statements without them. `@startuml` may name the
diagram (`@startuml name`), which is ignored. Other `@start...` kinds are an error naming the
kind. Comments are a line starting with `'` and `/' ... '/`, which may span lines. Blank lines
are ignored. Keywords match in any case; participant names match exactly.

A document is a sequence diagram unless a statement only another kind accepts comes first
(PUML-1): `start` or `:action;` is an activity diagram (PACT), `class`, `enum`, or `object` a
class diagram (PCLS), `state` or `[*]` a state diagram (PSTA), `usecase`, `component`, `(x)`,
or `[x]` a use case, component, or deployment diagram (PDSC), and `robust`, `concise`, and the
like are kinds that are not drawn yet, an error naming the kind ("PlantUML timing diagrams are
not supported yet").

## PSEQ-2 Participants

- `participant`, `actor`, `boundary`, `control`, `entity`, `database`, `collections`, and
  `queue` declare a participant of that kind (MSEQ-4) in declaration order; a message to an
  undeclared name declares a `participant`.
- Names are a word or a `"quoted string"`; `participant "Long name" as L` and
  `participant L as "Long name"` give an id and a label. `\n` in a label breaks the line.
- `order N` sorts participants by `N` (stable; unordered ones keep their place after the
  ordered ones before them, as PlantUML does). A `#color` and a `<<stereotype>>` are a
  warning and ignored.
- `box "Title" #color` ... `end box` groups participants (MSEQ-1's `box`).
- `create A` before a message, or `**` after its target, draws `A`'s header at that message
  (MSEQ-2 creations); `destroy A`, or `!!` after the target, ends its lifeline there.
- `hide footbox` drops the headers at the bottom.

## PSEQ-3 Messages

`FROM ARROW TO [: text]`, with optional spaces:

- The line is `-` (solid) or `--` (dashed); the head is `>` (filled), `>>`, `\`, `/`, `\\`, or
  `//` (open), `x` before or after the head (cross), and `o` after it (drawn as filled, a
  warning once). `<-` and `<<-` point back: the message runs from `TO` to `FROM`. `<->` has a
  head at both ends.
- `-[#color]>` and other bracketed styles are a warning and drawn plain.
- `[->`, `[o->`, and `[x->` come in from the left edge of the diagram, and `->]` (and `->o]`,
  `->x]`) go out to the right edge; the edges are hidden participants (MSEQ-2's hidden kind)
  at the start and end. `?->` and `->?` are the same, short.
- After the target, `++` activates it, `--` deactivates the source, `**` creates the target,
  `!!` destroys it, and `++` with a `--` (`++--`, in either order) does both.
- `return text` is a dashed message from the participant activated last to the one whose
  message activated it, and deactivates it; with nothing active it is an error.
- `autoactivate on` makes each following message activate its target, as `++` does (except
  one with `!!`), until `autoactivate off`; `return` then closes the activation. Any other
  argument is an error.
- `activate A`, `deactivate A`; activation colors (`activate A #red`) are a warning.
- `autonumber [start] [step]` numbers the following messages; `autonumber stop` and
  `autonumber resume [step]` pause and continue; a format string is a warning and ignored.

## PSEQ-4 Notes and frames

- `note left`, `note right` (of the previous message's source or target, as PlantUML places
  them), `note left of A`, `note right of A`, `note over A[, B]`, and `note across` (over every
  participant), each with `: text` on one line or the following lines up to `end note`
  (`end hnote` and `end rnote` for those keywords). `hnote` and `rnote` are notes. A `#color` is a warning.
- `alt`, `else`, `opt`, `loop`, `par`, `break`, `critical`, and `group` open frames, each
  closed by `end` (MSEQ-2 blocks); `group A [B]` is labeled `A`, with `B` after it. `else` is
  the only divider, in any frame, as PlantUML allows. Leading `#color` words on a frame or
  `else` label (`alt#Gold #LightBlue text`) are a warning and left out of the label.
- `== text ==` is a separator: a rule across the diagram with the text in the middle.
- `...` is a delay: a gap with dotted lifelines; `... text ...` labels it.
- `|||` is a blank row; `||N||` is `N` pixels of space, at least one row in ASCII.
- `ref over A[, B] : text` (or the lines up to `end ref`) is a frame tagged `ref` over the
  participants, holding the text.

## PSEQ-5 Model additions

The sequence model gains the events PSEQ-4 needs, drawn by MSEQ-3 and MSEQ-4 as described
there: `Event::Separator(text)`, `Event::Delay(Option<text>)`, `Event::Space(Option<pixels>)`
(`None` for `|||`), and `Event::Ref { participants, text }`; `BlockKind::Group` for `group`,
whose tab holds its label (`BlockKind::tab`); and `Sequence::hide_footbox`. Mermaid never
produces them. A separator and a delay take a row across the whole diagram and widen it to fit
their text; a separator is a double rule with its text in a box (`══ text ══` in ASCII), and a
delay dashes the lifelines (`┆`). A space is `N` pixels, or a text line for `|||`; in ASCII it
is one row per 20 pixels, at least one. A `ref` is placed as a note over its participants, with
a `ref` tab. Done: tests `plantuml_events` in `crates/sirene-core/tests/sequence.rs`.

## PSEQ-6 Not drawn yet

`title text`, and `title` ... `end title`, is the diagram's title. These are warnings and
ignored: `header`, `footer`, `caption`, and `legend` ... `endlegend`; `skinparam` and `!theme` (PUML-4); colors and stereotypes;
`newpage` (everything is on one page); `!pragma`; creole and HTML markup in
text, which is drawn as written. `!include`, `!includeurl`, and `!import` are errors (PUML-4);
`!define`, `!function`, variables, and conditionals are a warning and their lines ignored.
Unknown lines are errors, as in Mermaid (LANGS-6).

## PSEQ-7 Choosing the language

PlantUML is detected by its `@start` line, its extension, its Markdown block name, or
`--lang plantuml` (LANGS-1); `Lang::PlantUml` names it. Drawing edits are read-only, as for
Mermaid sequence diagrams (MSEQ-5).
