//! `<latex>...</latex>` formulas in PlantUML label text (LEMB-2, PCREO-4): inline runs on a
//! common baseline in sequence participants, messages, and notes and in the labels of the
//! graph-lowered kinds, typeset in text style; `$$` and `<math>` stay text, and a formula that
//! does not typeset is drawn as its source with one diagnostic on its body.

use std::fs;
use std::path::Path;

use sirene_core::diagram::{self, Diagram};
use sirene_core::flowchart::{Delim, Flowchart};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};
use sirene_core::sequence::Sequence;

fn fixture(dir: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plantuml")
        .join(dir)
        .join(format!("{name}.puml"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn valid(name: &str) -> String {
    fixture("valid", name)
}

fn sequence(src: &str) -> (Sequence, Vec<sirene_core::Diagnostic>) {
    let lowered = diagram::lower(src);
    match lowered.diagram {
        Diagram::Sequence(s) => (s, lowered.diagnostics),
        other => panic!("not a sequence diagram: {other:?}"),
    }
}

fn graph(src: &str) -> (Flowchart, Vec<sirene_core::Diagnostic>) {
    let lowered = diagram::lower(src);
    match lowered.diagram {
        Diagram::Graph(g) => (g, lowered.diagnostics),
        other => panic!("not a graph: {other:?}"),
    }
}

fn draw_sequence(seq: &Sequence) -> Scene {
    sirene_core::scene::sequence(seq, &Approx, &Theme::LIGHT)
}

fn draw_graph(chart: &Flowchart) -> Scene {
    sirene_core::scene::flowchart(chart, &Approx, &Theme::LIGHT).unwrap()
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

/// The documents with no formula that draw the same as text: `$$` and `<math>` are not
/// formulas in PlantUML, and neither is an unclosed or empty `<latex>` tag.
#[test]
fn text_without_a_latex_tag_is_unchanged() {
    for text in [
        "plain",
        "$$x^2$$",
        "<math>x</math>",
        "<latex>unclosed",
        "<latex></latex>",
        "a <b>bold</b> c",
    ] {
        let src = format!("@startuml\nA -> B : {text}\nnote over A, B : {text}\n@enduml\n");
        let (mut seq, d) = sequence(&src);
        assert!(d.is_empty(), "{text:?}: {d:?}");
        let with = draw_sequence(&seq);
        seq.math = None;
        assert_eq!(with, draw_sequence(&seq), "{text:?}");
        let src = format!("@startuml\nstart\n:{text};\nstop\n@enduml\n");
        let (g, d) = graph(&src);
        assert!(d.is_empty(), "{text:?}: {d:?}");
        assert!(g.nodes.iter().all(|n| !n.label.has_formula()), "{text:?}");
        // `$$x^2$$` would be a formula with Mermaid's delimiters; here it is text.
        let scene = draw_graph(&g);
        assert!(
            texts(&scene).iter().any(|t| t.contains(text.trim())),
            "{text:?}: {:?}",
            texts(&scene)
        );
    }
}

#[test]
fn the_models_know_their_delimiters() {
    let (seq, _) = sequence(&valid("latex_inline_message"));
    assert_eq!(seq.math, Some(Delim::Latex));
    for src in [
        valid("latex_inline_activity"),
        valid("latex_inline_class"),
        valid("latex_inline_state"),
        valid("latex_inline_component"),
    ] {
        let (g, _) = graph(&src);
        assert!(!g.nodes.is_empty());
        assert!(g.nodes.iter().all(|n| n.label.delim == Delim::Latex));
        assert!(
            g.edges
                .iter()
                .flat_map(|e| &e.label)
                .all(|l| l.delim == Delim::Latex)
        );
    }
}

#[test]
fn a_formula_keeps_a_literal_backslash_n() {
    let (seq, d) = sequence(&valid("latex_inline_message"));
    assert!(d.is_empty(), "{d:?}");
    let texts: Vec<_> = seq.messages().map(|m| m.text.clone()).collect();
    assert_eq!(texts[1], [r"<latex>\neq 0</latex> only"]);
    // Outside a formula `\n` still breaks the line.
    let (seq, _) = sequence("@startuml\nA -> B : one\\ntwo <latex>\\nu</latex>\n@enduml\n");
    assert_eq!(
        seq.messages().next().unwrap().text,
        ["one", r"two <latex>\nu</latex>"]
    );
    let (g, _) = graph("@startuml\nstart\n:a\\nb <latex>\\neq</latex>;\nstop\n@enduml\n");
    let label = g
        .nodes
        .iter()
        .find(|n| n.label.text.contains("neq"))
        .unwrap();
    assert_eq!(label.label.text, "a\nb <latex>\\neq</latex>");
}

#[cfg(feature = "math")]
mod math {
    use super::*;
    use sirene_core::Severity;
    use sirene_core::ascii::{Charset, Options};
    use sirene_core::flowchart::formula;
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

    /// The paths of node `i`: the first stroked one is its shape, the others its formulas.
    fn node_paths(scene: &Scene, i: usize) -> (Bounds, Vec<Bounds>) {
        let mut paths = scene
            .elements
            .iter()
            .filter(|e| e.owner == Owner::Node(i))
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
            .expect("the node has a shape");
        (
            shape.expect("the shape has points"),
            paths.filter_map(|(b, _)| b).collect(),
        )
    }

    fn no_markup(scene: &Scene, what: &str) {
        let t = texts(scene);
        assert!(
            t.iter().all(|t| !t.to_ascii_lowercase().contains("latex>")),
            "{what}: {t:?}"
        );
    }

    #[test]
    fn well_formed_fixtures_have_no_diagnostics() {
        for name in [
            "latex_inline_message",
            "latex_inline_note",
            "latex_inline_activity",
            "latex_inline_class",
            "latex_inline_state",
            "latex_inline_component",
            "latex_inline_case",
            "latex_inline_literal",
        ] {
            let d = diagram::lower(&valid(name)).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    #[test]
    fn the_model_keeps_the_source_text_and_runs_split_it() {
        let (seq, _) = sequence(&valid("latex_inline_message"));
        assert_eq!(seq.participants[0].label, [r"<latex>\alpha</latex>"]);
        let m = seq.messages().next().unwrap();
        assert_eq!(m.text, [r"Can you solve: <latex>\frac{a}{b} + x^2</latex>"]);
        let runs = formula::runs(&m.text[0], Delim::Latex);
        let shape: Vec<(&str, bool)> = runs.iter().map(|s| (s.text.as_str(), s.math)).collect();
        assert_eq!(
            shape,
            [
                ("Can you solve: ", false),
                (r"<latex>\frac{a}{b} + x^2</latex>", true),
            ]
        );
        assert_eq!(runs[1].formula().as_deref(), Some(r"\frac{a}{b} + x^2"));
        // `$$` is not a delimiter here.
        assert!(!formula::has("$$x$$", Delim::Latex));
        assert!(formula::has("$$x$$", Delim::Dollar));
    }

    #[test]
    fn a_participant_header_contains_its_formula_in_both_places() {
        let (seq, _) = sequence(&valid("latex_inline_message"));
        let scene = draw_sequence(&seq);
        let headers = boxes(&scene, Owner::Node(0), Theme::LIGHT.node_fill);
        assert_eq!(headers.len(), 2);
        for (shape, inside) in &headers {
            assert_inside(*shape, inside, "header");
        }
        no_markup(&scene, "message");
    }

    #[test]
    fn a_note_with_text_and_two_tags_contains_both_formulas() {
        let (seq, d) = sequence(&valid("latex_inline_note"));
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_sequence(&seq);
        let notes = boxes(&scene, Owner::Diagram, Theme::LIGHT.cluster_fill);
        assert_eq!(notes.len(), 2, "{notes:?}");
        // The first note holds two formulas and a plain second line.
        let (shape, inside) = &notes[0];
        assert!(inside.len() >= 2, "{inside:?}");
        assert_inside(*shape, inside, "note over");
        let (shape, inside) = &notes[1];
        assert_inside(*shape, inside, "note right");
        let t = texts(&scene);
        for want in ["Given", "and", "plain second line", "area"] {
            assert!(
                t.iter().any(|x| x.contains(want)),
                "{want} not in text {t:?}"
            );
        }
        no_markup(&scene, "note");
    }

    #[test]
    fn graph_labels_contain_their_formulas() {
        for (name, nodes) in [
            ("latex_inline_activity", vec!["Compute"]),
            ("latex_inline_state", vec!["Idle"]),
            ("latex_inline_class", vec!["Order"]),
        ] {
            let src = valid(name);
            let (chart, d) = graph(&src);
            assert!(d.is_empty(), "{name}: {d:?}");
            let scene = draw_graph(&chart);
            for want in nodes {
                let i = chart
                    .nodes
                    .iter()
                    .position(|n| n.label.text.contains(want) || n.id == want)
                    .unwrap_or_else(|| panic!("{name}: no node {want}"));
                if chart.nodes[i].label.has_formula() {
                    let (shape, formulas) = node_paths(&scene, i);
                    assert_inside(shape, &formulas, &format!("{name} {want}"));
                }
            }
            no_markup(&scene, name);
        }
    }

    #[test]
    fn the_activity_node_formula_is_drawn_not_its_source() {
        let (chart, _) = graph(&valid("latex_inline_activity"));
        let i = chart
            .nodes
            .iter()
            .position(|n| n.label.text.contains("Compute"))
            .unwrap();
        assert!(chart.nodes[i].label.has_formula());
        let scene = draw_graph(&chart);
        let (shape, formulas) = node_paths(&scene, i);
        assert_inside(shape, &formulas, "activity");
        assert!(texts(&scene).contains(&"Compute"), "{:?}", texts(&scene));
    }

    #[test]
    fn edge_notes_and_component_labels_draw_formulas() {
        for name in [
            "latex_inline_class",
            "latex_inline_state",
            "latex_inline_component",
        ] {
            let (chart, _) = graph(&valid(name));
            let scene = draw_graph(&chart);
            let formula_edges = chart
                .edges
                .iter()
                .filter(|e| e.label.as_ref().is_some_and(|l| l.has_formula()))
                .count();
            assert!(formula_edges > 0, "{name}");
            // An arrowhead is one filled path; a formula is several.
            let drawn = (0..chart.edges.len())
                .filter(|&k| {
                    scene
                        .elements
                        .iter()
                        .filter(|e| {
                            e.owner == Owner::Edge(k)
                                && matches!(
                                    &e.primitive,
                                    Primitive::Path { fill: Some(f), stroke: None, .. }
                                        if *f == Theme::LIGHT.text
                                )
                        })
                        .count()
                        > 1
                })
                .count();
            assert_eq!(drawn, formula_edges, "{name}");
            no_markup(&scene, name);
        }
    }

    #[test]
    fn tags_match_in_any_case() {
        let (seq, d) = sequence(&valid("latex_inline_case"));
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_sequence(&seq);
        no_markup(&scene, "case");
        let drawn = scene
            .elements
            .iter()
            .filter(|e| {
                matches!(&e.primitive, Primitive::Path { fill: Some(f), stroke: None, .. }
                    if *f == Theme::LIGHT.text)
            })
            .count();
        assert!(drawn > 0);
        let m = seq.messages().next().unwrap();
        let runs = formula::runs(&m.text[0], Delim::Latex);
        assert_eq!(runs.iter().filter(|s| s.math).count(), 2);
        assert_eq!(runs[0].formula().as_deref(), Some(r"\alpha"));
    }

    #[test]
    fn unclosed_empty_dollar_and_math_tags_stay_literal_without_a_diagnostic() {
        let (seq, d) = sequence(&valid("latex_inline_literal"));
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_sequence(&seq);
        let t = texts(&scene);
        for want in [
            r"<latex>\alpha unclosed",
            "$$x^2$$ stays text",
            "<latex></latex> empty and <math>x</math> literal",
        ] {
            assert!(t.contains(&want), "{want} in {t:?}");
        }
        // Read as text, the diagram draws the same.
        let mut as_text = seq.clone();
        as_text.math = None;
        assert_eq!(scene, draw_sequence(&as_text));
    }

    /// Compartment rows (class members, state bodies) and the multiplicities at the ends of an
    /// edge are drawn as written.
    #[test]
    fn rows_and_end_labels_are_not_formulas() {
        let src = "@startuml\nclass A {\n  +f : <latex>x</latex>\n}\nclass B\nA \"<latex>n</latex>\" --> B\n@enduml\n";
        let (chart, d) = graph(src);
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_graph(&chart);
        let t = texts(&scene);
        assert!(t.iter().any(|x| x.contains("<latex>x</latex>")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("<latex>n</latex>")), "{t:?}");
    }

    /// Mermaid sets a formula in display style (a sum's limits above and below, taller) and
    /// PlantUML in text style (limits beside).
    #[test]
    fn plantuml_formulas_are_set_inline() {
        let formula = r"\sum_{i=0}^{n} x_i";
        let (plantuml, _) = sequence(&format!(
            "@startuml\nA -> B : <latex>{formula}</latex>\nB -> A : back\n@enduml\n"
        ));
        let mermaid = diagram::lower(&format!(
            "sequenceDiagram\n    participant A\n    participant B\n    A->>B: $${formula}$$\n    B->>A: back\n"
        ));
        let Diagram::Sequence(mermaid) = mermaid.diagram else {
            panic!("not a sequence diagram");
        };
        let (p, m) = (draw_sequence(&plantuml), draw_sequence(&mermaid));
        assert!(
            p.height < m.height,
            "text style {} is not shorter than display style {}",
            p.height,
            m.height
        );
    }

    #[test]
    fn a_tall_formula_makes_its_row_taller() {
        let height = |text: &str| {
            let (seq, _) = sequence(&format!(
                "@startuml\nA -> B : {text}\nB -> A : back\n@enduml\n"
            ));
            draw_sequence(&seq).height
        };
        assert!(height(r"<latex>\frac{a}{b}</latex>") > height("x"));
    }

    #[test]
    fn a_failing_formula_draws_its_source_with_one_diagnostic_on_its_body() {
        let src = fixture("errors", "latex_inline_fail");
        let (seq, d) = sequence(&src);
        let [d] = d.as_slice() else {
            panic!("one diagnostic, got {d:?}");
        };
        assert_eq!(d.severity, Severity::Error);
        let range = usize::from(d.range.start())..usize::from(d.range.end());
        assert_eq!(&src[range], r"\foo", "{d:?}");
        let t = texts(&draw_sequence(&seq)).join("|");
        assert!(t.contains(r"<latex>a + \foo{x}</latex>"), "{t}");
        // The formula before it typeset.
        let drawn = draw_sequence(&seq).elements.iter().any(|e| {
            matches!(&e.primitive, Primitive::Path { fill: Some(f), stroke: None, .. }
                    if *f == Theme::LIGHT.text)
        });
        assert!(drawn);
    }

    #[test]
    fn a_failing_graph_label_is_diagnosed_once_on_its_line() {
        let src = fixture("errors", "latex_inline_fail_activity");
        let lowered = diagram::lower(&src);
        let [d] = lowered.diagnostics.as_slice() else {
            panic!("one diagnostic, got {:?}", lowered.diagnostics);
        };
        assert_eq!(d.severity, Severity::Error);
        let (line, _) = d.line_col(&src);
        assert_eq!(line, 3, "{d:?}");
        let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
        assert!(r"\bar{".contains(at.trim()) || at.is_empty(), "{at:?}");
        let Diagram::Graph(chart) = lowered.diagram else {
            panic!("not a graph");
        };
        let scene = draw_graph(&chart);
        assert!(
            texts(&scene)
                .iter()
                .any(|t| t.contains(r"<latex>\bar{</latex>")),
            "{:?}",
            texts(&scene)
        );
    }

    #[test]
    fn equal_failing_formulas_are_each_diagnosed_on_their_own_place() {
        let src =
            "@startuml\nstart\n:a <latex>\\foo</latex>;\n:b <latex>\\foo</latex>;\nstop\n@enduml\n";
        let d = diagram::lower(src).diagnostics;
        assert_eq!(d.len(), 2, "{d:?}");
        assert_ne!(d[0].range, d[1].range);
        for x in &d {
            assert_eq!(
                &src[usize::from(x.range.start())..usize::from(x.range.end())],
                r"\foo"
            );
        }
    }

    #[test]
    fn failing_notes_and_participants_are_diagnosed_on_their_spans() {
        let src = "@startuml\nparticipant \"<latex>\\foo</latex>\" as A\nA -> A : ok <latex>x</latex>\nnote over A\n  fine\n  <latex>\\baz</latex> bad\nend note\n@enduml\n";
        let (seq, d) = sequence(src);
        let at: Vec<&str> = d
            .iter()
            .map(|d| &src[usize::from(d.range.start())..usize::from(d.range.end())])
            .collect();
        assert_eq!(at, [r"\foo", r"\baz"], "{d:?}");
        let scene = draw_sequence(&seq);
        let t = texts(&scene);
        assert!(
            t.contains(&r"<latex>\foo</latex>") || t.iter().any(|x| x.contains(r"\foo")),
            "{t:?}"
        );
    }

    fn ascii_rows(src: &str) -> Vec<Vec<char>> {
        let out = diagram::lower(src)
            .diagram
            .ascii(&Options {
                charset: Charset::Unicode,
            })
            .unwrap();
        out.lines().map(|r| r.chars().collect()).collect()
    }

    #[test]
    fn ascii_rows_stay_aligned_in_a_sequence() {
        let rows = ascii_rows(
            "@startuml\nA -> B : a <latex>\\frac{1}{2}</latex> b\nB -> A : back\n@enduml\n",
        );
        let text = |i: usize| rows[i].iter().collect::<String>();
        let first = rows
            .iter()
            .position(|r| r.contains(&'a') && r.contains(&'b'))
            .expect("the text row");
        assert!(text(first - 1).contains('1'), "{rows:?}");
        assert!(text(first + 1).contains('2'), "{rows:?}");
        let col = |i: usize, c: char| rows[i].iter().position(|&x| x == c).unwrap();
        assert_eq!(col(first - 1, '1'), col(first + 1, '2'));
        assert!(rows.iter().all(|r| r.iter().all(|&c| c != '<')));
    }

    #[test]
    fn ascii_rows_stay_aligned_in_a_graph() {
        let rows =
            ascii_rows("@startuml\nstart\n:x <latex>\\frac{1}{2}</latex> y;\nstop\n@enduml\n");
        let text = |i: usize| rows[i].iter().collect::<String>();
        let first = rows
            .iter()
            .position(|r| r.contains(&'x') && r.contains(&'y'))
            .expect("the text row");
        assert!(text(first - 1).contains('1'), "{rows:?}");
        assert!(text(first + 1).contains('2'), "{rows:?}");
        let col = |i: usize, c: char| rows[i].iter().position(|&x| x == c).unwrap();
        assert_eq!(col(first - 1, '1'), col(first + 1, '2'));
        // The box borders on the text row and the rows around it line up.
        let widths: Vec<usize> = (first - 1..=first + 1).map(|i| rows[i].len()).collect();
        assert!(widths.iter().all(|&w| w == widths[0]), "{widths:?}");
    }

    #[test]
    fn every_inline_fixture_draws_and_renders() {
        for name in [
            "latex_inline_message",
            "latex_inline_note",
            "latex_inline_activity",
            "latex_inline_class",
            "latex_inline_state",
            "latex_inline_component",
            "latex_inline_case",
            "latex_inline_literal",
        ] {
            let d = diagram::lower(&valid(name)).diagram;
            for theme in [Theme::LIGHT, Theme::DARK] {
                assert!(
                    !d.scene(&Approx, &theme).unwrap().elements.is_empty(),
                    "{name}"
                );
            }
            let rows = d.ascii(&Options::default()).unwrap();
            assert!(
                !rows.contains("latex>") || name == "latex_inline_literal",
                "{name}: {rows}"
            );
        }
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    #[test]
    fn tags_stay_literal_without_the_math_feature() {
        let src = fixture("errors", "latex_inline_fail");
        let (seq, d) = sequence(&src);
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_sequence(&seq);
        let t = texts(&scene);
        assert!(
            t.iter().any(|x| x.contains(r"<latex>a + \foo{x}</latex>")),
            "{t:?}"
        );
        let (chart, d) = graph(&fixture("errors", "latex_inline_fail_activity"));
        assert!(d.is_empty(), "{d:?}");
        assert!(
            texts(&draw_graph(&chart))
                .iter()
                .any(|x| x.contains("<latex>"))
        );
        let (seq, d) = sequence(&valid("latex_inline_message"));
        assert!(d.is_empty(), "{d:?}");
        assert!(texts(&draw_sequence(&seq)).contains(&r"<latex>\alpha</latex>"));
    }
}
