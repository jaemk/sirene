# PlantUML

Status: research. PlantUML (<https://plantuml.com>) has a dozen diagram kinds and no formal
grammar; its reference is the Java implementation and the language reference guide. Full
compatibility is not the goal. The useful subset reuses the sequence and graph models.

## PUML-1 Documents

A document is the text between `@startuml` and `@enduml` (or another `@start...` pair); a file
can hold several, which the editor steps through as it steps through Markdown blocks
(WYSIWY-20). Text outside them is ignored. The kind is detected as PlantUML does, from the first
statement only one kind accepts, and a mismatch later is a diagnostic. `@startmindmap`,
`@startgantt`, `@startwbs`, `@startjson`, `@startyaml`, and `@startsalt` are other kinds
(PUML-5).

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
| Activity swimlanes (`\|Lane\|`) | graph with lane clusters, which need a LAYOUT constraint | later |

## PUML-3 Sequence

Participants (`participant`, `actor`, `boundary`, `control`, `entity`, `database`,
`collections`, `queue`), messages (`->`, `-->`, `->>`, `->x`, `<->`, colors in arrows), `activate`
and `deactivate`, `++` and `--` shorthands, `return`, notes, `alt`/`else`/`opt`/`loop`/`par`/
`break`/`critical`/`group` ... `end`, dividers (`== title ==`), delays (`...`), spacing (`|||`),
`autonumber`, and `title`, `header`, `footer`, all onto the sequence model.

## PUML-4 Preprocessor and settings

`!include`, `!includeurl`, and `!import` read other files or the network and are not followed:
each is an error naming it. `!define`, `!function`, variables, and conditionals are research:
they may be worth a small evaluator, since real files use them. `skinparam` maps a subset (colors,
fonts, and the monochrome and handwritten switches as warnings) to the theme; the rest are
warnings. `!theme` is a warning.

## PUML-5 Open questions

- Which PlantUML version's behavior is the reference, and how to build a fixture corpus without
  copying its examples' license terms.
- Whether the non-UML kinds (mindmap, WBS, Gantt, JSON, YAML, Salt) share Mermaid's chart work
  (MCHART).
- How far the preprocessor has to go before typical files render.

The outcome is one `pending` spec per kind, starting with sequence and activity.
