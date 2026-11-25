//! `<latex>` and `<math>` formulas in the plain strings of PlantUML's Gantt, chronology, and
//! timing diagrams (LEMB-2, PCREO-3): the Gantt title, section names, and task names, the
//! chronology title and event text, and every text of a timing diagram (title, signal and state
//! names, notes, change notes, constraint, highlight, and message labels). They are drawn as
//! formula paths in the
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
const VALID: [&str; 4] = [
    "latex_inline_gantt",
    "latex_inline_chronology",
    "latex_inline_timing",
    "latex_inline_timing_all",
];

#[cfg(not(feature = "math"))]
const ERRORS: [&str; 4] = [
    "latex_inline_fail_gantt",
    "latex_inline_fail_chronology",
    "latex_inline_fail_timing",
    "latex_inline_fail_timing_all",
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

    /// A timing diagram with `f` as the text of one position: each is `(position, source)`.
    fn timing_positions(f: &str) -> Vec<(&'static str, String)> {
        let head = "@startuml\nrobust A\nrobust B\n";
        let at = |body: &str| format!("{head}{body}@enduml\n");
        vec![
            (
                "title",
                format!("@startuml\ntitle {f}\nrobust A\n@0\nA is x\n@enduml\n"),
            ),
            (
                "signal name",
                format!("@startuml\nrobust \"{f}\" as A\nrobust B\n@0\nA is x\nB is y\n@enduml\n"),
            ),
            (
                "robust state label",
                at(&format!("A has \"{f}\" as x\n@0\nA is x\n@50\nA is y\n")),
            ),
            (
                "concise state label",
                format!(
                    "@startuml\nconcise A\nrobust B\n@0\nA is \"{f}\"\nB is y\n@50\nA is z\n@enduml\n"
                ),
            ),
            (
                "change note",
                at(&format!("@0\nA is x : {f}\n@50\nA is y\n")),
            ),
            (
                "note above",
                at(&format!("@0\nA is x\n@50\nA is y\nnote top of A : {f}\n")),
            ),
            (
                "note below",
                at(&format!(
                    "@0\nA is x\n@50\nA is y\nnote bottom of B : {f}\n"
                )),
            ),
            (
                "signal constraint",
                at(&format!("@0\nA is x\n@50\nA is y\nA@0 <-> @50 : {{{f}}}\n")),
            ),
            (
                "free constraint",
                at(&format!("@0\nA is x\n@50\nA is y\n@0 <-> @50 : {{{f}}}\n")),
            ),
            (
                "highlight",
                at(&format!(
                    "@0\nA is x\n@50\nA is y\nhighlight 0 to 50 : {f}\n"
                )),
            ),
            (
                "message",
                at(&format!(
                    "@0\nA is x\nB is y\n@50\nA is z\nA@0 -> B@50 : {f}\n"
                )),
            ),
        ]
    }

    /// A formula well over a robust signal's two levels (40 pixels) tall.
    const TALL: &str = r"<latex>\begin{matrix} a \\ b \\ c \\ d \\ e \end{matrix}</latex>";

    /// How many filled paths without a stroke the scene has, of any color: a formula's glyphs
    /// and rules, and a diagram's arrowheads.
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

    /// Every text position of a timing diagram draws a formula as paths, in both renderers.
    #[test]
    fn every_timing_position_draws_its_formula() {
        let plain = timing_positions("x");
        for ((position, src), (_, flat)) in
            timing_positions("<latex>x^2</latex>").iter().zip(&plain)
        {
            let lowered = diagram::lower(src);
            assert!(
                lowered.diagnostics.is_empty(),
                "{position}: {:?}",
                lowered.diagnostics
            );
            let s = scene(src);
            assert!(filled(&s) > filled(&scene(flat)), "{position}");
            assert!(
                texts(&s).iter().all(|t| !t.contains("latex>")),
                "{position}: {:?}",
                texts(&s)
            );
            let rows = ascii(src);
            assert!(!rows.contains("latex>"), "{position}: {rows}");
            assert!(rows.contains('2'), "{position}: {rows}");
        }
    }

    /// A formula taller than the text makes its row taller in the scene and its rows more in
    /// text, in every position, as a plain label does not.
    #[test]
    fn a_tall_formula_grows_the_row_it_is_in() {
        let plain = timing_positions("x");
        for ((position, tall), (_, flat)) in timing_positions(TALL).iter().zip(&plain) {
            let (a, b) = (scene(tall), scene(flat));
            // A title or a highlight caption is in a lane that grows, as a row does.
            assert!(a.height > b.height, "{position}: {} {}", a.height, b.height);
            let (ra, rb) = (ascii(tall), ascii(flat));
            assert!(
                ra.lines().count() > rb.lines().count(),
                "{position}:\n{ra}\n{rb}"
            );
        }
    }

    /// A plain label draws exactly as it did before formulas were read.
    #[test]
    fn plain_timing_labels_are_unchanged() {
        let flat = timing_positions("x");
        for ((position, src), (_, control)) in timing_positions("plain").iter().zip(&flat) {
            let lowered = diagram::lower(src);
            assert!(lowered.diagnostics.is_empty(), "{position}");
            let s = scene(src);
            assert_eq!(filled(&s), filled(&scene(control)), "{position}");
            assert!(texts(&s).contains(&"plain"), "{position}: {:?}", texts(&s));
        }
    }

    /// A message label of a formula is in the gap between its signals, which grows to hold it.
    #[test]
    fn a_message_formula_is_in_the_gap_between_its_signals() {
        let src = |label: &str| {
            format!(
                "@startuml\nrobust A\nrobust B\n@0\nA is x\nB is y\n@50\nA is z\nA@0 -> B@50 : {label}\n@enduml\n"
            )
        };
        let s = scene(&src(TALL));
        let flat = scene(&src("x"));
        assert!(s.height > flat.height + 8.0, "{} {}", s.height, flat.height);
        // As text, the label's rows lie between the two signals' lines: more rows than a plain
        // label's one.
        let rows = ascii(&src(TALL));
        let flat_rows = ascii(&src("x"));
        assert!(
            rows.lines().count() >= flat_rows.lines().count() + 3,
            "{rows}\n{flat_rows}"
        );
    }

    /// Formulas in every position are in the text color, light and dark.
    #[test]
    fn timing_formulas_are_in_the_text_color_in_every_position() {
        let flat = timing_positions("x");
        for ((position, src), (_, control)) in
            timing_positions("<latex>x^2</latex>").iter().zip(&flat)
        {
            if *position == "concise state label" {
                // A band's label is in the band's ink.
                continue;
            }
            // The paths in the text color, less those of the same diagram with a plain label
            // (arrowheads and rules, which a theme may color alike).
            let count = |src: &str, theme: Theme| {
                diagram::lower(src)
                    .diagram
                    .scene(&Approx, &theme)
                    .unwrap()
                    .elements
                    .iter()
                    .filter(|e| {
                        matches!(
                            &e.primitive,
                            Primitive::Path { fill: Some(f), stroke: None, .. } if *f == theme.text
                        )
                    })
                    .count()
            };
            let extra = |theme: Theme| count(src, theme) - count(control, theme);
            assert!(extra(Theme::LIGHT) > 0, "{position}");
            assert_eq!(extra(Theme::LIGHT), extra(Theme::DARK), "{position}");
        }
    }

    /// A formula's own `->`, `:`, `#`, and `,` are not a statement's.
    #[test]
    fn a_formula_may_hold_statement_punctuation() {
        let src = "@startuml\nrobust A\nrobust B\nA has \"<math>f(x,y)</math>\" as f, g\n@0\nA is f\nB is \"<math>a->b</math>\" : <math>c: d</math>\n@enduml\n";
        let lowered = diagram::lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let s = scene(src);
        assert!(
            texts(&s).iter().all(|t| !t.contains("math>")),
            "{:?}",
            texts(&s)
        );
        assert!(formula_paths(&s) > 0);
    }

    /// The fixture with a formula in every position has none failing, and each is typeset once.
    #[test]
    fn the_all_positions_fixture_is_clean() {
        let d = diagram::lower(&fixture("valid", "latex_inline_timing_all")).diagnostics;
        assert!(d.is_empty(), "{d:?}");
    }

    /// A failing formula in each position has one error on its body, on its own line, and is
    /// drawn as its source.
    #[test]
    fn failing_timing_formulas_have_one_error_each_on_their_body() {
        let src = fixture("errors", "latex_inline_fail_timing_all");
        let lowered = diagram::lower(&src);
        let at: Vec<(usize, &str)> = lowered
            .diagnostics
            .iter()
            .map(|d| {
                assert_eq!(d.severity, Severity::Error);
                (
                    d.line_col(&src).0,
                    &src[usize::from(d.range.start())..usize::from(d.range.end())],
                )
            })
            .collect();
        assert_eq!(
            at,
            [
                (2, r"\plugh"),
                (3, r"\foo"),
                (4, r"\bark"),
                (5, r"\baz"),
                (9, r"\qux"),
                (10, r"\quux"),
                (15, r"\corge"),
                (16, r"\grault"),
                (17, r"\garply"),
                (18, r"\waldo"),
                (19, r"\fred"),
            ]
        );
        let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        for body in [
            "plugh", "foo", "bark", "baz", "qux", "quux", "corge", "grault", "garply", "waldo",
            "fred",
        ] {
            assert!(
                texts(&s).iter().any(|t| t.contains(&format!(r"\{body}"))),
                "{body}: {:?}",
                texts(&s)
            );
        }
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
            ("latex_inline_timing_all", 13),
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
