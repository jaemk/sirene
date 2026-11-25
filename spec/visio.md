# Visio

Microsoft Visio `.vsdx` drawings (the Open Packaging Conventions format of Visio 2013 and later),
read from bytes and drawn as scene diagrams (LANGS-2, DISPLA-1). The binary `.vsd` and the XML
`.vdx` formats are not read. Code: `crates/sirene-core/src/visio/` (`zip.rs`, `opc.rs`,
`model.rs`, `draw.rs`, `limits.rs`); `diagram::lower_bytes` builds it. Tests:
`crates/sirene-core/tests/visio.rs` with the package writer in `tests/visio/`, and the unit tests
in `visio/`.

Sources read: the Visio file format reference on Microsoft Learn ("Introduction to the Visio file
format (.vsdx)"; "Cell element (Geometry Section)"; the row pages "EllipticalArcTo Row", "ArcTo
Row", "NURBSTo Row", "PolylineTo Row", "SplineStart Row (Geometry Section)"; the cell pages "Style
Cell (Character Section)" and "LinePattern Cell (Line Format Section)"), the structure of
MS-VSDX (package parts, `Cell`, `Row`, `Section`), and ECMA-376 Part 2 (Open Packaging
Conventions: part names, relationships, `TargetMode`, content types). Where a rule below comes
from memory of real files rather than those pages it is marked (unverified).

## VISIO-1 Fidelity

The drawing is approximate and structure-preserving: shapes, their geometry, fills, lines, and
text land where the file's cached cell values put them. There is no claim of a pixel match with
Visio. Fonts are sirene's (the file's font names are ignored), text is measured by the backend's
metrics (DISPLA-4), and the features in VISIO-15 are not drawn. sirene never evaluates a formula
(the `F` attribute of a `Cell`); it uses the cached `V`. A drawing whose cached values are stale
draws as saved.

No real Visio output was available when this was written. Every rule below was checked against
the pages named above and against hand-built packages (VISIO-16), not against files Visio wrote.

## VISIO-2 Package

A `.vsdx` is a ZIP archive holding OPC parts (ECMA-376 Part 2). `visio::zip` reads it itself: the
end-of-central-directory record is found by scanning back at most 64 KiB + 22 bytes for
`PK\x05\x06`; the central directory gives each entry's name, method, sizes, CRC-32, and local
header offset; a local header is checked to start with `PK\x03\x04`, and the data starts after its name
and extra field. Methods: 0 (stored) and 8
(deflate, by `miniz_oxide`, with the output capped at the entry's declared size). Not read:
zip64, encryption (general-purpose bit 0), other methods, data descriptors that change sizes
(the central directory's sizes are authoritative), and multi-disk archives. Each entry's CRC-32
must match. Entry names are UTF-8 (or CP437 treated as ASCII), `/`-separated; a name that is
absolute, has a `..` or empty segment, a backslash, a NUL, a drive prefix, or is a duplicate of
an earlier entry is an error. Directory entries (ending in `/`) are ignored.

Part names in relationships resolve against the source part's directory (`..` allowed only while
it stays inside the package), case-insensitively as OPC specifies. A relationship with
`TargetMode="External"` is ignored: sirene never opens anything outside the archive.

## VISIO-3 Parts

| Part | Content |
|------|---------|
| `[Content_Types].xml` | Read and required to exist; content types are not otherwise trusted |
| `_rels/.rels` | The package relationships; `.../relationships/document` names the document part |
| `visio/document.xml` | `DocumentSettings`, `Colors` (palette entries `ColorEntry IX RGB`), `StyleSheets` |
| `visio/_rels/document.xml.rels` | Relationships to `pages.xml`, `masters.xml`, themes, windows |
| `visio/pages/pages.xml` | `Pages/Page` (`ID`, `Name`, `Background`, `BackPage`, `PageSheet`, `Rel`) |
| `visio/pages/pageN.xml` | `PageContents`: `Shapes`, `Connects` |
| `visio/pages/_rels/pageN.xml.rels` | Images and other parts the page's Foreign shapes use |
| `visio/masters/masters.xml` | `Masters/Master` (`ID`, `NameU`, `PageSheet`, `Rel`) |
| `visio/masters/masterN.xml` | `MasterContents`: `Shapes` |
| `visio/theme/themeN.xml` | DrawingML theme: `a:clrScheme` colors |
| `docProps/*`, `visio/windows.xml`, `visio/media/*` | Not read (media only as Foreign placeholders) |

The part names above are the usual ones; sirene follows relationships and does not assume them.
The document part is the target of the package relationship of type ending `/document`; pages
and masters are the targets of the document part's relationships whose type ends `/pages` and
`/masters`; each `Page` and `Master` finds its contents through its `Rel r:id`. A missing
`document.xml`, `pages.xml`, or page part is an error; a missing masters, theme, or style part
only loses what it would have supplied, with one warning.

## VISIO-4 Model

`visio::Document`: the pages, masters, style sheets, palette, and themes, parsed into plain
typed structures (`model.rs`) with every XML part read through `roxmltree`.

- `Page`: id, name, `Background` flag, `BackPage` id, `PageSheet` cells and sections, shapes,
  connects.
- `Shape`: `ID`, `Type` (`Shape`, `Group`, `Foreign`; a missing `Type` is `Shape`; `Guide` and
  others are not drawn), `Master`, `MasterShape`, `LineStyle`, `FillStyle`, `TextStyle`, its
  cells (name to `Cell`), its sections, its text, its child shapes, and `ForeignData`.
- `Cell`: `N`, `V` (cached value, text), `F` (kept only for the three data formulas in
  VISIO-7), `U` (read but not applied; VISIO-5).
- `Section`: `N` (`Geometry`, `Character`, `Paragraph`, `Connection`, `User`, `Property`,
  `Layer`, `Field`, ...), `IX`, its own cells (`NoFill`, `NoLine`, `NoShow` for geometry), and
  its `Row`s: `T` (the row type, e.g. `MoveTo`), `IX`, cells, and `Del`.

Only the cells, sections, and rows named in this spec are read; the rest are kept as text and
ignored.

## VISIO-5 Units and the transform

Lengths are in inches and angles in radians, as cached in `V`; sirene reads `V` in those units
whatever `U` says (unverified: Visio stores internal units in `V` and the display unit in `U`).
Coordinates are y up with the origin at the bottom left of the page. The scene is y down, in
pixels: 96 px per inch (`visio::PX_PER_INCH`), so a point `(x, y)` on a page of height `H`
lands at `(96 x, 96 (H - y))`.

A shape's transform cells: `PinX`, `PinY` (the pin's position in the parent's frame), `Width`,
`Height`, `LocPinX`, `LocPinY` (the pin in the shape's own frame), `Angle` (counterclockwise,
radians), `FlipX`, `FlipY` (0 or 1). A point `(x, y)` in the shape's frame maps to its parent's:

```
x' = Width - x   when FlipX, else x         (likewise y' with Height and FlipY)
(dx, dy) = (x' - LocPinX, y' - LocPinY)
parent = (PinX + dx cos(Angle) - dy sin(Angle), PinY + dx sin(Angle) + dy cos(Angle))
```

A child of a group is in the group's frame (origin at the group's bottom left corner, before
its pin), so a child's full transform is its own followed by its parent's, up to the page. A
missing `Width`, `Height`, `PinX`, `PinY`, `Angle` is 0; a missing `LocPinX` or `LocPinY` is half
of `Width` or `Height`. Non-finite or absurd values (above 1e6 in magnitude) make the shape
skipped.

## VISIO-6 Inheritance

A cell's value is the first found of: the shape's own cell; the master shape's cell (the shape
named by `Master` and `MasterShape`; for a top-level shape with `Master` and no `MasterShape`,
the master's only top-level shape; a child of a master instance, which has `MasterShape` only,
uses its parent's `Master`); the style sheet chain; the default. The style sheet for a cell is
the shape's `LineStyle` for line cells (`LineWeight`, `LineColor`, `LinePattern`,
`BeginArrow`, `EndArrow`, ...), `FillStyle` for fill cells (`FillForegnd`, `FillBkgnd`,
`FillPattern`), and `TextStyle` for text and character cells; the master shape's styles apply
when the shape has none of its own. A style sheet's own cells fall back to the style named by
the same-kind attribute on it (`StyleSheet LineStyle="n"`), to a depth of `MAX_STYLE_DEPTH`.
Geometry sections merge by `IX` and rows by `IX`, cell by cell; a row with `Del="1"` is removed.
Text is the shape's own `Text`, else the master shape's. A cycle is cut at the depth limit.

Defaults: `LineWeight` 0.01, `LineColor` 0, `LinePattern` 1, `FillForegnd` 1 (white), `FillPattern` 1, character `Size` 0.1111, `Color` 0, `Style` 0, paragraph `HorzAlign`
1 (center), text margins 0.0556 each, `VerticalAlign` 1 (middle).

## VISIO-7 Geometry

`Geometry` sections draw in `IX` order, each as one path in the shape's frame, with cells
`NoFill`, `NoLine`, `NoShow` (`NoShow` skips it). A path starts at its first `MoveTo`; a row
that needs a current point and has none is skipped. Row types and cells (coordinates in the
shape's frame, inches):

| Row | Cells | Drawn as |
|-----|-------|----------|
| `MoveTo`, `LineTo` | `X`, `Y` | move, line |
| `RelMoveTo`, `RelLineTo` | `X`, `Y` | as above with `X`, `Y` times `Width`, `Height` |
| `ArcTo` | `X`, `Y`, `A` | circular arc, `A` the distance from the arc's midpoint to its chord's midpoint, signed |
| `EllipticalArcTo` | `X`, `Y`, `A`, `B`, `C`, `D` | elliptical arc to `(X, Y)` through `(A, B)`, major axis at angle `C`, major to minor ratio `D` |
| `Ellipse` | `X`, `Y`, `A`, `B`, `C`, `D` | closed ellipse: center `(X, Y)`, `(A, B)` on the major axis end, `(C, D)` on the minor axis end |
| `PolylineTo` | `X`, `Y`, `A` | lines through the points in `A`'s `POLYLINE(...)` formula, then `(X, Y)` |
| `SplineStart`, `SplineKnot` | `X`, `Y`, `A` ... | uniform or clamped B-spline of the given degree through the knots |
| `NURBSTo` | `X`, `Y`, `A`, `B`, `C`, `D`, `E` | NURBS from `E`'s `NURBS(...)` formula |
| `CubBezTo`, `RelCubBezTo` | `X`, `Y`, `A`, `B`, `C`, `D` | cubic Bezier, `(A, B)` and `(C, D)` the controls |
| `QuadBezTo`, `RelQuadBezTo` | `X`, `Y`, `A`, `B` | quadratic Bezier lifted to a cubic |
| `InfiniteLine` | `X`, `Y`, `A`, `B` | the line through two points, cut to the page's extent |

Arcs: a circular arc with bulge `a` over chord length `c` has sagitta `|a|` and radius
`r = (c^2 / 4 + a^2) / (2 |a|)`; the sign of `a` picks the side (positive is to the left of the
chord direction, unverified). An elliptical arc is solved from its endpoints and the control
point on a rotated ellipse of the given ratio, and emitted as `Segment::Arc` (the scene's SVG
arc form, DISPLA-2). `Ellipse` is four cubic arcs. Splines and NURBS are evaluated by the
Cox-de Boor recursion at a fixed number of samples per span and emitted as cubic segments
(approximate).

The three formulas read as data, not evaluated: `POLYLINE(xType, yType, x1, y1, ...)`,
`NURBS(lastKnot, degree, xType, yType, x1, y1, knot1, weight1, ...)`, and the spline rows' own
cells. Only numeric literals are read; one with a reference or function skips the row's
extra points and draws a line to `(X, Y)`, with one warning. `xType` and `yType` 0 mean
relative to `Width` and `Height`, 1 absolute (unverified).

A path is closed (and filled) when its last point equals its first; an open path is stroked
only. `Rounding` (rounded corners) is not applied.

## VISIO-8 Line, fill, color

`LineWeight` (inches) times 96 gives the stroke width, at least 0.5 px. `LinePattern` 0 draws
no line, 1 solid, and 2 to 23 a dash and gap pair scaled by the width (the scene supports one
pair; VISIO-15): 2 is `(4, 2)`, 3 is `(1, 2)`, and 4 to 23 are `(3, 2)`, in multiples of the width.
`FillPattern` 0 is no fill, 1 solid `FillForegnd`; 2 to 40 (hatches and gradients) and
`FillGradientEnabled` draw solid `FillForegnd` with one warning per page. `FillForegndTrans` and
`LineColorTrans` (0 to 1) give the alpha.

A color cell's `V` is `#RRGGBB`, or an integer index: 0 to 23 the classic palette below, 24 and up
the document's `Colors/ColorEntry` with that `IX` (else black). An unparseable `V` on a cell
whose `F` starts `THEMEVAL(` is resolved from the theme through the shape's `QuickStyle*Color`
cell: 0 `dk1`, 1 to 6 `accent1` to `accent6`, 7 `lt1`, 8 `dk2`, 9 `lt2` (unverified); the
theme's `a:clrScheme` gives `srgbClr val` or `sysClr lastClr`. The theme is the one named by
the page's or shape's `ThemeIndex` (`themeN.xml`), else the first theme part.

| Index | Color | Index | Color | Index | Color | Index | Color |
|-------|-------|-------|-------|-------|-------|-------|-------|
| 0 | `#000000` | 6 | `#FF00FF` | 12 | `#800080` | 18 | `#9A9A9A` |
| 1 | `#FFFFFF` | 7 | `#00FFFF` | 13 | `#008080` | 19 | `#808080` |
| 2 | `#FF0000` | 8 | `#800000` | 14 | `#C0C0C0` | 20 | `#666666` |
| 3 | `#00FF00` | 9 | `#008000` | 15 | `#E6E6E6` | 21 | `#4D4D4D` |
| 4 | `#0000FF` | 10 | `#000080` | 16 | `#CDCDCD` | 22 | `#333333` |
| 5 | `#FFFF00` | 11 | `#808000` | 17 | `#B3B3B3` | 23 | `#1A1A1A` |

`BeginArrow` and `EndArrow` other than 0 draw a filled triangle at the open path's first or last
point, sized by the line weight and `BeginArrowSize` or `EndArrowSize`, the same shape for every
arrow index.

## VISIO-9 Text

The shape's `Text` is mixed content: character data, and the markers `cp`, `pp`, `tp` (each with
`IX`, starting the `Character` row, `Paragraph` row, or tab row of that `IX` for the text after
it) and `fld` (a field, drawn as its cached text). `Character` cells: `Size` (inches),
`Color`, `Style` (bit 1 bold, 2 italic, 4 underline, 8 small caps; small caps is not drawn),
`Font` (ignored). `Paragraph` cells: `HorzAlign` (0 left, 1 center, 2 right, 3 and 4 as left),
`IndLeft`, `IndRight`, `IndFirst`, `SpLine` (negative is a multiple of the font size, positive
inches; default 1.2 times the size), `SpBefore`, `SpAfter`. Text block cells: `TxtPinX`,
`TxtPinY`, `TxtLocPinX`, `TxtLocPinY`, `TxtWidth`, `TxtHeight`, `TxtAngle` (default: the whole
shape box, pin at its center), `LeftMargin`, `RightMargin`, `TopMargin`, `BottomMargin`, and
`VerticalAlign` (0 top, 1 middle, 2 bottom).

Lines break at newlines and wrap at word boundaries inside the block's width less margins; a
word wider than a line stays whole. Each line is one `Primitive::Text` (centered on its box, so
left and right lines are placed by measured width), underline is a path under it, in the line's
first run's color. Text is drawn upright: a shape's `Angle` and a `TxtAngle` move the text block's
center but do not rotate the text (VISIO-15). Text with no visible characters draws nothing.

## VISIO-10 Groups, connectors, layers, backgrounds, foreign shapes

- `Group` shapes draw their children in order, each with the group's transform (VISIO-5); a
  group's own geometry is drawn first.
- A connector is a shape with its own geometry (a `Geometry` section from `Dynamic connector`
  masters or its own); it is drawn like any shape, from its cached `BeginX`, `BeginY`, `EndX`,
  `EndY` and geometry. The page's `Connects` records are not used to reroute it.
- Layers: the page's `Layer` section rows have `Visible` (0 or 1) and the shape's `LayerMember`
  is a `;` separated list of row `IX`; a shape on only hidden layers is skipped (and its
  children). A shape on no layer is shown.
- A page with `BackPage` draws the page of that `ID` first, as a background, with the same
  size rules and then its own shapes. A page with `Background="1"` is never output by itself.
- A `Foreign` shape (`ForeignData` with `ForeignType` `Bitmap`, `Metafile`, `EnhMetaFile`, or
  `Object`) draws a placeholder: its box outlined in gray with both diagonals, plus any geometry
  and text it has, and one warning per `ForeignType`. Images are not drawn (the display list has
  no image item; DISPLA-2).

## VISIO-11 Pages and output

Every non-background page is one output, in `pages.xml` order, numbered from 1. A file with one
such page is one output with no suffix; with several, outputs are `STEM-N.ext` as Markdown blocks
are (CLI-12), and `--page N` picks one (no suffix). A page's scene is `PageWidth` by
`PageHeight` inches at 96 px per inch (a page with a missing, zero, or over-limit size is an
error), on the theme's background. The library draws one page: `Diagram::Visio(Visio)` holds the
`Document` (shared) and a page index; `Visio::pages` lists the drawable pages and `with_page`
picks one.

## VISIO-12 Integration

- `Lang::Visio`, `Kind::Visio` (name `Visio drawing`), extension `.vsdx` in any case, and `--lang
  visio` (alias `vsdx`). A file whose bytes start with `PK\x03\x04` and whose name is not another
  language's is read as Visio. Markdown has no Visio block; stdin reads bytes.
- `diagram::lower_bytes(&[u8], Option<Lang>) -> Lowered` is the bytes entry point; the string
  entry points never return Visio. A `Lowered` has `diagnostics` with empty ranges (a Visio
  file has no source text); a package error is one error diagnostic and an empty drawing.
- `sirene svg` and `sirene png` write each page; `sirene check` reads the package and prints
  diagnostics as `FILE: error: MESSAGE` (no line or column); `sirene ascii` fails with `error:
  FILE: a Visio drawing has no text form` and exit 1 (free-form drawings have no text layout).
  `--page N` is an option of `svg` and `png`. Directory scans pick up `.vsdx` files.
- wasm: `render_bytes(bytes, lang, theme)` next to the string entry points. GUI: opens `.vsdx`
  read-only like other decorated diagrams. Packaging adds the `application/vnd.ms-visio.drawing`
  MIME type and the `.vsdx` glob.
- The fuzz target `visio` feeds arbitrary bytes to `lower_bytes` and draws the result.

## VISIO-13 Limits and failure

A shape that cannot be drawn (a bad transform, an unreadable row) is skipped with one
diagnostic; a corrupt package, a missing required part, or a limit exceeded is one error and an
empty drawing. Limits (`visio/limits.rs`, LANGS-8):

| Limit | Value |
|-------|-------|
| Archive size | 64 MiB |
| Entries | 4096 |
| One part, uncompressed | 32 MiB |
| All parts read, uncompressed | 128 MiB |
| Compression ratio of an entry | 200 to 1 (above 1 KiB uncompressed) |
| XML part size | 16 MiB; nodes 1,000,000; element depth 256 |
| Pages | 256 |
| Masters | 4096 |
| Shapes per page (masters count in each instance) | 50,000 |
| Group nesting | 64 |
| Geometry rows per shape | 4096 |
| Master and style inheritance depth | 16 |
| Text length per shape | 64 KiB |

A DOCTYPE in any part is an error (`roxmltree`'s `allow_dtd` is false; entities beyond the five
predefined and numeric references are therefore never expanded). Compressed data is inflated
with a cap of the entry's declared size, itself checked against the limits before inflating.

## VISIO-14 Diagnostics

Warnings, each once per page or per kind: a fill pattern or gradient drawn solid; a Foreign
shape kind drawn as a placeholder; a formula too complex to read as data; a missing masters or
theme part; a shape with an unsupported `Type`; text drawn unrotated. Errors: VISIO-13's
package errors. Messages begin with the part name when they come from one
(`visio/pages/page1.xml: ...`).

## VISIO-15 Not supported

Not drawn: shadows, gradients and fill patterns beyond solid, line patterns beyond one dash
pair, line caps and joins, rounded corners (`Rounding`), embedded images and metafiles (VISIO-10),
rotated text, fonts, small caps, bullets, tab stops, text backgrounds, hyperlinks and shape
data, containers, callouts, 3D, connector rerouting from `Connects`, page and window settings
other than size, guides, and the binary `.vsd` and `.vdx` formats. Formulas are never evaluated
(VISIO-1). Zip64, encrypted, and multi-disk archives are errors.

## VISIO-16 Verification

Tests build packages from XML strings with a stored-entry zip writer (`tests/visio/package.rs`)
and check the display list and SVG against numbers worked by hand from VISIO-5 and VISIO-7.
They show sirene follows this spec, not that it matches Visio: the cached-value, unit, flip,
bulge-sign, `POLYLINE` argument, and theme-index rules marked unverified were not compared with
real Visio files.

## Feature status

| Feature | Status |
|---------|--------|
| Zip reader (stored, deflate), limits | pending |
| OPC relationships, content types | pending |
| Document, pages, masters, themes model | pending |
| Transform, groups, flips, rotation | pending |
| Geometry rows | pending |
| Line, fill, indexed and theme colors | pending |
| Text | pending |
| Inheritance (master, style sheets) | pending |
| Connectors, layers, backgrounds, Foreign placeholders | pending |
| CLI, wasm, GUI, packaging | pending |
| Fuzz target | pending |

## VISIO-T Tasks

- [x] VISIO-T1 Read the format pages named at the top; VISIO-3 to VISIO-9 are written from them.
- [ ] VISIO-T2 Compare unit handling (`U`), flip about the pin, bulge sign, `POLYLINE` argument
  types, and `QuickStyle*Color` indexes with files Visio wrote.
- [ ] VISIO-T3 Rotate text with the shape (needs a rotation field on the text item).
- [ ] VISIO-T4 Draw embedded images (needs an image item in the display list).
- [ ] VISIO-T5 Fill patterns, gradients, and multi-dash line patterns.
