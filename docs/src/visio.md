# Visio drawings

sirene draws Microsoft Visio `.vsdx` drawings as SVG and PNG. A drawing is read-only: the editor
opens it as a picture, and `sirene ascii` and `sirene text` have no text form for it.

```
sirene svg plan.vsdx                 print or write the SVG of a one-page drawing
sirene png plan.vsdx --page 2        write page 2 only
sirene png plan.vsdx                 write plan-1.png, plan-2.png, ... for several pages
sirene check plan.vsdx               print diagnostics only
```

A `.vsdx` file, any input that starts with the ZIP magic `PK\x03\x04`, and any source given with
`--lang visio` (also spelled `vsdx`) is a Visio drawing. Markdown has no Visio block. Directories
are scanned for `.vsdx` files. `--page N` counts from 1; a page past the last is an error.

## What is read

| Part of the drawing | Drawn as |
|---|---|
| Pages, background pages | one output per page, in order; a page's background page is drawn first |
| Shapes, groups | geometry in the shape's frame, with `PinX`, `PinY`, `Width`, `Height`, `Angle`, flips |
| Masters, style sheets | inherited cells, rows, and text, as Visio resolves them |
| Geometry rows | lines, arcs, ellipses, Beziers, polylines, splines, and NURBS |
| Line and fill | solid fill and line color (`#RRGGBB`, the 24 classic indexes, document colors, theme colors), weight, transparency, dash |
| Connectors | their own cached geometry, with `BeginArrow` and `EndArrow` heads |
| Text | size, color, bold, italic, underline, alignment, indents, spacing, margins, wrapping |
| Layers | shapes only on hidden layers are skipped |
| Foreign shapes | a gray placeholder box with diagonals |

Cached values are drawn; formulas are never evaluated, so a cell whose cached value is stale
draws as saved.

## What is not drawn

Gradients and fill patterns (drawn solid, with a warning), line patterns beyond one dash and gap
pair, line caps and joins, rounded corners, shadows, embedded images and metafiles, rotated text
(drawn upright, with a warning), fonts, small caps, bullets, tab stops, and connector rerouting.
The `.vsd` and `.vdx` formats are not read.

## Limits

A package is refused, with one error, when it is corrupt, encrypted, zip64, or larger than the
limits: 64 MiB archive, 4096 entries, 32 MiB per part, 128 MiB in all, 200 to 1 compression, 256
pages, 4096 masters, 50,000 shapes per page, 1,000,000 geometry rows and 4 MiB of text per page.
Past the per-page budgets the remaining shapes are left out, with a warning. A DOCTYPE in any
part is an error.

sirene's output is not compared with Visio's for every drawing: see `spec/visio.md` (VISIO-16)
for what is verified.

## In a web page and the library

`render` in `sirene-wasm.js` takes a `Uint8Array` source with `lang: "visio"` and a `page` from 0;
see [JavaScript library](embedding.md#a-visio-drawing). In Rust, `sirene_core::diagram::lower_bytes` reads the package
bytes; see [Rust library](library.md).
