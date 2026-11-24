# PlantUML Creole and Math

PlantUML `@startcreole`, `@startmath`, and `@startlatex` documents (PUML-2). Reference:
<https://plantuml.com/creole> and <https://plantuml.com/ascii-math>. Today each kind is an
error naming it, except `@startlatex` (PCREO-3).

## PCREO-1 Creole

A `@startcreole` document is formatted text: headings (`== text`), bold, italic, monospace,
strikethrough, underline, and wave underline, bullet and numbered lists with nesting, horizontal
rules, `<color:...>`, `<size:...>`, `<back:...>`, and `<b>`-style tags. It draws as a text block.
Icons (`<&name>`), images (`<img:...>`), and `<U+XXXX>` codes are a warning naming them, except
`<U+XXXX>`, which is the character.

## PCREO-2 Math

`@startmath` is AsciiMath. It draws as a typeset formula: fractions (`a/b`), roots (`sqrt`,
`root`), sub- and superscripts, sums and integrals with limits, matrices, and Greek letters and
operators as their characters. A construct sirene does not typeset is a warning, and its source
is drawn as text.

## PCREO-3 LaTeX

`@startlatex` is one formula in TeX math mode, typeset by sirene's own typesetter
(`latex-math.md`) in display style. The body is the lines between `@startlatex` and
`@endlatex`, each trimmed and joined with a space; `'` starts no comment. It draws as the
formula's paths at the theme font size as one em, inside a 16 px margin, and as rows of text in
ASCII (`ascii_formula`). A formula that does not typeset is drawn as its source, with one error
on the offending command in the source (LEMB-4, LEMB-5). A missing `@endlatex` is a warning.
An empty body draws nothing and reports nothing. With the `math` cargo feature off, the source
is drawn as text with one warning that math support is off. The model is `Diagram::Equation`.
Tests: `latex_*` in `crates/sirene-core/tests/plantuml.rs` and `tests/fixtures/plantuml`.
AsciiMath (PCREO-2) shares its math list.

## PCREO-4 Open questions

- Neither the scene nor the ASCII renderer has rich text spans; Creole needs them, so this
  starts with a text-run model in the display list (DISPLA).
- AsciiMath typesetting needs a math layout (boxes, baselines, stretchy delimiters) that no
  other model shares. It builds on the typesetter of `latex-math.md` (LMATH-7), not on its own.
