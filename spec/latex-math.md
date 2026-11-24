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
is at most 10,000 steps and 1 MiB of output (checked while a body is built, so an argument
repeated many times in one body is stopped before it is allocated), the tokens a `\def`
parameter must be followed by are at most 256, a table has at most 256 cells per side, and a
`\left` ... `\right` stretches to at most 16 times the font size, and a length in the source is
at most 1000 em either way. A formula past a limit reports
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

## LMATH-14 Built so far: lexer, errors, limits, symbols, expander

`sirene_core::math` (crate-private until the typesetter draws a formula) has:

- `MathError { message, span }`: KaTeX's wording without its `KaTeX parse error:` prefix, and a
  byte range in the formula (LEMB-5).
- `limits`: `MAX_SOURCE` (64 KiB), `MAX_DEPTH`, `MAX_EXPANSIONS`, `MAX_EXPANDED` (LMATH-6), and
  `check_source`, which `Lexer::new` applies. The expansion and depth bounds are consumed by the
  expander and parser, which do not exist yet.
- `Lexer` and `Token { kind, span }` with typed kinds `Space`, `ControlWord(&str)`,
  `ControlSymbol(char)`, `Char(&str)`, `Verb { star, delimiter, body }`, and `Eof`, borrowing
  the source. It reads what KaTeX's lexer reads (its token regular expression, `src/Lexer.ts`)
  and agrees with KaTeX on every input of the baseline (`math::baseline`, from
  `katex-oracle.json`); it has unit tests per token class and a property test that spans are
  ordered and valid for any input.

Deviations from KaTeX (LMATH-T9):

| Where | KaTeX | Here | Why |
|---|---|---|---|
| Token type | text strings, `\frac` as `"\\frac"` | typed kinds | match on a kind and a name, not on text with a backslash |
| Catcodes | `%` and `~` are table entries a macro can change | `%` always starts a comment; `~` is a plain character the expander treats as the non-breaking space | `\catcode` is out of scope (LPA-2) |
| `%` with no newline after it | a strict-mode warning | silent: the comment ends the formula | no strictness setting (LMATH-12) |
| `\` before a character outside the BMP | "Unexpected character: '\\'" | same | KaTeX parity |
| Source length | unbounded | 64 KiB | LMATH-6 |

- `symbols`: `Mode`, `Font`, `AtomGroup`, `Symbol { font, group, replace }`, and
  `lookup(mode, Name::Char(_) | Name::Control(_))`, a binary search over four byte-sorted
  tables (math and text, characters and control sequences) generated from KaTeX's symbol table
  by `scripts/katex-symbols.mjs` into `symbols_data.rs` (998 and 490 math entries, 682 and 68
  text entries; KaTeX 0.19.1). Tests: sortedness and size, spot lookups, and a baseline check
  that every symbol leaf in KaTeX's parse trees for its own test suite (4,415 of them) is in the
  table with the same atom group. The 87 leaves where KaTeX's parser makes a `textord` without
  the table (characters of a supported script such as Cyrillic or CJK, `\char` results, the
  backslash in `\href` text) belong to the parser stage and are counted, not skipped.

- `expander`: TeX's gullet. `Expander::new(src, Mode)`; `future`, `pop_token`, `push_token`,
  `consume_spaces`, `consume_arg(delimiters)` (an undelimited argument skips spaces and loses its
  braces; a delimited one is the shortest balanced run ending in the delimiters),
  `scan_argument(optional)` (the parser's reading of a function argument, ended by an `Eof`
  marker), `expand_once`, `expand_next_token`, `begin_group`, `end_group`, `end_groups`,
  `switch_mode`, and `is_defined`. A macro is looked up by a borrowed `Key` (control word,
  control symbol, or active character) in this order: definitions made in the formula, the
  commands that read their own arguments, then KaTeX's predefined macros. Definitions are
  scoped to groups with an undo list per group, and `\gdef` is global.
- Implemented here: `\newcommand`, `\renewcommand`, `\providecommand` (up to nine parameters),
  `\def` and `\gdef` (delimited parameters, `#{`), `\@firstoftwo`, `\@secondoftwo`,
  `\@ifnextchar`, `\TextOrMath`, `\char` (to `\@char{N}`, in the four forms), `\dots`
  (amsmath's choice of `\dotsb`, `\dotsc`, `\dotsi` or `\dotso` from the token after it),
  `\dotso`, `\dotsc`, `\cdots` (a thin space before a closing symbol or punctuation),
  `\expandafter`, `\noexpand` (a `Frozen` token, which the parser reads as `\relax`),
  `\message`, `\errmessage`, `\show` (read their argument, write nothing), and `\tag@literal`.
  `\expandafter` leaves `\def` alone, since it is a function of the parser in KaTeX. The other
  277 control-sequence and 42 character macros are
  KaTeX's table of TeX source, generated into `macros_data.rs` by `scripts/katex-macros.mjs`
  (Node 24 loads `macros.ts` through the resolver hook `scripts/katex-ts-loader.mjs`).
- Bounds: `MAX_EXPANSIONS` macro calls and `MAX_EXPANDED` bytes of produced tokens, groups at
  `MAX_DEPTH`.
- Not implemented yet, each an error naming the command: `\bra@ket`, `\bra@set` (which
  `\Braket` and `\Set` use). A test keeps this list equal to KaTeX's function-defined macros
  minus the implemented ones. Also out of scope (LPA-2): `\edef`, `\xdef`, `\let`, `\futurelet`,
  `\global`, `\long`.
- Baseline tests (`math::baseline`): the expander reads all 449 inputs KaTeX parses; 448 end
  cleanly and the other one stops at `\bra@set`, so that count falls when it lands. For the 22 inputs KaTeX rejects with an expansion-level error (`Extra }`,
  a `\def` or `\newcommand` error, too many expansions, and the like) the expander gives the
  same message. KaTeX's "Unexpected end of input in a macro argument" for a function's missing
  argument (`\frac 1`) is raised by the parser's `scan_argument` and is checked in that stage.

More deviations (LMATH-T9):

| Where | KaTeX | Here | Why |
|---|---|---|---|
| Expansion limit message | names its `maxExpand` setting | "Too many expansions: infinite loop or more than 10000 macro calls" | no setting; the bound is fixed (LMATH-6) |
| `\newcommand` parameters | any count | at most 9 | TeX's limit, and a bound on argument reading |
| Output bound | none | 1 MiB of produced tokens | LMATH-6 |
| Delimiter length | none | at most 256 tokens per `\def` parameter | reading a delimited argument compares at every token; LMATH-6 |
| Delimiters after a partial match | the token that broke the match is dropped, so `\def\f#1ab{..}\f aab` never matches | restarted from that token, as TeX does | TeX's rule: the shortest run ending in the delimiters |
| A braced group argument | the outer braces of `{a}{b}` are removed, leaving `a}{b` | removed only when one group is the whole argument | TeX's rule; KaTeX's check looks only at the first and last token |
| `\def` and `\newcommand` | `\def` is a parser function; `\newcommand` a macro | both run in the expander | one place for definitions and their scope |
| Macro lookup | strings keyed by text | `Key` enum, borrowed | no allocation per lookup |
| `\arraystretch` | the macro is expanded to text | the macro's body text is read without expanding it | a table's stretch is a plain number in practice |
| `\def` as a script argument | "Got function '\def' with no arguments as superscript" | the script reads what follows the definition (`2^\def\foo{1}2` is `2^2`) | definitions are macros (the row above) |
| Parse nodes | JS objects with a `type` string | `Node { kind: NodeKind, mode, span }`, one enum | LMATH-13 |
| Bare size text | the longest run of tokens that fits a size, without limit | at most 64 bytes (`MAX_SIZE_TEXT`) | each token is tested against the text so far; LMATH-6 |
| Nesting depth | none | braces and functions together at most `MAX_DEPTH` (64) | the parser recurses; LMATH-6 |
| `\@char` of a lone surrogate (`\@char{55296}`) | a string with that code unit | "\@char with invalid code point 55296" | a Rust `char` is a scalar value |
| `\href`, `\url`, `\includegraphics`, `\htmlClass`, `\htmlId`, `\htmlStyle`, `\htmlData` | run if the `trust` setting allows | always as KaTeX does with `trust` off: the command's name in `#cc0000` | no `trust` setting; a link or an attribute from formula text is not wanted (LPA-2) |

Parser (`ast.rs`, `parser.rs`, `functions.rs`, `unicode_data.rs`), the first of its stages:

- `parse(src)` reads a formula in math mode to `Vec<Node>`: `Symbol { group, name }` (the table's
  atom group, so `mathord`, `textord`, `atom` with a family, `spacing`), `Group { body,
  semisimple }` (`{...}` and `\begingroup ... \endgroup`), and `SupSub { base, sup, sub }`
  (`^`, `_`, and `'` primes as a group of `\prime`). Every node has its mode and byte span.
  The parser holds one expanded token ahead, as KaTeX's does, and closes a group's definitions
  before it reads the token after the closing brace. A non-ASCII character the table does not
  list is a `textord` in text mode, as in KaTeX.
- `functions_data.rs` is KaTeX's registry of 312 function names (name, node type, argument
  counts, where allowed), generated by `scripts/katex-functions.mjs`. A command on that list
  that the parser has not implemented is an error ending "is not supported yet"; `is_defined`
  in the expander reads the list, so `\newcommand` cannot take a function's name.
- `unicode_data.rs` lists the 344 accented letters KaTeX splits into base and accent and the 102
  subscript and superscript characters it reads as scripts; both fail as not supported yet until
  accents and the script rewrite land.
- Errors use KaTeX's messages: `Expected 'EOF', got '}'`, `Expected '}', got 'EOF'`, `Double
  superscript`, `Expected group after '^'`, `Undefined control sequence: \foo`, `Limit controls
  must follow a math operator`.
- Functions (`parser/handlers.rs`): `parse_function` reads a command's arguments as the table's
  argument kinds say (a group in the current mode, a group forced to math or text mode, or one
  primitive group or symbol), through the expander's `scan_argument`, which ends each argument
  with an end marker so it parses as a formula of its own. A command is built only if
  `Build::of` names it, so a command not built yet fails before its arguments are read. Built:
  `\frac`, `\dfrac`, `\tfrac`, `\cfrac`, `\binom`, `\dbinom`, `\tbinom` (`Frac`, wrapped in
  `Styling` when the command forces a style), `\genfrac` (delimiters, bar size, and a style
  digit 0 to 3), the infix commands `\over`, `\atop`, `\choose`, `\brace`, `\brack`, `\above`
  (an `Infix` node, which the expression holding it splits into a `Frac` of what is before and
  after it; a second one in the same group is "only one infix operator per group"),
  `\sqrt` with its index (`Sqrt`), the four style
  commands (`Styling`, which takes the rest of the group), and `\text` with its nine font
  variants (`Text`, read in text mode, where spaces are symbols and `--`, `---`, ```` `` ````,
  and `''` join into one). A function after `^` or `_` must be allowed there, and a function not
  allowed in the current mode fails with KaTeX's message. Also built: the math fonts
  (`\mathbf`, `\mathbb`, aliases resolved, and the old `\bf` family, which set the rest of the
  group) as `Font`; `\mathbin` and the other class commands, `\@binrel`, and `\boldsymbol` as
  `MClass` (a class and whether the body is one character); and `\overline` and `\underline`.
  Delimiters: `\big` through `\Biggm` (`DelimSizing`: size 1 to 4 and a class), `\left ...
  \right` (`LeftRight`, which reads its own `\right` and fails with "Expected '\right', got
  ..."), and `\middle`. A delimiter must be on KaTeX's list of 56. Function nesting is bounded by
  `MAX_DEPTH` as group nesting is (LMATH-6), since `\left(\left(...` and `\displaystyle\displaystyle...`
  recurse without a brace.
  Accents: `\hat` and the 21 other accents over a base (`Accent`, stretchy or not as KaTeX's
  list says), the text accents (`\'`, `\"`, `\c`, ...; the node is text even in math), and the
  accents under a base (`AccentUnder`). A combining mark in the source, or a precomposed letter
  the symbol table lacks (`é`, split by `DECOMPOSED`, 344 letters), becomes the same
  `Accent` node around the base letter, with `\i` and `\j` undotted; a mark with no command is
  "Unknown accent". `unicode_data.rs` holds the decompositions and the 12 accent commands.
  Operators: the large operators (`\sum`, `\int`, ... and their Unicode characters, which read
  as the command), the named ones (`\sin`, `\lim`), and `\mathop` build `Op`; `\operatorname`
  (a macro that reaches `\operatorname@` or `\operatornamewithlimits`) builds `OperatorName`.
  `\limits` and `\nolimits` set the limit choice of an `Op` (and of the starred operator name)
  and fail anywhere else with "Limit controls must follow a math operator".
  Sizes and colors (`parser/arguments.rs`): a size argument is read as text and checked as KaTeX
  does (a number and one of 15 units; braced or bare, as in `\kern1em`), a color as a name or a
  hex code (a bare six-digit code gets its `#`). Built on them: `\kern`, `\mkern`, `\hskip`,
  `\mskip` (`Kern`), `\rule`, `\textcolor` and `\color` (`Color`), `\phantom`, `\vphantom`,
  `\smash` with its `[tb]` (`Phantom`, `VPhantom`, `Smash`), `\mathllap`, `\mathrlap`,
  `\mathclap` (`Lap`), `\raisebox` (`RaiseBox`, with a text-style box argument), the eleven
  size commands (`Sizing`, which take the rest of the group), and `\relax` (`Internal`, kept out
  of the tree).
  Boxes and lines: `\fbox`, `\colorbox`, `\fcolorbox`, `\cancel`, `\bcancel`, `\xcancel`,
  `\sout`, `\angl`, `\phase` (`Enclose`, with the label and the colors; `\fbox` and the other
  box commands read the body as text), `\pmb` (`Pmb`), `\hbox`, `\vcenter`, `\mathchoice`
  (the four choices), and `\reflectbox` with `\mathreflectbox`. The stretchy arrows
  (`\xrightarrow[below]{above}` and 21 others; `XArrow`) and the braces `\overbrace`,
  `\underbrace`, `\overbracket`, `\underbracket` (`HorizBrace`) are built too, and
  `\overset`, `\underset`, `\stackrel` (an `MClass` around an `Op` with scripts).
  Text and math: `$ ... $` and `\(` ... `\)` inside text mode (`Styling`, text style, font
  reset); `\)` and `\]` alone are "Mismatched"; `\@char{65}` (`CharCode`, read as
  JavaScript's `parseInt` does); `\html@mathml` (`HtmlMathml`). The link and HTML commands read
  and check their arguments (a `url` or `raw` string, with `%` a plain character in it) and then
  give the command's name in the error color, as KaTeX does when the formula is not trusted. `\sout` in math mode is a
  KaTeX strict-mode warning, which the parser does not report.
  Stack: braces and functions share one nesting budget of `MAX_DEPTH` (the parser reads both by
  recursion), and the functions that read the rest of the group are built in a function of their
  own so the frames of that recursion stay small. A test runs the deepest allowed mixes on a
  2 MiB thread.
  Environments (`parser/environments.rs`): `\begin{name} ... \end{name}` reads the name as text,
  the environment's arguments, and its body, then reads its own `\end` and checks the names
  ("Mismatch: \begin{x} matched by \end{y}"). Tables are `Array` nodes (`parse_array`): cells
  split by `&`, rows by `\\` (with an optional `[size]`) or the `\cr` macro the table defines,
  `\hline` and `\hdashline` before rows, a group per cell, and a style per environment.
  Built: `array`, `darray`, the six matrices and their starred forms, `smallmatrix`,
  `subarray`, `cases` and its three variants, `align`, `aligned`, `alignat`, `alignedat`,
  `split`, `gather`, `gathered`, and `equation`. The aligned ones prepend the empty group that
  makes a leading operator binary, count column pairs as KaTeX does (an odd cell is half a
  pair), and need display mode when they are not `...ed`. Row tags (`\tag`, `\nonumber`, and
  automatic numbering) are `RowTag`s; `\tag` records its text in a global `\df@tag`, which a row
  or the whole formula reads (a formula outside a table becomes a `Tag` node; `\tag` outside
  display mode is an error). `\\` outside a table is a `Cr` node, `\hline` outside one an error,
  and `\color` is remembered for the `\right` after it (`right_color`). `{CD}` is not built.
  `parse_with` takes the display-mode setting. Functions that read the rest of a table cell
  (`\displaystyle`, `\color`, the size commands) stop at the cell's `\\` as KaTeX's do.
- Baseline (`math::baseline::the_parser_agrees_with_katex`): of the 615 inputs that need no
  macro settings, 605 give exactly KaTeX's tree or KaTeX's error, 10 stop at a function not
  built yet (`{CD}`, `\global`, `\long`, `\edef`, `\xdef`, `\bra@set`), and one is a listed
  deviation. No input differs. The first number rises and the second falls as functions land.
  On a difference the test prints the path of the first differing field.
- `\verb|x|` (`Verb`, always text mode) is read from the lexer's `Verb` token.

Layout state (`style.rs`, `metrics.rs`, `options.rs`), the first of the layout stage:

- `Style`: the eight styles (display, text, script, scriptscript, each plain or cramped) with
  `sup`, `sub`, `frac_num`, `frac_den`, `cramp`, `text`, `size`, and `is_tight`, matching
  KaTeX's lookup tables (a test compares all six moves).
- `metrics::for_size`: the font-wide parameters (sigma 1 to 22 of cmsy10, 7, and 5; xi 8 to 13
  of cmex10; KaTeX's rule, box, and array constants; a mu) for a size index, as one
  `FontMetrics` struct per column.
- `metrics::glyph(char, Face, Mode)`: a glyph's depth, height, italic correction, skew, and
  width, from KaTeX's `fontMetricsData.js` (18 faces, 2,038 glyphs), generated into
  `metrics_data.rs` by `scripts/katex-metrics.mjs` with the `Face` enum. As KaTeX's
  `getCharacterMetrics`: a Latin-1 or Cyrillic letter without metrics borrows a similar letter's
  (70 entries), and in text mode a character of a supported script (Latin Extended, Cyrillic,
  Armenian, Brahmic, Georgian, CJK, Hangul) borrows `M`'s.
- `Options`: a `Copy` value of style, size index (1 to 11), text size, color, phantom, math
  font, and text family, weight, and shape, with `having_style`, `having_cramped_style`,
  `having_size`, `having_base_style`, `having_base_sizing`, and the `with_*` builders, as
  KaTeX's `Options.ts`. `Em` is a newtype for lengths. `Options::length` converts a
  `Measurement` to em as KaTeX's `calculateSize` does: absolute units unscaled by size, `em` and
  `ex` from the text style, `mu` from the current size.

More deviations (LMATH-T9):

| Where | KaTeX | Here | Why |
|---|---|---|---|
| Length cap | `maxSize` setting, no cap by default, from above only | within 1000 em either way (`MAX_LENGTH_EM`) | a kern or rule sets the drawing's size; LMATH-6 |
| Font fields | strings, `""` for none | enums in an `Option` | match on a kind, not text |

Box tree (`boxes.rs`, LMATH-T3):

- `Bx { width, height, depth, kind }` in em of the font the box was built in, as KaTeX's spans.
  Kinds: `Glyph` (text, `Face`, italic correction, skew, color), `Rule`, `Space`, `HList`,
  `VList` (children each with a raise and a horizontal offset), and `Scaled` (a child built at
  another size, sized in the outer em, where KaTeX rescales at a size change).
- Builders: `glyph` (KaTeX's `makeSymbol`: metrics of the first character, no italic correction
  in text mode or `\mathit`, no size without metrics), `rule`, `line` (at least
  `min_rule_thickness`), `kern`, `space`, `hlist` (height and depth never below zero),
  `scaled`, `vlist` with KaTeX's `top`, `bottom`, `shift`, and `firstBaseline` positions, and
  `vlist_shifted` (`individualShift`). A stack's height and depth count its kerns, as
  `makeVList`'s do.
- `items` flattens a tree into glyphs and rules at absolute positions in the root's em (y down),
  the display list of LMATH-4.
- A glyph's width includes a positive italic correction, which KaTeX draws as a right margin.
- `Path` is an SVG path over the whole box (KaTeX's `SvgSpan`), with a view box and a fit:
  `Crop` scales to the box's height and crops to its width at the left, middle, or right
  (`preserveAspectRatio` `x{Min,Mid,Max}YMin slice`, how KaTeX stretches its 400,000-unit-wide
  paths), `Stretch` fills the box both ways (`none`). `Mirrored` draws its child left to right
  reversed (`\reflectbox`), and `Shadowed` draws an offset copy (0.02em right, 0.01em down)
  behind its child at the child's size (`\pmb`'s text shadow). `items` yields `Path` items with
  their rectangle, which outputs clip to, and a `mirrored` flag on glyphs and paths.

Layout (`layout/`, KaTeX's `buildHTML.ts` and the HTML builders of `functions/`):

- `layout/mod.rs` holds the list rules below and the dispatch from each `NodeKind` to its
  module: `symbol`, `scripts`, `frac`, `delimiter`, `sqrt`, `op`, `accent`, `arrow`,
  `horiz_brace`, `stretchy`, `enclose`, `array`, and `misc`. A base that places its own scripts
  is laid out by its module (`op::supsub`, `accent::supsub`, `horiz_brace::supsub`, as KaTeX's
  `htmlBuilderDelegate`), and a fraction's delimiters come from `delimiter::frac_delimiter`. The
  argument nodes KaTeX has no builder for (`infix`, `size`, `color-token`, `environment`,
  `raw`, `url`) fail with KaTeX's "Got group of unknown type".

- `layout(nodes, display)` builds the tree in display or text style at `\normalsize`. Each node
  builds to atoms: a box with the class TeX spaces it by (ord, op, bin, rel, open, close, punct,
  inner), or none.
- A list of nodes turns a binary operator ordinary where it has no operand on one side (TeX's
  rules 5 and 6), then puts the spaces of TeX's class table between neighbors (3, 4, or 5 mu;
  only the thin ones around operators in script styles). Explicit spaces are passed over. A
  `\color`, a size or style command, and a `\begingroup` group join the list around them, as
  KaTeX's fragments do; a list of one size or style command takes its spaces in that size or
  style.
- Built so far: symbols (`makeOrd` font choice for letters and digits, `mathsym` for the
  rest, spaces), `\char`, groups, scripts (rules 18a to 18f), fractions without delimiters
  (rules 15a to 15e), style, size, color, kerns, math fonts, `\text` and its font variants,
  `\mathbin` and the class commands, `\rule`, large and named operators with their limits
  (`op`, `\operatorname`, KaTeX's `assembleSupSub`), arrays and the aligned environments
  (`arraystretch`, `\jot`, column separation, `\hline`, `\hdashline`, solid and dashed
  vertical rules, row gaps, row tags), `\\` outside a table, `\tag`, phantoms, `\smash`, the
  laps, `\raisebox`, `\vcenter`, `\underline`, `\overline`, `\pmb`, `\reflectbox`,
  `\mathchoice`, `\html@mathml`, `\hbox`, and `\verb`. Every other node fails with "... is not
  laid out yet".
- Deviations from KaTeX's CSS: an equation tag follows the formula on its right (KaTeX pins it
  to the page edge; `leqno` is ignored), and an automatic number is an empty box of the row's
  size. A lone operator symbol is not moved onto the axis, as KaTeX's `position: relative`
  does without changing the box; the offset is a few thousandths of an em.
- `svg_data.rs` (generated by `scripts/katex-svg.mjs`) holds KaTeX's 59 fixed stretchy paths,
  and `svg_paths.rs` the paths built from parameters (radicals, tall delimiters, `\phase`),
  both tested against KaTeX's strings in `tests/fixtures/math/katex-svg.json`.
- `math::baseline` holds layout to KaTeX's `katex-html` height and depth: 286 formulas agree to
  1e-4 em, and the rest of the 445 KaTeX builds stop at a node not laid out yet.

Not yet built, in order (LMATH-9): `\bra@ket`, `\bra@set`, `{CD}`, and layout of the remaining
nodes (accents, `\left`/`\right` and delimiters, roots, stretchy arrows and braces,
enclosures, `\oiint`). The fuzz target for the lexer waits for a
public entry point, since `fuzz/` reaches only public items; until then the property test
covers it.

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
  tokens, parse nodes, options, and boxes (LMATH-13) before any logic, and review them. Done:
  errors, limits, tokens, the lexer, the expander, the parser and its functions, `Style`,
  `Options`, the font parameters, the glyph metrics, the box tree (LMATH-14), and the core of
  layout. Next: layout of the remaining nodes.
- [ ] LMATH-T8 Port the baseline tests as data (LMATH-13). Done so far:
  `scripts/katex-fixtures.mjs` (needs node, KaTeX 0.19.1, and the `yaml` package; setup in its
  header) extracts the `expect`...`` cases of `katex-spec.ts` and the `ss_data.yaml` inputs, runs
  KaTeX over them, and writes `crates/sirene-core/tests/fixtures/math/katex-cases.json` (708
  cases: 537 usable as they are, 157 that pass `Settings` the script records as
  `settings_raw` for a person to convert, and 14 flagged `disagrees` because KaTeX's own answer
  differs from the test's expectation without those settings) and `katex-oracle.json` (617
  distinct inputs: KaTeX's parse tree, with locations dropped, and the `katex-html` box height
  and depth in em for the 449 that build, or the error message for the 168 that do not). Not
  done: the cases written without the tagged-template form (the `expect(expression)` loops and
  the `getParsed` and `parseTree` helpers), `errors-spec.ts`, `unicode-spec.ts`,
  `symgroups.ts`, and box widths (KaTeX's DOM nodes carry none for spans). The Rust reader is
  `math::baseline` (LMATH-14); it checks the lexer so far, and the parse and layout checks join
  it as those stages land.
- [ ] LMATH-T9 Keep the deviations table (LMATH-13) as cases are ported; started in LMATH-14.
- [ ] LMATH-T7 Dropped: the RaTeX spike. RaTeX stays a comparison target for the layout tests
  (LPA-3), not a dependency.

## LMATH-10 Open questions

- Which math font; a subset keeps the wasm bundle small (the docs site loads `sirene-wasm`)
  but a subset must cover every symbol the command table can produce.
- Whether the parser is the first consumer of a shared TeX-ish lexer or stays standalone.
- Mermaid's `$$` syntax and its fallbacks need checking against the Mermaid reference.
