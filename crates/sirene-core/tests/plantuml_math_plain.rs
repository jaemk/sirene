//! `<latex>` and `<math>` formulas in the plain strings of PlantUML's Gantt, chronology, and
//! timing diagrams (LEMB-2, PCREO-3): the Gantt title, section names, and task names, the
//! chronology title and event text, and the timing title. They are drawn as formula paths in the
//! scene and as rows in text, a failing one drawn as its source with one diagnostic on its body,
//! and with the `math` feature off the tags stay text with one warning per formula. Mermaid's
//! Gantt and timeline draw the same tags as text.

use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::diagram;
use sirene_core::scene::{Approx, Primitive, Scene, Theme};

fn fixture(dir: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plantuml")
        .join(dir)
        .join(format!("{name}.puml"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
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

#[cfg(feature = "math")]
const VALID: [&str; 3] = [
    "latex_inline_gantt",
    "latex_inline_chronology",
    "latex_inline_timing",
];

#[cfg(not(feature = "math"))]
const ERRORS: [&str; 3] = [
    "latex_inline_fail_gantt",
    "latex_inline_fail_chronology",
    "latex_inline_fail_timing",
];

#[cfg(feature = "math")]
mod math {
    use super::*;
    use sirene_core::ascii::{Charset, Options};

    /// How many filled paths of the scene are in the text color: a formula's glyphs and rules.
    fn formula_paths(scene: &Scene) -> usize {
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

    fn scene(src: &str) -> Scene {
        diagram::lower(src)
            .diagram
            .scene(&Approx, &Theme::LIGHT)
            .unwrap()
    }

    fn ascii(src: &str) -> String {
        diagram::lower(src)
            .diagram
            .ascii(&Options {
                charset: Charset::Unicode,
            })
            .unwrap()
    }

    #[test]
    fn well_formed_fixtures_have_no_diagnostics() {
        for name in VALID {
            let d = diagram::lower(&fixture("valid", name)).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    #[test]
    fn every_fixture_draws_formulas_in_both_renderers_without_markup() {
        for name in VALID {
            let src = fixture("valid", name);
            let s = scene(&src);
            assert!(formula_paths(&s) > 0, "{name}");
            assert!(
                texts(&s)
                    .iter()
                    .all(|t| !t.contains("latex>") && !t.contains("math>")),
                "{name}: {:?}",
                texts(&s)
            );
            let rows = ascii(&src);
            assert!(
                !rows.contains("latex>") && !rows.contains("math>"),
                "{name}: {rows}"
            );
        }
    }

    /// Every formula of the Gantt fixture is drawn: the title, the section name, and two task
    /// names, with the text around them still text.
    const PLAIN_GANTT: &str = "@startgantt\ntitle Area over time\nProject starts 2026-01-05\n-- Phase --\n[Solve] lasts 3 days\nthen [Check plain] lasts 2 days\n-- Release --\n[Ship] starts at [Check plain]'s end and lasts 2 days\n@endgantt\n";

    #[test]
    fn a_gantt_draws_formulas_in_its_title_sections_and_tasks() {
        let src = fixture("valid", "latex_inline_gantt");
        let plain = scene(PLAIN_GANTT);
        let with = scene(&src);
        assert!(formula_paths(&with) > formula_paths(&plain) + 4);
        let t = texts(&with);
        assert!(t.contains(&"Area"), "{t:?}");
        assert!(t.contains(&"plain"), "{t:?}");
        assert!(t.contains(&"Ship"), "{t:?}");
        // A fraction in a task name is taller than a row, so the chart grows.
        assert!(
            with.height > plain.height,
            "{} {}",
            with.height,
            plain.height
        );
        let rows = ascii(&src);
        assert!(rows.contains("Ship"), "{rows}");
        assert!(rows.contains("plain"), "{rows}");
        // The fraction is rows in the label column, its bar on one of them.
        let bar_rows = rows.lines().filter(|l| l.contains('█')).count();
        assert_eq!(bar_rows, 3, "{rows}");
        assert!(
            rows.lines().count() > ascii(PLAIN_GANTT).lines().count(),
            "the fraction adds rows: {rows}"
        );
    }

    #[test]
    fn a_chronology_draws_formulas_in_its_title_and_events() {
        let src = fixture("valid", "latex_inline_chronology");
        let s = scene(&src);
        let t = texts(&s);
        assert!(t.contains(&"Announce"), "{t:?}");
        assert!(t.contains(&"Freeze"), "{t:?}");
        assert!(t.contains(&"done"), "{t:?}");
        assert!(formula_paths(&s) >= 3, "{}", formula_paths(&s));
        let rows = ascii(&src);
        assert!(rows.contains("Freeze") && rows.contains("done"), "{rows}");
        assert!(rows.contains("Announce"), "{rows}");
        // The title's rows come first, centered over the drawing.
        let first = rows.lines().next().unwrap_or_default();
        assert!(
            first.starts_with(' ') || first.contains("Release"),
            "{rows}"
        );
    }

    /// A tall formula makes its event box taller, and a formula is not broken across lines.
    #[test]
    fn a_tall_event_formula_makes_its_box_taller_and_is_one_piece() {
        let height = |name: &str| {
            scene(&format!(
                "@startchronology\n[{name}] happens on 2024-01-15\n@endchronology\n"
            ))
            .height
        };
        assert!(height(r"<latex>\frac{\frac{a}{b}}{\frac{c}{d}}</latex>") > height("x"));
        // A long event wraps by words with the formula whole.
        let s = scene(
            "@startchronology\n[one two three four five six seven eight nine ten <latex>\\frac{1}{2}</latex> end] happens on 2024-01-15\n@endchronology\n",
        );
        assert!(formula_paths(&s) > 0);
        assert!(texts(&s).contains(&"end"), "{:?}", texts(&s));
    }

    #[test]
    fn a_timing_title_with_a_formula_makes_room_for_itself() {
        let plain =
            scene("@startuml\ntitle Request handling\nrobust \"B\" as B\n@0\nB is Idle\n@enduml\n");
        let with = scene(&fixture("valid", "latex_inline_timing"));
        assert!(formula_paths(&with) > 0);
        assert!(texts(&with).contains(&"Request"), "{:?}", texts(&with));
        assert!(
            with.height > plain.height,
            "{} {}",
            with.height,
            plain.height
        );
        let rows = ascii(&fixture("valid", "latex_inline_timing"));
        assert!(rows.contains("Request"), "{rows}");
        assert!(!rows.contains("latex>"), "{rows}");
    }

    /// A failing formula is drawn as its source and has one error on its body.
    #[test]
    fn failing_formulas_have_one_error_each_on_their_body_and_draw_their_source() {
        for (name, bodies) in [
            ("latex_inline_fail_gantt", vec![r"\foo", r"\qux", r"\baz"]),
            ("latex_inline_fail_chronology", vec![r"\foo", r"\qux"]),
            ("latex_inline_fail_timing", vec![r"\foo"]),
        ] {
            let src = fixture("errors", name);
            let lowered = diagram::lower(&src);
            assert_eq!(lowered.diagnostics.len(), bodies.len(), "{name}");
            let mut at: Vec<(usize, String)> = lowered
                .diagnostics
                .iter()
                .map(|d| {
                    assert_eq!(d.severity, Severity::Error, "{name}");
                    (
                        d.line_col(&src).0,
                        src[usize::from(d.range.start())..usize::from(d.range.end())].to_owned(),
                    )
                })
                .collect();
            at.sort();
            let got: Vec<&str> = at.iter().map(|(_, t)| t.as_str()).collect();
            assert_eq!(got, bodies, "{name}: {at:?}");
            // The source is drawn, tags and all, in both renderers.
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            assert!(
                texts(&s).iter().any(|t| t.contains("<latex>")),
                "{name}: {:?}",
                texts(&s)
            );
            assert!(ascii(&src).contains("<latex>"), "{name}");
        }
    }

    /// Each diagnostic is on the line the formula is written on.
    #[test]
    fn gantt_and_chronology_diagnostics_are_on_their_lines() {
        let src = fixture("errors", "latex_inline_fail_gantt");
        let lines: Vec<usize> = diagram::lower(&src)
            .diagnostics
            .iter()
            .map(|d| d.line_col(&src).0)
            .collect();
        assert_eq!(lines, [2, 4, 5]);
        let src = fixture("errors", "latex_inline_fail_chronology");
        let lines: Vec<usize> = diagram::lower(&src)
            .diagnostics
            .iter()
            .map(|d| d.line_col(&src).0)
            .collect();
        assert_eq!(lines, [2, 3]);
    }

    /// Mermaid's Gantt and timeline do not read the tags: they stay text, with no diagnostic.
    #[test]
    fn mermaid_gantt_and_timeline_keep_the_tags_as_text() {
        for src in [
            "gantt\n    title <latex>x^2</latex>\n    dateFormat YYYY-MM-DD\n    section <latex>s</latex>\n    <latex>y</latex> :a, 2026-01-05, 3d\n",
            "timeline\n    title <latex>x^2</latex>\n    <latex>y</latex> : <latex>z</latex>\n",
        ] {
            let lowered = diagram::lower(src);
            assert!(
                lowered.diagnostics.is_empty(),
                "{src}: {:?}",
                lowered.diagnostics
            );
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            assert!(
                texts(&s).iter().any(|t| t.contains("<latex>x^2</latex>")),
                "{src}: {:?}",
                texts(&s)
            );
        }
    }

    /// Formulas are drawn in the text's color: a task name on a bar is in the bar's ink, the
    /// others in the theme's text color.
    #[test]
    fn title_formulas_are_in_the_text_color() {
        let s = scene(&fixture("valid", "latex_inline_timing"));
        assert!(formula_paths(&s) > 0);
        let dark = diagram::lower(&fixture("valid", "latex_inline_timing"))
            .diagram
            .scene(&Approx, &Theme::DARK)
            .unwrap();
        let in_dark = dark
            .elements
            .iter()
            .filter(|e| {
                matches!(
                    &e.primitive,
                    Primitive::Path { fill: Some(f), stroke: None, .. } if *f == Theme::DARK.text
                )
            })
            .count();
        assert_eq!(in_dark, formula_paths(&s));
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    /// One warning per formula, and the tags stay text.
    #[test]
    fn tags_stay_literal_with_one_warning_per_formula() {
        // The formulas of each fixture's texts: the Gantt chart's task `Check` is written
        // twice, and warned about once.
        for (name, formulas) in [
            ("latex_inline_gantt", 4),
            ("latex_inline_chronology", 3),
            ("latex_inline_timing", 1),
        ] {
            let src = fixture("valid", name);
            let lowered = diagram::lower(&src);
            assert_eq!(lowered.diagnostics.len(), formulas, "{name}");
            assert!(
                lowered
                    .diagnostics
                    .iter()
                    .all(|d| d.severity == Severity::Warning),
                "{name}"
            );
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            assert!(
                texts(&s)
                    .iter()
                    .any(|t| t.contains("latex>") || t.contains("math>")),
                "{name}: {:?}",
                texts(&s)
            );
        }
        for name in ERRORS {
            let src = fixture("errors", name);
            let formulas = src.matches("<latex>").count();
            let d = diagram::lower(&src).diagnostics;
            assert_eq!(d.len(), formulas, "{name}: {d:?}");
        }
    }
}
