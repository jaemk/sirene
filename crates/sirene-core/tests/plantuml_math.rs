//! `<latex>...</latex>` (TeX) and `<math>...</math>` (AsciiMath) formulas in PlantUML label text
//! (LEMB-2, PCREO-4): inline runs on a common baseline in sequence participants, messages, and
//! notes and in the labels of the graph-lowered kinds, typeset in text style; `$$` stays text,
//! and a formula that does not typeset is drawn as its source with one diagnostic on its body.

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

/// The documents with no formula that draw the same as text: `$$` is not a formula in
/// PlantUML, and neither is an unclosed or empty `<latex>` or `<math>` tag.
#[test]
fn text_without_a_latex_tag_is_unchanged() {
    for text in [
        "plain",
        "$$x^2$$",
        "<math>unclosed",
        "<math></math>",
        "<math>  </math>",
        "<mathx>x</mathx>",
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
    // Without the `math` feature each formula is a warning; with it, none is wrong.
    assert!(
        d.iter()
            .all(|d| d.severity == sirene_core::Severity::Warning),
        "{d:?}"
    );
    assert!(cfg!(not(feature = "math")) || d.is_empty(), "{d:?}");
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
            "latex_inline_members",
            "latex_inline_end_labels",
            "latex_inline_package",
            "latex_inline_state_body",
        ] {
            let d = diagram::lower(&valid(name)).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    #[test]
    fn well_formed_math_fixtures_have_no_diagnostics() {
        for name in [
            "math_inline_message",
            "math_inline_activity",
            "math_inline_mixed",
            "math_fraction",
            "math_sum",
            "math_matrix",
            "math_lines",
        ] {
            let d = diagram::lower(&valid(name)).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    /// PCREO-2: a `<math>` tag is AsciiMath, a `<latex>` tag TeX, in the same line and label.
    #[test]
    fn a_math_tag_is_an_asciimath_run_beside_tex_runs() {
        let (seq, _) = sequence(&valid("math_inline_mixed"));
        assert_eq!(seq.math, Some(Delim::Latex));
        assert_eq!(seq.participants[0].label, ["<math>alpha</math>"]);
        let m = seq.messages().next().unwrap();
        let runs = formula::runs(&m.text[0], Delim::Latex);
        let shape: Vec<(&str, bool, bool)> = runs
            .iter()
            .map(|s| (s.text.as_str(), s.math, s.is_asciimath()))
            .collect();
        assert_eq!(
            shape,
            [
                ("<math>a/b</math>", true, true),
                (" and ", false, false),
                (r"<latex>\frac{a}{b}</latex>", true, false),
                (" and ", false, false),
                ("<MATH>sqrt x</MATH>", true, true),
            ]
        );
        assert_eq!(runs[0].formula().as_deref(), Some("a/b"));
        assert_eq!(runs[4].formula().as_deref(), Some("sqrt x"));
        // An unclosed tag is text.
        let m = seq.messages().nth(1).unwrap();
        assert!(!formula::has(&m.text[0], Delim::Latex), "{:?}", m.text);
        // `<math>` is nothing in Mermaid's `$$` mode.
        assert!(!formula::has("<math>x</math>", Delim::Dollar));
    }

    /// The AsciiMath `a/b` and the TeX `\frac{a}{b}` are the same formula, so they take the same
    /// room, and neither leaves its tags in the text.
    #[test]
    fn asciimath_and_tex_runs_set_alike() {
        let size = |tag: &str, body: &str| {
            let src = format!("@startuml\nA -> B : <{tag}>{body}</{tag}>\n@enduml\n");
            let (seq, d) = sequence(&src);
            assert!(d.is_empty(), "{d:?}");
            let scene = draw_sequence(&seq);
            no_markup(&scene, tag);
            assert!(
                texts(&scene).iter().all(|t| !t.contains("math>")),
                "{:?}",
                texts(&scene)
            );
            (scene.width, scene.height)
        };
        let (aw, ah) = size("math", "a/b");
        let (tw, th) = size("latex", r"\frac{a}{b}");
        assert!(
            (aw - tw).abs() < 0.5 && (ah - th).abs() < 0.5,
            "{aw}x{ah} {tw}x{th}"
        );
        // A formula is taller than the text line it replaces.
        let (_, plain) = size("b", "a b");
        assert!(ah > plain, "{ah} {plain}");
    }

    #[test]
    fn math_formulas_draw_in_notes_and_graph_labels() {
        let (chart, d) = graph(&valid("math_inline_activity"));
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_graph(&chart);
        let formula_nodes: Vec<usize> = chart
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.label.has_formula())
            .map(|(i, _)| i)
            .collect();
        assert!(formula_nodes.len() >= 2, "{formula_nodes:?}");
        // The node's shape and the filled formula paths after it. The note beside the last node
        // draws its formulas as part of that node's paths, so only the first node is held to
        // contain all of them; the others to contain the first.
        for (k, &i) in formula_nodes.iter().enumerate() {
            let found = boxes(&scene, Owner::Node(i), Theme::LIGHT.node_fill);
            let [(shape, formulas)] = &found[..] else {
                panic!("node {i}: {found:?}");
            };
            let held = if k == 0 { formulas.len() } else { 1 };
            assert_inside(
                *shape,
                &formulas[..held],
                &format!("activity node {i} {:?}", chart.nodes[i].label.text),
            );
        }
        let t = texts(&scene);
        assert!(t.iter().all(|x| !x.contains("math>")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("Try also")), "{t:?}");
    }

    /// LEMB-5: a failing `<math>` formula is drawn as its source, with one diagnostic on the
    /// AsciiMath that made the TeX fail.
    #[test]
    fn a_failing_math_formula_has_one_diagnostic_on_its_source() {
        let src = fixture("errors", "math_inline_fail");
        let lowered = diagram::lower(&src);
        assert_eq!(lowered.diagnostics.len(), 1, "{:?}", lowered.diagnostics);
        let d = &lowered.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("Double superscript"), "{}", d.message);
        let (from, to) = (usize::from(d.range.start()), usize::from(d.range.end()));
        assert_eq!(&src[from..to], "^");
        // The second `^` of `x^1^2`.
        assert_eq!(&src[from - 3..from + 2], "x^1^2");
        let Diagram::Sequence(seq) = lowered.diagram else {
            panic!("not a sequence diagram");
        };
        let scene = draw_sequence(&seq);
        assert!(
            texts(&scene).contains(&"<math>a + x^1^2 + c</math>"),
            "{:?}",
            texts(&scene)
        );
        // The formula that does typeset is not diagnosed.
        assert!(!texts(&scene).iter().any(|t| t.contains("ok <math>")));
    }

    #[test]
    fn math_rows_are_aligned_in_text() {
        let out = diagram::lower(&valid("math_inline_message"))
            .diagram
            .ascii(&Options {
                charset: Charset::Unicode,
            })
            .unwrap();
        assert!(out.contains("ax\u{b2} + bx + c = 0"), "{out}");
        assert!(
            out.contains("\u{2212}b \u{b1} \u{221a}b\u{b2} \u{2212} 4ac"),
            "{out}"
        );
        assert!(!out.contains("math>"), "{out}");
        // The fraction is three rows: its bar sits between the numerator and the denominator.
        let rows: Vec<&str> = out.lines().collect();
        let bar = rows.iter().position(|r| r.contains("x = ---")).unwrap();
        assert!(
            rows[bar - 1].contains('\u{2212}') && rows[bar + 1].contains("2a"),
            "{out}"
        );
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
            "<latex></latex> empty and <math>x unclosed",
        ] {
            assert!(t.contains(&want), "{want} in {t:?}");
        }
        // Read as text, the diagram draws the same.
        let mut as_text = seq.clone();
        as_text.math = None;
        assert_eq!(scene, draw_sequence(&as_text));
    }

    /// The paths of `owner` that are filled in the text color with no stroke: a formula's glyphs
    /// and rules.
    fn formula_paths(scene: &Scene, owner: Owner) -> Vec<Bounds> {
        scene
            .elements
            .iter()
            .filter(|e| e.owner == owner)
            .filter_map(|e| match &e.primitive {
                Primitive::Path {
                    segments,
                    fill: Some(f),
                    stroke: None,
                    clip,
                } if *f == Theme::LIGHT.text => bounds(segments, *clip),
                _ => None,
            })
            .collect()
    }

    /// The first stroked path of `owner`: the outline of a cluster.
    fn outline(scene: &Scene, owner: Owner) -> Bounds {
        scene
            .elements
            .iter()
            .filter(|e| e.owner == owner)
            .find_map(|e| match &e.primitive {
                Primitive::Path {
                    segments,
                    stroke: Some(_),
                    clip,
                    ..
                } => bounds(segments, *clip),
                _ => None,
            })
            .expect("the owner has an outline")
    }

    fn drawn_graph(src: &str) -> (Flowchart, Scene) {
        let (chart, d) = graph(src);
        assert!(d.is_empty(), "{d:?}");
        let scene = draw_graph(&chart);
        no_markup(&scene, src);
        (chart, scene)
    }

    /// The rows of a drawing's box: every row that is not blank is as wide as the first.
    fn assert_box_aligned(rows: &[Vec<char>], what: &str) {
        let boxed: Vec<&Vec<char>> = rows
            .iter()
            .filter(|r| r.first().is_some_and(|&c| "┌│├└╭╰".contains(c)))
            .collect();
        assert!(!boxed.is_empty(), "{what}: no box in {rows:?}");
        for r in boxed {
            let last = r.last().copied().unwrap_or(' ');
            assert!(
                "┐│┤┘╮╯".contains(last),
                "{what}: {}",
                r.iter().collect::<String>()
            );
        }
    }

    /// Class members and state bodies (compartment rows) set formulas in a row: the row grows
    /// to hold a tall one, a formula that does not typeset stays as source, and Mermaid's
    /// `$$` is still text there.
    #[test]
    fn compartment_rows_draw_formulas() {
        let (chart, scene) = drawn_graph(&valid("latex_inline_members"));
        let sum = chart.nodes.iter().position(|n| n.id == "Sum").unwrap();
        let (shape, _) = node_paths(&scene, sum);
        let paths = formula_paths(&scene, Owner::Node(sum));
        // The title's, the field's, and the method's formulas.
        assert!(paths.len() > 3, "{}", paths.len());
        assert_inside(shape, &paths, "members");
        let t = texts(&scene);
        assert!(t.contains(&"-count : int"), "{t:?}");
        assert!(t.contains(&"+total :"), "{t:?}");

        let height = |member: &str| {
            let src = format!("@startuml\nclass A {{\n  {member}\n}}\n@enduml\n");
            let (chart, scene) = drawn_graph(&src);
            let i = chart.nodes.iter().position(|n| n.id == "A").unwrap();
            let (b, _) = node_paths(&scene, i);
            b.3 - b.1
        };
        assert!(height(r"+x : <latex>\frac{a}{b}</latex>") > height("+x : y"));

        let (chart, scene) = drawn_graph(&valid("latex_inline_state_body"));
        let idle = chart.nodes.iter().position(|n| n.id == "Idle").unwrap();
        let (shape, _) = node_paths(&scene, idle);
        let paths = formula_paths(&scene, Owner::Node(idle));
        assert_inside(shape, &paths, "state body");
    }

    #[test]
    fn compartment_titles_draw_formulas() {
        let (chart, scene) = drawn_graph(&valid("latex_inline_members"));
        let sum = chart.nodes.iter().position(|n| n.id == "Sum").unwrap();
        let title = &chart.nodes[sum].label;
        assert!(title.has_formula(), "{title:?}");
        // The title is above the first rule: some formula path sits in the title area.
        let (shape, _) = node_paths(&scene, sum);
        let top = formula_paths(&scene, Owner::Node(sum))
            .into_iter()
            .map(|b| b.1)
            .fold(f32::MAX, f32::min);
        assert!(top < shape.1 + 30.0, "{top} {shape:?}");
    }

    #[test]
    fn compartment_rows_are_aligned_in_text() {
        let rows = ascii_rows(&valid("latex_inline_members"));
        assert_box_aligned(&rows, "members");
        let out: String = rows.iter().map(|r| r.iter().collect::<String>()).collect();
        assert!(!out.contains('<') && out.contains('∑'), "{out}");
        let rows = ascii_rows(&valid("latex_inline_state_body"));
        assert!(rows.iter().all(|r| r.iter().all(|&c| c != '<')));
        let body = |s: &str| {
            rows.iter()
                .any(|r| r.iter().collect::<String>().contains(s))
        };
        assert!(body("entry / x"), "{rows:?}");
    }

    /// A row with a tall formula is several lines of the box, its text on the baseline row.
    #[test]
    fn a_tall_member_is_several_aligned_lines() {
        let rows = ascii_rows(
            "@startuml\nclass A {\n  +x : <latex>\\frac{a}{b}</latex> ok\n  +y : 1\n}\n@enduml\n",
        );
        assert_box_aligned(&rows, "tall member");
        let text = |r: &Vec<char>| r.iter().collect::<String>();
        let first = rows
            .iter()
            .position(|r| text(r).contains("+x :"))
            .expect("the baseline row");
        assert!(text(&rows[first]).contains("ok"), "{rows:?}");
        assert!(text(&rows[first - 1]).contains('a'), "{rows:?}");
        assert!(text(&rows[first + 1]).contains('b'), "{rows:?}");
        let y = rows.iter().position(|r| text(r).contains("+y :")).unwrap();
        assert!(y > first + 1, "{rows:?}");
    }

    #[test]
    fn mermaid_dollars_stay_text_in_titles_and_end_labels() {
        let lowered =
            diagram::lower("flowchart TD\n    subgraph S[\"$$x^2$$\"]\n        A\n    end\n");
        let Diagram::Graph(chart) = lowered.diagram else {
            panic!("not a graph");
        };
        let scene = draw_graph(&chart);
        assert!(texts(&scene).contains(&"$$x^2$$"), "{:?}", texts(&scene));
        assert!(formula_paths(&scene, Owner::Cluster(0)).is_empty());
        let lowered = diagram::lower("classDiagram\n    A \"$$n$$\" --> \"1\" B\n");
        let Diagram::Graph(chart) = lowered.diagram else {
            panic!("not a graph");
        };
        let scene = draw_graph(&chart);
        assert!(texts(&scene).contains(&"$$n$$"), "{:?}", texts(&scene));
        // The same paths as with plain text there (an arrowhead is one).
        let plain = diagram::lower("classDiagram\n    A \"n\" --> \"1\" B\n");
        let Diagram::Graph(plain) = plain.diagram else {
            panic!("not a graph");
        };
        assert_eq!(
            formula_paths(&scene, Owner::Edge(0)).len(),
            formula_paths(&draw_graph(&plain), Owner::Edge(0)).len()
        );
    }

    /// The multiplicities and roles at the ends of an edge are formulas beside the line.
    #[test]
    fn edge_end_labels_draw_formulas() {
        let (chart, scene) = drawn_graph(&valid("latex_inline_end_labels"));
        let e = &chart.edges[0];
        assert!(e.start_label.as_ref().is_some_and(|l| l.has_formula()));
        assert!(e.end_label.as_ref().is_some_and(|l| l.has_formula()));
        let (_, plain) = drawn_graph(
            "@startuml\nclass Order\nclass Item\nOrder \"a\" --> \"b\" Item : lines\n@enduml\n",
        );
        let with = formula_paths(&scene, Owner::Edge(0)).len();
        let without = formula_paths(&plain, Owner::Edge(0)).len();
        assert!(with > without, "{with} {without}");
        let t = texts(&scene);
        assert!(t.contains(&"lines"), "{t:?}");
        let out = ascii_rows(&valid("latex_inline_end_labels"));
        let all: String = out.iter().map(|r| r.iter().collect::<String>()).collect();
        assert!(
            all.contains('α') && all.contains('²') && !all.contains('<'),
            "{all}"
        );
    }

    /// A tall end label is rows beside the line, not a line of source.
    #[test]
    fn a_tall_end_label_is_rows_in_text() {
        let rows = ascii_rows(
            "@startuml\nclass A\nclass B\nA --> \"<latex>\\frac{1}{2}</latex>\" B\n@enduml\n",
        );
        let all: String = rows
            .iter()
            .map(|r| r.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains('1') && all.contains('2') && !all.contains('<'),
            "{all}"
        );
    }

    /// The title of a package or a composite state is a formula, and its box holds it.
    #[test]
    fn subgraph_titles_draw_formulas() {
        let (chart, scene) = drawn_graph(&valid("latex_inline_package"));
        assert!(chart.subgraphs[0].title.has_formula());
        let paths = formula_paths(&scene, Owner::Cluster(0));
        assert!(!paths.is_empty());
        assert_inside(outline(&scene, Owner::Cluster(0)), &paths, "package");
        let (chart, scene) = drawn_graph(&valid("latex_inline_state_body"));
        let busy = chart
            .subgraphs
            .iter()
            .position(|s| s.title.has_formula())
            .expect("a composite state with a formula title");
        let paths = formula_paths(&scene, Owner::Cluster(busy));
        assert_inside(outline(&scene, Owner::Cluster(busy)), &paths, "state");
        let t = texts(&scene);
        assert!(!t.contains(&"Run <latex>\\tau</latex>"), "{t:?}");
    }

    #[test]
    fn a_tall_subgraph_title_grows_the_box_and_stays_in_it() {
        let height = |title: &str| {
            let src = format!("@startuml\npackage \"{title}\" {{\n  class A\n}}\n@enduml\n");
            let (_, scene) = drawn_graph(&src);
            let b = outline(&scene, Owner::Cluster(0));
            b.3 - b.1
        };
        assert!(height(r"<latex>\frac{a}{b}</latex>") > height("x"));
        let rows = ascii_rows(
            "@startuml\npackage \"<latex>\\frac{1}{2}</latex>\" {\n  class A\n}\n@enduml\n",
        );
        assert_box_aligned(&rows, "package title");
        let all: String = rows
            .iter()
            .map(|r| r.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains('1') && all.contains('2') && !all.contains('<'),
            "{all}"
        );
        assert!(all.contains("class") || all.contains("A"), "{all}");
        let rows = ascii_rows(&valid("latex_inline_package"));
        let top: String = rows
            .iter()
            .map(|r| r.iter().collect::<String>())
            .find(|r| r.contains('ℕ'))
            .expect("the title row");
        assert!(top.starts_with('┌') && top.ends_with('┐'), "{top}");
    }

    /// A failing formula in any of the new positions is source with one diagnostic on it.
    #[test]
    fn failing_rows_end_labels_and_titles_have_one_diagnostic_each() {
        let at = |name: &str| {
            let src = fixture("errors", name);
            let lowered = diagram::lower(&src);
            let [d] = lowered.diagnostics.as_slice() else {
                panic!("{name}: one diagnostic, got {:?}", lowered.diagnostics);
            };
            assert_eq!(d.severity, Severity::Error, "{name}");
            let text = src[usize::from(d.range.start())..usize::from(d.range.end())].to_owned();
            (src, d.clone(), lowered.diagram, text)
        };
        let (src, d, diagram, text) = at("latex_inline_fail_member");
        assert_eq!(text, r"\foo");
        assert_eq!(d.line_col(&src).0, 4);
        let scene = diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert!(
            texts(&scene)
                .iter()
                .any(|t| t.contains(r"<latex>a + \foo{x}</latex>")),
            "{:?}",
            texts(&scene)
        );
        // The row before it typeset.
        let Diagram::Graph(chart) = &diagram else {
            panic!("not a graph");
        };
        assert!(
            !formula_paths(&scene, Owner::Node(0)).is_empty(),
            "{chart:?}"
        );

        let (src, d, diagram, text) = at("latex_inline_fail_end_label");
        assert_eq!(d.line_col(&src).0, 4);
        assert!(
            r"\bar{".contains(text.trim()) || text.is_empty(),
            "{text:?}"
        );
        let scene = diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert!(
            texts(&scene)
                .iter()
                .any(|t| t.contains(r"<latex>\bar{</latex>")),
            "{:?}",
            texts(&scene)
        );

        let (_, _, diagram, text) = at("latex_inline_fail_package");
        assert_eq!(text, r"\foo");
        let scene = diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert!(
            texts(&scene)
                .iter()
                .any(|t| t.contains(r"<latex>\foo</latex>")),
            "{:?}",
            texts(&scene)
        );
        // The title's first formula typeset.
        assert!(!formula_paths(&scene, Owner::Cluster(0)).is_empty());
    }

    /// A row has no range of its own: the same failing formula in a label and a row are each
    /// diagnosed in their own place.
    #[test]
    fn a_failing_row_is_diagnosed_where_it_is_written() {
        let src = "@startuml\nclass A {\n  +f : <latex>\\foo</latex>\n}\nnote right of A : <latex>\\foo</latex>\n@enduml\n";
        let d = diagram::lower(src).diagnostics;
        assert_eq!(d.len(), 2, "{d:?}");
        let lines: Vec<usize> = d.iter().map(|d| d.line_col(src).0).collect();
        assert_eq!(lines, [3, 5], "{d:?}");
        for x in &d {
            assert_eq!(
                &src[usize::from(x.range.start())..usize::from(x.range.end())],
                r"\foo"
            );
        }
    }

    /// The document title reads the tags (`plantuml_math_titles.rs` has the drawings); a legend
    /// is not drawn at all.
    #[test]
    fn the_document_title_is_a_formula() {
        let src = "@startuml\ntitle <latex>x</latex>\nclass A\n@enduml\n";
        let lowered = diagram::lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let scene = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert!(
            !texts(&scene).contains(&"<latex>x</latex>"),
            "{:?}",
            texts(&scene)
        );
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

    /// The formula is found with its tags: the same text before it is not it.
    #[test]
    fn a_diagnostic_is_on_the_formula_not_the_same_text_before_it() {
        for src in [
            "@startuml\nA -> B : \\foo is <latex>\\foo</latex>\n@enduml\n",
            "@startuml\nstart\n:\\foo is <latex>\\foo</latex>;\nstop\n@enduml\n",
        ] {
            let d = diagram::lower(src).diagnostics;
            let [d] = d.as_slice() else {
                panic!("one diagnostic, got {d:?}");
            };
            let (from, to) = (usize::from(d.range.start()), usize::from(d.range.end()));
            assert_eq!(&src[from..to], r"\foo", "{src}");
            assert_eq!(&src[from - 7..from], "<latex>", "{src}");
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
            "latex_inline_members",
            "latex_inline_end_labels",
            "latex_inline_package",
            "latex_inline_state_body",
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

    /// One warning per formula, on its body in the source, as `@startlatex` gives.
    fn assert_warned(src: &str, d: &[sirene_core::Diagnostic], formulas: usize) {
        assert_eq!(d.len(), formulas, "{d:?}");
        for x in d {
            assert_eq!(x.severity, sirene_core::Severity::Warning, "{x:?}");
            let at = &src[usize::from(x.range.start())..usize::from(x.range.end())];
            assert!(!at.is_empty() && !at.contains('<'), "{x:?}: {at:?}");
        }
    }

    #[test]
    fn tags_stay_literal_without_the_math_feature() {
        let src = fixture("errors", "latex_inline_fail");
        let (seq, d) = sequence(&src);
        assert_warned(&src, &d, src.matches("<latex>").count());
        let scene = draw_sequence(&seq);
        let t = texts(&scene);
        assert!(
            t.iter().any(|x| x.contains(r"<latex>a + \foo{x}</latex>")),
            "{t:?}"
        );
        let src = fixture("errors", "latex_inline_fail_activity");
        let (chart, d) = graph(&src);
        assert_warned(&src, &d, src.matches("<latex>").count());
        assert!(
            texts(&draw_graph(&chart))
                .iter()
                .any(|x| x.contains("<latex>"))
        );
        let src = valid("latex_inline_message");
        let (seq, d) = sequence(&src);
        assert_warned(&src, &d, src.matches("<latex>").count());
        assert!(texts(&draw_sequence(&seq)).contains(&r"<latex>\alpha</latex>"));
    }

    /// Rows, end labels, and titles keep their tags as text and warn once per formula.
    #[test]
    fn rows_end_labels_and_titles_stay_literal_without_the_math_feature() {
        for (name, tag) in [
            ("latex_inline_fail_member", "<latex>"),
            ("latex_inline_fail_end_label", "<latex>"),
            ("latex_inline_fail_package", "<latex>"),
        ] {
            let src = fixture("errors", name);
            let (chart, d) = graph(&src);
            assert_warned(&src, &d, src.matches(tag).count());
            assert!(
                texts(&draw_graph(&chart))
                    .iter()
                    .any(|x| x.contains("<latex>")),
                "{name}"
            );
        }
        for name in [
            "latex_inline_members",
            "latex_inline_end_labels",
            "latex_inline_package",
            "latex_inline_state_body",
        ] {
            let src = valid(name);
            let (chart, d) = graph(&src);
            let formulas = src.matches("<latex>").count() + src.matches("<math>").count();
            assert_warned(&src, &d, formulas);
            let scene = draw_graph(&chart);
            let t = texts(&scene);
            assert!(
                t.iter()
                    .any(|x| x.contains("latex>") || x.contains("math>")),
                "{name}: {t:?}"
            );
        }
    }
}
