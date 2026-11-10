# Mermaid Info Diagrams

`info`, which draws the version of the tool rendering it. Reference: Mermaid's `info` grammar
(`packages/parser/src/language/info/info.langium`). Code: `crates/sirene-core/src/info/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/info.rs`.

## MINFO-1 Syntax

- Header: `info`, optionally followed by `showInfo` on the same line.
- `showInfo` on a line of its own, `title`, `accTitle`, `accDescr`, Mermaid comments and
  directives, and frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Extra text after
the header or `showInfo`, and any other line, are errors. Tests: `errors` and the parser's unit
tests.

## MINFO-2 Model

`info::parse(src).lower()` gives an `info::Info` with the title, the accessibility text, and
`show_info`. `Info::text` is `sirene v` and `info::VERSION`. Tests: `model`.

## MINFO-3 Drawing

The title, if any, over the version in 32px text, at least 400px wide as in Mermaid. `showInfo`
draws the same. Tests: `drawing`.

## MINFO-4 Text

The title and a blank line, if there is a title, then the version. Tests: `drawing`.

Mermaid draws its own version; sirene draws its own. The diagram is read-only in the editor
(LANGS-5).
