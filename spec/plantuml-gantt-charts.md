# PlantUML Gantt Charts

PlantUML Gantt charts (PUML-2), lowered to the gantt model (MGANTT-3) and drawn by its scene
builder and ASCII renderer (`sirene_core::plantuml`). Reference:
<https://plantuml.com/gantt-diagram>; the fixtures are written for sirene. Tests: unit tests in
`crates/sirene-core/src/plantuml/gantt.rs`, the fixtures `gantt.puml` and `errors/gantt.puml`
with `crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PGANTT-1 Documents

A `@startgantt` document is a Gantt chart, as is a `@startuml` document whose first deciding
line is `Project starts ...` or `[Task] lasts|requires|starts|ends|happens ...` (PUML-1).
`title` applies; a direction is a warning, and other settings are warnings as in PACT-9.

## PGANTT-2 Tasks

`[Name]` names a task, created at its first mention; `[Name] as [N]` gives it an alias. A task
line is clauses joined by `and`:

- `lasts N days` or `requires N weeks` (a week is 7 days);
- `starts WHEN`, `ends WHEN`, and `happens WHEN` (a milestone, which takes no days);
- `is N% complete` (100% draws the task done, less draws it active), and `is deleted`;
- `is colored in ...`, `on {resource}`, `displays on same row as`, `links to`, and `pauses`
  are warnings.

`then [Task] ...` starts the task at the end of the task before it. `[A] -> [B]` starts `B` at
the end of `A`. `-- Name --` starts a section.

## PGANTT-3 Placing

`WHEN` is a date (`2026-01-05` or `2026/01/05`, with an optional `at`, `on`, or `the`),
`D+N` (days after the project's start), `[X]'s start` or `[X]'s end`, or
`N days after|before [X]'s end|start`. An `ends` date is the task's last day. A task with a
start and a duration ends that many open days later, a task with an end and a duration starts
that many open days earlier, and a task with neither starts at the project's start; the
duration defaults to one day. A start on a closed day moves to the next open day.
`Project starts DATE` sets the project's start; with none, it is the earliest date written,
and with no dates the axis counts days from 1. A task that refers to a task no line names, or
whose references form a cycle, is an error and is not drawn.

## PGANTT-4 Calendar and axis

`saturday are closed` (any weekdays, singular or plural), `DATE is closed`, and
`DATE to DATE are closed` close days, which durations skip and the chart shades; `... is
open` opens them again. `printscale` (or `projectscale`, `ganttscale`) `daily`, `weekly`,
`monthly`, `quarterly`, or `yearly` sets the axis ticks. `today is DATE` (or `today is N days
after start`) marks that day; without it no day is marked.

## PGANTT-5 Diagnostics

A line that is not a Gantt statement, a clause that cannot be read, a bad date, and a task that
cannot be placed are errors.
