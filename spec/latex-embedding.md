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

Text and a formula share a label (the sequence example mixes them). The `legacyMathML` and
`forceLegacyMathML` settings choose between the browser's MathML and KaTeX's stylesheet
rendering; sirene draws the formula itself, so neither applies and both are ignored. The
reference states no KaTeX version, so KaTeX's current supported functions list is the syntax
target (LPA-2).

In Mermaid 12.3.0's source (LEMB-T6), `$$` is read wherever a label goes through `createText`
with HTML labels (the default): flowchart, state, class, ER, mindmap, requirement, kanban, block,
and the agentflow, usecase, and swimlane types that reuse the flowchart renderer (node, edge, edge
end, and subgraph titles, class members, ER attributes, and so on), and in sequence participant
names, messages, notes, and `loop`, `alt`, `opt`, `par`, `critical`, and `break` labels and their
section titles. Titles never render math. Architecture, C4, gantt, pie, journey, timeline,
quadrant, xychart, sankey, packet, radar, gitGraph, treemap, and treeView draw `$$` as text. With
`htmlLabels: false` nothing renders math (class diagrams force HTML labels when `$$` is present).

Mermaid's rules (LEMB-T7; `diagrams/common/common.ts:295-365`,
`rendering-util/createText.ts:340-353`): a formula is found with the lazy `/\$\$(.*?)\$\$/g`
(`.` does not cross a newline, so a pair sits on one line; several per label; no escape for
`$$`) and is set in display style (`displayMode: true`). `\\` is collapsed to `\` in every
diagram except sequence: `createText` replaces `\\` with `\` in the whole label before KaTeX,
while sequence text is passed through unchanged. A label with `$$` skips markdown and entity
decoding. A formula KaTeX rejects throws (`throwOnError: true`) and Mermaid draws no fallback.
Not checked against a live render: `\\` in the reference's own `cases` examples, `<br>` inside a
math label, and the error diagram.

Decisions: sirene reads `$$...$$` in the flowchart and sequence label positions above and in
the label positions of the kinds that lower to the flowchart model and of mindmap and kanban
(Status), and nowhere else in Mermaid (LEMB-T5 lists what is left). An unclosed `$$` is plain text. A
formula that fails to parse is drawn as its source with a diagnostic (LEMB-4). A formula is set
in display style at the theme font size in the label's text color. `\\` is read as `\` before
typesetting in flowchart-model labels and kept in sequence text, as Mermaid does. A label with a
formula is not read as markdown.

Status: flowchart node and edge labels are done (`Label::runs`, `scene/mathtext.rs`,
`ascii/mathtext.rs`; tests in `tests/flowchart_math.rs`). Every kind that lowers to the
flowchart model reads `$$` in the positions that model draws, in both renderers, through
`Label::delim` (`Delim::Dollar`): subgraph titles (flowchart subgraphs, state composite titles,
class namespace titles, block groups), edge end labels (class cardinalities), compartment titles
(class and ER names, requirement names) and rows (class members, ER attributes, requirement
fields), notes, and state names, descriptions, and transition labels. Tests:
`tests/mermaid_math.rs` and `scene::mathtext` (one typeset per formula per render). Agentflow,
swimlane, and use case diagrams lower to the flowchart model (agentflow and swimlane share the
flowchart lowering, use case has its own), so they read `$$` in the same positions, with
fixtures in `tests/mermaid_math.rs`. Mindmap node text, and kanban card text, ticket, and
assignee and column titles, are read too, though those kinds have their own models and painters:
the painters take `Label::runs` and share the render's `Formulas` and `Rows`, a card's ticket
and assignee with a formula are one unbreakable line (not cut with `...`), and in text a kanban
column title with a formula is rows under the column's top border. The diagram `title` of every
kind stays text (Mermaid never renders math in a title). DOT, C4, and architecture build
the same `Label` and so also read `$$` in these positions; Mermaid's C4 and architecture draw it
as text (known difference). Sequence participant names and aliases, message
text, notes, and the labels of `loop`, `alt`, `opt`, `par`, `critical`, and `break` blocks and
of their `else`, `and`, and `option` sections are done (`tests/sequence_math.rs`); `box` and
diagram titles draw the formula as text, as Mermaid does, and a `rect` label is a color. The
sequence model keeps `Text = Vec<String>` (and block labels as `String`) with the source, and
`Sequence::math` (set by the Mermaid lowering to `Delim::DollarRaw`) says `$$` is a formula;
`flowchart::formula::runs` gives a line's runs. `Delim::DollarRaw` keeps `\\` as written: a
sequence diagram does not collapse it (a TeX row break in `cases`), while flowchart labels
(`Delim::Dollar`) do.

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
improved in V1.2024.5. v1.2026.10 vendors JLaTeXMath 1.0.7; earlier releases and the reference
page need the jars installed beside PlantUML. The page's `\_` and doubled backslashes are an
artifact of its Markdown rendering: its HTML has single backslashes and plain `_`
(`<latex>\int_0^1f(x)dx</latex>`), and `\_` is not special in a tag.

Decisions: AsciiMath is a front end that translates to the LaTeX math list, as PlantUML does,
so the typesetter has one input. A `<math>` or `<latex>` tag is read wherever the PlantUML
kind already reads Creole text (PCREO-1), not only in the three positions the reference
shows. An unclosed tag is plain text.

Status (`<latex>`): read in sequence participant names, messages, notes, and `ref` text and in the
labels of the graph kinds, including class and state compartment rows and titles, the labels at
the ends of an edge, subgraph titles, the document title, sequence group and frame text, and JSON
and YAML values, and the Gantt title, section names, and task names, the chronology title and
event text, and the timing title, signal names, state labels, notes, constraints, highlights,
messages, and change notes (PCREO-3 lists them and what is not read). The tag is matched in any
case; several per line; no nesting; the body is verbatim, so a doubled backslash is a TeX line
break and `\n` inside a closed tag is not a line break. Typeset in text style (inline), not
Mermaid's display style. `<math>` (AsciiMath) is read in the same places, translated to TeX by
`math/asciimath.rs` (PCREO-2) and typeset in text style; `@startmath` is the document form.
`$$` is text in PlantUML, and `<latex>` is text in Mermaid: `flowchart::Delim` says which delimiters a label
(`Label::delim`) or a sequence (`Sequence::math`) reads.

PlantUML v1.2026.10 (LEMB-T8): the tag is `<latex>(.+?)</latex>` (and `<math>`), case-insensitive,
one line (`Splitter.java:79-80`); a line that is exactly `<latex>` opens a block ending at a line
that is exactly `</latex>`. The body is verbatim (`Display.getWithNewlines` is in raw mode in
the tags), so `\\` stays two backslashes and `\n` is not a line break. The formula is set in
display style at a fixed size 20 (`math/TeXIconBuilder.java:57`) with a 1 px inset, not at the
surrounding font size. Its color is the text's font color (black if not a plain color). Its box
is bottom-aligned with the line, with no baseline matching (`Sea.java:72-80`). A failing formula
is drawn as its source in black monospaced text, and the diagram still renders. sirene differs
deliberately: text style at the font size, the shared baseline rule (LEMB-4), and the source in
the label's color with a diagnostic. Not checked: JLaTeXMath's top-level `\\` and AsciiMath's own
backslash handling (LEMB-T10).

## LEMB-3 Markdown

Decided (LEMB-T3). A Markdown file's formulas are blocks beside its diagram blocks (MARKDO-5),
numbered and drawn like them; sirene draws no prose. They are:

- fenced blocks named `math`, `latex`, or `tex`, read as a TeX file (below), in display style;
- `$$...$$`, in display style, not across a blank line;
- inline `$...$` by GitHub's rules, in text style: the opening `$` is not followed by whitespace,
  the closing one is not preceded by whitespace or followed by a digit, the body is not empty,
  `\$` is a literal dollar sign, and the formula stays on one line.

Nothing is read in a fenced block of any other language (so not in a diagram), an indented code
block, or a code span. A diagnostic lands on the offending command in the Markdown file. Without
the `math` feature a formula is skipped with one warning per file.

A TeX file (`.tex`, `.latex`, `--lang latex` or `tex`) is one formula, `Equation::tex`: a bare
formula, or one wrapped in `$$...$$`, `$...$`, `\[...\]`, `\(...\)`, or a `displaymath` or `math`
environment (taken off); an `equation`, `align`, `gather`, or other math environment is passed to
the typesetter whole. `%` comments around the wrapper are skipped. A source with `\documentclass`
or `\begin{document}` is read only when the text between `\begin{document}` and
`\end{document}` is one such block; otherwise it gets one error on its first line, `full LaTeX
documents are not supported; give one formula`, and is drawn as its source. It lowers to
`Equation` with `Flavor::Tex`, so every output path and LEMB-4 apply. Directory scans do not pick
up `.tex` files. Tests: `crates/sirene-core/tests/latex.rs` and `markdown_math.rs`.

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
text and formulas aligned on the baseline row, every row of a line the same width. The
charset of the render (`Options::charset`, held by `Rows`) picks the formula's characters when the
block is built, so widths are those of the characters drawn (see LMATH-4 in `latex-math.md`).

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

Status (state, class, ER, requirement, block, flowchart subgraphs): the lowering reports each
failing formula of a label on the offending command in its token (`flowchart::formula::check_label`),
the token's range as the fallback; one error per failing formula, and without the `math`
feature one warning per formula. Tests: `tests/mermaid_math.rs`.

Status (mindmap, kanban, agentflow, swimlane, use case): one error per failing formula on its span,
and without the `math` feature one warning per formula, for mindmap node text (`mindmap::lower`),
kanban card text and column titles (the token or the `@{ ... }` body they are read from) and
`ticket`, `assigned`, and `label` metadata, flowchart-model node `label` metadata (agentflow
`@{ label: ... }`), and use case labels, which have no range of their own once lowered and are
found in the source as written. Tests: `tests/mermaid_math.rs`.

Status (sequence): the lowering reports each failing formula of a participant alias (`as` text
or a metadata `alias`), message, note, block label, or section title on the offending command,
with the statement's range as the fallback (`flowchart::formula::errors` is shared with the
flowchart lowering). Without the `math` feature each formula gets one warning on its body.

## LEMB-T Tasks

- [x] LEMB-T1 Read the Mermaid math reference; LEMB-1 is filled from it.
- [x] LEMB-T2 Read the PlantUML math reference; LEMB-2 is filled from it.
- [x] LEMB-T6 Check which other Mermaid types render `$$` in the Mermaid release sirene tracks
  (12.3.0), against its source, and extend LEMB-1. Not checked against a live render.
- [x] LEMB-T7 Read Mermaid's source for the `$$` rules; they are in LEMB-1.
- [x] LEMB-T8 Read PlantUML's source for the escape and alignment rules; they are in LEMB-2.
- [x] LEMB-T9 Read `$$` in mindmap node text, kanban cards, and the agentflow, usecase, and
  swimlane types.
- [ ] LEMB-T10 Confirm JLaTeXMath's `\\` outside an environment and AsciiMath's backslash
  handling, and the Mermaid live-render items at the end of the LEMB-1 rules.
- [x] LEMB-T3 Decide LEMB-3 with the Markdown spec.
- [x] LEMB-T4 Define `Formula` and the inline-label item in the display list. A formula is drawn
  as `Primitive::Path` items (with an optional clip), placed on a baseline shared with the text
  runs of its line (`scene/mathtext.rs`, `ascii/mathtext.rs`); no new display-list item.
- [ ] LEMB-T5 Per diagram kind: where a label is measured and drawn today, and what must change
  to take a formula (a table in this spec). Done for PlantUML's graph kinds: node, edge, end,
  note, subgraph title, and compartment title and row labels are measured and drawn from
  `Label::runs` and `Formulas` (`scene/paint.rs`) and `Rows` (`ascii/mod.rs`); also the document
  `title`, sequence group titles, frame tabs and labels, dividers, separators, delays, and JSON
  and YAML values (`scene/sequence.rs`, `ascii/sequence.rs`, `Flowchart::title_delim`); and, for
  Mermaid sequence diagrams, block labels and section titles (the same code). PlantUML mindmap
  node text is done: it shares the Mermaid mindmap model and reads `<latex>` and `<math>`. Done,
  for the PlantUML kinds that keep plain strings: the Gantt title, section names, and task names;
  the chronology title and event text; and the timing title, signal names, state labels, change
  notes, notes, constraints, highlights, and messages, each with a row that grows to its formula
  (`scene/gantt.rs`, `scene/timeline.rs`, `scene/timing.rs`, and their `ascii` twins, drawn from
  `tag_runs` and `tag_rows`). Left, for PlantUML: headers, footers, captions, and legends (not
  drawn at all, so none is added); the timing axis ticks (moments, not text); the chronology's
  date labels; and DOT (`@startdot` is Graphviz's language, with no tags; PCREO-3).
  Done, for Mermaid (LEMB-1): flowchart subgraph titles; state names, descriptions, notes,
  composite titles, and transition labels; class names, members, notes, namespace titles,
  relation labels, and cardinalities; ER entity names, attributes, and relationship labels;
  requirement names and fields; block labels, group titles, and edge labels (all measured and
  drawn from `Label::runs`, `Formulas`, and `Rows`). Also done, for Mermaid: mindmap node text;
  kanban card text, ticket, and assignee and column titles; and agentflow, swimlane, and use case
  labels (the same painters as the flowchart). Left, for Mermaid: requirement relationship labels
  (always the fixed `<<kind>>` text). Diagram titles stay
  text, as in Mermaid. Mermaid sequence `box` and diagram titles stay text by design.
