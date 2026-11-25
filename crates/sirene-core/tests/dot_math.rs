//! `$$...$$` formulas in the labels of a DOT graph (LEMB-1, LEMB-2, LEMB-T5): a sirene extension,
//! not Graphviz behavior. Every label position the painter draws reads them: a node's `label`
//! (and a record's fields), an edge's `label`, `headlabel`, and `taillabel`, a cluster's `label`,
//! and the graph's `label`. Each position draws its formula in the scene and as rows in text, a
//! failing formula is drawn as its source with one diagnostic on its body, and with the `math`
//! feature off each formula is one warning. A formula is read as written: no DOT escape is read
//! in it. DOT labels are not Creole, so `<latex>` and `<math>` stay text.

#[cfg(feature = "math")]
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dot");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "dot"))
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

/// The formulas of the valid fixture: the graph, two nodes, a `\neq` node, two record fields, an
/// edge label, tail label, and head label, and a cluster.
const FORMULAS: usize = 10;

/// The formulas that do not typeset in the error fixture: one per position.
const FAILING: usize = 7;

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

fn count(src: &str) -> usize {
    src.matches("$$").count() / 2
}

#[test]
fn the_fixtures_hold_the_counted_formulas() {
    let valid = fixtures("math");
    assert_eq!(valid.len(), 1);
    assert_eq!(count(&valid[0].1), FORMULAS);
    let errors = fixtures("math_errors");
    assert_eq!(errors.len(), 1);
    assert_eq!(count(&errors[0].1), FAILING);
}

/// `xlabel` is not drawn, so it is one warning, not a position.
#[test]
fn xlabel_is_not_drawn() {
    let l = lower("digraph { a [xlabel=\"$$x$$\"]; a -> b [xlabel=\"$$y$$\"] }");
    let warnings: Vec<_> = l
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("xlabel"))
        .collect();
    assert_eq!(warnings.len(), 2, "{:?}", l.diagnostics);
    assert!(
        warnings
            .iter()
            .all(|d| d.severity == Severity::Warning && d.message.contains("not supported"))
    );
}

/// Creole tags are PlantUML's: a DOT label keeps them as text, with no diagnostic.
#[test]
fn creole_tags_stay_text() {
    let src = "digraph { a [label=\"<latex>x</latex> $$\"] }";
    assert!(lower(src).diagnostics.is_empty());
    assert!(ascii(src).contains("<latex>x</latex>"), "{}", ascii(src));
}

/// An unclosed `$$` is text.
#[test]
fn an_unclosed_delimiter_is_text() {
    let src = "digraph { label=\"open $$x\"; a [label=\"open $$y\"] }";
    assert!(lower(src).diagnostics.is_empty());
    let text = ascii(src);
    assert!(
        text.contains("open $$x") && text.contains("open $$y"),
        "{text}"
    );
}

#[cfg(feature = "math")]
mod math {
    use super::*;

    /// How many filled paths of the scene are in the text color: formula glyphs and rules.
    fn filled(scene: &Scene) -> usize {
        scene
            .elements
            .iter()
            .filter(|e| {
                matches!(
                    &e.primitive,
                    Primitive::Path { fill: Some(f), stroke: None, .. } if *f == Theme::LIGHT.text
                )
            })
            .count()
    }

    /// `src` with every formula but number `keep` (from 0) as plain text, and with `keep` of
    /// `None`, with none. A formula in a fixture sits on one line, so the odd pieces between
    /// `$$` are bodies.
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
    fn well_formed_fixtures_have_no_diagnostics() {
        for (name, src) in fixtures("math") {
            let d = lower(&src).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    /// Each formula, alone, adds formula paths to the scene and takes its source out of the
    /// text, so every position draws its formula in both renderers.
    #[test]
    fn every_formula_is_drawn_in_scene_and_text() {
        for (name, src) in fixtures("math") {
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
        for (dir, prefix) in [("math", "valid"), ("math_errors", "errors")] {
            for (name, src) in fixtures(dir) {
                let lowered = lower(&src);
                let mut out = diagnostics(&src, &lowered);
                out.push('\n');
                out.push_str(&ascii(&src));
                insta::assert_snapshot!(format!("dot_math__{prefix}__{name}"), out, &src);
            }
        }
    }

    #[test]
    fn a_failing_formula_is_one_error_on_its_body_and_drawn_as_source() {
        for (name, src) in fixtures("math_errors") {
            let lowered = lower(&src);
            assert_eq!(
                lowered.diagnostics.len(),
                FAILING,
                "{name}: {:?}",
                lowered.diagnostics
            );
            for d in &lowered.diagnostics {
                assert_eq!(d.severity, Severity::Error, "{name}");
                let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
                assert!(at.starts_with("\\foo"), "{name}: {at:?} {}", d.message);
            }
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

    /// A formula is read as written: `\neq` is TeX, not a line break, and `\frac` keeps its
    /// backslash. The text around it still reads DOT's escapes.
    #[test]
    fn a_formula_is_not_read_for_dot_escapes() {
        let src = "digraph { a [label=\"$$a \\neq b$$\\nline\"] }";
        let chart = lower(src).diagram.into_graph();
        assert_eq!(chart.nodes[0].label.text, "$$a \\neq b$$\nline");
        assert!(lower(src).diagnostics.is_empty());
    }

    /// A default `label` that applies to several nodes reports a failing formula once.
    #[test]
    fn a_default_label_reports_once() {
        let src = "digraph { node [label=\"$$\\foo$$\"]; a; b; c }";
        let d = lower(src).diagnostics;
        assert_eq!(d.len(), 1, "{d:?}");
    }

    /// The graph label is drawn above the graph in both renderers, and Mermaid's title is not.
    #[test]
    fn the_graph_label_is_a_formula_in_both_renderers() {
        let src = "digraph { label=\"$$x^2$$\"; a }";
        assert!(filled(&scene(src)) > filled(&scene("digraph { label=\"x^2\"; a }")));
        assert!(!ascii(src).contains("$$"));
        let mermaid = "---\ntitle: $$x^2$$\n---\nflowchart LR\n    A\n";
        assert_eq!(
            filled(&scene(mermaid)),
            filled(&scene(&mermaid.replace("$$", "")))
        );
    }

    /// PlantUML's `@startdot` reads them too.
    #[test]
    fn startdot_reads_them() {
        let src = "@startdot\ndigraph { a [label=\"$$x^2$$\"] }\n@enddot\n";
        let l = lower(src);
        assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
        assert!(filled(&scene(src)) > filled(&scene(&src.replace("$$", ""))));
        let bad = "@startdot\ndigraph { a [label=\"$$\\foo$$\"] }\n@enddot\n";
        let d = lower(bad).diagnostics;
        assert_eq!(d.len(), 1, "{d:?}");
        let at = &bad[usize::from(d[0].range.start())..usize::from(d[0].range.end())];
        assert!(at.starts_with("\\foo"), "{at:?}");
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    /// Without the `math` feature each formula is one warning, and is drawn as its source.
    #[test]
    fn each_formula_is_one_warning_and_drawn_as_source() {
        for (name, src) in fixtures("math") {
            let lowered = lower(&src);
            assert_eq!(lowered.diagnostics.len(), FORMULAS, "{name}");
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
