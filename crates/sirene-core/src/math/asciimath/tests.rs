//! Tests of the AsciiMath translator, by group of the grammar. The expected TeX is what
//! ASCIIMathTeXImg.js writes for the input, worked out by hand from its source.

use super::*;

fn tex(src: &str) -> String {
    to_tex(src).unwrap()
}

#[test]
fn constants_are_braced_unless_they_are_commands() {
    assert_eq!(tex("x"), "{x}");
    assert_eq!(tex("2"), "{2}");
    assert_eq!(tex("3.14"), "{3.14}");
    assert_eq!(tex("alpha"), "\\alpha");
    assert_eq!(tex("+"), "+");
    assert_eq!(tex("oo"), "\\infty");
    assert_eq!(tex("xx"), "\\times");
    assert_eq!(tex("epsi"), "\\epsilon");
}

#[test]
fn a_blank_source_is_empty() {
    assert_eq!(tex(""), "");
    assert_eq!(tex("  \n "), "");
}

#[test]
fn a_tex_name_is_an_input_too() {
    assert_eq!(tex("\\alpha"), "\\alpha");
    assert_eq!(tex("epsilon"), "\\epsilon");
    assert_eq!(tex("times"), "\\times");
}

#[test]
fn fractions() {
    assert_eq!(tex("2/3"), "\\frac{{2}}{{3}}");
    assert_eq!(tex("frac a b"), "{\\frac{{a}}{{b}}}");
    assert_eq!(tex("(a+b)/c"), "\\frac{{{a}+{b}}}{{c}}");
}

#[test]
fn roots_and_accents() {
    assert_eq!(tex("sqrt x"), "\\sqrt{{x}}");
    assert_eq!(tex("sqrt(x+1)"), "\\sqrt{{{x}+{1}}}");
    assert_eq!(tex("root 3 x"), "{\\sqrt[{3}]{{x}}}");
    assert_eq!(tex("hat x"), "\\hat{{x}}");
    assert_eq!(tex("bar x"), "\\overline{{x}}");
    assert_eq!(tex("bb x"), "{\\mathbf{{x}}}");
    assert_eq!(tex("cancel x"), "\\cancel{{x}}");
}

#[test]
fn scripts() {
    assert_eq!(tex("x^2"), "{x}^{{2}}");
    assert_eq!(tex("x_1"), "{x}_{{1}}");
    assert_eq!(tex("x_1^2"), "{{x}_{{1}}^{{2}}}");
    assert_eq!(tex("x^(a+b)"), "{x}^{{{a}+{b}}}");
}

#[test]
fn brackets_make_left_right_pairs() {
    assert_eq!(tex("(x)"), "{\\left({x}\\right)}");
    assert_eq!(tex("[x]"), "{\\left[{x}\\right]}");
    assert_eq!(tex("{x}"), "{\\left\\lbrace{x}\\right\\rbrace}");
    assert_eq!(tex("(:x:)"), "{\\left\\langle{x}\\right\\rangle}");
    assert_eq!(tex("{:x:}"), "{\\left.{x}\\right.}");
}

#[test]
fn an_unclosed_bracket_closes_with_a_dot() {
    assert_eq!(tex("(x"), "{\\left({x}\\right.}");
}

#[test]
fn functions_take_the_next_expression() {
    assert_eq!(tex("sin x"), "{\\sin{{x}}}");
    assert_eq!(tex("sin(x)"), "{\\sin{{\\left({x}\\right)}}}");
    assert_eq!(tex("lim_(x->0)"), "\\lim_{{{x}\\to{0}}}");
}

#[test]
fn text_is_a_text_command_with_its_characters_escaped() {
    assert_eq!(tex("text(if x)"), "\\text{if x}");
    assert_eq!(tex("\"a b\""), "\\text{a b}");
    assert_eq!(tex("text(50% of $)"), "\\text{50\\% of \\$}");
    assert_eq!(tex("text(a_b)"), "\\text{a\\_b}");
    assert_eq!(tex("text( a )"), "\\ \\text{ a }\\ ");
}

#[test]
fn binary_symbols_take_two_arguments() {
    assert_eq!(tex("stackrel a b"), "{\\stackrel{{a}}{{b}}}");
    assert_eq!(tex("color(red)x"), "{\\color{red}{x}}");
    assert_eq!(tex("overset a b"), "{\\overset{{a}}{{b}}}");
    // At the end of the source, the missing argument is an empty one, as in the JavaScript.
    assert_eq!(tex("frac a"), "{\\frac{{a}}{}}");
    assert_eq!(tex("frac"), "{\\frac{}{}}");
}

#[test]
fn spacing_commands() {
    assert_eq!(tex("mspace(1em)"), "\\mkern{16mu}");
    assert_eq!(tex("mspace(3mu)"), "\\mkern{3mu}");
    assert_eq!(tex("mspace(2 mu)"), "\\mkern{2mu}");
    assert_eq!(tex("mspace(bad)"), "");
    assert_eq!(tex("quad"), "\\quad");
}

#[test]
fn fonts() {
    assert_eq!(tex("bbb R"), "{\\mathbb{{R}}}");
    assert_eq!(tex("RR"), "\\mathbb{R}");
    assert_eq!(tex("cc A"), "{\\mathcal{{A}}}");
    assert_eq!(tex("bold x"), "{\\mathbf{{x}}}");
}

#[test]
fn relations_and_arrows() {
    assert_eq!(tex("a <= b"), "{a}\\le{b}");
    assert_eq!(tex("a != b"), "{a}\\ne{b}");
    assert_eq!(tex("a -> b"), "{a}\\to{b}");
    assert_eq!(tex("a |-> b"), "{a}\\mapsto{b}");
    assert_eq!(tex("a => b"), "{a}\\Rightarrow{b}");
    assert_eq!(tex("a <=> b"), "{a}\\Leftrightarrow{b}");
    assert_eq!(tex("a -= b"), "{a}\\equiv{b}");
    assert_eq!(tex("a in B"), "{a}\\in{B}");
}

#[test]
fn big_operators() {
    assert_eq!(tex("sum_(i=1)^n i"), "{\\sum_{{{i}={1}}}^{{n}}}{i}");
    assert_eq!(tex("prod"), "\\prod");
    assert_eq!(tex("int"), "\\int");
    assert_eq!(tex("oint"), "\\oint");
}

#[test]
fn text_with_a_closing_bracket_missing_runs_to_the_end() {
    assert_eq!(tex("text(abc"), "\\text{abc}");
    // An unclosed quote has no text; what follows is read as AsciiMath.
    assert_eq!(tex("\"abc"), "\\text{}{a}{b}{c}");
}

#[test]
fn spaces_and_words() {
    assert_eq!(tex("a and b"), "{a}{\\quad\\text{and}\\quad}{b}");
    assert_eq!(tex("a or b"), "{a}{\\quad\\text{or}\\quad}{b}");
}

#[test]
fn definitions_expand() {
    assert_eq!(tex("iff"), "\\Leftrightarrow");
    assert_eq!(tex("lamda"), "\\lambda");
    assert_eq!(tex("notin"), "\\notin");
}

#[test]
fn a_minus_after_an_infix_symbol_is_a_sign() {
    assert_eq!(tex("x^-1"), "{x}^{{-{{1}}}}");
}

#[test]
fn matrices() {
    assert_eq!(
        tex("[[a,b],[c,d]]"),
        "{\\left[\\begin{array}{cc} {a}&{b}\\\\{c}&{d}\\end{array}\\right]}"
    );
    assert_eq!(
        tex("((a,b),(c,d))"),
        "{\\left(\\begin{array}{cc} {a}&{b}\\\\{c}&{d}\\end{array}\\right)}"
    );
}

#[test]
fn a_vertical_rule_in_a_matrix() {
    let t = tex("[[a,|,b],[c,|,d]]");
    assert!(t.contains("\\begin{array}{c|c}"), "{t}");
}

#[test]
fn absolute_values() {
    assert_eq!(tex("|x|"), "{\\left|{x}\\right|}");
    assert_eq!(tex("abs(x)"), "{\\left|{{x}}\\right|}");
    assert_eq!(tex("floor(x)"), "{\\left\\lfloor{{x}}\\right\\rfloor}");
}

#[test]
fn a_lone_bar_is_a_divider() {
    assert_eq!(tex("a | b"), "{a}{\\mid}{b}");
}

#[test]
fn unknown_words_are_letters() {
    assert_eq!(tex("ab"), "{a}{b}");
}

#[test]
fn nothing_panics_on_odd_input() {
    for src in [
        "(",
        ")",
        "{",
        "}",
        "[",
        "]",
        "|",
        "||",
        "|||",
        "_",
        "^",
        "/",
        "//",
        "\\",
        "\"",
        "\"a",
        "text(",
        "text(a",
        "mspace(",
        "mspace(1em)",
        "frac",
        "frac a",
        "root",
        "sqrt",
        "color",
        "color(red)",
        "color(red)x",
        "^^",
        "-",
        "x^-",
        "x_",
        "(((",
        ")))",
        "[[",
        "[[a,b]",
        "'",
        "''",
        ".",
        "1.",
        ".5",
        "1..2",
        "\u{e9}",
        "\u{1f600}",
        "\u{0}",
    ] {
        let _ = translate(src);
    }
}

#[test]
fn deep_nesting_is_an_error() {
    let src = format!("{}x{}", "(".repeat(2_000), ")".repeat(2_000));
    assert!(translate(&src).is_err());
    let src = "sqrt ".repeat(2_000);
    assert!(translate(&src).is_err());
    let src = "frac ".repeat(2_000);
    assert!(translate(&src).is_err());
}

#[test]
fn oversized_source_is_an_error() {
    assert!(translate(&"x".repeat(limits::MAX_SOURCE + 1)).is_err());
}

#[test]
fn source_spans_map_back() {
    let t = translate("a + b").unwrap();
    let at = t.tex.find('b').unwrap();
    assert_eq!(t.source_span(at..at + 1), 4..5);
    let whole = t.source_span(0..t.tex.len());
    assert_eq!(whole, 0..5);
}

/// The formulas of plantuml.com/ascii-math, with the TeX written for each.
const PAGE: &[(&str, &str)] = &[
    (
        "int_0^1f(x)dx",
        r"{\int_{{0}}^{{1}}}{f{{\left({x}\right)}}}{\left.{d}{x}\right.}",
    ),
    (
        "x^2+y_1+z_12^34",
        r"{x}^{{2}}+{y}_{{1}}+{{z}_{{12}}^{{34}}}",
    ),
    (
        "d/dxf(x)=lim_(h->0)(f(x+h)-f(x))/h",
        r"\frac{{d}}{{\left.{d}{x}\right.}}{f{{\left({x}\right)}}}=\lim_{{{h}\to{0}}}\frac{{{f{{\left({x}+{h}\right)}}}-{f{{\left({x}\right)}}}}}{{h}}",
    ),
    (
        "P(y|bb\"x\") or f(bb\"x\")+epsilon",
        r"{P}{\left({y}{\mid}{\mathbf{\text{x}}}\right)}{\quad\text{or}\quad}{f{{\left({\mathbf{\text{x}}}\right)}}}+\epsilon",
    ),
    ("ax^2+bx+c=0", r"{a}{x}^{{2}}+{b}{x}+{c}={0}"),
    (
        "x = (-b+-sqrt(b^2-4ac))/(2a)",
        r"{x}=\frac{{-{b}\pm\sqrt{{{b}^{{2}}-{4}{a}{c}}}}}{{{2}{a}}}",
    ),
    (
        "f(t)=(a_0)/2 + sum_(n=1)^ooa_ncos((npit)/L)+sum_(n=1)^oo b_n\\ sin((npit)/L)",
        r"{f{{\left({t}\right)}}}=\frac{{{a}_{{0}}}}{{2}}+{\sum_{{{n}={1}}}^{\infty}}{a}_{{n}}{\cos{{\left(\frac{{{n}\pi{t}}}{{L}}\right)}}}+{\sum_{{{n}={1}}}^{\infty}}{b}_{{n}}\ {\sin{{\left(\frac{{{n}\pi{t}}}{{L}}\right)}}}",
    ),
];

#[test]
fn the_formulas_of_the_plantuml_page() {
    let mut wrong = String::new();
    for (src, want) in PAGE {
        let got = tex(src);
        if got != *want {
            wrong.push_str(&format!("{src}\n  {got}\n"));
        }
    }
    assert!(wrong.is_empty(), "\n{wrong}");
}

#[cfg(feature = "math")]
#[test]
fn every_symbol_of_the_table_typesets() {
    use crate::math::formula::MathStyle;
    let mut failed = String::new();
    for s in table::SYMS
        .iter()
        .copied()
        .chain(table::SYMS.iter().filter_map(Sym::alias))
    {
        // A symbol that stands for another, or is a bracket, divider, or text, has tests of
        // its own.
        let sources = match s.ttype {
            Ty::Const | Ty::UnderOver => vec![
                s.input.to_owned(),
                format!("x {}_a^b", s.input),
                format!("({} x)", s.input),
            ],
            Ty::Unary | Ty::Binary => vec![
                format!("{} x y", s.input),
                format!("({} x y)", s.input),
                format!("{}(x) y", s.input),
            ],
            _ => continue,
        };
        for src in sources {
            let Ok(t) = translate(&src) else {
                failed.push_str(&format!("{src:?}: translate\n"));
                continue;
            };
            if let Err(e) = typeset_asciimath(&src, MathStyle::Display) {
                failed.push_str(&format!("{src:?} -> {}: {}\n", t.tex, e.message));
            }
        }
    }
    assert!(failed.is_empty(), "\n{failed}");
}

#[cfg(feature = "math")]
#[test]
fn an_error_of_the_tex_is_on_the_source() {
    use crate::math::formula::MathStyle;
    // `\mkern` with a bad length: the error is in the TeX; the span is in the AsciiMath.
    let src = "x + \\undefinedcmd + y";
    let e = typeset_asciimath(src, MathStyle::Text);
    if let Err(e) = e {
        assert!(e.span.end <= src.len(), "{e}");
    }
}

#[cfg(feature = "math")]
#[test]
fn ascii_output_reads_the_translation() {
    use crate::math::formula::MathStyle;
    let a = ascii_asciimath("x^2", MathStyle::Text).unwrap();
    assert_eq!(a, ascii_formula_via_tex("x^2"));
    let f = ascii_asciimath("1/2", MathStyle::Display).unwrap();
    assert!(
        f.contains('1') && f.contains('2') && f.lines().count() >= 3,
        "{f}"
    );
}

#[cfg(feature = "math")]
fn ascii_formula_via_tex(src: &str) -> String {
    use crate::math::formula::MathStyle;
    super::super::ascii::ascii_formula(&tex(src), MathStyle::Text).unwrap()
}

/// `AMgetSymbol`'s search as the JavaScript makes it: every symbol, then every TeX name, the
/// longest match winning and the first on a tie.
fn longest_symbol_by_scan(look: &str) -> (Option<Sym>, usize) {
    let mut best: Option<Sym> = None;
    let mut best_len = 0;
    for s in SYMS
        .iter()
        .copied()
        .chain(SYMS.iter().filter_map(Sym::alias))
    {
        if s.input.len() > best_len && look.starts_with(s.input) {
            best = Some(s);
            best_len = s.input.len();
        }
    }
    (best, best_len)
}

/// The lookup by first byte finds what a scan of the whole table finds: for every input and TeX
/// name, each of their prefixes, and each with text after it.
#[test]
fn symbol_lookup_matches_a_scan_of_the_table() {
    let mut looks: Vec<String> = Vec::new();
    for s in SYMS
        .iter()
        .copied()
        .chain(SYMS.iter().filter_map(Sym::alias))
    {
        for (at, _) in s.input.char_indices().skip(1) {
            looks.push(s.input[..at].to_string());
        }
        for after in ["", "x", " ", "(", "_1", "ow", "arrow"] {
            looks.push(format!("{}{after}", s.input));
        }
    }
    looks.extend(["", " ", "\u{e9}", "\u{1f600}x", "zzz", "\\alpha"].map(String::from));
    let mut wrong = String::new();
    for look in &looks {
        let (a, a_len) = longest_symbol(look);
        let (b, b_len) = longest_symbol_by_scan(look);
        if format!("{a:?}") != format!("{b:?}") || a_len != b_len {
            wrong.push_str(&format!("{look:?}: {a:?} {a_len} vs {b:?} {b_len}\n"));
        }
    }
    assert!(looks.len() > 1000, "{}", looks.len());
    assert!(wrong.is_empty(), "\n{wrong}");
}

/// The fuzz seeds of `fuzz/corpus/asciimath` and a long run of big operators: each translates
/// to bounded TeX, and translating and typesetting it in both styles takes well under the bound.
#[cfg(feature = "math")]
#[test]
fn seeds_and_long_inputs_translate_and_typeset_in_bounded_time_and_size() {
    use crate::math::formula::MathStyle;
    use std::time::{Duration, Instant};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/asciimath");
    let mut inputs: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.unwrap().path();
            let src = std::fs::read_to_string(&path).unwrap();
            (path.display().to_string(), src)
        })
        .collect();
    assert!(inputs.len() >= 5, "{} seeds", inputs.len());
    inputs.push(("2000 sums".to_owned(), "sum_(i=1)^n ".repeat(2000)));
    for (name, src) in &inputs {
        let started = Instant::now();
        let t = translate(src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            t.tex.len() <= 64 * src.len() + 4096,
            "{name}: {} bytes of TeX from {}",
            t.tex.len(),
            src.len()
        );
        for style in [MathStyle::Display, MathStyle::Text] {
            // An error is fine (the long run may pass a limit); a hang is not.
            let _ = typeset_asciimath(src, style);
        }
        let took = started.elapsed();
        assert!(took < Duration::from_secs(10), "{name}: {took:?}");
    }
}

/// Where the time of each seed goes, for comparing with the TeX path. Run with
/// `cargo test -p sirene-core --release --lib asciimath_seed_timings -- --ignored --nocapture`.
#[cfg(feature = "math")]
#[test]
#[ignore = "prints timings"]
fn asciimath_seed_timings() {
    use crate::math::formula::{MathStyle, typeset};
    use crate::scene::{Color, Pt};
    use std::time::Instant;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/asciimath");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    const RUNS: u32 = 200;
    for path in paths {
        let src = std::fs::read_to_string(&path).unwrap();
        let t = translate(&src).unwrap();
        let started = Instant::now();
        for _ in 0..RUNS {
            let _ = translate(&src);
        }
        let translating = started.elapsed() / RUNS;
        let mut typesetting = Vec::new();
        for style in [MathStyle::Display, MathStyle::Text] {
            let started = Instant::now();
            for _ in 0..RUNS {
                let _ = typeset(&t.tex, style);
            }
            typesetting.push(started.elapsed() / RUNS);
        }
        let started = Instant::now();
        for _ in 0..RUNS {
            if let Ok(f) = typeset_asciimath(&src, MathStyle::Display) {
                f.place(Pt::new(10.0, 40.0), 24.0, Color::rgb(0));
            }
        }
        let with_place = started.elapsed() / RUNS;
        // One symbol lookup at the start, and how many lookups the translation makes.
        let chars: Vec<(char, usize)> = src.char_indices().map(|(i, c)| (c, i)).collect();
        let cur = Cur {
            chars: &chars,
            pos: 0,
            pending: Vec::new(),
            end: src.len(),
        };
        let mut p = Parser {
            nest: 0,
            prev: Ty::Unset,
            curr: Ty::Unset,
            steps: 0,
            max_steps: max_steps(src.len()),
            frames: 0,
        };
        let started = Instant::now();
        for _ in 0..RUNS {
            let _ = p.symbol(&cur);
        }
        let lookup = started.elapsed() / RUNS;
        let mut cur = cur;
        let mut p = Parser {
            steps: 0,
            frames: 0,
            ..p
        };
        cur.remove_blanks(0);
        let _ = p.expr(&mut cur, false);
        println!(
            "{}: {} bytes -> {} bytes of TeX; translate {translating:?}, typeset display {:?}, text {:?}, translate+typeset+place {with_place:?}; one symbol lookup {lookup:?}, {} parser steps",
            path.display(),
            src.len(),
            t.tex.len(),
            typesetting[0],
            typesetting[1],
            p.steps,
        );
    }
}

mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Any printable text translates or fails; the TeX it makes is bounded, and the TeX
        /// parser accepts it or reports an error (never a panic).
        #[test]
        fn arbitrary_text_never_panics(src in "[ -~\\n]{0,120}") {
            if let Ok(t) = translate(&src) {
                prop_assert!(t.tex.len() <= 64 * src.len() + 4096, "{} {}", src.len(), t.tex.len());
                #[cfg(feature = "math")]
                {
                    let _ = typeset_asciimath(&src, crate::math::formula::MathStyle::Text);
                }
                let span = t.source_span(0..t.tex.len());
                prop_assert!(span.start <= span.end && span.end <= src.len());
            }
        }

        /// Any text, multibyte too: every span an error or a map gives is a range of the source
        /// on character boundaries.
        #[test]
        fn spans_are_on_character_boundaries(src in "\\PC{0,80}") {
            let ok = |r: Range<usize>| {
                r.start <= r.end && r.end <= src.len()
                    && src.is_char_boundary(r.start) && src.is_char_boundary(r.end)
            };
            match translate(&src) {
                Ok(t) => {
                    prop_assert!(ok(t.source_span(0..t.tex.len())));
                    for at in 0..t.tex.len().min(40) {
                        prop_assert!(ok(t.source_span(at..at + 1)));
                    }
                    #[cfg(feature = "math")]
                    if let Err(e) = typeset_asciimath(&src, crate::math::formula::MathStyle::Text) {
                        prop_assert!(ok(e.span));
                    }
                }
                Err(e) => prop_assert!(ok(e.span)),
            }
        }

        #[test]
        fn text_from_the_grammar_never_panics(src in proptest::collection::vec(
            prop_oneof![
                Just("("), Just(")"), Just("["), Just("]"), Just("{"), Just("}"), Just("|"),
                Just(","), Just("/"), Just("^"), Just("_"), Just("-"), Just("sqrt"),
                Just("frac"), Just("root"), Just("sum"), Just("sin"), Just("text("), Just("\""),
                Just("x"), Just("1"), Just("alpha"), Just("bb"), Just("color"), Just("iff"),
                Just(" "), Just("[["), Just("]]"), Just("{:"), Just(":}"), Just("abs"),
            ],
            0..40,
        )) {
            let src: String = src.concat();
            if let Ok(t) = translate(&src) {
                #[cfg(feature = "math")]
                {
                    let _ = typeset_asciimath(&src, crate::math::formula::MathStyle::Display);
                }
                prop_assert!(t.source_span(0..t.tex.len()).end <= src.len());
            }
        }
    }
}
