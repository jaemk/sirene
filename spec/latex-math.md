# LaTeX Math

A LaTeX math typesetter in `sirene_core::math`: source to a box tree to positioned glyphs and
rules, drawn through the display list (DISPLA) and the ASCII renderer. It is the engine behind
PlantUML `@startlatex` (PCREO-3) and any other place a diagram holds a formula (LMATH-7). Status:
research; this spec records what the build needs and what it costs.

## LMATH-1 Scope

Math mode only, no text mode or page layout. First subset: ordinary, binary, relation, opening,
closing, and punctuation atoms with TeX's spacing; sub- and superscripts; `\frac`, `\sqrt` and
`\sqrt[n]`; `\left` ... `\right` and `\big` sizes; big operators with limits (`\sum`, `\prod`,
`\int`, `\lim`); `\text`, `\mathrm`, `\mathbf`, `\mathit`, `\mathbb`, `\mathcal`; accents
(`\hat`, `\bar`, `\vec`, `\dot`); `matrix`, `pmatrix`, `bmatrix`, `cases`, and `align`-style
environments; Greek letters and the common symbols. Out of scope: `\newcommand` and `\def`
beyond simple macros with arguments, `\catcode`, packages, `tikz`, and `\begin{document}`. A
command not in the table is an error naming it on its span, and the source is drawn as text.

## LMATH-2 Pipeline

1. A lexer and parser to a lossless syntax tree with diagnostics (the `syntax` and `diagnostic`
   modules, as FLOWCH-1 to FLOWCH-3).
2. A macro expander for the simple macros of LMATH-1, bounded as LMATH-6.
3. A math list: atoms, fractions, radicals, scripts, delimiters, and tables, as TeX builds it.
4. Layout by the rules of TeXbook Appendix G, with the sizing parameters read from the font's
   OpenType MATH table instead of TeX's `\fontdimen`s, and styles (display, text, script,
   scriptscript) as TeX has them.
5. A box tree with glyph runs and rules at absolute positions, one display-list item per run.

## LMATH-3 Font

Layout needs a font with an OpenType MATH table: constants, italic corrections, script and
accent attachment, size variants, and glyph assemblies for stretchy delimiters and radicals. The
bundled Noto Sans faces (`crates/sirene-core/fonts`) have none, so the build embeds one math font
under the SIL OFL (STIX Two Math or Latin Modern Math are the candidates, each under 1 MiB
before subsetting) with its license file beside the others, and a THIRD-PARTY-NOTICES entry. The
`ttf-parser` the workspace already uses reads the MATH table (`ttf_parser::math`, behind its
`opentype-layout` feature) and `rustybuzz` shapes text runs, so the font reading needs no new
dependency.

## LMATH-4 Output

SVG and PNG draw glyphs as paths taken from the font's outlines, so a drawing does not depend on
the viewer having the math font (as with the bundled text font, DISPLA-4). A formula is one
scene item with its baseline, so a diagram can align it to its label. The ASCII renderer draws a
formula in text rows: a fraction is a numerator, a rule of `-`, and a denominator, scripts use
Unicode super- and subscript characters where they exist and `^`/`_` forms where not, and a
radical is a radical sign (U+221A) with an overline. ASCII is lossy by nature; the SVG is the reference.

## LMATH-5 Compatibility

Layout follows TeX's algorithm, but the metrics are the OpenType font's, so sizes differ from
LaTeX's Computer Modern output by small amounts. The reference for syntax is KaTeX's supported
functions list; a command KaTeX supports and this lacks is a gap noted in the diagnostics, not a
silent difference.

## LMATH-6 Limits

As LANGS-8: groups nest at most 64 deep, a formula is at most 64 KiB of source, macro expansion
is at most 10,000 steps and 1 MiB of output, a table has at most 256 cells per side, and a
`\left` ... `\right` stretches to at most 16 times the font size. A formula past a limit reports
one error and is drawn as text. The fuzz targets cover the parser and the expander (LANGS-11).

## LMATH-7 Consumers

- PlantUML `@startlatex` and `@startmath` (PCREO-2, PCREO-3): a document that is one formula.
  AsciiMath would be a second front end onto the same math list.
- Mermaid labels between `$$` delimiters, which Mermaid renders with KaTeX in flowcharts and
  sequence diagrams (to be confirmed against the Mermaid reference before a spec of its own).
- Markdown `$...$` and `$$...$$`, if MARKDO adds it.

## LMATH-8 Feasibility

Buildable in-tree with no new dependency beyond a font file. The algorithm is public and
specified, and the font reading is already a dependency. Rough size: the lexer, parser, and
math list about 2,500 lines of Rust; Appendix G layout with stretchy delimiters, radicals, and
big operators about 2,500; tables and environments about 1,000; the symbol and command table
(roughly 1,000 entries, generated from the Unicode math data) about 1,500 lines of data; the
ASCII renderer about 1,000; tests and fixtures on top. The first usable subset is a few weeks
of focused work, and broad KaTeX coverage is a multi-month effort that never ends, since
coverage is a long tail of commands.

Alternatives, not verified against their sources here: pure-Rust crates that already lay out
formulas from the OpenType MATH table (`formulary`, `formel`, `noad`, which take MathML) or follow
KaTeX's metrics (`RaTeX`, with a LaTeX parser). Adopting one saves the parser and layout but
brings its dependency, its maintenance risk, a MathML detour, and output sized to its model
rather than the display list; each needs a trial before a decision, as does their license. The
project's rule of owning its parsers and layout (sirene architecture decisions) favors writing
the subset in-tree and borrowing the TeX algorithm, not code.

## LMATH-9 Order

1. Font and MATH table reader, with a test that reads each constant from the bundled font.
2. Lexer, parser, and math list for atoms, scripts, `\frac`, and `\sqrt`; layout and SVG for
   those; snapshot tests of box positions.
3. Delimiters, big operators, accents, `\text` and font commands.
4. Environments and tables.
5. ASCII renderer.
6. `@startlatex` in PlantUML, then the other consumers.
7. Fuzz target and limits (LMATH-6), landing with step 2 so the parser is never unfuzzed.

## LMATH-T Tasks

The work is split over four specs: this one (the engine), `latex-embedding.md` (Mermaid,
PlantUML, and Markdown), `latex-resources.md` (font, tables, licenses, size), and
`latex-prior-art.md` (other libraries, syntax coverage, tests). Their task lists are the
tracker; this one holds what is left for the engine itself.

- [ ] LMATH-T1 Finish the three research specs (their `-T` lists) before any code.
- [ ] LMATH-T2 Fix the first subset (LMATH-1) from the command table of LPA-2.
- [ ] LMATH-T3 Write the box tree and `Formula` types, and the TeX-to-MATH parameter table
  (LPA-T4).
- [ ] LMATH-T4 Decide in-tree or an existing crate (LPA-T5) and record the decision here.
- [ ] LMATH-T5 Build in the order of LMATH-9.

## LMATH-10 Open questions

- Which math font; a subset keeps the wasm bundle small (the docs site loads `sirene-wasm`)
  but a subset must cover every symbol the command table can produce.
- Whether the parser is the first consumer of a shared TeX-ish lexer or stays standalone.
- Mermaid's `$$` syntax and its fallbacks need checking against the Mermaid reference.
