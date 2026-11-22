# PlantUML Salt Wireframes

PlantUML `@startsalt` documents (PUML-2) and `{{salt ... }}` blocks, drawn as a wireframe in a
new wireframe model with its own layout, scene builder, and ASCII renderer (LANGS-2).
Reference: <https://plantuml.com/salt>. Today the kind is an error naming it.

## PSALT-1 Widgets

Text, buttons `[ text ]`, radio buttons `( )` and `(X)`, checkboxes `[ ]` and `[X]`, text fields
`"text"`, and droplists `^a^b^`, each in a cell.

## PSALT-2 Containers

`{ ... }` is a grid, with `|` between columns and a line per row. The character after `{` picks
grid lines: `#` all, `!` vertical, `-` horizontal, `+` outside only. A nested `{ }` is a
sub-grid. `{^"title" ... }` is a group box, `{/ a | b }` tabs, `{* ... }` a menu bar with
submenus, `{T ...}` a tree whose `+` count is the depth, and `{S ...}` scroll bars. The
separators `..`, `==`, `~~`, and `--` draw a rule across the row. `*` spans a cell to the left
and `.` leaves one empty in a table.

## PSALT-3 Not drawn

Icons (`<&name>`), pseudo-sprites (`<<...>>`), and Creole or HTML markup in text are a warning
naming the construct, and the text is drawn as written. `skinparam` is a warning (PUML-4).

## PSALT-4 Editing

A wireframe draws read-only (LANGS-5), with every widget owned by the diagram.

## PSALT-5 Open questions

- Widget metrics: PlantUML sizes widgets from its font and a fixed padding; sirene's text
  metrics (DISPLA-4) decide the sizes here, so output differs.
- `{{salt ... }}` inside an activity diagram is a node holding a wireframe, which the graph
  model has no shape for.
