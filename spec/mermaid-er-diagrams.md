# Mermaid ER Diagrams

`erDiagram`, lowered to the graph model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/entityRelationshipDiagram.html>.

## MER-1 Syntax

- Relationships: `A ||--o{ B : label`. Each end is one of `|o` (zero or one), `||` (exactly
  one), `}o` (zero or more), `}|` (one or more), mirrored on the right (`o|`, `||`, `o{`, `|{`);
  `--` is identifying, `..` non-identifying. The word forms (`one or more`, `zero or many`, ...)
  are accepted too.
- Entities: `A`, `A["Alias"]`, and attribute blocks `A { string name PK, FK "comment" }` with
  type, name, keys (`PK`, `FK`, `UK`), and an optional quoted comment.
- `direction`, `classDef`, `class`, `A:::name`, `style`, comments.

## MER-2 Model

An entity is a compartment node (LANGS-3): the name, then one row per attribute, aligned in
columns of type, name, keys, and comment. A relationship is an edge with crow's-foot markers at
both ends, dashed when non-identifying, labeled in the middle.

## MER-3 Layout and drawing

Layered layout, top to bottom by default. In ASCII the markers are drawn with `|`, `o`, and `<`
or `>` (`{`, `}` for the many side) on the edge next to the entity.
