# LaTeX Prior Art and Tests

What existing open source LaTeX math implementations teach about syntax coverage, directives,
layout rules, and test cases, and how the typesetter of `latex-math.md` uses them. The rule
for any of it: code, data, and tests may be ported from MIT and Apache-2.0 projects with a
file header and a THIRD-PARTY-NOTICES entry giving the source and license (the attribution
rule); a project under another license is read for ideas only until its license is checked.
Status: research; the open items are in LPA-T. Facts below come from each project's README or
site on 2026-11-22 and are marked unverified where a source was not read.

## LPA-1 Libraries surveyed

| Project | Lang | License | Input | Output | Tests | Take |
|---|---|---|---|---|---|---|
| KaTeX | JS | MIT (Khan Academy) | LaTeX | HTML and MathML | `test/`: `katex-spec.ts`, `errors-spec.ts`, `unicode-spec.ts`, `mathml-spec.ts`, `symgroups.ts`, and `screenshotter/ss_data.yaml` (about 190 named inputs) | the syntax target (LMATH-5), the command families, the fixture format |
| Temml | JS | MIT | LaTeX | MathML Core | rendered examples; inputs drawn from Mozilla, Wikipedia, mhchem, and LaTeXML | KaTeX's parser and macro expander with the HTML half removed; extra commands (`\prescript`, `\label`, `\ref`, `\definecolor`) |
| MathJax | TS | Apache-2.0 | TeX, MathML, AsciiMath | HTML, SVG | `testsuite/` (format unread) | TeX input extensions model; error behavior (unverified) |
| RaTeX | Rust | MIT | LaTeX (KaTeX syntax) | display list, SVG, PNG, PDF | `cargo test --all`, `tests/golden`, a per-formula support table against KaTeX | crate split (lexer, parser, layout, font); the display-list shape; claims over 99.5% of KaTeX syntax |
| formulary | Rust | MIT or Apache-2.0; test font STIX Two Math under OFL | presentation MathML | display list of glyph IDs and rules, optional SVG | unit and property tests, SVG goldens, web-platform-tests `mathml/` (2,762 fragments, 98% render) | the MATH-table layout feature list, the WPT harness idea |
| formel | Rust | MIT | MathML (demo takes LaTeX) | glyph positions, SVG | `tests/` (unread) | MATH-table layout with rustybuzz, as sirene shapes text; a wasm bundle of about 1 MB |

Not surveyed yet (LPA-T1): noad, ReX, pulldown-latex, latex2mathml, Typst's math layout, MiTeX,
and JLaTeXMath (the renderer PlantUML uses).

## LPA-5 RaTeX evaluation

Read from its repository and GitHub API on 2026-11-22; no code built or run yet (the spike is
LMATH-T6).

- Project: created 2026-03-10, 1,599 stars, 88 forks, 15 open issues, last push 2026-11-22,
  releases roughly monthly (0.1.14 on 2026-07-28, 0.1.15 on 2026-10-16, 0.1.16 on 2026-11-16),
  so fast-moving and pre-1.0. Rust is 1.74 MB of source across 15 crates, plus platform
  bindings (iOS, Android, Flutter, web).
- License: MIT (copyright erweixin) at the root. The bundled KaTeX fonts are not under it; they
  are OFL 1.1, with `licenses/KaTeX-fonts-NOTICE.txt` and the OFL text beside them. GitHub's
  license detector reports no assertion because of that second paragraph.
- Crates a consumer would use: `ratex-types` (display list: `DisplayItem`, `MathStyle`,
  `PathCommand`, `Color`), `ratex-font` (KaTeX metrics `metrics_data.rs` 98 KB and symbol
  tables `symbols_data.rs` 90 KB, as Rust data), `ratex-lexer`, `ratex-parser`, `ratex-layout`
  (`engine.rs` 262 KB, `katex_svg.rs` 69 KB for the stretchy shapes, `stacked_delim.rs`,
  `spacing.rs`, `resource_limits.rs`), and optionally `ratex-katex-fonts` (the OFL TTFs).
- Dependencies of the parser: the workspace crates, `thiserror`, `serde`, `serde_json`,
  `unicode-normalization`, `regex-lite`; `fancy-regex` and `regex` only for the `mhchem`
  feature, which is on by default and can be turned off. The workspace's own dependencies are
  `phf`, `thiserror`, `serde`, `serde_json`, all pure Rust. No MSRV is declared (edition 2021).
- Limits: the parser has `expansion_budget.rs` and `stack_safety.rs`, the layout has
  `resource_limits.rs`; the shapes LMATH-6 needs are there, with their bounds to compare.
- Errors: `ParseError { message, loc: Option<SourceLocation> }`, modeled on KaTeX's, one error
  and no recovery, as KaTeX. A source range per error is enough for LEMB-5 if `loc` is the
  byte span in the formula text.
- Tests: `crates/ratex-parser/src/tests.rs` (51 KB), `tests/golden`, and a per-formula support
  table against KaTeX.

What a spike must answer: whether `loc` maps to byte ranges, how the display list converts to
sirene's scene items (glyph runs with fonts, rules, and paths), what the wasm and binary size
cost is with the KaTeX fonts subset, whether the crates build on Rust 1.92 and offline with
`mhchem` off, whether fuzzing finds panics or runaway time, and how much of `engine.rs` is
KaTeX-specific logic worth porting versus depending on.

## LPA-2 Language and directive support

KaTeX's supported-functions families (<https://katex.org/docs/supported.html>) as the command
table's skeleton, each marked for sirene: first subset (1), later (2), out of scope (3).

| Family | Commands | Phase |
|---|---|---|
| Letters and symbols | Greek, `\hbar`, `\ell`, `\aleph`, blackboard, script, fraktur styles | 1 |
| Operators | `\sum`, `\prod`, `\int`, `\oint`, `\bigcup`, `\bigcap`, `\sin`, `\log`, `\lim`, `\max`, `\operatorname`, binary operators, `\bmod`, `\pmod` | 1 |
| Fractions, roots | `\frac`, `\dfrac`, `\tfrac`, `\cfrac`, `\binom`, `\genfrac`, `\over`, `\atop`, `\sqrt[n]{}` | 1 |
| Delimiters | `\left`, `\right`, `\middle`, `\big` to `\Bigg`, `\langle`, `\lceil`, `\lfloor`, `\vert`, `\Vert` | 1 |
| Accents | `\hat`, `\bar`, `\vec`, `\dot`, `\overline`, `\underline`, `\widehat`, `\overbrace`, `\underbrace`, `\overrightarrow` | 1 |
| Fonts and styles | `\mathrm`, `\mathbf`, `\mathit`, `\mathbb`, `\mathcal`, `\mathfrak`, `\mathsf`, `\mathtt`, `\boldsymbol`, `\text`, `\displaystyle`, `\textstyle`, `\scriptstyle`, `\limits`, `\nolimits` | 1 |
| Relations, arrows, logic | `\le`, `\ge`, `\approx`, `\equiv`, `\to`, `\Rightarrow`, `\mapsto`, `\forall`, `\exists`, negations, `\not` | 1 |
| Spacing | `\,`, `\;`, `\quad`, `\qquad`, `\!`, `\kern`, `\hspace`, `\phantom`, `\smash` | 1 (`\,` family), 2 (rest) |
| Environments | `matrix`, `pmatrix`, `bmatrix`, `vmatrix`, `cases`, `array`, `aligned`, `gathered`, `align`, `split` | 1 (matrices, `cases`, `aligned`), 2 (rest) |
| Macros | `\newcommand`, `\renewcommand`, `\providecommand`, `\def` | 2 (arguments, no `\catcode`) |
| Macros, low level | `\gdef`, `\edef`, `\xdef`, `\let`, `\futurelet` | 3 |
| Color | `\color`, `\textcolor`, `\colorbox`, `\fcolorbox` | 2 |
| Annotation, layout | `\cancel`, `\boxed`, `\tag`, `\stackrel`, `\overset`, `\underset`, `\raisebox`, `\mathllap`, `\mathrlap`, `\mathclap`, `CD` | 2 |
| Extensible arrows | `\xrightarrow`, `\xleftarrow` | 2 |
| Bra-ket, chemistry | `\bra`, `\ket`, `\braket`, mhchem `\ce`, `\pu` | 3 |
| HTML and links | `\href`, `\url`, `\includegraphics`, `\html*` | 3 (a diagram does not follow links or load images from a formula) |

Directive behavior to record per library (LPA-T2): what an unknown command does. KaTeX throws
a parse error unless `throwOnError` is off, and then draws the source in an error color
(`errorColor`, `noThrow` in its screenshot data); sirene's rule is LMATH-1 and LEMB-4, an error
on the span and the source drawn as text. KaTeX does not support `\par`, ignores `\long`, and
limits `\tag` to certain top-level environments; display math gets no automatic line breaks.

## LPA-3 Test cases

KaTeX's `ss_data.yaml` is the model for the fixture format: a test name keyed to either a TeX
string, or a map with `tex`, `display` (display style), `macros`, `noThrow`, `errorColor`,
`pre`, `post`, `styles`, and `nolatex` (a note that real LaTeX rejects it). Sirene's fixtures
under `crates/sirene-core/tests/fixtures/math/` keep `tex`, `display`, and `macros`, and add
`expect` (the diagnostics) and the snapshot of the box tree. The names KaTeX uses show the
areas to cover: baseline, negative space between relations, tags, unsupported commands,
group macros, accents, arrays, fractions, delimiter sizing, spacing, fonts, limits, extensible
arrows, color, and `\rule`.

Plan:

- parse tests: accepted and rejected inputs, with diagnostics and spans;
- layout tests: box positions for a small set, as snapshots, checked against TeX's documented
  rules (TeXbook Appendix G) and the MathML Core layout sections (LPA-4), not against another
  library's pixels;
- command coverage: one input per supported command;
- error recovery: unbalanced braces, an unknown command, a stray `&` or `\\`, a missing
  `\right`;
- limit tests (LMATH-6);
- compatibility: inputs from KaTeX's data, formulary's WPT-style harness idea (render every
  fragment, assert no panic and sane boxes), and Temml's external sources (Mozilla,
  Wikipedia, LaTeXML) where their licenses allow;
- fuzz seeds: the above, for the parser and expander targets.

Every input taken from a project is listed in THIRD-PARTY-NOTICES with its source and license
(KaTeX: MIT, Khan Academy and other contributors, 2013-2020; Temml: MIT; MathJax: Apache-2.0).

## LPA-4 Layout rules

Two written algorithms exist, and the layout reads the font's OpenType MATH table either way:

- TeXbook Appendix G, which TeX and the TeX-style libraries follow, with its parameters mapped
  to MATH constants.
- MathML Core (<https://www.w3.org/TR/mathml-core/>), which states the OpenType MATH layout
  directly: layout of operators and the algorithm for stretching them (3.2.4.3, 3.3.1.1),
  fractions (3.3.2), radicals (3.3.3), scripts (3.4.1), and under and over scripts (3.4.2),
  with `AxisHeight`, `FractionRuleThickness`, `FractionNumeratorShiftUp`,
  `FractionNumeratorDisplayStyleShiftUp`, `DisplayOperatorMinHeight`, `MathItalicsCorrectionInfo`,
  `MathTopAccentAttachment`, `MathVariants`, and `GlyphAssembly`.

Decision to make (LPA-T4): follow MathML Core's formulation, since it is written against the
MATH table and formulary and formel implement it, and use Appendix G for the TeX behaviors it
does not cover (atom spacing between ordinary, operator, binary, relation, and punctuation
classes, and style changes).

## LPA-T Tasks

- [x] LPA-T1a Survey KaTeX, Temml, MathJax, RaTeX, formulary, formel (LPA-1).
- [ ] LPA-T1b Survey noad, ReX, pulldown-latex, latex2mathml, Typst math, MiTeX, JLaTeXMath.
- [ ] LPA-T1c Read the unread test formats: MathJax `testsuite/`, formel `tests/`, RaTeX
  `tests/golden`, and count each suite.
- [x] LPA-T2a Draft the command table by family (LPA-2) from KaTeX's list.
- [ ] LPA-T2b Complete it with per-library support, from Temml's support table and MathJax's
  extension list, and record each library's unknown-command behavior.
- [ ] LPA-T3 Harvest test inputs per LPA-3 into the fixture format, recording each source and
  license.
- [ ] LPA-T4 Read MathML Core's layout sections and TeXbook Appendix G in full; write the
  rules reference and the TeX-to-MATH parameter table; decide between them (LPA-4).
- [x] LPA-T5a Read RaTeX's repository for maintenance, license, dependencies, limits, and
  errors (LPA-5).
- [ ] LPA-T5b Run the RaTeX spike (LMATH-T6) and decide: depend, vendor with attribution, or
  port selectively; record the decision in LMATH-11.
- [ ] LPA-T5c Same read for formulary and formel if the MATH-table path (LPA-4) is pursued.
