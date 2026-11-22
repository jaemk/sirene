# LaTeX Prior Art and Tests

What existing open source LaTeX math implementations teach about syntax coverage, directives,
layout rules, and test cases, and how the typesetter of `latex-math.md` uses them. The rule
for any of it: borrow the algorithm and the test ideas, not the code, and give every
test input or data table taken from a project its license's attribution (THIRD-PARTY-NOTICES).
Status: research; the open items are in LPA-T.

## LPA-1 Libraries to survey

Each entry records: language, license, input syntax covered, output (MathML, HTML, boxes,
glyphs), how the project is tested, and what to take (LPA-T1).

- KaTeX (JavaScript): the reference for Mermaid; its supported-functions list is the syntax
  target (LMATH-5). Its test suite and screenshot data are a source of inputs.
- MathJax (JavaScript): TeX input with an extension model; its parse-error behavior.
- Temml (JavaScript): TeX to MathML; its macro set and its tests.
- RaTeX (Rust): a KaTeX-compatible parser and layout; claims broad KaTeX syntax coverage.
- formulary, formel, noad (Rust): MathML layout from the OpenType MATH table.
- ReX (Rust): a TeX-style math renderer from the OpenType MATH table.
- pulldown-latex, latex2mathml (Rust): LaTeX to MathML parsers.
- Typst math (Rust) and MiTeX: Typst's math layout and a LaTeX-to-Typst converter.
- JLaTeXMath (Java): the renderer PlantUML itself uses for `@startlatex`.

## LPA-2 Language and directive support

Output of the survey (LPA-T2): a table of commands and environments by family (atoms and
symbols, fractions, roots, scripts, delimiters, operators, accents, fonts and styles, spacing,
color, environments, text, macros), each marked as in the first subset, later, or out of scope
(LMATH-1), with which libraries support it. The same table lists the directives and
macro commands (`\newcommand`, `\def`, `\operatorname`, `\color`, `\displaystyle`,
`\textstyle`, `\limits`, `\nolimits`, `\tag`, `\label`, `\hspace`, `\\`) and how each library
treats an unsupported one: error, warning, or silent.

## LPA-3 Test cases

Output of the survey (LPA-T3): the fixture plan for `crates/sirene-core/tests/fixtures/math/`:

- parse tests: accepted and rejected inputs, with diagnostics and spans;
- layout tests: box positions for a small set, as snapshots, checked against TeX's
  documented rules (TeXbook Appendix G) rather than against another library's pixels;
- command coverage: one input per supported command;
- error recovery: unbalanced braces, an unknown command, a stray `&` or `\\`, a missing
  `\right`;
- limit tests (LMATH-6);
- compatibility: KaTeX's own examples, with the expected structure, where its license allows;
- fuzz seeds: the above, for the parser and expander targets.

## LPA-T Tasks

- [ ] LPA-T1 Survey each library in LPA-1: license, syntax, output, tests, what to take.
- [ ] LPA-T2 Build the command and directive table of LPA-2.
- [ ] LPA-T3 Harvest test inputs per LPA-3, recording each source and license.
- [ ] LPA-T4 Write the layout rules reference from TeXbook Appendix G and the OpenType MATH
  specification, with the table mapping TeX's parameters to MATH constants.
- [ ] LPA-T5 Decide whether to depend on, vendor, or only learn from each Rust crate.
