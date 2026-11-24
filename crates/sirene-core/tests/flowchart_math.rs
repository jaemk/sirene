//! `$$...$$` formulas in flowchart node and edge labels (LEMB-1, LEMB-4, LEMB-5): a label line is
//! a row of text runs and formula runs on one baseline, drawn inside its node, and a formula
//! that does not typeset is drawn as its source with one diagnostic.

use std::fs;
use std::path::Path;

use sirene_core::flowchart::{self, Flowchart, Label};
use sirene_core::scene::{self, Approx, Primitive, Scene, Theme};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/flowchart")
        .join(format!("{name}.mmd"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn lower(name: &str) -> flowchart::Lowered {
    flowchart::parse(&fixture(name)).lower()
}

fn draw(chart: &Flowchart) -> Scene {
    scene::flowchart(chart, &Approx, &Theme::LIGHT).unwrap()
}

fn texts(scene: &Scene) -> Vec<&str> {
    scene
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_label_without_a_formula_has_the_same_runs_as_spans() {
    for text in ["plain", "a $ b $ c", "$$a", "two\nlines", "", "**bold** x"] {
        let l = Label {
            text: text.to_owned(),
            markdown: text.contains("**"),
            ..Label::default()
        };
        assert_eq!(l.runs(), l.spans(), "{text:?}");
        assert!(!l.has_formula(), "{text:?}");
    }
}

#[cfg(feature = "math")]
mod math {
    use super::*;
    use sirene_core::ascii::{self, Charset, Options};
    use sirene_core::scene::{Owner, Rect, Segment};

    fn plain_label(text: &str) -> Label {
        Label::plain(text)
    }

    /// Left, top, right, bottom.
    type Bounds = (f32, f32, f32, f32);

    /// The bounds of a path's points, control points included, cut to its clip.
    fn bounds(segments: &[Segment], clip: Option<Rect>) -> Option<Bounds> {
        let mut b: Option<Bounds> = None;
        for s in segments {
            let pts = match s {
                Segment::Move(a) | Segment::Line(a) => vec![*a],
                Segment::Arc { to, .. } => vec![*to],
                Segment::Cubic { c1, c2, to } => vec![*c1, *c2, *to],
                _ => vec![],
            };
            for p in pts {
                b = Some(match b {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, c, d, e)) => (a.min(p.x), c.min(p.y), d.max(p.x), e.max(p.y)),
                });
            }
        }
        let (x0, y0, x1, y1) = b?;
        match clip {
            None => Some((x0, y0, x1, y1)),
            Some(c) => {
                let cut = (
                    x0.max(c.x),
                    y0.max(c.y),
                    x1.min(c.x + c.w),
                    y1.min(c.y + c.h),
                );
                (cut.0 <= cut.2 && cut.1 <= cut.3).then_some(cut)
            }
        }
    }

    /// The paths of `owner`: the first stroked one is its shape, the others its formulas.
    fn shape_and_formulas(scene: &Scene, owner: Owner) -> (Bounds, Vec<Bounds>) {
        let mut paths = scene
            .elements
            .iter()
            .filter(|e| e.owner == owner)
            .filter_map(|e| match &e.primitive {
                Primitive::Path {
                    segments,
                    stroke,
                    clip,
                    ..
                } => Some((bounds(segments, *clip), stroke.is_some())),
                _ => None,
            });
        let (shape, _) = paths
            .by_ref()
            .find(|(_, stroked)| *stroked)
            .expect("the owner has a shape");
        let formulas = paths.filter_map(|(b, _)| b).collect();
        (shape.expect("the shape has points"), formulas)
    }

    fn assert_inside(outer: Bounds, inner: &[Bounds], what: &str) {
        assert!(!inner.is_empty(), "{what}: no formula paths");
        let slack = 1.0;
        for (i, b) in inner.iter().enumerate() {
            assert!(
                b.0 >= outer.0 - slack
                    && b.1 >= outer.1 - slack
                    && b.2 <= outer.2 + slack
                    && b.3 <= outer.3 + slack,
                "{what}: path {i} {b:?} leaves {outer:?}"
            );
        }
    }

    #[test]
    fn runs_split_a_line_into_text_and_formula_spans() {
        let chart = lower("math_mixed").flowchart;
        let runs = chart.node("A").unwrap().label.runs();
        let [line] = runs.as_slice() else {
            panic!("one line, got {runs:?}");
        };
        let shape: Vec<(&str, bool)> = line.iter().map(|s| (s.text.as_str(), s.math)).collect();
        assert_eq!(
            shape,
            [
                ("Area ", false),
                (r"$$\pi r^2$$", true),
                (" and ", false),
                (r"$$\frac{a}{b}$$", true),
                (" done", false),
            ]
        );
        assert_eq!(line[1].formula().as_deref(), Some(r"\pi r^2"));
        assert_eq!(line[0].formula(), None);
    }

    #[test]
    fn recognition_is_lazy_and_per_line_with_no_escape() {
        let runs = |t: &str| {
            plain_label(t)
                .runs()
                .into_iter()
                .map(|l| {
                    l.into_iter()
                        .map(|s| (s.text, s.math))
                        .collect::<Vec<(String, bool)>>()
                })
                .collect::<Vec<_>>()
        };
        // `$$a$$b$$c$$` is two formulas with text between.
        assert_eq!(
            runs("$$a$$b$$c$$"),
            [[
                ("$$a$$".to_owned(), true),
                ("b".to_owned(), false),
                ("$$c$$".to_owned(), true)
            ]]
        );
        // Unclosed, and closed on another line: text.
        assert_eq!(runs("$$a"), [[("$$a".to_owned(), false)]]);
        assert!(!plain_label("$$a\nb$$").has_formula());
        // Empty body.
        assert_eq!(runs("$$$$"), [[("$$$$".to_owned(), true)]]);
        // A backslash does not escape the delimiter.
        assert!(plain_label(r"\$$a$$").has_formula());
    }

    #[test]
    fn a_double_backslash_is_read_as_one() {
        let l = plain_label(r"$$a\\b$$");
        let runs = l.runs();
        assert_eq!(runs[0][0].formula().as_deref(), Some(r"a\b"));
    }

    #[test]
    fn a_label_with_a_formula_is_not_markdown() {
        let l = Label {
            text: "**b** $$x$$".to_owned(),
            markdown: true,
            ..Label::default()
        };
        let line = &l.runs()[0];
        assert_eq!(line[0].text, "**b** ");
        assert!(!line[0].bold);
        assert!(line[1].math);
    }

    #[test]
    fn a_node_box_contains_its_formula() {
        for name in ["math_node", "math_mixed", "math_br"] {
            let scene = draw(&lower(name).flowchart);
            let (shape, formulas) = shape_and_formulas(&scene, Owner::Node(0));
            assert_inside(shape, &formulas, name);
        }
    }

    #[test]
    fn a_formula_replaces_its_source_text() {
        let scene = draw(&lower("math_node").flowchart);
        assert!(
            texts(&scene).iter().all(|t| !t.contains('$')),
            "{:?}",
            texts(&scene)
        );
    }

    #[test]
    fn text_runs_keep_their_text_beside_formulas() {
        let scene = draw(&lower("math_mixed").flowchart);
        let t = texts(&scene);
        for want in ["Area", "and", "done"] {
            assert!(t.contains(&want), "{want} in {t:?}");
        }
        let scene = draw(&lower("math_br").flowchart);
        assert!(texts(&scene).contains(&"first line"));
    }

    #[test]
    fn a_tall_formula_makes_its_line_and_node_taller() {
        let height = |src: &str| {
            let chart = flowchart::parse(src).lower().flowchart;
            let (shape, _) = shape_and_formulas(&draw(&chart), Owner::Node(0));
            shape.3 - shape.1
        };
        let plain = height("flowchart LR\n  A[x]\n");
        let tall = height("flowchart LR\n  A[\"$$\\frac{a}{b}$$\"]\n");
        assert!(tall > plain, "{tall} vs {plain}");
    }

    #[test]
    fn text_and_formula_share_a_baseline() {
        // `x` has no descender, so the bottom of the formula `x` is the baseline, and the text
        // run is centered 0.35 em (the theme's font size is 14 px) above it.
        let chart = flowchart::parse("flowchart LR\n  A[\"x $$x$$\"]\n")
            .lower()
            .flowchart;
        let scene = draw(&chart);
        let center = scene
            .elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == "x" => Some(*center),
                _ => None,
            })
            .expect("a text run `x`");
        let (_, formulas) = shape_and_formulas(&scene, Owner::Node(0));
        let baseline = formulas.iter().map(|b| b.3).fold(f32::MIN, f32::max);
        let want = 0.35 * Theme::LIGHT.font_size;
        assert!(
            (baseline - center.y - want).abs() < 0.5,
            "baseline {baseline}, text center {}",
            center.y
        );
    }

    #[test]
    fn an_edge_label_box_contains_its_formula() {
        let scene = draw(&lower("math_edge").flowchart);
        let (label, inside) = edge_label_box(&scene);
        assert_inside(label, &inside, "math_edge");
        assert!(texts(&scene).iter().all(|t| !t.contains('$')));
    }

    /// The edge label's box and the paths drawn after it.
    fn edge_label_box(scene: &Scene) -> (Bounds, Vec<Bounds>) {
        let mut label = None;
        let mut inside = Vec::new();
        for e in scene.elements.iter().filter(|e| e.owner == Owner::Edge(0)) {
            let Primitive::Path {
                segments,
                fill,
                stroke,
                clip,
            } = &e.primitive
            else {
                continue;
            };
            if *fill == Some(Theme::LIGHT.edge_label_fill) && stroke.is_none() {
                label = bounds(segments, *clip);
            } else if label.is_some() {
                inside.extend(bounds(segments, *clip));
            }
        }
        (label.expect("the label has a box"), inside)
    }

    #[test]
    fn an_edge_label_grows_to_hold_its_formula() {
        let height = |label: &str| {
            let src = format!("flowchart LR\n  A -->|\"{label}\"| B\n");
            let (b, _) = edge_label_box(&draw(&flowchart::parse(&src).lower().flowchart));
            b.3 - b.1
        };
        let (short, tall) = (height("x"), height(r"$$\frac{a}{b}$$"));
        assert!(tall > short, "{tall} vs {short}");
    }

    #[test]
    fn well_formed_fixtures_have_no_diagnostics() {
        for name in [
            "math_node",
            "math_edge",
            "math_mixed",
            "math_unclosed",
            "math_empty",
            "math_br",
        ] {
            let l = lower(name);
            assert!(l.diagnostics.is_empty(), "{name}: {:?}", l.diagnostics);
        }
    }

    #[test]
    fn a_failing_formula_draws_its_source_with_one_diagnostic_on_its_span() {
        let src = fixture("math_fail");
        let l = flowchart::parse(&src).lower();
        let [d] = l.diagnostics.as_slice() else {
            panic!("one diagnostic, got {:?}", l.diagnostics);
        };
        let range = usize::from(d.range.start())..usize::from(d.range.end());
        assert_eq!(&src[range], r"\foo", "{d:?}");
        let t = texts(&draw(&l.flowchart)).join("|");
        assert!(t.contains(r"$$\foo$$"), "{t}");
    }

    #[test]
    fn a_diagnostic_lands_on_the_offending_command_in_the_source() {
        let src = "flowchart LR\n  A -->|\"$$x + \\bar$$ and $$\\frac{a}$$\"| B\n";
        let l = flowchart::parse(src).lower();
        assert_eq!(l.diagnostics.len(), 2, "{:?}", l.diagnostics);
        for d in &l.diagnostics {
            let range = usize::from(d.range.start())..usize::from(d.range.end());
            let at = &src[range.clone()];
            // Inside the delimiters of one formula.
            let before = &src[..range.start];
            let after = &src[range.end..];
            assert!(
                before.rsplit("$$").next().is_some_and(|s| !s.contains('"'))
                    && after.split("$$").next().is_some_and(|s| !s.contains('"')),
                "{at:?}"
            );
        }
    }

    #[test]
    fn a_formula_on_a_failing_edge_label_is_diagnosed_once_per_formula() {
        let src = "flowchart LR\n  A & B -->|\"$$\\foo$$\"| C\n";
        let l = flowchart::parse(src).lower();
        assert_eq!(l.diagnostics.len(), 1, "{:?}", l.diagnostics);
    }

    #[test]
    fn an_unclosed_delimiter_is_text_without_a_diagnostic() {
        let l = lower("math_unclosed");
        assert!(l.diagnostics.is_empty());
        let label = &l.flowchart.node("A").unwrap().label;
        assert!(!label.has_formula());
        assert!(texts(&draw(&l.flowchart)).contains(&"$$a"));
    }

    #[test]
    fn an_empty_formula_draws_nothing_and_does_not_fail() {
        let l = lower("math_empty");
        assert!(l.diagnostics.is_empty());
        let scene = draw(&l.flowchart);
        assert!(texts(&scene).iter().all(|t| !t.contains('$')));
    }

    #[test]
    fn ascii_formula_rows_stay_aligned_in_the_box() {
        let chart = flowchart::parse("flowchart TD\n  A[\"a $$\\frac{1}{2}$$ b\"]\n")
            .lower()
            .flowchart;
        let out = ascii::render(
            &chart,
            &Options {
                charset: Charset::Unicode,
            },
        )
        .unwrap();
        let rows: Vec<&str> = out.lines().collect();
        // A box of five rows: border, numerator, rule line with the text, denominator, border.
        assert_eq!(rows.len(), 5, "{out}");
        let width = |s: &str| s.chars().count();
        assert!(rows.iter().all(|r| width(r) == width(rows[0])), "{out}");
        assert!(rows[1].contains('1') && !rows[1].contains('a'), "{out}");
        assert!(rows[2].contains('a') && rows[2].contains('b'), "{out}");
        assert!(rows[3].contains('2'), "{out}");
        // The text row holds the fraction bar between `a` and `b`; the other rows are blank there.
        let col = rows[1].chars().position(|c| c == '1').unwrap();
        assert_eq!(rows[3].chars().position(|c| c == '2'), Some(col), "{out}");
    }

    #[test]
    fn ascii_edge_labels_and_failures_keep_working() {
        for name in [
            "math_node",
            "math_edge",
            "math_mixed",
            "math_br",
            "math_fail",
        ] {
            let chart = lower(name).flowchart;
            let out = ascii::render(
                &chart,
                &Options {
                    charset: Charset::Ascii,
                },
            )
            .unwrap();
            assert!(!out.is_empty(), "{name}");
        }
        let chart = lower("math_fail").flowchart;
        let out = ascii::render(
            &chart,
            &Options {
                charset: Charset::Unicode,
            },
        )
        .unwrap();
        assert!(out.contains(r"$$\foo$$"), "{out}");
    }

    #[test]
    fn a_wrapped_label_keeps_a_formula_whole() {
        let src = "flowchart LR\n  A[\"alpha beta gamma delta epsilon zeta eta theta iota kappa $$a + b + c$$ lambda mu\"]\n";
        let chart = flowchart::parse(src).lower().flowchart;
        let scene = draw(&chart);
        let (shape, formulas) = shape_and_formulas(&scene, Owner::Node(0));
        assert_inside(shape, &formulas, "wrapped");
        // The formula is one run: its glyph paths are all on rows of one line, never split by
        // the spaces inside `a + b + c`.
        let top = formulas.iter().map(|b| b.1).fold(f32::MAX, f32::min);
        let bottom = formulas.iter().map(|b| b.3).fold(f32::MIN, f32::max);
        assert!(bottom - top < 30.0, "{top} {bottom}");
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;
    use sirene_core::flowchart::Span;

    #[test]
    fn dollars_stay_literal_without_the_math_feature() {
        let l = lower("math_fail");
        assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
        let label = &l.flowchart.node("A").unwrap().label;
        let spans: Vec<Span> = label.runs().concat();
        assert_eq!(spans, [Span::plain(r"$$\foo$$")]);
        assert!(texts(&draw(&l.flowchart)).contains(&r"$$\foo$$"));
    }
}
