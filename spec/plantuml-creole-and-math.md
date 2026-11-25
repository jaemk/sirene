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
- mindmap node text (the Mermaid mindmap model, read with `<latex>` and `<math>`, not `$$`);
- the labels of the kinds lowered to graphs: activity, class, state, description (component,
  deployment, usecase), WBS, Chen ER, and nwdiag, in node labels, edge labels, the labels at
  the ends of an edge (multiplicities and roles), subgraph titles (packages, composite states),
  and notes;
- the compartments of the class and state kinds: the title of a compartment node and each cell
  of a row (class, interface, enum, and object members, state bodies). A row with a formula is
  as tall as its tallest cell's line box, and in the text renderer as many lines as that, its
  cells on one baseline. Only nodes of these kinds read PlantUML text (`Label::delim`); a
  Mermaid `$$` in a title, a row, or an end label is text (LEMB-1).

A row has no range of its own, so a failing formula in one is found by its tags in the whole
source, after the labels have taken the places they are written in; it is on its place when
the text is written once there, and each of several equal formulas gets its own place in
order. A subgraph title is one line (its lines joined by spaces) in the scene and the rows of
its formulas in the text renderer, the first on the border.

- the document `title` of a graph kind (the chart's `title_delim` is `Delim::Latex`; the title
  is one line of runs set at 1.2 times the text size, centered, and as tall as its formulas,
  and in the text renderer as many rows as its formulas) and of a sequence (`Sequence::math`).
  Its formulas have no range of their own in a graph, so a failing one is found by its tags in
  the whole source, as a row's is;
- sequence group (`box`) titles, the tab and label of a frame (`alt`, `opt`, `loop`, `par`,
  `break`, `critical`, `group`), the label of `else`, the text of `==` separators and `...`
  delays. Each is one line of runs; its row is as tall as its tallest formula (a group's
  title row, a frame's tab, a divider, a separator, a delay), and in the text renderer as many
  rows as that, the first on the border or rule. The lowering checks each on its own line;
- JSON and YAML keys and values (they are rows of compartments, read as a class's are);
- the plain strings of three kinds that keep their text as strings: the Gantt title, section
  names, and task names; the chronology title and event text; and the timing diagram's title,
  signal names, state labels, change notes, notes, constraint and highlight labels, and message
  labels (see below).
  `Gantt::tags`, `Timeline::tags`, and `Timing::tags` (set by the PlantUML lowering) say the tags
  are formulas, and Mermaid's Gantt and timeline draw them as text. A Gantt row is as tall as its
  tallest name (never under the plain row), a chronology box wraps its text by words with a
  formula as one piece, and the title is as tall as its formulas. In the text renderer a Gantt
  name is as many lines as its formula, the bar on the baseline line; a chronology text with a
  formula is its rows, not wrapped. Each text is checked on its own line (a title in the whole
  source, as a graph's is), and a Gantt task name ends at the first `]`, so a formula in one
  cannot hold a `]`.
- the timing diagram's positions each grow their row to the tallest formula in it: a signal's
  name column and its robust level, concise band, or rectangle lane; a change note; a note; a
  constraint label; a highlight caption; and a message label, which grows the gap between its
  two signals. In the text renderer each is as many rows as its formula, around the baseline
  row. A formula is drawn in the label's text color. Plain labels keep their old layout. Each
  text is checked on its own line (a note's on its body lines), and a `:`, `->`, `<->`, `#`, or
  `,` inside a formula is not the statement's. A signal declared with a formula name needs the
  name quoted (`robust "<latex>x</latex>" as A`), since an unquoted name stops at `<`. A message
  label's formula between non-adjacent signals, or on one signal, is centered on the plain
  label's point and can overlap a band, as plain text does. Axis tick labels are moments or
  dates, not authored text, and are not read.

- the lines of a `header`, `footer`, `caption`, and `legend` (PCOM-1): each line is one row of
  runs set as the title's are, a line as tall as its tallest formula, in the scene and in text,
  of a diagram of any kind that reads them. The lines are found in the source on their own line,
  so a failing formula has one error on its body, and an equal one in a label does not take it.
  A replaced statement is not checked.

Not drawn: participant names only given as ids (an id has no tags), and the kinds that keep
their text as plain strings with their own painters, in the positions not listed above: the
timing diagram's axis ticks, the chronology's date labels (dates), and DOT (`@startdot` is
Graphviz's language, which has no tags, so its labels are Graphviz's, not Creole). Tags there
are text with no diagnostic; a DOT label reads `$$` instead, as a sirene extension (DOT-3,
LEMB-1), and their `header`, `footer`, `caption`, and `legend` do take formulas. In the text
renderer an end label is placed beside the line's
first or last cell and is not reserved room by the layout, so a wide one is clipped at the
drawing's edge, as a text one is.
Tests: `latex_inline_*` and `math_inline_*` fixtures,
`crates/sirene-core/tests/plantuml_math.rs`, and
`crates/sirene-core/tests/plantuml_math_titles.rs` (titles, group and frame text, JSON and
YAML), and `crates/sirene-core/tests/plantuml_math_plain.rs` (Gantt, chronology, timing), with
`plantuml_titles_and_frame_labels_typeset_each_formula_once_per_render` and
`plain_string_kinds_typeset_each_formula_once_per_render` in `scene/mathtext.rs` for the
typeset-once count.

## PCREO-4 Open questions

- Creole beyond the inline `<latex>` and `<math>` tags (PCREO-1) is not read. The scene and
  the ASCII renderer carry formula runs (`flowchart::Span`) but not other rich text spans, so
  the rest starts with a text-run model in the display list (DISPLA).
- Title formulas are set at one size (1.2 times the text), not mixed with bold text. DOT labels
  read `$$`, not tags (DOT-3). The Creole
  of a header, footer, caption, or legend is read by `decor::lines` with the run reader in
  `plantuml/creole.rs` (`creole::styled`: bold, italic, underline, strike, monospace,
  `<color>`, `<size>`, `<back>`; lists, rules, tables; PCOM-1), not yet elsewhere. The scene
  text primitive has no font family (monospace is drawn in the normal font) and the text
  renderer draws none of the styles.
