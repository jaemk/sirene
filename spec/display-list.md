# Display List

Backend-independent drawing of diagrams (`sirene_core::scene`): paths and text in pixels, painted
by the GUI and written as SVG. Tests: `crates/sirene-core/src/scene/tests.rs`,
`crates/sirene-core/tests/svg.rs`, and `check_scene` in `flowchart_props.rs`.

## DISPLA-1 Scenes

`scene::flowchart(chart, metrics, theme)` lays out a flowchart with the layout engine
(`layout.md`) in pixel spacing (`scene::CONFIG`) and returns a `Scene`: a width, a height, a
background color, and elements in paint order. Every coordinate is finite and inside the scene.
There is a 16px margin around the drawing.

## DISPLA-2 Primitives

An element is a path or one line of text. A path is a list of segments (move, line, elliptical
arc with SVG flag semantics, cubic Bezier, close) with an optional fill and an optional stroke
(width, color, optional dash and gap). Text is centered on a point both ways, with a size and
color.

## DISPLA-3 Owners

Every element names the diagram item it belongs to: the diagram (its title), a node, an edge, or
a cluster, by index into the flowchart's lists.

## DISPLA-4 Text metrics

Text is measured through the `TextMetrics` trait, supplied by the backend that will draw the
scene, so labels fit the font that renders them. `Approx` estimates 0.6 em per terminal column
(wide characters count double) and is used for SVG output and tests.

## DISPLA-5 Themes

`Theme` holds the font size, line height, and colors, including an accent color for editing marks
such as selection outlines. `Theme::LIGHT` follows mermaid's default theme; `Theme::DARK` is a dark
variant.

## DISPLA-6 Styles

Node, edge, and subgraph styles apply `fill`, `stroke`, `stroke-width`, `stroke-dasharray`, and
`color` (text). Precedence, lowest first: for nodes, `classDef default`; for edges,
`linkStyle default`; then the item's classes in order; then its own `style` or `linkStyle`
declarations. Colors may be `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, a common
CSS name, `none`, or `transparent`. Unknown properties and unparsable values are ignored.

## DISPLA-7 Shapes

Each node shape is sized around its label and drawn with a distinct outline for the classic
shapes, circles, ellipse, triangles, fork, notched and sloped rectangles; the remaining mermaid
shapes draw as rectangles. Small, filled, and framed circles and forks show no label. Edge ends
are clipped to the outline of the shape they touch, so arrow tips meet the visible border.

## DISPLA-8 Drawing order and edges

Clusters paint first, outer before inner, with their titles; then edges; then nodes; then edge
labels on an opaque background. Edges are orthogonal paths with rounded corners; a short jog
between two parallel runs is drawn as one S-curve. Arrowheads are filled triangles, filled
circles, or crosses, scaled with the stroke width. Normal edges are 1.5px, thick 3.5px, dotted
2px dashed; invisible edges and their labels are not drawn. A diagram title is centered above
the drawing.

## DISPLA-9 SVG

`scene::svg::write` writes a scene as a standalone SVG document with a background rect, one
`path` per path element, and one `text` per text element (anchored at its center). Numbers have
at most two decimals; translucent colors use `fill-opacity`/`stroke-opacity`; text is
XML-escaped.

## DISPLA-10 Hit testing

`Scene::hit(point, slop)` returns the owner of the topmost element drawn at a point: a filled path
that contains it (even-odd), or a stroked path within `slop` plus half the stroke width. Text and
the diagram's own elements (its title) are never hit, so a label hits the shape or label
background under it. `Scene::bounds(owner)` is the bounding box of an owner's paths. Tests: unit
tests in `crates/sirene-core/src/scene/hit.rs`.

## DISPLA-11 PNG

With the `png` feature, `scene::png::write(scene, scale)` draws a scene's SVG (DISPLA-9) into an
RGBA PNG of `ceil(width * scale)` by `ceil(height * scale)` pixels, with all text
in the embedded Noto Sans Regular (`crates/sirene-core/fonts`, SIL OFL 1.1). No system fonts are
read, so the output is the same on every machine; characters Noto Sans lacks are not drawn.
`png::FontMetrics` measures text with the same font (DISPLA-4), using `Approx` for characters it
lacks. A scale that is not finite and positive, an image over 16384 pixels on a side, or a scene
with no area is an error. Tests: `crates/sirene-core/tests/png.rs`.
