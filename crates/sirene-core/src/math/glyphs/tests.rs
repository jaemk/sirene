//! The character sets of the text renderer (LMATH-4): what each set draws for the constructs
//! that need a glyph, and that no set draws a character it does not have.

use proptest::prelude::*;
use unicode_width::UnicodeWidthChar;

use super::{ALLOWED, fold, is_allowed, is_ascii_cell, text};
use crate::ascii::Charset;
use crate::math::{MathStyle, ascii_asciimath_rows, ascii_formula, ascii_formula_rows};

const SETS: [Charset; 3] = [Charset::Unicode, Charset::Ascii, Charset::UnicodeMath];

/// The formulas to draw: KaTeX's test inputs and the oracle's, and the commands below.
pub(in crate::math) fn corpus() -> Vec<String> {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/math/katex-cases.json"
    ))
    .unwrap();
    let oracle: serde_json::Map<String, serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/math/katex-oracle.json"
    ))
    .unwrap();
    let mut out: Vec<String> = cases
        .iter()
        .filter_map(|c| c["input"].as_str().map(str::to_owned))
        .collect();
    for key in oracle.keys() {
        let (input, _display, macros): (String, bool, serde_json::Value) =
            serde_json::from_str(key).unwrap();
        if macros.is_null() {
            out.push(input);
        }
    }
    out.extend(COMMANDS.iter().map(|c| (*c).to_owned()));
    out.sort();
    out.dedup();
    out
}

/// Commands that draw a symbol, one of each kind, so the corpus has them whatever KaTeX tests.
const COMMANDS: &[&str] = &[
    r"\alpha\beta\gamma\delta\epsilon\varepsilon\zeta\eta\theta\vartheta\iota\kappa\lambda\mu\nu",
    r"\xi\pi\varpi\rho\varrho\sigma\varsigma\tau\upsilon\phi\varphi\chi\psi\omega",
    r"\Gamma\Delta\Theta\Lambda\Xi\Pi\Sigma\Upsilon\Phi\Psi\Omega",
    r"a\leq b\geq c\neq d\approx e\equiv f\sim g\simeq h\cong i\propto j\ll k\gg l",
    r"a\in B\notin C\ni D\subset E\supset F\subseteq G\supseteq H\cup I\cap J\setminus K",
    r"a\to b\gets c\leftrightarrow d\Rightarrow e\Leftarrow f\Leftrightarrow g\mapsto h",
    r"a\longrightarrow b\longleftarrow c\Longrightarrow d\iff e\implies f\uparrow\downarrow",
    r"a\times b\div c\pm d\mp e\cdot f\circ g\bullet h\ast i\star j\oplus k\otimes l\odot m",
    r"\forall x\exists y\nexists z\nabla\partial\infty\emptyset\varnothing\neg p\wedge q\vee r",
    r"\ldots\cdots\vdots\ddots\dots\hbar\ell\Re\Im\aleph\angle\perp\parallel\mid",
    r"\sum_{i=1}^n x_i\prod_{k}^{m} a_k\int_0^1 f\,dx\iint\oint\bigcup_i A_i\bigcap B\coprod C",
    r"\lim_{x\to 0}\sin x\cos x\log n\max_i a_i",
    r"\sqrt{x}\sqrt[3]{x+1}\sqrt[n]{\frac{a}{b}}\sqrt{\sqrt{x}}",
    r"\frac{a}{b}\frac{1}{1+\frac{1}{x}}\binom{n}{k}\dfrac{a}{b}",
    r"x^2 y_i z_{ij}^{n+1} f'(x) f''(x) a^{b^c} x_1^2",
    r"\overline{x}\underline{x}\hat{x}\tilde{x}\bar{x}\vec{x}\dot{x}\ddot{x}\acute{x}\grave{x}",
    r"\breve{x}\check{x}\mathring{x}\widehat{xyz}\widetilde{xyz}\overrightarrow{AB}\overleftarrow{AB}",
    r"\overbrace{abc}^{n}\underbrace{abc}_{m}\xrightarrow{f}\xleftarrow[g]{h}\xRightarrow{ab}",
    r"\left(\frac{a}{b}\right)\left[\frac{a}{b}\right]\left\{\frac{a}{b}\right\}\left|\frac{a}{b}\right|",
    r"\left\langle\frac{a}{b}\right\rangle\left\lfloor\frac{a}{b}\right\rfloor\left\lceil\frac{a}{b}\right\rceil",
    r"\left\|\frac{a}{b}\right\|\left(\frac{a}{b}\middle|\frac{c}{d}\right)\left.\frac{a}{b}\right|",
    r"\begin{pmatrix}a&b\\c&d\end{pmatrix}\begin{bmatrix}1&2\\3&4\end{bmatrix}",
    r"\begin{vmatrix}a&b\\c&d\end{vmatrix}\begin{Vmatrix}a&b\\c&d\end{Vmatrix}\begin{Bmatrix}a\\b\end{Bmatrix}",
    r"\begin{cases}x&y\\z&w\end{cases}\begin{array}{c|c}a&b\\\hline c&d\end{array}",
    r"\text{if } x>0 \text{ café}\textbf{bold}\mathrm{d}\mathbb{R}\mathbb{N}\mathcal{L}\mathfrak{g}",
    r"\boxed{x}\fbox{y}\not=\not<\not\in x\notin y\neq\ncong\nleq",
    r"\circledast\boxtimes\lozenge\square\triangle\spadesuit\clubsuit\flat\sharp\natural",
    r"\mathbb{CHNPQRZ}\mathbb{abc}\mathbf{x}\mathit{y}\mathsf{z}\mathtt{w}\mathscr{A}",
    r"\degree\prime\backprime\ell\imath\jmath\wp\Box\Diamond\checkmark\dag\ddag\S\P\pounds",
    r"a\sqsubset b\sqcup c\uplus d\bigvee e\bigwedge f\bigodot g\bigoplus h\bigotimes i",
    r"\char`\a\verb|a b|\textrm{\AA\ae\o}\unicode{x3b1}\!\,\;\:\ \quad\qquad",
    r"x\over y {a \atop b} {n \choose k} {a \brace b}",
];

fn rows(src: &str, charset: Charset) -> Option<Vec<String>> {
    ascii_formula_rows(src, MathStyle::Display, charset)
        .ok()
        .map(|r| r.0)
}

#[test]
fn the_allow_list_is_one_cell_wide() {
    for &(lo, hi) in ALLOWED {
        assert!(lo <= hi, "{lo:?}..{hi:?}");
        for c in lo..=hi {
            assert_eq!(c.width(), Some(1), "U+{:04X}", c as u32);
        }
    }
    // What the sets exist to avoid.
    for c in "\u{221a}\u{af}\u{2212}\u{2211}\u{222b}\u{239b}\u{23ad}\u{2070}\u{2082}\u{1d43}\u{2044}\u{2192}\u{2264}\u{2208}".chars() {
        assert!(!is_allowed(c), "U+{:04X}", c as u32);
    }
    // What they allow.
    for c in
        "a~\u{b1}\u{b7}\u{d7}\u{f7}\u{b0}\u{b2}\u{b3}\u{b9}\u{3b1}\u{3a9}\u{2500}\u{256d}".chars()
    {
        assert!(is_allowed(c), "U+{:04X}", c as u32);
    }
}

#[test]
fn every_character_of_the_forms_table_has_a_form_in_each_set() {
    // `fold` of an allowed character in the `Unicode` set is itself, and of any character in the
    // `Ascii` set is printable ASCII or nothing.
    for c in (0x20_u32..0x2c00).filter_map(char::from_u32) {
        if is_allowed(c) {
            assert_eq!(fold(c, Charset::Unicode).as_deref(), Some(&*c.to_string()));
        }
        if let Some(form) = fold(c, Charset::Ascii) {
            assert!(
                form.chars().all(is_ascii_cell),
                "U+{:04X} {form:?}",
                c as u32
            );
        }
        if let Some(form) = fold(c, Charset::Unicode) {
            assert!(form.chars().all(is_allowed), "U+{:04X} {form:?}", c as u32);
        }
        assert_eq!(
            fold(c, Charset::UnicodeMath).as_deref(),
            Some(&*c.to_string())
        );
    }
    assert_eq!(text("\u{2264}\u{1f600}", Charset::Ascii), "<=?");
    assert_eq!(text("a\u{301}", Charset::Ascii), "a");
    assert_eq!(text("\u{3b1}\u{2211}", Charset::Unicode), "\u{3b1}sum");
}

#[test]
fn the_ascii_set_draws_printable_ascii_only() {
    let mut bad = Vec::new();
    for src in corpus() {
        if let Some(rows) = rows(&src, Charset::Ascii)
            && let Some(c) = rows
                .iter()
                .flat_map(|r| r.chars())
                .find(|c| !is_ascii_cell(*c))
        {
            bad.push(format!("{src:?}: U+{:04X}", c as u32));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_unicode_set_draws_allowed_characters_only() {
    let mut bad = Vec::new();
    for src in corpus() {
        if let Some(rows) = rows(&src, Charset::Unicode)
            // A character written in the source (a letter of `\text`) is the author's own.
            && let Some(c) = rows
                .iter()
                .flat_map(|r| r.chars())
                .find(|c| !is_allowed(*c) && !src.contains(*c))
        {
            bad.push(format!("{src:?}: U+{:04X}", c as u32));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn every_character_is_one_cell_wide_in_the_sets_that_avoid_the_wide_ones() {
    let mut bad = Vec::new();
    for src in corpus() {
        for set in [Charset::Unicode, Charset::Ascii] {
            if let Some(rows) = rows(&src, set)
                && let Some(c) = rows
                    .iter()
                    .flat_map(|r| r.chars())
                    .find(|c| c.width() != Some(1) && !src.contains(*c))
            {
                bad.push(format!("{src:?} {set:?}: U+{:04X}", c as u32));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_unicode_math_set_is_unchanged_by_the_forms() {
    // The set the other two replace: the radical, the overline, scripts, and operators are the
    // symbol table's own.
    let got = rows(r"\sqrt{x}^2 \le \sum_i \alpha", Charset::UnicodeMath).unwrap();
    let all: String = got.concat();
    for c in [
        '\u{221a}', '\u{af}', '\u{b2}', '\u{2264}', '\u{2211}', '\u{3b1}',
    ] {
        assert!(all.contains(c), "{c:?} in {got:?}");
    }
}

/// Source, then the rows in `Unicode`, `Ascii`, and `UnicodeMath`.
type Lines = &'static [&'static str];

const CASES: &[(&str, [Lines; 3])] = &[
    (
        r"\sqrt{x+1}",
        [
            &["  _____", r"\/x + 1"],
            &["  _____", r"\/x + 1"],
            &[" ¯¯¯¯¯", "√x + 1"],
        ],
    ),
    (
        r"\sqrt[3]{x}",
        [&["   _", r"3\/x"], &["   _", r"3\/x"], &["  ¯", "³√x"]],
    ),
    (
        r"\sqrt[n+1]{x}",
        [
            &["         _", r"(n + 1)\/x"],
            &["         _", r"(n + 1)\/x"],
            &["        ¯", "(n + 1)√x"],
        ],
    ),
    (
        r"\frac{a+b}{c}",
        [
            &[" a + b", "-------", "   c"],
            &[" a + b", "-------", "   c"],
            &[" a + b", "-------", "   c"],
        ],
    ),
    (
        r"\sum_{i=1}^{n} x_i^2",
        [
            &["  n", " sum  x_i^2", "i = 1"],
            &["  n", " sum  x_i^2", "i = 1"],
            &["  n", "  ∑   xᵢ²", "i = 1"],
        ],
    ),
    (
        r"\int_0^1 f(x)\,dx",
        [&["int_0^1 f(x) dx"], &["int_0^1 f(x) dx"], &["∫₀¹ f(x) dx"]],
    ),
    (
        r"\int_0^n i\,di",
        [&["int_0^n i di"], &["int_0^n i di"], &["∫₀ⁿ i di"]],
    ),
    (
        r"x^n + y^n i",
        [&["x^n + y^n i"], &["x^n + y^n i"], &["xⁿ + yⁿi"]],
    ),
    (
        r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
        [
            &["╭a  b╮", "╰c  d╯"],
            &[r"/a  b\", r"\c  d/"],
            &["⎛a  b⎞", "⎝c  d⎠"],
        ],
    ),
    (
        r"\left(\frac{a}{b}\right)",
        [
            &["╭ a ╮", "│---│", "╰ b ╯"],
            &[r"/ a \", "|---|", r"\ b /"],
            &["⎛ a ⎞", "⎜---⎟", "⎝ b ⎠"],
        ],
    ),
    (
        r"\left[\frac{a}{b}\right]",
        [
            &["┌ a ┐", "│---│", "└ b ┘"],
            &["[ a ]", "[---]", "[ b ]"],
            &["⎡ a ⎤", "⎢---⎥", "⎣ b ⎦"],
        ],
    ),
    (
        r"\hat{x}+\bar{y}+\vec{v}",
        [
            &["^   _   >", "x + y + v"],
            &["^   _   >", "x + y + v"],
            &["^   ¯   →", "x + y + v"],
        ],
    ),
    (
        r"A\xrightarrow{f}B",
        [
            &["   f", "A ──> B"],
            &["   f", "A --> B"],
            &["   f", "A ──→ B"],
        ],
    ),
    (
        r"\text{if } x \geq 0",
        [&["if x >= 0"], &["if x >= 0"], &["if x ≥ 0"]],
    ),
    (
        r"\alpha+\beta=\pi r^2",
        [
            &["α + β = πr^2"],
            &["alpha + beta = pi r^2"],
            &["α + β = πr²"],
        ],
    ),
    (
        r"a\leq b\neq c\to d\Rightarrow e",
        [
            &["a <= b != c -> d => e"],
            &["a <= b != c -> d => e"],
            &["a ≤ b ≠ c → d ⇒ e"],
        ],
    ),
    (
        r"\pm 1 \times 2 \div 3 \cdot 4",
        [
            &["±1 × 2 ÷ 3 · 4"],
            &["+/-1 x 2 / 3 * 4"],
            &["±1 × 2 ÷ 3 ⋅ 4"],
        ],
    ),
    (
        r"\overline{xy}+\underline{z}",
        [
            &["__", "xy + z", "     _"],
            &["__", "xy + z", "     _"],
            &["¯¯", "xy + z", "     _"],
        ],
    ),
    (
        r"x^{n+1}_{i,j}",
        [&["x_(i,j)^(n+1)"], &["x_(i,j)^(n+1)"], &["x_(i,j)ⁿ⁺¹"]],
    ),
    (
        r"f'(x)=\infty",
        [&["f'(x) = inf"], &["f'(x) = inf"], &["f′(x) = ∞"]],
    ),
];

#[test]
fn exact_rows_in_every_set() {
    let mut bad = String::new();
    for (src, expected) in CASES {
        for (set, want) in SETS.into_iter().zip(expected) {
            let got = ascii_formula(src, MathStyle::Display, set).unwrap();
            let got: Vec<&str> = got.lines().map(str::trim_end).collect();
            if got != *want {
                bad += &format!("{src}\n{set:?}\n{got:#?}\n\n");
            }
        }
    }
    assert!(bad.is_empty(), "{bad}");
}

fn piece() -> impl Strategy<Value = String> {
    let commands = COMMANDS.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
    prop_oneof![
        proptest::sample::select(commands),
        proptest::sample::select(vec![
            "\\frac",
            "\\sqrt",
            "\\left(",
            "\\right)",
            "\\left[",
            "\\right]",
            "\\left\\{",
            "\\right\\}",
            "\\left\\langle",
            "\\right\\rangle",
            "\\left|",
            "\\right|",
            "\\sum",
            "\\int",
            "\\lim",
            "\\not",
            "\\overline",
            "\\underline",
            "\\hat",
            "\\vec",
            "\\text",
            "\\mathbb",
            "\\xrightarrow",
            "\\overbrace",
            "\\underbrace",
            "\\boxed",
            "\\begin{pmatrix}",
            "\\end{pmatrix}",
            "\\begin{cases}",
            "\\end{cases}",
            "\\\\",
            "&",
            "{",
            "}",
            "^",
            "_",
            "'",
            "\\alpha",
            "\\leq",
            "\\infty",
            "\\cdot",
            "\\ldots",
            "\\pm",
        ])
        .prop_map(String::from),
        "[a-z0-9 +=]{0,4}".prop_map(String::from),
        any::<String>().prop_map(|s| s.chars().take(6).collect()),
        proptest::sample::select(vec![
            "\u{301}",
            "\u{300}\u{302}",
            "\u{2211}",
            "\u{221a}",
            "\u{3b1}",
            "\u{1d49c}",
            "\u{65e5}\u{672c}",
            "\u{1f600}",
            "\u{0}",
            "\u{200d}",
            "\u{ff21}",
            "\u{e9}",
        ])
        .prop_map(String::from),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn no_set_panics_and_ascii_stays_ascii(
        parts in proptest::collection::vec(piece(), 0..40),
    ) {
        let src: String = parts.concat();
        for style in [MathStyle::Display, MathStyle::Text] {
            for set in SETS {
                let got = ascii_formula_rows(&src, style, set);
                let asciimath = ascii_asciimath_rows(&src, style, set);
                if set == Charset::Ascii {
                    for out in [got, asciimath].into_iter().flatten() {
                        for row in &out.0 {
                            prop_assert!(row.chars().all(is_ascii_cell), "{src:?}: {row:?}");
                        }
                    }
                }
            }
        }
    }
}
