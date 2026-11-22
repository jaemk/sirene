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
formula that fails to parse is drawn as its source (LEMB-4).

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

To verify (LEMB-T8): the exact escape rules inside the tags, nesting and multiple tags per line,
and how PlantUML aligns an inline formula with the text baseline.

## LEMB-3 Markdown

Open (LEMB-T3). Whether MARKDO renders `$...$` and `$$...$$` outside diagram fences. Not
decided; a Markdown file's prose is not drawn by sirene today.

## LEMB-4 One interface

Every consumer calls the same entry point, `math::typeset(source, MathStyle) -> Formula` (the
`latex-math.md` box tree with a baseline and a size), and treats a `Formula` as an inline item in
a label: width, ascent, and descent come from it, and the label's line grows to fit. A label
with several formulas and text is a row of runs. A formula that fails to parse is drawn as its
source text with one diagnostic on its span, never an empty label.

## LEMB-5 Positions

A formula's source range is in the user's source, inside its delimiters, so a diagnostic from
the typesetter lands on the offending command (as LANGS-10 and PPRE-2 require for other
nested text).

## LEMB-T Tasks

- [x] LEMB-T1 Read the Mermaid math reference; LEMB-1 is filled from it.
- [x] LEMB-T2 Read the PlantUML math reference; LEMB-2 is filled from it.
- [ ] LEMB-T6 Check which other Mermaid types render `$$` in the Mermaid release sirene tracks,
  against its source and a live render, and extend LEMB-1.
- [ ] LEMB-T7 Read Mermaid's source for the `$$` rules listed at the end of LEMB-1.
- [ ] LEMB-T8 Read PlantUML's source or run it for the escape and alignment rules in LEMB-2.
- [ ] LEMB-T3 Decide LEMB-3 with the Markdown spec.
- [ ] LEMB-T4 Define `Formula` and the inline-label item in the display list (DISPLA).
- [ ] LEMB-T5 Per diagram kind: where a label is measured and drawn today, and what must change
  to take a formula (a table in this spec).
