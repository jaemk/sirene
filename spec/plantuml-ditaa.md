# PlantUML Ditaa

PlantUML `@startditaa` documents (PUML-2): ASCII art read as shapes. Reference:
<https://plantuml.com/ditaa>. Today the kind is an error naming it.

## PDITAA-1 Shapes

A closed outline of `+`, `-`, and `|` is a box; `/` and `\` corners round it. A tag in the box
sets a shape: `{c}` choice, `{d}` document, `{io}` input and output, `{mo}` manual operation,
`{o}` ellipse, `{s}` storage, `{tr}` trapezoid. `cRED`, `cBLU`, and the other color codes fill
it, and the text inside is its label. Lines of `-`, `|`, `=`, and `:` join boxes, with `>`, `<`,
`^`, and `v` as arrowheads; `=` and `:` are dashed.

## PDITAA-2 Options

`-E` and `--no-separation`, `-S` and `--no-shadows`, and `scale=N` follow `@startditaa`. An
option sirene does not read is a warning naming it.

## PDITAA-3 Drawing

Boxes and lines are drawn at their character-grid positions, scaled to a cell size; there is no
layout pass, since the source is the layout. ASCII output redraws the grid as read.

## PDITAA-4 Open questions

- Ditaa's recognizer handles arbitrary closed outlines, text that overlaps lines, and corner
  heuristics. This covers rectangles and orthogonal lines first, and a shape it cannot close is
  a warning with the text left in place.
- The output is a picture of ASCII art; the value is mostly in the SVG and PNG exports.
