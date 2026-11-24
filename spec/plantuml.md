# PlantUML

Status: done; sequence diagrams are specified in `plantuml-sequence-diagrams.md` (PSEQ)
activity diagrams in `plantuml-activity-diagrams.md` (PACT), class diagrams in
`plantuml-class-diagrams.md` (PCLS), state diagrams in `plantuml-state-diagrams.md` (PSTA), and
use case, component, and deployment diagrams in `plantuml-description-diagrams.md` (PDSC), and
mindmaps and WBS in `plantuml-mindmaps-and-wbs.md` (PMIND), Gantt charts in
`plantuml-gantt-charts.md` (PGANTT), JSON and YAML in `plantuml-json-and-yaml.md` (PDATA),
DOT and EBNF in `plantuml-embedded-languages.md` (PEMB), file trees in
`plantuml-file-trees.md` (PFILES), regexes in `plantuml-regex-diagrams.md` (PREGEX),
Chen diagrams in `plantuml-chen-diagrams.md` (PCHEN), network diagrams in
`plantuml-network-diagrams.md` (PNW), chronologies in `plantuml-chronology-diagrams.md`
(PCHRON), and timing diagrams in `plantuml-timing-diagrams.md` (PTIM).
PlantUML (<https://plantuml.com>) has a dozen diagram kinds and no formal grammar; its
reference is the Java implementation and the language reference guide. Full compatibility is
not the goal. The useful subset reuses sirene's models.

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
| Activity swimlanes (`\|Lane\|`) | graph with lane clusters (PACT-7) | 2 |
| Mindmap (`@startmindmap`) | mindmap | 5 |
| WBS (`@startwbs`) | graph | 5 |
| Gantt (`@startgantt`) | gantt (MGANTT-2) | 5 |
| JSON and YAML (`@startjson`, `@startyaml`) | graph, untitled compartments | 5 |
| DOT (`@startdot`) | graph, via the DOT reader | 5 |
| EBNF (`@startebnf`) and regex (`@startregex`) | railroad (MRAIL-2) | 5 |
| Files (`@startfiles`) | tree view (MTREEV-2) | 5 |
| Chen (`@startchen`) and network (`nwdiag`) | graph | 5 |
| Chronology (`@startchronology`) | timeline (TIMELI-2) | 5 |
| Timing (`robust`, `concise`, `@0`, ...) | timing (TIMING-1) | 5 |
| LaTeX (`@startlatex`) | equation (PCREO-3) | 5 |
| Salt (`plantuml-salt.md`), ditaa (`plantuml-ditaa.md`), creole and math (`plantuml-creole-and-math.md`), wire and board (`plantuml-wire-and-board.md`), def | not drawn: an error naming the kind | - |

## PUML-3 Sequence

Participants (`participant`, `actor`, `boundary`, `control`, `entity`, `database`,
`collections`, `queue`), messages (`->`, `-->`, `->>`, `->x`, `<->`, colors in arrows), `activate`
and `deactivate`, `++` and `--` shorthands, `return`, notes, `alt`/`else`/`opt`/`loop`/`par`/
`break`/`critical`/`group` ... `end`, dividers (`== title ==`), delays (`...`), spacing (`|||`),
`autonumber`, and `title`, `header`, `footer`, all onto the sequence model. Specified in detail
by PSEQ.

## PUML-4 Preprocessor and settings

`!include`, `!includeurl`, `!includesub`, `!include_many`, `!include_once`, and `!import` read
other files or the network and are not followed: each is an error naming it. `!define`,
`!function`, variables, conditionals, and the other preprocessor lines are a warning naming the
directive (`` `!define` is not supported; ignoring the line ``) and are skipped. `skinparam` is a
warning, except `actorStyle` in use case diagrams, and `!theme` is a warning. Tests:
`common_preprocessor_lines` in `crates/sirene-core/src/plantuml/lines.rs`.

## PUML-5 Open questions

- Resolved: the reference is the current language reference guide and plantuml.com, and the
  fixtures are written for sirene rather than copied from PlantUML's examples (PSEQ).
- Resolved: the non-UML kinds draw on the Mermaid models (PUML-2).
- Open: an evaluator for `!define`, variables, and conditionals (`plantuml-preprocessor.md`),
  since real files use them, and mapping `skinparam` colors and fonts to the theme
  (`plantuml-skinparam.md`). `@startdef` blocks go with the preprocessor.

## PUML-6 Remaining work

What is not drawn or read yet, by value and cost. Each has its own spec and feature row.

| Item | Spec | Status | Cost | Notes |
|---|---|---|---|---|
| Preprocessor | `plantuml-preprocessor.md` | research | medium | Most real files use it; stdlib `!include`s still fail. Needs diagnostics mapped to the source and fuzzed limits. |
| Skinparam | `plantuml-skinparam.md` | research | medium | A subset maps onto the graph model's `Style` and theme fields. |
| Salt | `plantuml-salt.md` | pending | large | New wireframe model, layout, scene, and ASCII renderer. |
| Creole | `plantuml-creole-and-math.md` | research | medium | Needs a rich text run model in the display list first. |
| Math, LaTeX | `plantuml-creole-and-math.md`, `latex-math.md` | partial | large | `@startlatex`, `@startmath` (AsciiMath), and inline `<latex>` and `<math>` tags are drawn (PCREO-2, PCREO-3). |
| Ditaa | `plantuml-ditaa.md` | research | medium | Rectangles and orthogonal lines first. |
| Wire, board | `plantuml-wire-and-board.md` | research | unknown | Beta in PlantUML, grammar only in its sources. |
| `@startdef` | `plantuml-preprocessor.md` | research | small | Draws nothing; goes with `!includedef`. |

Order: preprocessor, skinparam, Salt; the rest as demand shows.

Each kind has its own spec, listed above.
