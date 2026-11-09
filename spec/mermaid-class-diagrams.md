# Mermaid Class Diagrams

`classDiagram` and `classDiagram-v2`, lowered to the graph model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/classDiagram.html>.

## MCLASS-1 Syntax

- Classes: `class A`, `class A["Label"]`, generics `class List~T~`, members in a body
  `class A { +int id; +run() bool }` or one at a time `A : +int id`.
- Members: visibility `+ - # ~`, `$` static, `*` abstract; a member with `(` is a method, with its
  return type after `)`.
- Annotations: `<<interface>>`, `<<abstract>>`, `<<service>>`, `<<enumeration>>`, in the body or
  `<<interface>> A`.
- Relations: `<|--` inheritance, `*--` composition, `o--` aggregation, `-->` association, `--`
  link, `..>` dependency, `..|>` realization, `..` dashed link; either direction and both ends
  (`<|--|>`); cardinality `A "1" --> "*" B`; label `: text`.
- `namespace N { ... }`, `note "text"`, `note for A "text"`, `direction`.
- Styling: `classDef`, `cssClass`, `A:::name`, `style`.

## MCLASS-2 Model

A class is a compartment node (LANGS-3): the title row (annotation above the name, generics as
`<T>`), then attributes, then methods; empty sections are omitted unless the other is present.
Static members are underlined and abstract ones italic. Relations are edges with markers at the
ends their arrows name: hollow triangle, filled diamond, hollow diamond, arrow, or none; `..`
forms are dashed. Cardinalities are end labels; the relation label is the middle label. A
namespace is a cluster.

## MCLASS-3 Layout and drawing

Layered layout, top to bottom by default; an inheritance edge points from the subclass to the
superclass, so superclasses rank above. In ASCII compartments are split by `├─┤` rows (`+-+`),
and markers are `◁` (`<|`), `◆` (`*`), `◇` (`o`), and `▶` (`>`).

## MCLASS-4 Unsupported

`click`, `callback`, and `link` are warnings (LANGS-6).
