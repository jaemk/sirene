# PlantUML Creole and Math

PlantUML `@startcreole`, `@startmath`, and `@startlatex` documents (PUML-2). Reference:
<https://plantuml.com/creole> and <https://plantuml.com/ascii-math>. Today each kind is an
error naming it.

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

`@startlatex` needs the JLaTeXMath jars next to PlantUML, and the reference does not bundle
them. It is an error naming the kind: `@startlatex is not supported`.

## PCREO-4 Open questions

- Neither the scene nor the ASCII renderer has rich text spans; Creole needs them, so this
  starts with a text-run model in the display list (DISPLA).
- AsciiMath typesetting needs a math layout (boxes, baselines, stretchy delimiters) that no
  other model shares. Whether a small one is worth it for one input language.
