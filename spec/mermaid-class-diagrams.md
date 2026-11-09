# Mermaid Class Diagrams

`classDiagram` and `classDiagram-v2`, lowered to the graph model (LANGS-2). Reference:
<https://mermaid.js.org/syntax/classDiagram.html>. Code: `crates/sirene-core/src/class/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/class.rs` and the
corpus under `crates/sirene-core/tests/fixtures/class/` (LANGS-6).

## MCLASS-1 Syntax

- Classes: `class A`, `class A["Label"]`, generics `class List~T~`, members in a body (one per
  line, `class A {` ... `}`) or one at a time `A : +int id`.
- Members: visibility `+ - # ~`, `$` static, `*` abstract (at the end, or right after a method's
  `)`); a member with `(` is a method, with its return type after `)`. `~T~` in a member is
  `<T>`.
- Annotations: `<<interface>>`, `<<abstract>>`, `<<service>>`, `<<enumeration>>`, in the body or
  `<<interface>> A`.
- Relations: `<|--` inheritance, `*--` composition, `o--` aggregation, `-->` association, `--`
  link, `..>` dependency, `..|>` realization, `..` dashed link; either direction and both ends
  (`<|--|>`); cardinality `A "1" --> "*" B`; label `: text`. A relation needs space around an
  `o` end, so `ClassA o-- B`, not `ClassAo--B`.
- `namespace N { ... }`, `note for A "text"` (`\n` breaks lines), `direction`.
- Styling: `classDef`, `cssClass "A,B" name`, `class A:::name`, `A:::name` in a relation,
  `style`; applied once every class exists.
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares the line-oriented machinery of the state parser (`crate::lines`) and is
lossless. Tests: `snapshots`, `syntax_errors`.

## MCLASS-2 Model

A class is a compartment node (LANGS-3, `Node::sections`): the title row (annotation above the
name as `«interface»`, generics as `<T>`), then attributes, then methods. A class with no
members is a plain box; one with either kind has both sections. Static members are underlined
and abstract ones italic. Relations are edges with markers at the ends their arrows name:
hollow triangle, filled diamond, hollow diamond, arrow, or none; `..` forms are dotted.
Cardinalities are end labels (`Edge::start_label`, `Edge::end_label`); the relation label is the
middle label. A namespace is a cluster. Tests: `members_and_sections`, `labels_and_generics`,
`relations`, `namespaces_notes_and_styling`.

## MCLASS-3 Layout and drawing

Layered layout, top to bottom by default. An edge runs from the left class to the right one, so
in the usual `Super <|-- Sub` the superclass ranks above. Markers are drawn over the line, the
hollow ones filled with the background. End labels sit beside the line near their end. In
ASCII compartments are split by `├─┤` rows (`+-+`), and markers are `△▽◁▷` (`^v<>`), `◆` (`*`),
and `◇` (`o`); an arrow is as in flowcharts. Tests: `superclasses_rank_above`, `drawing`,
`snapshots`, `ascii_charset_snapshots`.

## MCLASS-4 Unsupported

`click`, `callback`, `link`, notes not attached to a class, and styling of an unknown class are
warnings (LANGS-6). Tests: `unsupported_is_a_warning`.
