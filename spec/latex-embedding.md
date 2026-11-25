# LaTeX Embedding

How formulas reach the typesetter of `latex-math.md` from Mermaid, PlantUML, and Markdown
sources: where the delimiters are, what text is a formula, how a formula sits in a label, and
what each language's reference says about unsupported math. Status: research; the open items
are in LEMB-T.

## LEMB-1 Mermaid

From the Mermaid math reference (<https://mermaid.js.org/config/math.html>): a formula is
written between `$$` delimiters, e.g. `$$x^2$$`, and is rendered by KaTeX. The reference lists
only two diagram types as supporting it:

- flowcharts: node labels (`A["$$x^2$$"]`) and edge labels (`|"$$\sqrt{x+3}$$"|`);
- sequence diagrams: participant aliases (`participant 1 as $$\alpha$$`), message text
  (`Solve: $$\sqrt{2+2}$$`), and notes (`Note right of 2: $$...$$`).

Text and a formula share a label (the sequence example mixes them); the reference gives no rule
for several formulas in one label, for escaping `$$`, or for what an unparsable formula
becomes. The `legacyMathML` and `forceLegacyMathML` settings choose between the browser's
MathML and KaTeX's stylesheet rendering; sirene draws the formula itself, so neither applies
and both are ignored. The reference states no KaTeX version, so KaTeX's current supported
functions list is the syntax target (LPA-2).

Decisions: sirene reads `$$...$$` in the flowchart and sequence label positions above, and
nowhere else in Mermaid (LEMB-T6 tracks whether other types should follow, since a source
that uses it elsewhere draws as plain text in Mermaid too). An unclosed `$$` is plain text. A
formula that fails to parse is drawn as its source (LEMB-4). A formula is set in display
style, as Mermaid's `displayMode: true`, at the theme font size in the label's text color. It
is found per line with the lazy `/\$\$(.*?)\$\$/`, has no escape, and `\\` in it is read as `\`
before typesetting, as Mermaid does. A label with a formula is not read as markdown.

Status: flowchart node and edge labels are done (`Label::runs`, `scene/mathtext.rs`,
`ascii/mathtext.rs`; tests in `tests/flowchart_math.rs`). Other kinds that lower to the
flowchart model share the label type but are not wired for `$$`: subgraph titles, end labels,
and compartment titles and rows draw the formula as text (PlantUML's tags are read there, see
LEMB-2). Sequence participant names and aliases, message
text, and notes are done (`tests/sequence_math.rs`); block, divider, group, and title text and
the PlantUML-only separators, delays, and `ref` text draw the formula as text. The sequence
model keeps `Text = Vec<String>` with the source, and `Sequence::math` (set by the Mermaid
lowering) says `$$` is a formula; `flowchart::formula::runs` gives a line's runs.

To verify in Mermaid's source (LEMB-T7): the delimiter regular expression, multiple formulas per
label, `$$` inside quoted and Markdown strings, and whether display or inline style is used.

## LEMB-2 PlantUML

From the PlantUML math reference (<https://plantuml.com/ascii-math>; the Creole page does not
list the tags):

- inline tags: `<math>...</math>` is AsciiMath and `<latex>...</latex>` is LaTeX, used in
  activity actions (`:<math>int_0^1f(x)dx</math>;`), notes, and sequence messages
  (`Bob -> Alice : Can you solve: <math>ax^2+bx+c=0</math>`);
- documents: `@startmath` ... `@endmath` (AsciiMath) and `@startlatex` ... `@endlatex` (LaTeX),
  one formula each (PCREO-2, PCREO-3).

PlantUML converts AsciiMath to LaTeX and renders that with JLaTeXMath, which is not bundled and
must be installed beside PlantUML. AsciiMath support was ported to Java in V1.2021.8 and
improved in V1.2024.5. The page's examples show `_` written as `\_` and LaTeX commands with a
doubled backslash; this looks like a Markdown artifact of the page, to confirm (LEMB-T8).

Decisions: AsciiMath is a front end that translates to the LaTeX math list, as PlantUML does,
so the typesetter has one input. A `<math>` or `<latex>` tag is read wherever the PlantUML
kind already reads Creole text (PCREO-1), not only in the three positions the reference
shows. An unclosed tag is plain text.

Status (`<latex>`): read in sequence participant names, messages, notes, and `ref` text and in the
labels of the graph kinds, including class and state compartment rows and titles, the labels at
the ends of an edge, subgraph titles, the document title, sequence group and frame text, and JSON
and YAML values (PCREO-3 lists them and what is not read). The tag is matched in any
case; several per line; no nesting; the body is verbatim, so a doubled backslash is a TeX line
break and `\n` inside a closed tag is not a line break. Typeset in text style (inline), not
Mermaid's display style. `<math>` (AsciiMath) is read in the same places, translated to TeX by
`math/asciimath.rs` (PCREO-2) and typeset in text style; `@startmath` is the document form.
`$$` is text in PlantUML, and `<latex>` is text in Mermaid: `flowchart::Delim` says which delimiters a label
(`Label::delim`) or a sequence (`Sequence::math`) reads.

To verify (LEMB-T8): the escape rules inside the tags (the reference page's doubled backslashes),
and how PlantUML aligns an inline formula with the text baseline.

## LEMB-3 Markdown

Open (LEMB-T3). Whether MARKDO renders `$...$` and `$$...$$` outside diagram fences. Not
decided; a Markdown file's prose is not drawn by sirene today.

## LEMB-4 One interface

Every consumer calls the same entry point, `math::typeset(source, MathStyle) -> Result<Formula, MathError>`
(`MathStyle` is `Display` or `Text`; a `Formula` has `width`, `ascent` and `descent` in em and
`place(origin, size, color)` returns its scene primitives), and treats a `Formula` as an inline item in
a label: width, ascent, and descent come from it, and the label's line grows to fit. A label
with several formulas and text is a row of runs. A formula that fails to parse is drawn as its
source text with one diagnostic on its span, never an empty label.

Status (flowchart): a line is a row of text and formula runs on one baseline. The baseline is
0.35 em below the center of a text line (`TextMetrics` has no ascent: `BASELINE` in
`scene/mathtext.rs`). A line is as tall as the larger of the text's and the formulas' extent on
each side of the baseline (a formula's ascent and descent in pixels plus 1 px), so never under
a line height. A formula is one unbreakable piece when a label wraps. The model keeps the
formula's source (`Span::formula`), not a `Formula`; the painter typesets for measuring and
drawing. In the text renderer a line is as many rows as its tallest formula (`ascii_formula_rows`),
text and formulas aligned on the baseline row, every row of a line the same width.

Status (sequence): the same runs and baseline rule (`scene/mathtext.rs` `draw_lines`,
`block_size`; shared with the flowchart painter). A header, message label, or note is as tall
as its lines' boxes and as wide as its widest line, so lanes and notes grow for a wide formula
and rows for a tall one; the arrow sits below the whole label. In the text renderer a message
label, note, or name is expanded to rows before sizing (`ascii/mathtext.rs` `text_rows`), so
the lifelines stay aligned.

## LEMB-5 Positions

A formula's source range is in the user's source, inside its delimiters, so a diagnostic from
the typesetter lands on the offending command (as LANGS-10 and PPRE-2 require for other
nested text).

Status (flowchart): the lowering typesets each formula of a node or edge label and reports a
failure as an error on the offending command, found in the label's source text (the label's
whole range when the decoded text is not found there as written). One diagnostic per failing
formula, once per edge for an `&` product. Without the `math` feature each formula gets one
warning on its body and is drawn as its source.

Status (sequence): the lowering reports each failing formula of a participant alias (`as` text
or a metadata `alias`), message, or note on the offending command, with the statement's range
as the fallback (`flowchart::formula::errors` is shared with the flowchart lowering).

## LEMB-T Tasks

- [x] LEMB-T1 Read the Mermaid math reference; LEMB-1 is filled from it.
- [x] LEMB-T2 Read the PlantUML math reference; LEMB-2 is filled from it.
- [ ] LEMB-T6 Check which other Mermaid types render `$$` in the Mermaid release sirene tracks,
  against its source and a live render, and extend LEMB-1.
- [ ] LEMB-T7 Read Mermaid's source for the `$$` rules listed at the end of LEMB-1.
- [ ] LEMB-T8 Read PlantUML's source or run it for the escape and alignment rules in LEMB-2.
- [ ] LEMB-T3 Decide LEMB-3 with the Markdown spec.
- [x] LEMB-T4 Define `Formula` and the inline-label item in the display list. A formula is drawn
  as `Primitive::Path` items (with an optional clip), placed on a baseline shared with the text
  runs of its line (`scene/mathtext.rs`, `ascii/mathtext.rs`); no new display-list item.
- [ ] LEMB-T5 Per diagram kind: where a label is measured and drawn today, and what must change
  to take a formula (a table in this spec). Done for PlantUML's graph kinds: node, edge, end,
  note, subgraph title, and compartment title and row labels are measured and drawn from
  `Label::runs` and `Formulas` (`scene/paint.rs`) and `Rows` (`ascii/mod.rs`); also the document
  `title`, sequence group titles, frame tabs and labels, dividers, separators, delays, and JSON
  and YAML values (`scene/sequence.rs`, `ascii/sequence.rs`, `Flowchart::title_delim`). Left,
  for PlantUML: headers, footers, captions, and legends (not drawn at all), and the kinds that
  keep plain strings with their own painters (mindmap, Gantt, timing, chronology, DOT; PCREO-3).
  Left, for Mermaid (LEMB-1): subgraph titles, end labels, and compartments, which stay text.
