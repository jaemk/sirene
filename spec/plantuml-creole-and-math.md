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

Inline `<latex>...</latex>`: a formula in label text, in any case (`<LaTeX>`), set in text style
at the theme font size in the label's text color, on a common baseline with the text around it
(LEMB-4). Several tags per line are allowed. The body is opaque up to the first closing tag
(no nesting) and is read verbatim: no unescaping, so `\neq` and `\nu` are TeX commands and a
literal `\n` inside a closed tag is not a line break. An unclosed tag and a tag with an empty or
blank body are text, with no diagnostic. A formula that does not typeset is drawn as its source,
tags included, with one error on the offending command inside the tag. `$$...$$` and
`<math>...</math>` stay text (`<math>` waits for AsciiMath). With the `math` feature off the tags
are text and nothing is reported. `plantuml/creole.rs` finds the tags (`latex_tags`) and marks
the labels of a lowered graph; the runs are `flowchart::Span`s with `math` set, read with
`Delim::Latex` (`flowchart::formula`). Drawn in:

- sequence participant names (`as` text or a quoted name), message text, notes, and `ref` text;
- the labels of the kinds lowered to graphs: activity, class, state, description (component,
  deployment, usecase), WBS, Chen ER, and nwdiag, in node labels, edge labels, subgraph titles,
  and notes.

Not drawn: compartment rows (class and object members, state bodies), the multiplicity labels
at the ends of an edge, titles, headers and footers, legends, sequence group and divider text,
participant names only given as ids, mindmap, JSON, YAML, Gantt, timing, chronology, DOT, and
the other kinds that do not carry label runs. Tags there are text with no diagnostic.
Tests: `latex_inline_*` fixtures and `crates/sirene-core/tests/plantuml_math.rs`.

## PCREO-4 Open questions

- Creole beyond the inline `<latex>` tag (PCREO-1) is not read. The scene and the ASCII
  renderer carry formula runs (`flowchart::Span`) but not other rich text spans, so the rest
  starts with a text-run model in the display list (DISPLA).
- AsciiMath typesetting needs a math layout (boxes, baselines, stretchy delimiters) that no
  other model shares. It builds on the typesetter of `latex-math.md` (LMATH-7), not on its own.
