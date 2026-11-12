# PlantUML

sirene draws PlantUML diagrams as images and as text, with its own layouts:

```
$ sirene ascii login.puml
  o
 /|\    ┌─────────┐
 / \    │ Server  │
User    └────┬────┘
  │          │
  │  log in  │
  ├─────────►│
  │          │
  │  token   │
  │◄┄┄┄┄┄┄┄┄┄┤
  │          │
  o     ┌────┴────┐
 /|\    │ Server  │
 / \    └─────────┘
User
```

```
@startuml
actor User
participant Server
User -> Server : log in
Server --> User : token
@enduml
```

A `.puml`, `.plantuml`, `.pu`, or `.iuml` file, a `plantuml` or `puml` block in Markdown, and any
source given with `--lang plantuml` is PlantUML; so is stdin or another file that starts with an
`@start` line. Directories are scanned for those extensions. Inside `@startuml`, the kind is
picked from the statements, as PlantUML does. In the editor a PlantUML diagram is read-only on
the drawing side. Only the first `@start` ... `@end` block in a file is drawn.

## Kinds

| Kind | Start |
|---|---|
| Sequence | `@startuml` with participants and messages |
| Activity, with swimlanes | `@startuml` with `start`, `:action;`, `if`, `while`, `fork`, `\|Lane\|` |
| Class and object | `@startuml` with `class`, `interface`, `enum`, `object`, `map` |
| State | `@startuml` with `state`, `[*]` |
| Use case, component, and deployment | `@startuml` with `actor`, `usecase`, `component`, `node`, `database`, ... |
| Timing | `@startuml` with `robust`, `concise`, `clock`, `binary`, `analog`, `@0` |
| Network | `nwdiag { ... }` in `@startuml` or `@startnwdiag` |
| Mindmap and WBS | `@startmindmap`, `@startwbs` |
| Gantt | `@startgantt` |
| JSON and YAML | `@startjson`, `@startyaml` |
| DOT | `@startdot` |
| EBNF and regular expressions | `@startebnf`, `@startregex` |
| Chen entity-relationship | `@startchen` |
| Chronology | `@startchronology` |
| File trees | `@startfiles` |

The [examples](examples.md) include a sequence, class, activity, and timing diagram. See
`spec/plantuml.md` and the `spec/plantuml-*.md` pages for what each kind reads.

## Errors

Syntax a kind does not read is an error on its line; a construct that is read but not drawn
(a stereotype, an unknown attribute, `manual time-axis`) is a warning. `!include`, `!includeurl`,
and `!import` are errors, since sirene reads no other files; other `!` lines (`!define`,
`!theme`, ...) are warnings and skipped, as are unmapped `skinparam` settings. A second `@start`
block in a file is a warning.

## Not drawn

Salt (`@startsalt`), ditaa, math, LaTeX, wire, board, creole, and def diagrams are errors naming
the kind. The preprocessor (`!define`, `!function`, variables, conditionals) is not evaluated.
The layouts are sirene's, not PlantUML's.
