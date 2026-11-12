# PlantUML Chen Diagrams

PlantUML `@startchen` documents (PUML-2), drawn as entity-relationship diagrams in Chen
notation on the graph model (`sirene_core::plantuml`). Reference: <https://plantuml.com/er-diagram>.
Tests: unit tests in `crates/sirene-core/src/plantuml/chen.rs`, and the fixtures `chen.puml` and
`errors/chen.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PCHEN-1 Entities and relationships

- `entity NAME` is a box and `relationship NAME` a diamond; `"Label" as NAME` names it apart
  from its label.
- `<<weak>>` on an entity draws a double box; `<<identifying>>` on a relationship draws a
  heavier diamond. Other stereotypes are warnings.
- `title`, `left to right direction`, and the shared settings apply (PSEQ-6).

## PCHEN-2 Attributes

A `{` after a declaration opens its attributes, one per line up to `}`. Each is an ellipse
linked to its owner, labeled `name` or `name : type`. `<<key>>` is bold, `<<derived>>` dashed,
and `<<multi>>` heavier; other stereotypes are warnings. An attribute ending in `{` opens its
own composite attributes.

## PCHEN-3 Links

- `A -N- B` links `A` and `B` with the cardinality `N` (written bare or as `(1,N)`) as its
  label; `A -- B` has no label. `=` in place of `-` draws a thick line (total participation).
- `A ->- B` and `A -<- B` point at `B` and `A` respectively.
- `A ->- d { B, C }` draws a circle labeled `d` (or `U`) under `A`, with arrows to each of
  `B` and `C` (specialization).

## PCHEN-4 Diagnostics

A redeclared name, a link to an undeclared name, an unclosed `{`, attributes nested deeper
than the shared limit, and an unknown line are errors.
