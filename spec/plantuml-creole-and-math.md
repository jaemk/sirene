# PlantUML Creole and Math

PlantUML `@startcreole`, `@startmath`, and `@startlatex` documents (PUML-2). Reference:
<https://plantuml.com/creole> and <https://plantuml.com/ascii-math>. Today each kind is an
error naming it, except `@startmath` (PCREO-2) and `@startlatex` (PCREO-3).

## PCREO-1 Creole

A `@startcreole` document is formatted text: headings (`== text`), bold, italic, monospace,
strikethrough, underline, and wave underline, bullet and numbered lists with nesting, horizontal
rules, `<color:...>`, `<size:...>`, `<back:...>`, and `<b>`-style tags. It draws as a text block.
Icons (`<&name>`), images (`<img:...>`), and `<U+XXXX>` codes are a warning naming them, except
`<U+XXXX>`, which is the character.

## PCREO-2 Math

`@startmath` is AsciiMath. It is translated to TeX (`math/asciimath.rs`, a port of
ASCIIMathTeXImg.js, the translator PlantUML uses) and the TeX is typeset as in PCREO-3, in
display style: fractions (`a/b`), roots (`sqrt`, `root`), sub- and superscripts, sums and
integrals with limits, matrices, and Greek letters and operators. The body is read as for
`@startlatex`. The model is `Diagram::Equation` with `Flavor::AsciiMath`; the ASCII output is
the rows of the translated TeX.

The translator never fails: text it does not know is letters, and an unclosed bracket closes
with a dot. It follows the JavaScript's rules for previous-symbol sign handling, definitions
(`lceiling`, `rceiling`), brackets as `\left`/`\right` pairs, matrices of bracketed
rows, and two-argument symbols. Deviations: `text(...)` escapes TeX specials, `mspace` is
`\mkern`, HTML entities are not read, rows that are not groups do not make a matrix, and a few
commands KaTeX lacks are mapped to ones it has (`\operatorname`, `\prec`, `\square`,
`\twoheadrightarrow`). Source over 64 KiB or nested past the depth limit is an error.

Errors are reported on the source. The translation keeps a map from each piece of TeX to the
AsciiMath it came from; an error range in the TeX maps to the union of the source of the
pieces it touches, else the nearest piece before it, else after it, else the whole source.
Tests: `math::asciimath` unit tests (the plantuml.com examples with their TeX), `math_*`
fixtures, and `crates/sirene-core/tests/plantuml.rs`. Fuzz target: `fuzz/fuzz_targets/asciimath.rs`.

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
AsciiMath (PCREO-2) shares this path through the translated TeX.

Inline `<latex>...</latex>`: a formula in label text, in any case (`<LaTeX>`), set in text style
at the theme font size in the label's text color, on a common baseline with the text around it
(LEMB-4). Several tags per line are allowed. The body is opaque up to the first closing tag
(no nesting) and is read verbatim: no unescaping, so `\neq` and `\nu` are TeX commands and a
literal `\n` inside a closed tag is not a line break. An unclosed tag and a tag with an empty or
blank body are text, with no diagnostic. A formula that does not typeset is drawn as its source,
tags included, with one error on the offending command inside the tag. `<math>...</math>` is
the same with an AsciiMath body (PCREO-2), set in text style; each tag closes with its own name,
and the two may be mixed in a line. An unclosed tag does not stop a tag of the other name.
`$$...$$` stays text. With the `math` feature off the tags are text and nothing is reported.
`plantuml/creole.rs` finds the tags (`latex_tags`) and marks the labels of a lowered graph; the
runs are `flowchart::Span`s with `math` set, read with `Delim::Latex` (`flowchart::formula`),
whose tag picks the language per run (`Span::is_asciimath`). Drawn in:

- sequence participant names (`as` text or a quoted name), message text, notes, and `ref` text;
- the labels of the kinds lowered to graphs: activity, class, state, description (component,
  deployment, usecase), WBS, Chen ER, and nwdiag, in node labels, edge labels, subgraph titles,
  and notes.

Not drawn: compartment rows (class and object members, state bodies), the multiplicity labels
at the ends of an edge, titles, headers and footers, legends, sequence group and divider text,
participant names only given as ids, mindmap, JSON, YAML, Gantt, timing, chronology, DOT, and
the other kinds that do not carry label runs. Tags there are text with no diagnostic.
Tests: `latex_inline_*` and `math_inline_*` fixtures and
`crates/sirene-core/tests/plantuml_math.rs`.

## PCREO-4 Open questions

- Creole beyond the inline `<latex>` and `<math>` tags (PCREO-1) is not read. The scene and
  the ASCII renderer carry formula runs (`flowchart::Span`) but not other rich text spans, so
  the rest starts with a text-run model in the display list (DISPLA).
