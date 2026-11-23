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
- Mermaid labels between `$$` delimiters in flowcharts and sequence diagrams, which Mermaid
  renders with KaTeX (LEMB-1, confirmed against its reference).
- PlantUML `<latex>` and `<math>` tags in Creole text (LEMB-2).
- Markdown `$...$` and `$$...$$`, if MARKDO adds it (LEMB-3).

## LMATH-8 Feasibility

Buildable in-tree with no new dependency beyond a font file. The algorithm is public and
specified, and the font reading is already a dependency. Rough size: the lexer, parser, and
math list about 2,500 lines of Rust; Appendix G layout with stretchy delimiters, radicals, and
big operators about 2,500; tables and environments about 1,000; the symbol and command table
(roughly 1,000 entries, generated from the Unicode math data) about 1,500 lines of data; the
ASCII renderer about 1,000; tests and fixtures on top. The first usable subset is a few weeks
of focused work, and broad KaTeX coverage is a multi-month effort that never ends, since
coverage is a long tail of commands.

Alternatives (details in `latex-prior-art.md`, read from their READMEs, not their sources):
`formulary` (MIT or Apache-2.0) and `formel` (MIT) lay out MathML from the OpenType MATH table
and give glyph positions, so a LaTeX parser would still be ours; `RaTeX` (MIT) has a LaTeX
parser and layout on KaTeX's fonts and metrics. They are small projects (formulary: 35 commits;
formel: 224). Adopting one saves layout work but brings its dependency, its maintenance risk, a
MathML detour, and output shaped by its model rather than the display list; each needs a trial
before a decision (LPA-T5). Their license is compatible; the other costs are open. The
project's rule of owning its parsers and layout (sirene architecture decisions) favors writing
the subset in-tree, porting from KaTeX with attribution where useful (LMATH-11).

## LMATH-9 Order

1. Font and MATH table reader, with a test that reads each constant from the bundled font.
2. Lexer, parser, and math list for atoms, scripts, `\frac`, and `\sqrt`; layout and SVG for
   those; snapshot tests of box positions.
3. Delimiters, big operators, accents, `\text` and font commands.
4. Environments and tables.
5. ASCII renderer.
6. `@startlatex` in PlantUML, then the other consumers.
7. Fuzz target and limits (LMATH-6), landing with step 2 so the parser is never unfuzzed.

## LMATH-11 Decision: KaTeX as the target, built in-tree (decided 2026-11-22)

Sirene writes its own typesetter so the project stays self-sufficient: no dependency on a
young third-party engine, and the limits, diagnostics, fuzzing, and display-list fit are
ours. KaTeX is the target in three ways, in this order of confidence:

1. Syntax and behavior: the command set, macros, error behavior, and `$$` use in Mermaid are
   KaTeX's (LMATH-5, LEMB-1). Settled; it is what Mermaid users write.
2. Reference and test source: KaTeX is MIT licensed (Khan Academy and other contributors,
   2013-2020), so its algorithms, symbol tables, metrics, and test inputs may be ported with a
   file header and a THIRD-PARTY-NOTICES entry (the attribution rule), as can Temml's (MIT).
   This replaces the earlier "borrow the algorithm, not code": the rule holds only for
   sources whose licenses do not allow it. Reading KaTeX's `buildHTML`, delimiter, radical,
   and spacing code is the fastest route to a correct layout.
3. Layout path: KaTeX's fonts and metrics (LRES-1), since they are 514 KB as TTF against a
   734 KB MATH-table font, match Mermaid's own drawing, and have a working reference. A
   MATH-table path (LPA-4) stays possible behind the same box tree, for a user-chosen font or
   for AsciiMath-heavy PlantUML documents, if there is demand.

The parser and layout are written in-tree in `sirene_core::math` as an idiomatic Rust design,
not a one-for-one port (LMATH-13). KaTeX's source is the reference for behavior and for
the data it holds (symbols, metrics, stretchy paths), and its test cases are the baseline the
implementation must satisfy (LPA-3). A file that adapts KaTeX code or data carries a header
naming the KaTeX file and its license (LMATH-12). RaTeX (LPA-5) and Temml are references to
compare against, not dependencies:
RaTeX is a 0.1.x project that releases monthly with a 262 KB layout file, and depending on it
would tie sirene's drawing, limits, and release timing to it. The box tree and `Formula`
interface (LEMB-4) are sirene's own, so the engine behind them can still change.

Still open: the metrics data's provenance (LRES-T7b) and the stretchy-shape data (LRES-T7d)
are copied data, so both are read before the port starts.

Where PlantUML differs: it renders with JLaTeXMath, not KaTeX, so a `<latex>` formula can differ
slightly from PlantUML's own output. KaTeX's behavior is the target for both, since there is
one typesetter (LMATH-7).

## LMATH-12 Module plan and KaTeX reference map

Sizes are the TypeScript files in `KaTeX/KaTeX` `src/` on 2026-11-22. Measured lines: the
modules of `src/` itself are 10,762 lines (of which the DOM and MathML builders, `Settings`,
and the data are not ported), `functions/` and `environments/` 6,750 lines, and the generated
`fontMetricsData.js` 2,077. The parse path (lexer 126, expander 470, `macros.ts` 1,038,
`Parser.ts` 1,072, `symbols.ts` 879) is about 3,600 lines and the layout path
(`buildCommon.ts` 783, `buildHTML.ts` 411, `delimiter.ts` 833, `stretchy.ts` 358,
`svgGeometry.ts` 561) about 2,950. A Rust port is expected to be of the same order, so the
first subset (LMATH-1) is about 6,000 to 7,000 lines and full KaTeX coverage about 11,000 to
13,000, plus generated data.

| Rust module (`sirene_core::math`) | KaTeX reference (behavior and data, not structure) | Size | Notes |
|---|---|---|---|
| `lexer` | `Lexer.ts`, `Token.ts`, `SourceLocation.ts` | 5 KB | Tokens carry byte ranges in the source, for LEMB-5. |
| `macros`, `expand` | `MacroExpander.ts`, `macros.ts`, `Namespace.ts`, `defineMacro.ts` | 16 KB, 42 KB | Expansion budget and depth bound (LMATH-6). `\gdef`, `\edef`, `\xdef`, `\let`, `\futurelet` are out of scope (LPA-2). |
| `symbols` | `symbols.ts`, `unicodeSymbols.js`, `unicodeAccents.js`, `unicodeScripts.ts`, `unicodeSupOrSub.ts`, `wide-character.ts` | 45 KB and small files | Data; generated and tested against the font (LRES-3). |
| `parser`, `node` | `Parser.ts`, `parseNode.ts`, `parseTree.ts`, `ParseError.ts`, `Settings.ts`, `strict.ts` | 40 KB | One error with a span; `Settings` reduced to what sirene exposes (no `trust`). |
| `functions/*` | `functions/*.ts` (accent, arrow, delimsizing, enclose, font, genfrac, horizBrace, kern, lap, mclass, op, operatorname, phantom, sizing, smash, sqrt, styling, supsub, text, underline, and more), `defineFunction.ts` | many small files | Each function is a parse rule plus a box builder; `href`, `includegraphics`, `html*`, `verb` are not ported (LPA-2). |
| `environments` | `environments/array.ts`, `defineEnvironment.ts` | to read | Matrices, `cases`, `aligned`. |
| `style`, `options`, `units` | `Style.ts`, `Options.ts`, `units.ts` | 3 KB, 9 KB, 4 KB | Display, text, script, and scriptscript styles and sizes. |
| `metrics` | `fontMetrics.ts`, `fontMetricsData.js` | 9 KB, 96 KB | The data file is generated (LRES-T7b). |
| `spacing` | `spacingData.ts` | 2 KB | TeX's spacing between atom classes. |
| `layout` | `buildCommon.ts`, `buildHTML.ts`, `delimiter.ts`, `stretchy.ts`, `svgGeometry.ts` | 26, 15, 30, 14, 30 KB | Produces the box tree of LMATH-2 instead of KaTeX's DOM tree (`domTree.ts`, `tree.ts` are not ported). `buildMathML.ts` and `mathMLTree.ts` are not ported. |
| `font_data` | `fonts/` | 514 KB TTF | Subset per LRES-T7c. |

Not reproduced: `Settings` options that load resources or follow links, `trust`, the DOM and
MathML builders, `\tag` numbering beyond the single formula, and the `contrib/` extensions
(`mhchem`, `copy-tex`, `auto-render`). The module split above is a starting point; the Rust
design of LMATH-13 may merge or split modules where KaTeX's file boundaries are artifacts of
its JavaScript registry pattern.

## LMATH-13 Design: Rust idioms, tests as the baseline

The typesetter is designed for Rust, with KaTeX's behavior as the specification and its test
cases as the oracle, not its code structure as the template.

- Types, not registries. KaTeX registers functions, environments, macros, and symbols in
  global maps filled by `defineFunction` calls with string keys. Here a command is an enum
  variant or a static table entry resolved at compile time, a function's parse rule and box
  builder sit together in one match arm or one small type, and there is no mutable global
  state. Parse nodes are an enum (a sum type) with a byte span on each node, not an object with
  a string `type` field.
- Errors are values. Every stage returns `Result<_, MathError>` with a span and a message
  naming the command; nothing panics, as the `production_panics` test requires of production
  sources (`xtask/tests/production_panics.rs`). Limits are one `Budget` value passed down,
  checked at each expansion and nesting step (LMATH-6).
- Ownership, not mutation. The lexer is an iterator of tokens over a `&str` with byte offsets;
  the macro expander is a stack of token sources behind the same iterator interface; the parser
  consumes tokens and returns a tree. Layout is a pure function from a node and an immutable
  `Options` (style, size, font, color) to a box; changing style or size builds a new `Options`.
  No DOM-style class lists, no CSS: a box is a struct with width, height, depth, and children,
  in typed units (`Em` as a newtype, not a bare float).
- One box tree for every output. SVG, PNG, and ASCII read the same tree through the display
  list (LMATH-4); KaTeX's HTML and MathML builders are not reproduced.
- Data is data. The symbol table, metrics, and stretchy paths are generated static arrays
  (LRES-3, LRES-T7b), with the generator in the repo and a staleness test; they are not
  code.
- Dependencies stay small: the crate needs no new dependency beyond what `sirene-core` already
  uses, and `#![forbid(unsafe_code)]` holds as in the rest of the workspace.

Baseline tests. KaTeX's suites are behavioral specifications to port as data, not as code
(LPA-3): each case is a TeX input plus an expected outcome in sirene's terms.

1. Parse expectations from `katex-spec.ts` (4,990 lines, 772 cases): the input and the shape of
   the resulting tree (atom classes, groups, scripts, fractions), expressed against sirene's
   node enum, and the inputs that must fail with the error KaTeX gives, from `errors-spec.ts`.
2. Layout expectations: KaTeX's rendered DOM carries each box's height, depth, and margins
   in em (`katex-spec.ts.snap`, and a node run of KaTeX over the fixtures to dump a metrics
   file, kept as a checked-in fixture with its generator script). The Rust layout is held to
   those numbers within a stated tolerance, which pins the algorithm without copying it.
3. Screenshot inputs from `ss_data.yaml` (128 named inputs) as drawing fixtures: SVG output
   must be non-empty, within its box, and match a snapshot reviewed once against KaTeX's own
   image.
4. Unicode and symbol cases from `unicode-spec.ts` and `symgroups.ts`.
5. Sirene's own cases on top: limits, diagnostics' spans, the display-list contract, ASCII
   output, and fuzz seeds.

A ported case that the Rust design deliberately changes (a different error text, a limit KaTeX
lacks) is listed in a deviations table in this spec, with the reason, so every difference from
KaTeX is a decision on record.

Per LMATH-6, the lexer, expander, and parser each carry their bound from the start, and
`sirene-fuzz` gets a `math` target with them (LANGS-11).

## LMATH-T Tasks

The work is split over four specs: this one (the engine), `latex-embedding.md` (Mermaid,
PlantUML, and Markdown), `latex-resources.md` (font, tables, licenses, size), and
`latex-prior-art.md` (other libraries, syntax coverage, tests). Their task lists are the
tracker; this one holds what is left for the engine itself.

- [ ] LMATH-T1 Finish the three research specs (their `-T` lists) before any code.
- [ ] LMATH-T2 Fix the first subset (LMATH-1) from the command table of LPA-2.
- [ ] LMATH-T3 Write the box tree and `Formula` types, and the TeX-to-MATH parameter table
  (LPA-T4).
- [x] LMATH-T4 Decide in-tree or an existing crate: in-tree (LMATH-11).
- [ ] LMATH-T5 Build in the order of LMATH-9, to the design of LMATH-13.
- [ ] LMATH-T6 Read the KaTeX behavior for the first subset, then write the Rust types for
  tokens, parse nodes, options, and boxes (LMATH-13) before any logic, and review them.
- [ ] LMATH-T8 Port the baseline tests as data (LMATH-13): extract the cases from
  `katex-spec.ts`, `errors-spec.ts`, `ss_data.yaml`, `unicode-spec.ts`, and `symgroups.ts` into
  fixtures, and write the script that dumps KaTeX's box metrics (needs node) with the
  generated fixture checked in.
- [ ] LMATH-T9 Start the deviations table (LMATH-13) as cases are ported.
- [ ] LMATH-T7 Dropped: the RaTeX spike. RaTeX stays a comparison target for the layout tests
  (LPA-3), not a dependency.

## LMATH-10 Open questions

- Which math font; a subset keeps the wasm bundle small (the docs site loads `sirene-wasm`)
  but a subset must cover every symbol the command table can produce.
- Whether the parser is the first consumer of a shared TeX-ish lexer or stays standalone.
- Mermaid's `$$` syntax and its fallbacks need checking against the Mermaid reference.
