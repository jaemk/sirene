# PlantUML JSON and YAML

PlantUML JSON and YAML documents (PUML-2), lowered to the graph model as tables
(`sirene_core::plantuml`). Reference: <https://plantuml.com/json> and
<https://plantuml.com/yaml>; the fixtures are written for sirene. Tests: unit tests in
`crates/sirene-core/src/plantuml/data.rs`, the fixtures `json.puml`, `yaml.puml`,
`errors/json.puml`, and `errors/yaml.puml` with `crates/sirene-core/tests/plantuml.rs`, and
property tests in `crates/sirene-core/tests/plantuml_props.rs`.

## PDATA-1 Documents

A `@startjson` document holds one JSON value and a `@startyaml` document one YAML value.
PlantUML comments (`'`) are skipped. `#highlight` lines are warnings and draw without
highlighting; a `<style>` block is a warning.

## PDATA-2 JSON

Objects, arrays, strings (with escapes), numbers, `true`, `false`, and `null`, as RFC 8259
writes them. A string is drawn without its quotes; the other scalars as written.

## PDATA-3 YAML

Block mappings (`key: value`) and sequences (`- item`) nested by indentation, a sequence at
its key's indentation, `- key: value` items, flow collections (`[a, b]`, `{a: 1}`), quoted and
plain scalars, `|` and `>` block scalars (their lines joined by spaces), `#` comments, and
`---` markers. A document may be a lone scalar. Anchors, aliases, and tags are drawn as text.

## PDATA-4 Drawing

The chart runs left to right. Each object and array is a table with no title: an object's
rows are its keys and values, an array's rows its items. A nested object or array is its own
table, joined by a line from its row, which is blank; an empty one is drawn in its row as `{}`
or `[]`. A scalar document is a table of one row.

## PDATA-5 Diagnostics

Malformed data is an error at the first problem, and what was read before it is drawn (the
open collections end there). A document with no data is an error.
