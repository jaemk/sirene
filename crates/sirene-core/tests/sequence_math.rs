//! `$$...$$` formulas in sequence diagram participant names, message text, and notes (LEMB-1,
//! LEMB-4, LEMB-5): a line is a row of text and formula runs on one baseline, drawn inside its
//! header, note, or lane, and a formula that does not typeset is drawn as its source with one
//! diagnostic.

use std::fs;
use std::path::Path;

use sirene_core::diagram::{self, Diagram};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};
use sirene_core::sequence::Sequence;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sequence")
        .join(format!("{name}.mmd"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn lower(src: &str) -> (Sequence, Vec<sirene_core::Diagnostic>) {
    let lowered = diagram::lower(src);
    match lowered.diagram {
        Diagram::Sequence(s) => (s, lowered.diagnostics),
        other => panic!("not a sequence diagram: {other:?}"),
    }
}

fn draw(seq: &Sequence) -> Scene {
    sirene_core::scene::sequence(seq, &Approx, &Theme::LIGHT)
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
fn a_diagram_without_a_formula_draws_the_same_when_read_as_text() {
    for text in [
        "plain",
        "a $ b $ c",
        "$$a",
        "a$$",
        "two<br>lines",
        "**bold** x",
    ] {
        let src = format!(
            "sequenceDiagram\n    participant 1\n    participant 2\n    1->>2: {text}\n    Note over 1,2: {text}\n"
        );
        let (mut seq, diagnostics) = lower(&src);
        assert!(diagnostics.is_empty(), "{text:?}: {diagnostics:?}");
        let with = draw(&seq);
        seq.math = None;
        assert_eq!(with, draw(&seq), "{text:?}");
    }
}

#[cfg(feature = "math")]
mod math {
    use super::*;
    use sirene_core::Severity;
    use sirene_core::ascii::{Charset, Options};
    use sirene_core::flowchart::{Delim, formula};
    use sirene_core::scene::{Owner, Rect, Segment};

    /// Left, top, right, bottom.
    type Bounds = (f32, f32, f32, f32);

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

    /// A box and the formula paths drawn in it.
    type Boxed = (Bounds, Vec<Bounds>);

    /// The boxes with `fill` of `owner`, each with the text-colored fills after it (a formula's
    /// glyphs and rules), in drawing order.
    fn boxes(scene: &Scene, owner: Owner, fill: sirene_core::scene::Color) -> Vec<Boxed> {
        let mut out: Vec<Boxed> = Vec::new();
        for e in scene.elements.iter().filter(|e| e.owner == owner) {
            let Primitive::Path {
                segments,
                fill: f,
                stroke,
                clip,
            } = &e.primitive
            else {
                continue;
            };
            if *f == Some(fill) && stroke.is_some() {
                if let Some(b) = bounds(segments, *clip) {
                    out.push((b, Vec::new()));
                }
            } else if *f == Some(Theme::LIGHT.text)
                && stroke.is_none()
                && let (Some(last), Some(b)) = (out.last_mut(), bounds(segments, *clip))
            {
                last.1.push(b);
            }
        }
        out
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
    fn well_formed_fixtures_have_no_diagnostics() {
        for name in [
            "math_participant",
            "math_message",
            "math_note",
            "math_mixed",
            "math_unclosed",
            "math_empty",
        ] {
            let (_, diagnostics) = lower(&fixture(name));
            assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
        }
    }

    #[test]
    fn the_model_keeps_the_source_text() {
        let (seq, _) = lower(&fixture("math_participant"));
        assert_eq!(seq.math, Some(Delim::Dollar));
        assert_eq!(seq.participants[0].label, [r"$$\alpha$$"]);
        let (seq, _) = lower(&fixture("math_mixed"));
        let m = seq.messages().next().unwrap();
        assert_eq!(m.text, [r"Area $$\pi r^2$$ and $$\frac{a}{b}$$ done"]);
        let runs = formula::runs(&m.text[0], Delim::Dollar);
        let shape: Vec<(&str, bool)> = runs.iter().map(|s| (s.text.as_str(), s.math)).collect();
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
    }

    #[test]
    fn a_header_contains_its_formula_in_both_places() {
        let (seq, _) = lower(&fixture("math_participant"));
        let scene = draw(&seq);
        let headers = boxes(&scene, Owner::Node(0), Theme::LIGHT.node_fill);
        // The header at the top and the one at the bottom.
        assert_eq!(headers.len(), 2);
        for (shape, inside) in &headers {
            assert_inside(*shape, inside, "header");
        }
        assert!(texts(&scene).iter().all(|t| !t.contains('$')));
    }

    #[test]
    fn a_note_contains_its_formula() {
        let (seq, _) = lower(&fixture("math_note"));
        let scene = draw(&seq);
        let notes = boxes(&scene, Owner::Diagram, Theme::LIGHT.cluster_fill);
        let [(shape, inside)] = notes.as_slice() else {
            panic!("one note, got {notes:?}");
        };
        assert_inside(*shape, inside, "note");
    }

    #[test]
    fn a_message_label_sits_above_its_arrow_between_the_lifelines() {
        for name in ["math_message", "math_mixed"] {
            let (seq, _) = lower(&fixture(name));
            let scene = draw(&seq);
            // The head and the line are drawn first, then the label.
            let line = scene
                .elements
                .iter()
                .position(|e| {
                    e.owner == Owner::Edge(0)
                        && matches!(
                            &e.primitive,
                            Primitive::Path {
                                stroke: Some(_),
                                ..
                            }
                        )
                })
                .expect("the message has a line");
            let formulas: Vec<Bounds> = scene.elements[line..]
                .iter()
                .filter(|e| e.owner == Owner::Edge(0))
                .filter_map(|e| match &e.primitive {
                    Primitive::Path {
                        segments,
                        fill,
                        stroke,
                        clip,
                    } if *fill == Some(Theme::LIGHT.text) && stroke.is_none() => {
                        bounds(segments, *clip)
                    }
                    _ => None,
                })
                .collect();
            // The arrow's line: the one stroked path of the message.
            let arrow = scene
                .elements
                .iter()
                .filter(|e| e.owner == Owner::Edge(0))
                .find_map(|e| match &e.primitive {
                    Primitive::Path {
                        segments, stroke, ..
                    } if stroke.is_some() => bounds(segments, None),
                    _ => None,
                })
                .expect("the message has an arrow");
            assert!(!formulas.is_empty(), "{name}");
            for b in &formulas {
                assert!(
                    b.3 <= arrow.1 + 1.0,
                    "{name}: {b:?} is below the arrow {arrow:?}"
                );
                assert!(
                    b.0 >= arrow.0 - 1.0 && b.2 <= arrow.2 + 1.0,
                    "{name}: {b:?} leaves the lane {arrow:?}"
                );
            }
        }
    }

    #[test]
    fn text_runs_keep_their_text_beside_formulas() {
        let (seq, _) = lower(&fixture("math_mixed"));
        let scene = draw(&seq);
        let t = texts(&scene);
        for want in ["Area", "and", "done", "self"] {
            assert!(t.contains(&want), "{want} in {t:?}");
        }
        assert!(t.iter().all(|t| !t.contains('$')), "{t:?}");
    }

    #[test]
    fn a_tall_formula_makes_its_message_row_taller_and_lane_wider() {
        let size = |text: &str| {
            let src = format!(
                "sequenceDiagram\n    participant 1\n    participant 2\n    1->>2: {text}\n    2->>1: back\n"
            );
            let (seq, _) = lower(&src);
            let scene = draw(&seq);
            (scene.width, scene.height)
        };
        let (plain_w, plain_h) = size("x");
        let (_, tall_h) = size(r"$$\frac{a}{b}$$");
        assert!(tall_h > plain_h, "{tall_h} vs {plain_h}");
        let (wide_w, _) = size(r"$$a + b + c + d + e + f + g + h + i + j + k + l + m + n$$");
        assert!(wide_w > plain_w, "{wide_w} vs {plain_w}");
    }

    #[test]
    fn a_tall_header_formula_makes_the_header_taller() {
        let height = |name: &str| {
            let src = format!("sequenceDiagram\n    participant 1 as {name}\n    1->>1: x\n");
            let (seq, _) = lower(&src);
            let scene = draw(&seq);
            let (shape, _) = boxes(&scene, Owner::Node(0), Theme::LIGHT.node_fill)[0].clone();
            shape.3 - shape.1
        };
        assert!(height(r"$$\frac{a}{b}$$") > height("x"));
    }

    #[test]
    fn a_failing_formula_draws_its_source_with_one_diagnostic_on_its_span() {
        let src = fixture("math_fail");
        let (seq, diagnostics) = lower(&src);
        let [d] = diagnostics.as_slice() else {
            panic!("one diagnostic, got {diagnostics:?}");
        };
        assert_eq!(d.severity, Severity::Error);
        let range = usize::from(d.range.start())..usize::from(d.range.end());
        assert_eq!(&src[range], r"\foo", "{d:?}");
        let t = texts(&draw(&seq)).join("|");
        assert!(t.contains(r"$$\foo$$"), "{t}");
    }

    #[test]
    fn a_failing_alias_and_note_are_diagnosed_on_their_spans() {
        let src = "sequenceDiagram\n    participant 1 as $$\\foo$$\n    participant 2\n    Note over 1,2: ok $$x$$ and $$\\baz$$\n";
        let (seq, diagnostics) = lower(src);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let at: Vec<&str> = diagnostics
            .iter()
            .map(|d| &src[usize::from(d.range.start())..usize::from(d.range.end())])
            .collect();
        assert_eq!(at, [r"\foo", r"\baz"]);
        let scene = draw(&seq);
        let t = texts(&scene);
        assert!(
            t.contains(&r"$$\foo$$") && t.contains(&r"$$\baz$$"),
            "{t:?}"
        );
    }

    #[test]
    fn a_double_backslash_is_read_as_one() {
        let src =
            "sequenceDiagram\n    participant 1\n    participant 2\n    1->>2: $$\\\\alpha$$\n";
        let (seq, diagnostics) = lower(src);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let runs = formula::runs(&seq.messages().next().unwrap().text[0], Delim::Dollar);
        assert_eq!(runs[0].formula().as_deref(), Some(r"\alpha"));
    }

    #[test]
    fn an_unclosed_delimiter_is_text_without_a_diagnostic() {
        let (seq, diagnostics) = lower(&fixture("math_unclosed"));
        assert!(diagnostics.is_empty());
        assert!(texts(&draw(&seq)).contains(&"open $$a"));
    }

    #[test]
    fn an_empty_formula_draws_nothing_and_does_not_fail() {
        let (seq, diagnostics) = lower(&fixture("math_empty"));
        assert!(diagnostics.is_empty());
        assert!(texts(&draw(&seq)).iter().all(|t| !t.contains('$')));
    }

    fn ascii(seq: &Sequence) -> String {
        sirene_core::ascii::sequence(
            seq,
            &Options {
                charset: Charset::Unicode,
            },
        )
    }

    #[test]
    fn ascii_formula_rows_stay_aligned_and_the_lanes_line_up() {
        let src = "sequenceDiagram\n    participant 1\n    participant 2\n    1->>2: a $$\\frac{1}{2}$$ b\n    2->>1: back\n";
        let (seq, _) = lower(src);
        let out = ascii(&seq);
        let rows: Vec<Vec<char>> = out.lines().map(|r| r.chars().collect()).collect();
        let text = |i: usize| rows[i].iter().collect::<String>();
        // The label is three rows: numerator, the text row, denominator.
        let first = rows
            .iter()
            .position(|r| r.contains(&'a'))
            .expect("the text row");
        assert!(text(first).contains('b'), "{out}");
        assert!(text(first - 1).contains('1'), "{out}");
        assert!(text(first + 1).contains('2'), "{out}");
        let col = |i: usize, c: char| rows[i].iter().position(|&x| x == c).unwrap();
        assert_eq!(col(first - 1, '1'), col(first + 1, '2'), "{out}");
        // Both lifelines run unbroken from the headers to the footers: the cell of each lifeline
        // column is not blank on any row between.
        let header = rows
            .iter()
            .position(|r| r.contains(&'└'))
            .expect("the headers' bottom border");
        let lanes: Vec<usize> = (0..rows[header + 1].len())
            .filter(|&c| rows[header + 1][c] == '│')
            .collect();
        assert_eq!(lanes.len(), 2, "{out}");
        let footer = rows.iter().rposition(|r| r.contains(&'┌')).unwrap();
        for (y, row) in rows.iter().enumerate().take(footer).skip(header + 1) {
            for &c in &lanes {
                assert!(
                    row.get(c).is_some_and(|&ch| ch != ' '),
                    "row {y} col {c} is blank:\n{out}"
                );
            }
        }
    }

    #[test]
    fn ascii_headers_notes_and_failures_work() {
        for name in [
            "math_participant",
            "math_message",
            "math_note",
            "math_mixed",
            "math_fail",
            "math_unclosed",
            "math_empty",
        ] {
            let (seq, _) = lower(&fixture(name));
            assert!(!ascii(&seq).is_empty(), "{name}");
        }
        let (seq, _) = lower(&fixture("math_fail"));
        assert!(ascii(&seq).contains(r"$$\foo$$"));
        let (seq, _) = lower(&fixture("math_participant"));
        assert!(!ascii(&seq).contains('$'));
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    #[test]
    fn dollars_stay_literal_without_the_math_feature() {
        let (seq, diagnostics) = lower(&fixture("math_fail"));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(texts(&draw(&seq)).contains(&"bad $$\\foo$$"));
        let (seq, _) = lower(&fixture("math_participant"));
        assert!(texts(&draw(&seq)).contains(&r"$$\alpha$$"));
    }
}
