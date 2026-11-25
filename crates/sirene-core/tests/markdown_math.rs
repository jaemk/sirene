//! Formulas in Markdown, found beside the diagram blocks (LEMB-3, MARKDO-5): fenced `math`,
//! `latex`, and `tex` blocks, `$$...$$`, and inline `$...$` by GitHub's rules, in source order
//! with the diagrams and never inside code.

use std::fs;
use std::path::Path;

use proptest::prelude::*;
use sirene_core::diagnostic::LineIndex;
use sirene_core::markdown::{self, MathKind, Part};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/markdown")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The formulas of `src` as (kind, source).
fn math(src: &str) -> Vec<(MathKind, String)> {
    markdown::parts(src)
        .into_iter()
        .filter_map(|p| match p {
            Part::Math(m) => Some((m.kind, m.source)),
            Part::Diagram(_) => None,
        })
        .collect()
}

fn sources(src: &str) -> Vec<String> {
    math(src).into_iter().map(|(_, s)| s).collect()
}

/// The ranges are in the text, on character boundaries, in order, and do not overlap.
fn assert_well_formed(src: &str) {
    let parts = markdown::parts(src);
    let mut end = 0;
    for p in &parts {
        let r = p.range();
        assert!(r.start <= r.end && r.end <= src.len(), "{r:?} in {src:?}");
        assert!(src.get(r.clone()).is_some(), "{r:?} in {src:?}");
        assert!(
            r.start >= end,
            "{r:?} overlaps or precedes {end} in {src:?}"
        );
        end = r.end;
        if let Part::Math(m) = p {
            assert!(m.line >= 1, "{src:?}");
            // Lowering and positioning do not panic.
            let lowered = m.lower();
            let index = LineIndex::new(&m.source);
            for d in &lowered.diagnostics {
                let (line, col) = index.line_col(d);
                let (l, c) = m.position(line, col);
                assert!(l >= m.line && c >= 1, "{src:?}");
            }
        }
    }
}

/// The character at a Markdown line and column (from 1).
fn char_at(src: &str, line: usize, col: usize) -> Option<char> {
    src.lines().nth(line - 1)?.chars().nth(col - 1)
}

#[test]
fn fenced_blocks_named_math_latex_or_tex() {
    for info in [
        "math",
        "latex",
        "tex",
        "Math",
        "LaTeX",
        "{.math}",
        "math {#eq}",
        "tex title=x",
    ] {
        let src = format!("```{info}\nx^2\n```\n");
        assert_eq!(
            math(&src),
            [(MathKind::Fenced, "x^2\n".to_owned())],
            "{info}"
        );
    }
    let tilde = "~~~math\na\n~~~\n";
    assert_eq!(sources(tilde), ["a\n"]);
    for info in ["mathematica", "text", "texinfo", "", "rust"] {
        let src = format!("```{info}\nx^2\n```\n");
        assert_eq!(math(&src), [], "{info}");
    }
    // An unclosed block runs to the end.
    assert_eq!(sources("```math\nx\n"), ["x\n"]);
    // A fenced block keeps its TeX wrappers for the formula's own reader.
    let src = "```latex\n$$ x^2 $$\n```\n";
    let Some(Part::Math(m)) = markdown::parts(src).pop() else {
        panic!("one formula");
    };
    assert_eq!(m.source, "$$ x^2 $$\n");
    assert_eq!(m.lower().diagnostics, []);
}

#[test]
fn display_blocks() {
    let cases: &[(&str, &[&str])] = &[
        ("$$x$$\n", &["x"]),
        ("$$\nx\n$$\n", &["\nx\n"]),
        (
            "$$\na &= b \\\\\nc &= d\n$$\n",
            &["\na &= b \\\\\nc &= d\n"],
        ),
        ("a $$x$$ b\n", &["x"]),
        ("a $$x$$ b $$y$$ c\n", &["x", "y"]),
        ("a $$\nx\n$$ b\n", &["\nx\n"]),
        ("$$ x\n  y $$\n", &[" x\ny "]),
        // Not closed, empty, closed only after a blank line, or three dollar signs.
        ("$$ x\n", &[]),
        ("$$ $$\n", &[]),
        ("$$$$\n", &[]),
        ("$$\nx\n\ny\n$$\n", &[]),
        ("$$$x$$$\n", &[]),
        // An escaped delimiter is not one.
        ("$$a \\$$ b$$\n", &["a \\$$ b"]),
    ];
    for (src, expected) in cases {
        let found = math(src);
        assert!(
            found.iter().all(|(k, _)| *k == MathKind::Display),
            "{src:?}: {found:?}"
        );
        assert_eq!(sources(src), *expected, "{src:?}");
        assert_well_formed(src);
    }
}

#[test]
fn inline_formulas_follow_githubs_rules() {
    let cases: &[(&str, &[&str])] = &[
        ("$x$", &["x"]),
        ("a $x^2$ b", &["x^2"]),
        ("$a$ and $b$", &["a", "b"]),
        ("$x$.", &["x"]),
        ("($x$)", &["x"]),
        // The opening dollar sign is not followed by whitespace; the closing one is not
        // preceded by whitespace and not followed by a digit.
        ("$ x$", &[]),
        ("$x $", &[]),
        ("$x$5", &[]),
        ("$5 and $6", &[]),
        ("it costs $5 or $6.", &[]),
        ("$", &[]),
        ("$$", &[]),
        ("a $ b", &[]),
        // The body is not empty, and a dollar sign can be escaped.
        ("$$ $", &[]),
        ("\\$x$", &[]),
        ("\\$x\\$", &[]),
        ("$x\\$y$", &["x\\$y"]),
        ("\\\\$x$", &["x"]),
        // A formula stays on its line.
        ("$a\nb$", &[]),
        ("$a\n$b$", &["b"]),
        ("$a $b$", &["a $b"]),
        ("$a $b\n$c$", &["c"]),
        // Code spans.
        ("`$x$`", &[]),
        ("`` $x$ `` and $y$", &["y"]),
        ("`a $x$ b` $z$", &["z"]),
        ("`unclosed $x$", &["x"]),
        // A code span between the delimiters is not searched for the closing one.
        ("$a `$` b$", &["a `$` b"]),
        ("$5 and `$y$`", &[]),
    ];
    for (src, expected) in cases {
        let found = math(src);
        assert!(
            found.iter().all(|(k, _)| *k == MathKind::Inline),
            "{src:?}: {found:?}"
        );
        assert_eq!(sources(src), *expected, "{src:?}");
        assert_well_formed(src);
    }
}

#[test]
fn unmatched_dollar_signs_are_read_in_linear_time() {
    // Each `$` opens a search for a closer; a search per `$` over one line is quadratic. 60,000
    // bytes took 10 seconds that way.
    for unit in ["$a ", "$`a "] {
        let src = unit.repeat(40_000);
        let start = std::time::Instant::now();
        assert!(math(&src).is_empty());
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "{unit:?}: {:?}",
            start.elapsed()
        );
    }
}

#[test]
fn code_is_never_read_for_math() {
    for src in [
        "```rust\nlet s = \"$x$\";\n```\n",
        "```\n$$\nx\n$$\n```\n",
        "~~~\n$x$\n~~~\n",
        "    $x$\n",
        "text\n\n    $$x$$\n",
        "- a\n\n      $x$\n",
        "```\n```math\nx\n```\n```\n",
    ] {
        assert_eq!(math(src), [], "{src:?}");
        assert_well_formed(src);
    }
    // An indented line that continues a paragraph is not code.
    assert_eq!(sources("a\n    $x$\n"), ["x"]);
}

#[test]
fn a_diagram_fence_is_never_read_for_math() {
    for lang in ["mermaid", "dot", "plantuml", "puml", "graphviz"] {
        let src = format!("```{lang}\nA[\"$$x^2$$\"] and $y$\n$$z$$\n```\n");
        assert_eq!(math(&src), [], "{lang}");
        let parts = markdown::parts(&src);
        assert_eq!(parts.len(), 1, "{lang}");
        assert!(matches!(parts[0], Part::Diagram(_)));
    }
    // `latex` and `tex` fences are formulas, not diagram blocks.
    assert_eq!(markdown::mermaid_blocks("```latex\nx\n```\n").len(), 0);
    assert_eq!(markdown::mermaid_blocks("```math\nx\n```\n").len(), 0);
    // `mermaid_blocks` does not see the inline formulas either.
    assert_eq!(markdown::mermaid_blocks("$x$ and $$y$$\n").len(), 0);
}

#[test]
fn headings_breaks_and_paragraphs_end_a_formula() {
    assert_eq!(sources("# a $x$\n"), ["x"]);
    assert_eq!(sources("# a $x\n# y$\n"), Vec::<String>::new());
    assert_eq!(sources("$$ x\n---\ny $$\n"), Vec::<String>::new());
    assert_eq!(sources("$$ x\n***\ny $$\n"), Vec::<String>::new());
    assert_eq!(sources("Title\n=====\n$x$\n"), ["x"]);
    assert_eq!(sources("$$ x\n\n$$ y $$\n"), [" y "]);
}

#[test]
fn formulas_in_lists_and_blockquotes() {
    let src = "- item $a$\n  and $$b$$\n  1. nested $c$\n\n> quote $d$\n>\n> $$\n> e\n> $$\n\n> > deep $f$\n\n1. one\n   $$\n   g\n   $$\n";
    assert_eq!(sources(src), ["a", "b", "c", "d", "\ne\n", "f", "\ng\n"]);
    assert_well_formed(src);
    // A line less indented than the item is not in it, and a fence inside one is code.
    assert_eq!(sources("- $a$\n\n$b$\n"), ["a", "b"]);
    assert_eq!(sources("- ```\n  $a$\n  ```\n"), Vec::<String>::new());
    assert_eq!(sources("> ```\n> $a$\n> ```\n"), Vec::<String>::new());
    // A formula does not cross the end of its blockquote or list item.
    assert_eq!(sources("> $a\nb$\n"), Vec::<String>::new());
    assert_eq!(sources("> $$ a\n\n> b $$\n"), Vec::<String>::new());
    assert_eq!(sources("- $$ a\n- b $$\n"), Vec::<String>::new());
    // A fenced formula in a container takes its markers off.
    let src = "> ```math\n> x^2\n> ```\n";
    assert_eq!(sources(src), ["x^2\n"]);
    let src = "- ```tex\n  a\n  b\n  ```\n";
    assert_eq!(sources(src), ["a\nb\n"]);
}

#[test]
fn positions_map_back_to_the_markdown() {
    // Inline, after text and a multibyte character, in a list item and a blockquote.
    for (src, bad) in [
        ("see \u{e9} $x^1^2$ ok\n", '^'),
        ("- a\n  see $x^1^2$ ok\n", '^'),
        ("> \u{e9}\u{e9} $x^1^2$\n", '^'),
        ("> - $x^1^2$\n", '^'),
        ("$$\nx^1^2\n$$\n", '^'),
        ("a\n\n$$ a\n   y + x^1^2 $$\n", '^'),
        ("> $$\n> x^1^2\n> $$\n", '^'),
        ("- item\n\n  ```math\n  x^1^2\n  ```\n", '^'),
        ("```tex\n$$ x^1^2 $$\n```\n", '^'),
    ] {
        let parts = markdown::parts(src);
        let [Part::Math(m)] = parts.as_slice() else {
            panic!("one formula in {src:?}: {parts:?}");
        };
        let lowered = m.lower();
        assert_eq!(lowered.diagnostics.len(), 1, "{src:?}");
        let (line, col) = LineIndex::new(&m.source).line_col(&lowered.diagnostics[0]);
        let (line, col) = m.position(line, col);
        assert_eq!(
            char_at(src, line, col),
            Some(bad),
            "{src:?} at {line}:{col}"
        );
        // It is the second `^` of `x^1^2`.
        let start: usize = src.split_inclusive('\n').take(line - 1).map(str::len).sum();
        let text = src.lines().nth(line - 1).unwrap_or("");
        let byte = text.char_indices().nth(col - 1).map_or(0, |(i, _)| i);
        let off = start + byte;
        assert_eq!(&src[off - 3..off + 2], "x^1^2", "{src:?}");
    }
}

#[test]
fn ranges_cover_the_delimiters() {
    let src = "a $x$ b\n\n$$\ny\n$$\n\n```math\nz\n```\nend\n";
    let texts: Vec<&str> = markdown::parts(src)
        .iter()
        .map(|p| &src[p.range()])
        .collect();
    assert_eq!(texts, ["$x$", "$$\ny\n$$", "```math\nz\n```\n"]);
    // A formula across the lines of a blockquote takes the markers in its range.
    let src = "> $$ a\n> b $$\n";
    let parts = markdown::parts(src);
    assert_eq!(&src[parts[0].range()], "$$ a\n> b $$");
}

#[test]
fn diagrams_and_formulas_are_in_source_order() {
    let src = fixture("math.md");
    let parts = markdown::parts(&src);
    let kinds: Vec<String> = parts
        .iter()
        .map(|p| match p {
            Part::Diagram(b) => format!("diagram {:?}", b.lang),
            Part::Math(m) => format!("{:?}", m.kind),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Inline",
            "Display",
            "diagram Mermaid",
            "Fenced",
            "Inline",
            "Display",
            "Inline",
            "Display",
            "Fenced",
            "diagram Dot",
            "Display",
        ]
    );
    assert_well_formed(&src);
    // Diagram blocks are the same as `mermaid_blocks` finds.
    let diagrams: Vec<_> = parts
        .iter()
        .filter_map(|p| match p {
            Part::Diagram(b) => Some(b.clone()),
            Part::Math(_) => None,
        })
        .collect();
    assert_eq!(diagrams, markdown::mermaid_blocks(&src));
}

#[test]
fn snapshot_of_the_fixture() {
    let src = fixture("math.md");
    let mut out = String::new();
    for (i, p) in markdown::parts(&src).iter().enumerate() {
        let r = p.range();
        let what = match p {
            Part::Diagram(b) => format!("diagram {:?} line {}", b.lang, b.line),
            Part::Math(m) => format!("{:?} line {} source {:?}", m.kind, m.line, m.source),
        };
        out.push_str(&format!(
            "{}: {what}\n   range {r:?} {:?}\n",
            i + 1,
            &src[r.clone()]
        ));
    }
    insta::assert_snapshot!("parts", out, &src);
}

#[test]
fn the_fixture_formulas_typeset() {
    let src = fixture("math.md");
    for p in markdown::parts(&src) {
        if let Part::Math(m) = p {
            let lowered = m.lower();
            assert_eq!(lowered.diagnostics, [], "{:?}", m.source);
            let svg = lowered.svg(&sirene_core::scene::Theme::LIGHT).expect("svg");
            assert!(
                svg.contains("<path") || !sirene_core::equation::MATH,
                "{:?}",
                m.source
            );
        }
    }
}

#[test]
fn inline_formulas_are_in_text_style_and_display_ones_are_not() {
    let (e, _) = sirene_core::equation::Equation::formula("x", true);
    assert!(e.inline);
    let (e, _) = sirene_core::equation::Equation::formula("x", false);
    assert!(!e.inline);
    let parts = markdown::parts("$\\sum_{i=0}^n i$ and $$\\sum_{i=0}^n i$$\n");
    let heights: Vec<f32> = parts
        .iter()
        .map(|p| match p {
            Part::Math(m) => {
                let lowered = m.lower();
                let scene = lowered
                    .scene(
                        &sirene_core::scene::Approx,
                        &sirene_core::scene::Theme::LIGHT,
                    )
                    .expect("scene");
                scene.height
            }
            Part::Diagram(_) => 0.0,
        })
        .collect();
    // Display style sets the limits above and below the sum, so it is taller.
    assert!(heights[1] > heights[0], "{heights:?}");
}

#[test]
fn a_block_has_its_line() {
    let src = "a\n\n$$\nx\n$$\n\n- $y$\n";
    let lines: Vec<usize> = markdown::parts(src)
        .iter()
        .filter_map(|p| match p {
            Part::Math(m) => Some(m.line),
            Part::Diagram(_) => None,
        })
        .collect();
    assert_eq!(lines, [3, 7]);
}

proptest! {
    /// Any text finds blocks whose ranges are in bounds, on character boundaries, in order, and
    /// not overlapping, and lowering them never panics.
    #[test]
    fn arbitrary_markdown_never_panics(src in "\\PC{0,300}") {
        assert_well_formed(&src);
    }

    #[test]
    fn markdown_made_of_its_own_syntax_never_panics(
        src in "[ $\\\\`~>*+#=.0-9a-z\\n\\r{}()_\\-\u{e9}]{0,240}"
    ) {
        assert_well_formed(&src);
    }

    #[test]
    fn dollar_heavy_text_keeps_ranges_valid(
        words in proptest::collection::vec(
            prop_oneof![
                Just("$"), Just("$$"), Just("\\$"), Just("`"), Just("```math"), Just("```"),
                Just("x^2"), Just("\n"), Just("\n\n"), Just("> "), Just("- "), Just("  "),
                Just("```mermaid"), Just("\\frac{a}{b}"), Just("\u{e9}"), Just("5"),
            ],
            0..60,
        )
    ) {
        assert_well_formed(&words.concat());
    }
}
