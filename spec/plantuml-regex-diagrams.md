# PlantUML Regex Diagrams

PlantUML `@startregex` documents (PUML-2), drawn as railroad diagrams (MRAIL-2)
(`sirene_core::plantuml`). Reference: <https://plantuml.com/regex>. Tests: unit tests in
`crates/sirene-core/src/plantuml/regex.rs`, and the fixtures `regex.puml` and
`errors/regex.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PREGEX-1 Documents

Each line is a regular expression, drawn as an unnamed rule: no `name =`, its track starting
at the left. `title TEXT` and `skinparam` lines are settings (PSEQ-6); every other line is a
regex, even one starting with another setting's word (`set|get`, `page\d+`, `<style>`). `'`
starts no PlantUML comment.

## PREGEX-2 Expressions

- Adjacent literal characters are one rounded box; `\` before any other character is that
  character.
- `.`, `^`, `$`, and the escapes `\d`, `\w`, `\s`, `\b` (and their capitals), `\n`, `\r`,
  `\t`, `\A`, `\z` are boxes naming them ("any character", "digit", ...).
- `[...]` is a box "one of ..." and `[^...]` "none of ...", the class as written.
- `|` is a choice, `( )`, `(?: )`, `(?> )`, `(?<name> )`, and `(?P<name> )` group, and a
  lookaround (`(?= )`, `(?! )`, `(?<= )`, `(?<! )`) is drawn as a group, with a warning.
  Inline flags (`(?i)`, `(?i: )`) are not drawn, with a warning; an empty group draws nothing.
- `?` is an option; `*`, `+`, `{n}`, `{n,}`, and `{n,m}` repeat, drawn with a bypass when
  the minimum is 0. A lazy or possessive suffix draws the same. A `{` that is not a repeat is
  a literal.

## PREGEX-3 Diagnostics

An unclosed group or class, a `)` with no group, a quantifier with nothing before it, a
trailing `\`, and a repeat whose maximum is below its minimum are errors; what was read before
the error is drawn.
