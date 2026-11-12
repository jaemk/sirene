# PlantUML File Trees

PlantUML `@startfiles` documents (PUML-2), lowered to the tree view model (MTREEV-2)
(`sirene_core::plantuml`). Reference: <https://plantuml.com/files>. Tests: unit tests in
`crates/sirene-core/src/plantuml/files.rs`, and the fixtures `files.puml` and
`errors/files.puml` with `crates/sirene-core/tests/plantuml.rs`.

## PFILES-1 Paths

Each line is a path from the root, `/a/b.txt` (or `~/a/b.txt`). Its directories are made as
needed, once each, in the order first named; a path ending in `/` is a directory, and a name
that later has entries under it becomes one. Entries are drawn with folder and file icons.
`title` and the shared settings apply (PSEQ-6).

## PFILES-2 Notes

`<note>` ... `</note>` describes the path before it: its lines, joined by spaces, are the
entry's description, drawn beside it.

## PFILES-3 Diagnostics

A line that is not a path, a note before any path, and a `<note>` with no `</note>` are
errors.
