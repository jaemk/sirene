# Mermaid ER Diagrams

`erDiagram`, lowered to the graph model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/entityRelationshipDiagram.html>. Code: `crates/sirene-core/src/er/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/er.rs` and the
corpus under `crates/sirene-core/tests/fixtures/er/` (LANGS-6).

## MER-1 Syntax

- Relationships: `A ||--o{ B : label`. Each end is one of `|o` (zero or one), `||` (exactly
  one), `}o` (zero or more), `}|` (one or more), mirrored on the right (`o|`, `||`, `o{`, `|{`);
  `--` is identifying, `..`, `.-`, and `-.` non-identifying. The word forms are accepted too:
  `zero or one`, `one or zero`, `zero or more`, `zero or many`, `one or more`, `one or many`,
  `only one`, `many(0)`, `many(1)`, `many`, `one`, `0+`, `1+`, `1`, joined by `to` (identifying)
  or `optionally to` (non-identifying). The label after `:` may be quoted.
- Entities: `A`, `"Quoted name"`, an alias `A[Alias]` or `A["Alias"]`, and attribute blocks
  `A { string name PK, FK "comment" }`, one attribute per line: type, name, keys (`PK`, `FK`,
  `UK`, comma separated), and an optional quoted comment. `~T~` in a type is `<T>`.
- `direction`, `classDef`, `class A,B name`, `A:::name` (also in a relationship), `style`;
  applied once every entity exists.
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares the line-oriented machinery of the state and class parsers (`crate::lines`)
and is lossless. Tests: `snapshots`, `diagnostics_messages`, and the unit tests in
`crates/sirene-core/src/er/`.

## MER-2 Model

An entity is a compartment node (LANGS-3): the name (or alias), then one section with a row per
attribute, aligned in columns of type, name, keys, and comment; an entity without attributes is
a plain box. A relationship is an edge with crow's-foot markers at both ends (`Arrow::ZeroOne`,
`One`, `ZeroMany`, `OneMany`), dotted when non-identifying, labeled in the middle. Every entity
named is drawn, even in an incomplete relationship. Styling of an unknown entity is a warning.
Tests: `relationships`, `attributes`, `names_and_styling`.

## MER-3 Layout and drawing

Layered layout, top to bottom by default; an edge runs from the left entity to the right one.
The markers are drawn across the line next to the entity: bars for one, a ring for zero, and a
crow's foot for many. In ASCII each marker is one character on the edge next to the entity: `?`
(zero or one), `1` (exactly one), `*` (zero or more), and `+` (one or more), the same in both
charsets. Tests: `drawing`, `snapshots`, `ascii_charset_snapshots`.
