# PlantUML Chronology Diagrams

PlantUML `@startchronology` documents (PUML-2), drawn as a timeline (TIMELI-2)
(`sirene_core::plantuml`). Reference: <https://plantuml.com/chronology-diagram>. Tests: unit
tests in `crates/sirene-core/src/plantuml/chronology.rs`, and the fixtures `chronology.puml`
and `errors/chronology.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PCHRON-1 Events

`[Name] happens on DATE` (or `happens at`) is an event. `DATE` is `YYYY-MM-DD`, optionally
followed by `HH:MM` or `HH:MM:SS`. `title` and the shared settings apply (PSEQ-6).

## PCHRON-2 Drawing

Events are drawn in date order, one period per moment labeled with the date as written; events
at the same moment share a period, in source order.

## PCHRON-3 Diagnostics

An invalid date, an unclosed `[`, and any other line are errors.
