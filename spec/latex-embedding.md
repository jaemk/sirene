# LaTeX Embedding

How formulas reach the typesetter of `latex-math.md` from Mermaid, PlantUML, and Markdown
sources: where the delimiters are, what text is a formula, how a formula sits in a label, and
what each language's reference says about unsupported math. Status: research; the open items
are in LEMB-T.

## LEMB-1 Mermaid

Research (LEMB-T1). Mermaid renders KaTeX in labels written between `$$` delimiters. To record
from the Mermaid reference: the delimiter rule and escaping, the diagram types that accept math
(flowchart and sequence are expected; check the rest), whether it works in node, edge, subgraph,
participant, and note text, what the `legacyMathML` and `forceLegacyMathML` settings change, how
a formula mixes with plain text and Markdown strings in one label, and what an unparsable
formula renders as.

## LEMB-2 PlantUML

Research (LEMB-T2). Two forms are expected: a whole document (`@startlatex`, `@startmath`,
PCREO-2, PCREO-3), and an inline tag in Creole text (`<math>` and `<latex>`), usable wherever
Creole is (labels, notes, titles). To record from the PlantUML reference: the exact tags and
closers, whether `<math>` is AsciiMath and `<latex>` is LaTeX, how an inline formula is aligned
with surrounding text, and the document options.

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

- [ ] LEMB-T1 Read the Mermaid math reference and its source; fill LEMB-1; list the fixtures to
  write.
- [ ] LEMB-T2 Read the PlantUML creole, math, and latex references; fill LEMB-2.
- [ ] LEMB-T3 Decide LEMB-3 with the Markdown spec.
- [ ] LEMB-T4 Define `Formula` and the inline-label item in the display list (DISPLA).
- [ ] LEMB-T5 Per diagram kind: where a label is measured and drawn today, and what must change
  to take a formula (a table in this spec).
