//! `$$...$$` formulas in the labels of Mermaid flowchart subgraph titles, of the kinds that
//! lower to the flowchart model (state, class, ER, requirement, block, agentflow, swimlane, and
//! use case), and of the mindmap and kanban kinds, which have models of their own (LEMB-1,
//! LEMB-4, LEMB-5, LEMB-T5, LEMB-T9). Each position draws its formulas in the scene and as rows in text, a failing
//! formula is drawn as its source with one diagnostic on its span, and with the `math` feature
//! off each formula is one warning.

#[cfg(feature = "math")]
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mermaid");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p
                .file_stem()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&p).expect("readable fixture"))
        })
        .collect();
    out.sort();
    out
}

/// The formulas of each valid fixture: one per position it wires.
const FORMULAS: [(&str, usize); 11] = [
    ("agentflow_labels", 4),
    ("block_labels", 5),
    ("class_labels", 7),
    ("er_labels", 3),
    ("flowchart_subgraph", 3),
    ("kanban_labels", 6),
    ("mindmap_labels", 5),
    ("requirement_labels", 4),
    ("state_labels", 6),
    ("swimlane_labels", 4),
    ("usecase_labels", 5),
];

/// The formulas that do not typeset in each error fixture.
const FAILING: [(&str, usize); 11] = [
    ("agentflow_fail", 3),
    ("block_fail", 3),
    ("class_fail", 5),
    ("er_fail", 3),
    ("flowchart_subgraph_fail", 1),
    ("kanban_fail", 5),
    ("mindmap_fail", 3),
    ("requirement_fail", 2),
    ("state_fail", 4),
    ("swimlane_fail", 3),
    ("usecase_fail", 4),
];

fn lower(src: &str) -> Lowered {
    diagram::lower(src)
}

fn scene(src: &str) -> Scene {
    lower(src).diagram.scene(&Approx, &Theme::LIGHT).unwrap()
}

fn ascii(src: &str) -> String {
    lower(src)
        .diagram
        .ascii(&Options {
            charset: Charset::Unicode,
        })
        .unwrap()
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

/// How many filled paths without a stroke the scene has: formula glyphs, and shapes and
/// arrowheads, which a formula does not change the number of (a mindmap's text is not in the theme's
/// text color, so the color is not tested).
#[cfg(feature = "math")]
fn filled(scene: &Scene) -> usize {
    scene
        .elements
        .iter()
        .filter(|e| {
            matches!(
                &e.primitive,
                Primitive::Path {
                    fill: Some(_),
                    stroke: None,
                    ..
                }
            )
        })
        .count()
}

/// `src` with every formula but number `keep` (from 0) as plain text, and with `keep` of
/// `None`, with none. A formula in a fixture sits on one line, so the odd pieces between `$$`
/// are bodies.
#[cfg(feature = "math")]
fn only(src: &str, keep: Option<usize>) -> String {
    let mut out = String::new();
    for (i, piece) in src.split("$$").enumerate() {
        if i % 2 == 1 && keep == Some(i / 2) {
            write!(out, "$${piece}$$").unwrap();
        } else {
            out.push_str(piece);
        }
    }
    out
}

fn count(src: &str) -> usize {
    src.matches("$$").count() / 2
}

#[cfg(feature = "math")]
fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
        writeln!(out, "{line}:{col}: {severity}: {} [{at}]", d.message).unwrap();
    }
    out
}

#[test]
fn the_tables_list_every_fixture() {
    for (dir, table) in [
        ("valid", FORMULAS.iter().map(|f| f.0).collect::<Vec<&str>>()),
        ("errors", FAILING.iter().map(|f| f.0).collect()),
    ] {
        let names: Vec<String> = fixtures(dir).into_iter().map(|f| f.0).collect();
        assert_eq!(names, table, "{dir}");
    }
    for (name, n) in FORMULAS {
        let src = &fixtures("valid")
            .into_iter()
            .find(|f| f.0 == name)
            .unwrap()
            .1;
        assert_eq!(count(src), n, "{name}");
    }
}

#[cfg(feature = "math")]
mod math {
    use super::*;

    #[test]
    fn well_formed_fixtures_have_no_diagnostics() {
        for (name, src) in fixtures("valid") {
            let d = lower(&src).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    /// Each formula, alone, adds formula paths to the scene and takes its source out of the
    /// text, so every wired position draws its formula in both renderers.
    #[test]
    fn every_formula_is_drawn_in_scene_and_text() {
        for (name, src) in fixtures("valid") {
            let n = count(&src);
            let (plain_scene, plain_text) = (scene(&only(&src, None)), ascii(&only(&src, None)));
            for k in 0..n {
                let one = only(&src, Some(k));
                let s = scene(&one);
                assert!(
                    filled(&s) > filled(&plain_scene),
                    "{name} formula {k}: no paths added"
                );
                assert!(
                    texts(&s).iter().all(|t| !t.contains("$$")),
                    "{name} formula {k}: {:?}",
                    texts(&s)
                );
                let text = ascii(&one);
                assert!(!text.contains("$$"), "{name} formula {k}:\n{text}");
                assert_ne!(text, plain_text, "{name} formula {k}");
            }
        }
    }

    #[test]
    fn snapshots() {
        for dir in ["valid", "errors"] {
            for (name, src) in fixtures(dir) {
                let lowered = lower(&src);
                let mut out = diagnostics(&src, &lowered);
                out.push('\n');
                out.push_str(&ascii(&src));
                insta::assert_snapshot!(format!("mermaid_math__{dir}__{name}"), out, &src);
            }
        }
    }

    #[test]
    fn a_failing_formula_is_one_error_on_its_span_and_drawn_as_source() {
        for (name, src) in fixtures("errors") {
            let lowered = lower(&src);
            let want = FAILING.iter().find(|f| f.0 == name).unwrap().1;
            let errors: Vec<_> = lowered
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .collect();
            assert_eq!(errors.len(), want, "{name}: {:?}", lowered.diagnostics);
            assert_eq!(errors.len(), lowered.diagnostics.len(), "{name}");
            for d in errors {
                let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
                assert!(at.starts_with("\\foo"), "{name}: {at:?} {}", d.message);
            }
            // The source of each failing formula is drawn, in both renderers.
            let text = ascii(&src);
            assert!(text.contains("$$\\foo"), "{name}:\n{text}");
            let s = scene(&src);
            assert!(
                texts(&s).iter().any(|t| t.contains("$$\\foo")),
                "{name}: {:?}",
                texts(&s)
            );
        }
    }

    /// A formula that typesets next to one that does not is still drawn.
    #[test]
    fn a_good_formula_beside_a_failing_one_is_drawn() {
        let src = &fixtures("errors")
            .into_iter()
            .find(|f| f.0 == "flowchart_subgraph_fail")
            .unwrap()
            .1;
        let s = scene(src);
        assert!(filled(&s) > filled(&scene(&only(src, None))));
    }

    #[test]
    fn a_double_backslash_is_read_as_one_in_these_labels() {
        let src = "stateDiagram-v2\n    A : $$a\\\\foo$$\n";
        // `\\foo` is `\foo` after the collapse, which is not a command.
        let d = lower(src).diagnostics;
        assert_eq!(d.len(), 1, "{d:?}");
        let at = &src[usize::from(d[0].range.start())..usize::from(d[0].range.end())];
        assert_eq!(at, "\\\\foo");
    }

    #[test]
    fn titles_and_unclosed_delimiters_stay_text() {
        let src = "---\ntitle: $$x^2$$\n---\nflowchart LR\n    subgraph s[\"open $$x\"]\n    A\n    end\n";
        let lowered = lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let plain = src.replace("$$", "");
        assert_eq!(filled(&scene(src)), filled(&scene(&plain)));
        assert!(texts(&scene(src)).iter().any(|t| t.contains("x^2")));
    }

    /// A mindmap's and a kanban board's `title` is text, as in Mermaid, and an unclosed `$$` in
    /// a node, card, or column is text.
    #[test]
    fn mindmap_and_kanban_titles_and_unclosed_delimiters_stay_text() {
        for src in [
            "---\ntitle: $$x^2$$\n---\nmindmap\n  root[open $$x]\n    a[b]\n",
            "---\ntitle: $$x^2$$\n---\nkanban\n  todo[open $$x]\n    a[b $$y]\n",
        ] {
            let lowered = lower(src);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            let plain = src.replace("$$", "");
            assert_eq!(filled(&scene(src)), filled(&scene(&plain)), "{src}");
            assert!(texts(&scene(src)).iter().any(|t| t.contains("$$x^2$$")));
            assert!(ascii(src).contains("open $$x"), "{}", ascii(src));
        }
    }

    /// A card's ticket and assignee are read for formulas, and a formula in one is not cut to
    /// fit the card's width.
    #[test]
    fn a_kanban_ticket_and_assignee_formula_is_drawn_whole() {
        let src = "kanban\n  todo[T]\n    a[Card]@{ ticket: '$$\\alpha+\\beta+\\gamma+\\delta+\\lambda$$', assigned: '$$\\sum_i x_i+\\theta+\\mu$$' }\n";
        let lowered = lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let plain = src.replace("$$", "");
        assert!(filled(&scene(src)) > filled(&scene(&plain)));
        assert!(!ascii(src).contains("..."), "{}", ascii(src));
        assert!(!ascii(src).contains("$$"), "{}", ascii(src));
    }

    /// Labels of a PlantUML source keep reading their tags, and `$$` stays text there.
    #[test]
    fn plantuml_dollars_stay_text() {
        let src = "@startuml\nclass A {\n  +f : $$x$$\n}\n@enduml\n";
        let lowered = lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        assert!(ascii(src).contains("$$x$$"), "{}", ascii(src));
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    /// Without the `math` feature each formula is one warning, and is drawn as its source.
    #[test]
    fn each_formula_is_one_warning_and_drawn_as_source() {
        for (name, src) in fixtures("valid") {
            let lowered = lower(&src);
            let want = FORMULAS.iter().find(|f| f.0 == name).unwrap().1;
            assert_eq!(lowered.diagnostics.len(), want, "{name}");
            assert!(
                lowered
                    .diagnostics
                    .iter()
                    .all(|d| d.severity == Severity::Warning),
                "{name}"
            );
            let text = ascii(&src);
            assert!(text.contains("$$"), "{name}:\n{text}");
            let s = scene(&src);
            assert!(texts(&s).iter().any(|t| t.contains("$$")), "{name}");
        }
    }
}
