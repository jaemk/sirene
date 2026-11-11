# Mermaid themes

A diagram's own Mermaid theme settings, read from its source and applied to SVG, PNG, and the
editor's drawing (`sirene_core::theme`). Tests: unit tests in `crates/sirene-core/src/theme.rs`
and `diagram_themes` in `crates/sirene/tests/cli.rs`.

## MTHEME-1 Sources

`theme` and `themeVariables` are read from the frontmatter's `config:` map and from every
`%%{init: ...}%%` or `%%{initialize: ...}%%` directive, in any diagram type. Directives take
precedence over the frontmatter, and a later directive over an earlier one. Directive bodies are
JSON or the lenient form Mermaid accepts (single quotes, bare keys).

## MTHEME-2 Themes

The theme names are `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`,
`redux-dark`, `redux-color`, and `redux-dark-color`. A named theme replaces the application's
theme (`--theme`, the editor's light or dark setting) with the background, text, node, edge,
edge label, and subgraph colors Mermaid 12.3 derives for it. Any other name is a warning on its
value and leaves the application's theme in place.

## MTHEME-3 Theme variables

`themeVariables` override the theme's own values before the derived colors are computed, as in
Mermaid (e.g. `primaryColor` on `base` recolors nodes, borders, and subgraphs). Variables
without a theme name apply to the application's theme. Values that are not colors are ignored.
`fontSize` (a number, optionally with `px`, from 1 to 200) sets the font size and scales the
line height.

## MTHEME-4 Chart colors

A named theme also sets the section colors of mindmaps, timelines, radar charts, and treemaps
(`cScale0` to `cScale11`) and the pie slice colors (`pie1` to `pie12`), derived as Mermaid 12.3's
theme derives them; `cScaleN` and `pieN` variables override single colors. Without a theme name,
these colors keep sirene's series colors unless a `cScaleN` or `pieN` variable is set, and then
the others are the `default` theme's. Other charts (git graphs, gantt charts, journeys, sankey
diagrams, XY charts, Venn diagrams) keep sirene's series colors, and other diagram-specific
variables are not applied. Text output is not themed. Tests: `chart_colors` in
`crates/sirene-core/src/theme.rs`, `diagram_themes`.
