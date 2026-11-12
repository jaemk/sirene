# PlantUML Description Diagrams

PlantUML use case, component, and deployment diagrams (PUML-2), which PlantUML parses alike,
lowered to the graph model and drawn by the layered layout, scene builder, and ASCII renderer
(`sirene_core::plantuml`). References: <https://plantuml.com/use-case-diagram>,
<https://plantuml.com/component-diagram>, and <https://plantuml.com/deployment-diagram>; the
fixtures are written for sirene. Tests: unit tests in
`crates/sirene-core/src/plantuml/description.rs`, the fixtures `usecases.puml`,
`components.puml`, `deployment.puml`, and `errors/descriptions.puml` with
`crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PDSC-1 Documents

A document (PSEQ-1) is a description diagram when its first deciding line declares an element
only these diagrams have (`usecase`, `component`, `node`, `cloud`, `artifact`, and the like), a
short form (`(x)`, `[x]`, `:x:`, `() x`), or a link with a `(x)` or `[x]` end (PUML-1); a
`rectangle` is a weaker hint. `title` and `left to right direction` apply as in PACT-1.

## PDSC-2 Elements

`KEYWORD name` declares an element, with `"Display" as Id` or `Id as "Display"` naming it:

| Keyword | Shape |
|---|---|
| `actor` | actor (stick figure) |
| `usecase` | use case ellipse |
| `component`, `node`, `card`, `agent`, `rectangle`, `process`, `action` | rectangle |
| `interface`, `circle`, `boundary`, `control`, `entity`, `port`, `portin`, `portout` | circle |
| `database` | cylinder |
| `queue` | horizontal cylinder |
| `storage` | rounded rectangle |
| `cloud` | cloud |
| `folder`, `package` | folder |
| `frame` | framed rectangle |
| `file`, `artifact` | document |
| `stack`, `collections` | stacked rectangles |
| `hexagon` | hexagon |
| `person` | person |
| `label` | text |

After the name, `<<stereotype>>` adds a «stereotype» line above it, `#color` fills it, and
`[text]` (or lines up to a `]` line) is a description drawn in place of the name. A `/` after
the keyword or bracket (PlantUML's business variants) is drawn alike. `skinparam actorStyle
awesome` or `hollow` draws the actors in that style. A word such as `Card` that starts a link
names an element, not a keyword.

## PDSC-3 Short forms

`:Name:` is an actor, `(Name)` a use case, `[Name]` a component, and `() Name` an interface,
each with an optional `as Id`, alone on a line or as a link end. A use case or component is
referred to by its bracketed text or its alias.

## PDSC-4 Containers

An element keyword followed by `{` (`package`, `node`, `rectangle`, `frame`, `cloud`, and so
on) opens a container, drawn as a titled cluster around the elements declared in it until `}`;
containers nest, and opening one again adds to it. `together {` groups without a cluster. A
`#color` fills the container. Links and notes may name a container.

## PDSC-5 Links

Links use the syntax of PCLS-4: `-->`, `..>`, `--`, one dash keeping both ends in a rank,
direction words, `-[#color,dashed]->` styles, quoted end labels, and a label after `:`, in which
`<<extend>>` is drawn `«extend»`. A name used only in links (with no brackets and no
declaration) is an actor in a diagram that has use cases and an interface otherwise, as
PlantUML draws it.

## PDSC-6 Notes

Notes follow PCLS-8: `note left|right|top|bottom of X : text` or a block to `end note`, where
`X` may be a short form such as `(Use)`; a note after a declaration with no `of`;
`note "text" as N`; and `note on link`. A note may be on a container.

## PDSC-7 Diagnostics

A line that is not a description statement, a declaration with no name, a `}` with no open
container, an unclosed container or `[` description are errors. Unknown words after a name,
link styles that are not drawn, and settings that are not drawn are warnings, as in PACT-9.
