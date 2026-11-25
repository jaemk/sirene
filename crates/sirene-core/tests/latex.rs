//! TeX files as one equation (LEMB-3): the extension and `--lang` pick the language, the
//! wrappers around a formula are taken off, a full document is refused, and every diagnostic is
//! positioned in the file.

#![cfg(feature = "math")]

use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::diagram::{self, Diagram, Kind, Lang, Lowered};
use sirene_core::equation::{Equation, Flavor};
use sirene_core::scene::{Approx, Primitive, Theme};

fn fixture(dir: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/latex")
        .join(dir)
        .join(format!("{name}.tex"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn lower(src: &str) -> Lowered {
    diagram::lower_as(src, Some(Lang::Latex))
}

fn equation(l: &Lowered) -> &Equation {
    match &l.diagram {
        Diagram::Equation(e) => e,
        other => panic!("not an equation: {other:?}"),
    }
}

/// The text a range of the source covers.
fn at<'a>(src: &'a str, d: &sirene_core::Diagnostic) -> &'a str {
    &src[usize::from(d.range.start())..usize::from(d.range.end())]
}

const VALID: &[(&str, &str)] = &[
    ("bare", "\\frac{a}{b} + x^2"),
    ("display", "\\sum_{i=1}^n i = \\frac{n(n+1)}{2}"),
    ("inline", "e^{i\\pi} + 1 = 0"),
    ("brackets", "\n  \\int_0^1 x \\, dx\n"),
    ("parens", " a^2 + b^2 = c^2 "),
    ("equation", "\\begin{equation}\n  E = mc^2\n\\end{equation}"),
    (
        "equation_star",
        "\\begin{equation*}\n  E = mc^2\n\\end{equation*}",
    ),
    ("displaymath", "\n  x_1 + x_2\n"),
    ("math_env", "x^2 + y^2"),
    (
        "align",
        "\\begin{align}\n  a &= b + c \\\\\n  d &= e\n\\end{align}",
    ),
    (
        "gather",
        "\\begin{gather*}\n  a = b \\\\\n  c = d\n\\end{gather*}",
    ),
    (
        "commented",
        " \\int_{-\\infty}^{\\infty} e^{-x^2} \\, dx % the integrand\n   = \\sqrt{\\pi} ",
    ),
    ("document", "\n  \\frac{1}{2} + \\frac{1}{3}\n"),
];

#[test]
fn extensions_and_names_pick_the_language() {
    for name in ["a.tex", "a.latex", "A.TEX", "dir/a.LaTeX"] {
        assert_eq!(Lang::of_path(Path::new(name)), Some(Lang::Latex), "{name}");
    }
    assert_eq!(Lang::of_path(Path::new("a.texinfo")), None);
    assert_eq!(Lang::of_path(Path::new("tex")), None);
    // The language is read whatever the source looks like.
    let l = diagram::lower_as("pie\n", Some(Lang::Latex));
    assert_eq!(l.kind, Kind::Latex);
    assert_eq!(Kind::of_lang("pie\n", Some(Lang::Latex)), Kind::Latex);
    assert_eq!(Kind::Latex.lang(), Lang::Latex);
    assert_eq!(Kind::Latex.name(), "LaTeX formula");
    assert!(Kind::ALL.contains(&Kind::Latex));
    // Detection never picks TeX.
    assert_ne!(Kind::of("$$x$$\n"), Kind::Latex);
    // Markdown info words do not name it: `latex` fences are formulas, not diagrams.
    assert_eq!(Lang::of_info("latex"), None);
}

#[test]
fn every_wrapper_is_taken_off() {
    for (name, source) in VALID {
        let src = fixture("valid", name);
        let l = lower(&src);
        assert_eq!(l.kind, Kind::Latex, "{name}");
        assert_eq!(l.diagnostics, [], "{name}");
        let e = equation(&l);
        assert_eq!(
            (e.source.as_str(), e.flavor, e.inline),
            (*source, Flavor::Tex, false)
        );
    }
}

#[test]
fn every_output_path_draws_the_formula() {
    for (name, _) in VALID {
        let l = lower(&fixture("valid", name));
        let svg = l.svg(&Theme::LIGHT).expect("svg");
        assert!(svg.starts_with("<svg"), "{name}");
        // Paths, not the source text.
        assert!(svg.contains("<path"), "{name}: {svg}");
        let ascii = l.ascii(&Default::default()).expect("ascii");
        assert!(!ascii.trim().is_empty(), "{name}");
        assert!(
            !ascii.contains('$') && !ascii.contains("\\begin"),
            "{name}: {ascii}"
        );
        let scene = l.scene(&Approx, &Theme::LIGHT).expect("scene");
        assert!(
            scene
                .elements
                .iter()
                .all(|e| !matches!(e.primitive, Primitive::Text { .. })),
            "{name}"
        );
    }
}

#[test]
fn snapshots_of_what_is_read() {
    for (name, _) in VALID {
        let src = fixture("valid", name);
        let l = lower(&src);
        let e = equation(&l);
        let out = format!("{:?}\n{:?}\n{:?}\n", e.source, e.flavor, l.diagnostics);
        insta::assert_snapshot!(format!("valid__{name}"), out, &src);
    }
    for name in [
        "full_document",
        "two_blocks",
        "unknown_command",
        "wrapped_unknown",
    ] {
        let src = fixture("errors", name);
        let l = lower(&src);
        let e = equation(&l);
        let out = format!("{:?}\n{:#?}\n", e.source, l.diagnostics);
        insta::assert_snapshot!(format!("errors__{name}"), out, &src);
    }
}

#[test]
fn a_full_document_is_refused_on_its_first_line() {
    for name in ["full_document", "two_blocks"] {
        let src = fixture("errors", name);
        let l = lower(&src);
        assert_eq!(l.diagnostics.len(), 1, "{name}: {:?}", l.diagnostics);
        let d = &l.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(
            d.message,
            "full LaTeX documents are not supported; give one formula"
        );
        assert_eq!(at(&src, d), "\\documentclass{article}");
        assert_eq!(d.line_col(&src), (1, 1));
        // Drawn as its source, as one line of text.
        let svg = l.svg(&Theme::LIGHT).expect("svg");
        assert!(svg.contains("documentclass"), "{name}: {svg}");
    }
}

#[test]
fn a_document_without_one_math_block_is_refused() {
    for src in [
        "\\begin{document}\nx\n\\end{document}\n",
        "\\begin{document}\n\\end{document}\n",
        "\\begin{document}\n$x$\n",
        "\\documentclass{article}\n",
        "\\begin{document}\n\\begin{itemize}\\end{itemize}\n\\end{document}\n",
        "\\begin{document}\n$x$ and $y$\n\\end{document}\n",
    ] {
        let l = lower(src);
        assert_eq!(l.diagnostics.len(), 1, "{src:?}: {:?}", l.diagnostics);
        assert_eq!(
            l.diagnostics[0].message,
            "full LaTeX documents are not supported; give one formula"
        );
        let first = src.lines().next().expect("a line");
        assert_eq!(at(src, &l.diagnostics[0]), first, "{src:?}");
    }
    // A commented-out preamble is not one.
    let l = lower("% \\documentclass{article}\n$x$\n");
    assert_eq!(l.diagnostics, []);
    assert_eq!(equation(&l).source, "x");
    // An escaped percent sign starts no comment.
    let l = lower("\\% \\documentclass{article}\n");
    assert_eq!(l.diagnostics.len(), 1);
    assert_eq!(
        l.diagnostics[0].message,
        "full LaTeX documents are not supported; give one formula"
    );
}

#[test]
fn a_document_error_after_a_byte_order_mark_starts_after_it() {
    let src = "\u{feff}\\documentclass{article}\r\n\\begin{document}\r\nx\r\n\\end{document}\r\n";
    let l = lower(src);
    assert_eq!(l.diagnostics.len(), 1);
    assert_eq!(at(src, &l.diagnostics[0]), "\\documentclass{article}");
}

#[test]
fn errors_are_positioned_in_the_source() {
    let src = fixture("errors", "unknown_command");
    let l = lower(&src);
    assert_eq!(l.diagnostics.len(), 1, "{:?}", l.diagnostics);
    let d = &l.diagnostics[0];
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(at(&src, d), "\\notacommand");
    assert_eq!(d.line_col(&src), (3, 8));
    // The formula is drawn as its source, with the one diagnostic.
    let svg = l.svg(&Theme::LIGHT).expect("svg");
    assert!(svg.contains("notacommand"), "{svg}");

    // Inside a wrapper, after comments and indentation.
    let src = fixture("errors", "wrapped_unknown");
    let l = lower(&src);
    assert_eq!(l.diagnostics.len(), 1, "{:?}", l.diagnostics);
    assert_eq!(at(&src, &l.diagnostics[0]), "\\frobnicate");
    assert_eq!(l.diagnostics[0].line_col(&src), (3, 9));

    // A bare formula and the wrapper's own mismatches.
    let src = "x^1^2";
    let l = lower(src);
    assert_eq!(at(src, &l.diagnostics[0]), "^");
    assert_eq!(usize::from(l.diagnostics[0].range.start()), 3);
    let src = "$a$ and $b$";
    let l = lower(src);
    assert_eq!(l.diagnostics.len(), 1, "{:?}", l.diagnostics);
    assert!(l.diagnostics[0].range.start() < (src.len() as u32).into());
}

#[test]
fn text_and_ascii_draw_the_source_of_a_failed_formula() {
    let l = lower("$$ \\frobnicate $$");
    assert_eq!(l.diagnostics.len(), 1);
    let ascii = l.ascii(&Default::default()).expect("ascii");
    assert_eq!(ascii, " \\frobnicate \n");
}

#[test]
fn the_formula_is_the_same_as_the_unwrapped_one() {
    let wrapped = lower("$$x^2$$").svg(&Theme::LIGHT).expect("svg");
    let bare = lower("x^2").svg(&Theme::LIGHT).expect("svg");
    assert_eq!(wrapped, bare);
    for wrapper in [
        "$x^2$",
        "\\[x^2\\]",
        "\\(x^2\\)",
        "\\begin{math}x^2\\end{math}",
    ] {
        let svg = lower(wrapper).svg(&Theme::LIGHT).expect("svg");
        assert_eq!(svg, bare, "{wrapper}");
    }
}

#[test]
fn an_empty_source_does_not_panic() {
    for src in [
        "",
        "\n",
        "% only a comment\n",
        "$$$$",
        "$$",
        "$",
        "\\[\\]",
        "\u{feff}",
    ] {
        let l = lower(src);
        let _ = l.svg(&Theme::LIGHT);
        let _ = l.ascii(&Default::default());
    }
}
