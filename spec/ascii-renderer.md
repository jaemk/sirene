# ASCII Renderer

Renders a flowchart as terminal text using the layout (`layout.md`) with one cell per character.

## ASCIIR-1 Output

The output is the diagram's rows, one line each, with trailing spaces removed and a final newline.
A character as wide as two cells (per Unicode East Asian width) takes two cells. An empty diagram
renders as an empty string.

A drawing of more than 2^22 cells keeps its full width (up to 2^22 columns) and as many top rows
as fit, and ends with the line `... cut off: the drawing is <columns> columns by <rows> rows`.
Tests: `cuts_off_past_the_limit` in `ascii/canvas.rs`.

## ASCIIR-2 Charsets

`unicode` (the default) draws with box-drawing characters. `ascii` uses only `-`, `|`, `+`, `.`,
`'`, `:`, `=`, `/`, `\`, `<`, `>`, `(`, `)`, `^`, `v`, `x`, and `o`. Labels are printed as given.

## ASCIIR-3 Nodes

A node is a box one cell of padding wider than its label on each side, one row per label line.
The border marks the shape:

| Shapes | Unicode | ASCII |
|---|---|---|
| `rect` and shapes not listed | `┌┐└┘` | `+` corners |
| `rounded`, `event` | `╭╮╰╯` | `.` and `'` corners |
| `stadium`, circles, `ellipse` | `╭╮╰╯` with `(` `)` sides | `.` `'` corners, `(` `)` sides |
| `dbl-circ` | double lines | `=` lines, `+` corners |
| `diam` | `╱╲` corners | `/` `\` corners |
| `hex` | `╱╲` corners with `<` `>` sides | `/` `\` corners, `<` `>` sides |
| `odd` | `>` left side | `>` left side |
| `lean-r`, `lean-l`, `trap-b`, `trap-t` | `╱` / `╲` sides by slant | `/` `\` sides |
| `fr-rect` | inner vertical bars | inner `\|` bars |
| `cyl` and cylinders | rounded, with a line under the top | same with `-` |
| `text` | no border | no border |

Label lines are centered. Markdown labels drop `**` and `*` emphasis markers.

## ASCIIR-4 Edges

Edges are drawn along their routes. Normal strokes use light lines, thick strokes heavy lines
(`=` in ASCII), and dotted strokes dashed lines (`.` and `:` in ASCII); invisible edges and their
labels are not drawn. Arrowheads sit in the cell before the border: `▼▲►◄` (`v^><`) for points,
`x` for crosses, `o` for circles. Lines that meet merge into junction characters, and an edge
without an arrowhead at a node joins that node's border.

## ASCIIR-5 Edge labels

An edge label is drawn over its route in the label rect, lines centered.

## ASCIIR-6 Clusters

A cluster is a light box with its title, padded by a space on each side, on the top border where
the layout places it (LAYOUT-11). Edges crossing the border join it.

## ASCIIR-7 Title

A diagram title is printed centered on the first line, followed by a blank line. A blank title
prints nothing.

## ASCIIR-8 Spacing

| | Top-bottom | Left-right |
|---|---|---|
| Gap between nodes in a rank | 3 columns | 1 row |
| Gap between ranks | 2 rows | 4 columns |
| Cluster padding | 2 columns, 1 row | 1 row, 2 columns |
