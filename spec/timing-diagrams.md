# Timing Diagrams

The timing model (`sirene_core::timing`): signals whose state changes over time, drawn on one
time axis. PlantUML timing diagrams lower to it (`plantuml-timing-diagrams.md`, PTIM). Tests:
unit tests in `crates/sirene-core/src/timing/mod.rs`, and the PlantUML fixtures `timing.puml`
and `errors/timing.puml`.

## TIMING-1 Model

A `Timing` has a title, signals in order, messages, constraints, highlights, whether the axis is
hidden, and the axis labels of moments written as dates or times. Moments are numbers.

## TIMING-2 Signals

A signal has an id, a label, a kind (robust, concise, rectangle, binary, clock, analog), its
declared states (name and label, top first), its changes (moment, value, fill, note), and notes
above or below it. A value is a state, hidden, or undefined. A clock has a period, a pulse
(half the period by default), and an offset; an analog signal may have a fixed scale.

## TIMING-3 Time

The drawn span runs from the earliest moment to the latest plus a tail (a tenth of the span, at
least one unit and one clock period), so the last state shows. A signal's segments run from
each change to the next; a later change at the same moment wins. A clock's segments are its
periods, at most 1000 of them. Ticks are the written dates and times, or round steps (1, 2, or 5
times a power of ten) across the span.

## TIMING-4 Drawing

Names are in a left column, with robust state labels in a second column at their levels.
Robust, binary, and clock signals are lines stepping between levels; analog signals are lines
through their values; concise signals are bands with slanted ends and the state name in them,
in a box for rectangle signals. Undefined spans are grey, hidden spans empty. Messages are
arrows between signal centers, constraints two-headed arrows under their signal (or under the
axis), highlights shaded spans with their caption above, and notes boxes above or below their
signal. The axis has a tick and label per tick unless hidden.

## TIMING-5 Text

The text drawing (ASCIIR-1) has the same rows on a 60-column axis: level lines joined by
vertical steps, concise bands as boxes sharing borders, undefined spans dotted or `/`, messages
as dotted vertical lines with an arrowhead into the target signal, constraints as `<-- label -->`
spans, and highlights as a double rule over their span.
