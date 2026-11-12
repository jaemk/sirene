# PlantUML Embedded Languages

PlantUML documents written in another language sirene reads (PUML-2): `@startdot` and
`@startebnf` (`sirene_core::plantuml`). Tests: the fixtures `dot.puml`, `ebnf.puml`,
`errors/dot.puml`, and `errors/ebnf.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PEMB-1 Documents

The lines between `@startdot` or `@startebnf` and the `@end` line are the other language's
source, at their places in the file, so diagnostics point into it. `'` starts no PlantUML
comment in them.

## PEMB-2 DOT

A `@startdot` document is a Graphviz graph, read and drawn as a `.dot` file is (DOT-*). The
graph's `bgcolor` is a warning: a PlantUML document has no background of its own.

## PEMB-3 EBNF

A `@startebnf` document is EBNF rules, read and drawn as a `railroad-ebnf-beta` diagram is
(MRAIL-*): `title`, `name = definition ;`, `,` sequences, `|` choices, `[ ]` options,
`{ }` repetitions, `( )` groups, `? special ?`, and `(* comments *)`.
