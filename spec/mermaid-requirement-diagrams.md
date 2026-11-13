# Mermaid Requirement Diagrams

`requirementDiagram`, lowered to the graph model (LANGS-2, LANGS-3, MCHART-1). Reference:
<https://mermaid.js.org/syntax/requirementDiagram.html>. Code:
`crates/sirene-core/src/requirement/`; `diagram::lower` picks it by the header keyword. Tests:
`crates/sirene-core/tests/requirement.rs` and the corpus under
`crates/sirene-core/tests/fixtures/requirement/` (LANGS-6).

## MREQ-1 Syntax

- Header: `requirementDiagram`.
- A definition: a type, a name, optional `:::a,b` classes, then `{`, `key: value` lines, and `}`.
  The types are `requirement`, `functionalRequirement`, `interfaceRequirement`,
  `performanceRequirement`, `physicalRequirement`, `designConstraint`, and `element`, matched in
  any case, as are the field keys.
- A relationship: `a - type -> b` or `b <- type - a`, the type one of `contains`, `copies`,
  `derives`, `satisfies`, `verifies`, `refines`, or `traces` in any case.
- A name or value is a `"string"` or unquoted text. An unquoted name runs from a word character
  up to one of `:,{<>-=;` or the end of the line, so it may hold spaces; an unquoted value runs
  to the end of the line or a `}`.
- `name:::a,b`, `class a,b name`, `classDef`, `style`, `direction`, comments, directives,
  frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A definition
without a name or `{`, an unclosed block, a field without `:`, an unknown relationship type, and
a relationship missing its arrow or second name are errors. Tests: `snapshots`, `syntax_errors`,
and the parser's unit tests.

## MREQ-2 Model

Each requirement or element is a rectangle node named by its name. Its label is the type as a
stereotype above the name (`«Functional Requirement»`, `«Element»`), and its fields are one
compartment of two-column rows in a fixed order: `Id:`, `Text:`, `Risk:`, and `Verification:`
for a requirement, `Type:` and `Doc Ref:` for an element. A risk is written `Low`, `Medium`, or
`High`, and a verify method `Analysis`, `Inspection`, `Test`, or `Demonstration`, whatever their
case in the source. A field another type does not have, or a risk or verify method not in those
lists, is a warning and left out. A later definition of the same name is a warning and skipped,
as Mermaid keeps the first.

A relationship is an edge from its source to its destination in either form, labeled with its
type (`«satisfies»`). `contains` is a solid line from a circle at the source; the others are
dashed lines with an arrowhead at the destination. A name used in a relationship but never
defined is drawn as a plain box, with one warning. Classes and styles apply as in ER diagrams
(MER-2); a `class` or `style` for an unknown name is a warning. Tests: `definitions`,
`relationships`, `styling`, `errors`.

## MREQ-3 Drawing

Drawn, laid out, and rendered in ASCII as the graph model is (DISPLA, ASCIIR), with the
compartment rows of LANGS-3. Tests: `snapshots`, `ascii_charset_snapshots`.

## MREQ-4 Not drawn

Mermaid draws `contains` with a circle holding a plus; sirene uses its plain circle marker. Mermaid
12's `redux-color` theme, `neo` look, and ELK layout are not used, and markdown in names and
values is shown as written. Drawing edits are read-only (LANGS-5).
