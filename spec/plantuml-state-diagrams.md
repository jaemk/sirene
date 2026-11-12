# PlantUML State Diagrams

PlantUML state diagrams (PUML-2), lowered to the graph model with the state shapes of MSTATE-2
and drawn by the layered layout, scene builder, and ASCII renderer (`sirene_core::plantuml`).
Reference: <https://plantuml.com/state-diagram>; the fixtures are written for sirene. Tests:
unit tests in `crates/sirene-core/src/plantuml/state.rs`, the fixtures `states.puml` and
`regions.puml` with `crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PSTA-1 Documents and states

A document (PSEQ-1) is a state diagram when its first deciding line is `state ...` or starts
with `[*]` (PUML-1). `title` and `left to right direction` apply as in PACT-1. `state Name`,
`state "Display" as Name`, and `state Name as "Display"` declare a rounded state; a name used in
a transition is declared there. `hide empty description` is accepted.

## PSTA-2 Transitions

`A --> B : event` is a transition with the link syntax of PCLS-4: one dash keeps both states in
a rank, direction words and `-[#color,dotted]->` styles apply, and the label follows `:`.
`[*]` is the start state of the enclosing composite state as a source and its end state as a
target. `[H]` and `[H*]` are the shallow and deep history of the enclosing state, and
`Name[H]` that of composite state `Name`.

## PSTA-3 Pseudo-states

`<<choice>>` is a diamond, `<<fork>>` and `<<join>>` a bar, `<<start>>` and `<<end>>` the
start and end circles, `<<history>>` and `<<history*>>` a circle with `H` or `H*`, and
`<<entryPoint>>` and `<<exitPoint>>` a circle with the name. Other stereotypes are warnings, and
the state is drawn plain.

## PSTA-4 Descriptions and colors

`Name : text` and `state Name : text` add a line below the state's name, in a compartment;
each further one adds a line. On a composite state the text adds a line to its title. `#color`
after the name fills the state; a `##[style]color` border is a warning.

## PSTA-5 Composite states and regions

`state Name {` ... `}` is a composite state, drawn as a titled cluster around the states
declared in it; composite states nest, and opening one again adds to it. Inside one, a `--` or
`||` line starts a concurrent region, drawn as a dashed cluster; the statements before the
first separator are the first region. A separator outside a composite state is a warning, and
a transition between two regions of one composite state is a warning and is not drawn.

## PSTA-6 Notes

Notes follow PCLS-8: `note left|right|top|bottom of Name : text` or a block to `end note`, a
note after a declaration with no `of`, `note "text" as N`, and `note on link`. A note may be on
a composite state.

## PSTA-7 Diagnostics

A line that is not a state statement, a `}` with no open composite state, and an unclosed
composite state are errors. Settings that are not drawn are warnings, as in PACT-9.
