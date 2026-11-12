# PlantUML Timing Diagrams

PlantUML timing diagrams (PUML-2), lowered to the timing model (TIMING-1)
(`sirene_core::plantuml`). Reference: <https://plantuml.com/timing-diagram>. Tests: unit tests
in `crates/sirene-core/src/plantuml/timing.rs`, and the fixtures `timing.puml` and
`errors/timing.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PTIM-1 Signals

`robust`, `concise`, `rectangle`, `binary`, `clock`, and `analog` declare a signal:
`robust "Label" as ID`. A clock takes `with period N`, and optionally `pulse N` and `offset N`;
an analog signal takes `between LOW and HIGH`. `compact` before a declaration, `mode compact`,
and stereotypes are accepted and not drawn. `ID has a, b, c` and `ID has "Label" as name`
declare states in order and their labels.

## PTIM-2 Moments

`@100` sets the current moment; `@+50` adds to it; `@100 as :name` names it, and `:name`,
`:name+5`, and `:name-5` refer to it. `@clk*2` is two periods of the clock `clk`. Dates
(`@2019/07/02`) and times of day (`@1:15:00`) are moments in seconds, labeled on the axis as
written. `@ID` starts lines of `TIME is VALUE` for that signal, up to the next `@` line.

## PTIM-3 Changes

`ID is VALUE` changes a signal at the current moment, and `ID@TIME is VALUE` at another.
`{hidden}` hides it, `{-}` and `{a,b}` make it undefined, `#color` fills a concise state, and
`: text` adds a note at the change. A binary value is `high` or `low`; an analog value is a
number.

## PTIM-4 Messages, constraints, highlights, and notes

- `A -> B : text`, `A -> B@+50 : text`, `A@100 -> B@350 : text`: messages.
- `A@0 <-> @50 : {50 ms}` and `@200 <-> @+150 : {150 ms}`: constraints on a signal or under
  the axis.
- `highlight 200 to 450 #Gold : caption`, with moments or `:name` anchors.
- `note top of ID : text` and `note bottom of ID` ... `end note`.
- `hide time-axis` hides the axis.

## PTIM-5 Diagnostics

A redeclared signal, a clock with no period, a bad `between`, an unknown word in a
declaration, an unknown signal or anchor, an unreadable moment, an analog value that is not a
number, and an unknown line are errors. A change to a clock and `manual time-axis` are
warnings.
