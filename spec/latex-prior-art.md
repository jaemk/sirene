# LaTeX Prior Art and Tests

What existing open source LaTeX math implementations teach about syntax coverage, directives,
layout rules, and test cases, and how the typesetter of `latex-math.md` uses them. The rule
for any of it: borrow the algorithm and the test ideas, not the code, and give every
test input or data table taken from a project its license's attribution (THIRD-PARTY-NOTICES).
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
- [ ] LPA-T5 Decide whether to depend on, vendor, or only learn from each Rust crate:
  maintenance (formulary has 35 commits and 3 stars, formel 224 commits and 1 star, as of
  the survey), license, dependency weight, and fit to the display list.
