# PlantUML

Status: research; sequence diagrams are specified in `plantuml-sequence-diagrams.md` (PSEQ)
activity diagrams in `plantuml-activity-diagrams.md` (PACT), and class diagrams in
`plantuml-class-diagrams.md` (PCLS).
PlantUML (<https://plantuml.com>) has a dozen diagram kinds and no formal grammar; its
reference is the Java implementation and the language reference guide. Full compatibility is
not the goal. The useful subset reuses the sequence and graph models.

## PUML-1 Documents

A document is the text between `@startuml` and `@enduml` (or another `@start...` pair); a file
can hold several, which the editor steps through as it steps through Markdown blocks
(WYSIWY-20). Text outside them is ignored. The kind is detected as PlantUML does
(`plantuml::kind`): the `@start` word first (`@startmindmap`, `@startgantt`, `@startwbs`,
`@startjson`, `@startyaml`, `@startsalt`, and the like), then the first line only one kind
accepts (`start` or `:action;` for activity, `class` for class, `[*]` or `state` for state,
`(x)` or `[x]` links for description diagrams, `@0` for timing), then weaker hints (`package`,
`rectangle`), and otherwise sequence. A kind that is not drawn yet is an error naming it.

## PUML-2 Kinds and models

| Kind | Model | Priority |
|---|---|---|
| Sequence | sequence (MSEQ-2) | 1 |
| Activity (the newer syntax: `start`, `:action;`, `if`/`else`/`endif`, `while`, `repeat`, `fork`, `stop`) | graph | 2 |
| Class and object | graph, compartments (MCLASS-2) | 3 |
| State | graph, state shapes (MSTATE-2) | 3 |
| Use case | graph, actor and ellipse shapes | 4 |
| Component and deployment | graph, component, node, and database shapes | 4 |
| Timing | own model | later |
| Activity swimlanes (`\|Lane\|`) | graph with lane clusters (PACT-7) | 2 |

## PUML-3 Sequence

Participants (`participant`, `actor`, `boundary`, `control`, `entity`, `database`,
`collections`, `queue`), messages (`->`, `-->`, `->>`, `->x`, `<->`, colors in arrows), `activate`
and `deactivate`, `++` and `--` shorthands, `return`, notes, `alt`/`else`/`opt`/`loop`/`par`/
`break`/`critical`/`group` ... `end`, dividers (`== title ==`), delays (`...`), spacing (`|||`),
`autonumber`, and `title`, `header`, `footer`, all onto the sequence model. Specified in detail
by PSEQ.

## PUML-4 Preprocessor and settings

`!include`, `!includeurl`, and `!import` read other files or the network and are not followed:
each is an error naming it. `!define`, `!function`, variables, and conditionals are research:
they may be worth a small evaluator, since real files use them. `skinparam` maps a subset (colors,
fonts, and the monochrome and handwritten switches as warnings) to the theme; the rest are
warnings. `!theme` is a warning.

## PUML-5 Open questions

- Resolved: the reference is the current language reference guide and plantuml.com, and the
  fixtures are written for sirene rather than copied from PlantUML's examples (PSEQ).
- Whether the non-UML kinds (mindmap, WBS, Gantt, JSON, YAML, Salt) share Mermaid's chart work
  (MCHART).
- How far the preprocessor has to go before typical files render.

The outcome is one `pending` spec per kind, starting with sequence and activity.
