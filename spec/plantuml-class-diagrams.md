# PlantUML Class Diagrams

PlantUML class and object diagrams (PUML-2), lowered to the graph model with compartments
(MCLASS-2) and drawn by the layered layout, scene builder, and ASCII renderer
(`sirene_core::plantuml`). Reference: <https://plantuml.com/class-diagram> and
<https://plantuml.com/object-diagram>; the fixtures are written for sirene. Tests: unit tests in
`crates/sirene-core/src/plantuml/class.rs` and `link.rs`, the fixtures `classes.puml` and
`objects.puml` with `crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PCLS-1 Documents and detection

A document (PSEQ-1) is a class diagram when its first deciding line declares a class (`class`,
`enum`, `abstract`, `annotation`, `object`, `map`, `struct`, ...), or, with no deciding line, when
a line is `interface`, `package`, or `namespace`, or a link no sequence arrow looks like
(`A <|-- B`, `A ..> B`, `A -- B`) (PUML-1). `title` and `left to right direction` apply as in
PACT-1.

## PCLS-2 Declarations

- `class`, `interface`, `enum`, `annotation`, `entity`, `exception`, `metaclass`, `protocol`,
  `struct`, `stereotype`, `dataclass`, `record`, `abstract class` (or `abstract`), `object`, and
  `map` declare a class. Each kind but `class`, `object`, and `map` shows its name as a
  stereotype above the class name (`«interface»`, `«enumeration»`, `«abstract»`).
- `circle` and `diamond` declare a small circle or diamond with no compartments.
- The name is a word (`a.b.C` and `a::B` included), `"Display" as Id`, `Id as "Display"`, or a
  quoted string; generics (`Box<T>`) are part of the label, not the id.
- After the name: `<<stereotype>>` (any number, a `(S,#color)` spot dropped), `#color` fill,
  `extends A` and `implements B, C` (solid and dotted inheritance links), and `{` to open a
  body. Anything else is a warning.
- A class declared twice is one class; a later declaration adds to it.

## PCLS-3 Members

- A body to `}` holds one member per line; `A : member` adds one from outside.
- A member with `(` is a method unless marked `{field}`; `{method}` marks a method. With no
  separators, fields and methods are two compartments.
- `{static}` and `{classifier}` underline a member, `{abstract}` sets it in italics. The
  visibility marks `+ - # ~` stay as written.
- Separator lines `--`, `..`, `==`, and `__` start a new compartment in written order; a title
  between them (`.. Getters ..`) is its first row.
- `class A {}` and `class A { x }` on one line are accepted.

## PCLS-4 Links

`A <|-- B`, `A *-- B`, `A o-- B`, `A --> B`, `A ..> B`, `A ..|> B`, `A -- B`, `A .. B`, and the
crow's feet `|o`, `||`, `}o`, `}|` (and their mirrors) draw an edge with that head at each end.
Dots are a dotted line. Two dashes are one rank; one dash (`A -> B`) keeps both ends in a rank,
and each further dash adds a rank. `-up->`, `-down->`, `-left->`, and `-right->` (or `u`, `d`,
`l`, `r`) set the direction. `-[#color,dashed]->` sets the color and stroke (`dashed`, `dotted`,
`bold`, `hidden`, `thickness=N`), and `norank` stops the link from ranking its ends. Quoted labels
beside each end (`A "1" *-- "many" B`) are multiplicities, and `: text` labels the edge. A name
not declared yet is a new class. Association classes (`(A, B) .. C`) are a warning.

## PCLS-5 Objects and maps

`object o` has one compartment of fields (`name = value`). `map M { key => value }` has one
compartment of two-column rows; `key *-> Target` is a row with a link to `Target`.

## PCLS-6 Packages

`package`, `namespace`, and the package styles `rectangle`, `folder`, `frame`, `cloud`, `node`,
`database`, and `card` with a `{` body draw a titled cluster around the classes declared in it;
packages nest, and a `#color` fills them. A link may name a package. `together {` groups
classes with no cluster.

## PCLS-7 Hiding

`hide empty members` (or `fields`, `attributes`, `methods`) drops empty compartments; `hide
members`, `hide fields`, and `hide methods` drop the compartments; `hide stereotype` drops the
stereotypes; `show` turns each back on. `hide circle` is accepted (spots are not drawn). Other
`hide` and `show` forms are warnings.

## PCLS-8 Notes

`note left|right|top|bottom of A : text`, or a block to `end note`, attaches a note to `A`;
without `of`, it attaches to the last class declared. `note "text" as N` (or `note as N` and a
block) is a note of its own that links can name. `note on link` adds its text to the last link's
label. A note on a package is a warning.

## PCLS-9 Diagnostics

A line that is not a class statement, a `}` with no open body, and an unclosed body or package
are errors. Settings that are not drawn are warnings, as in PACT-9.
