//! `<latex>` and `<math>` formulas in the document `title`, a sequence's group titles and frame
//! labels (`alt`, `else`, `==`, `...`), and JSON and YAML values (LEMB-2, PCREO-3): drawn as
//! formula paths in the scene and as rows in text, a failing one drawn as its source with one
//! diagnostic on its body, and with the `math` feature off the tags stay text with one warning
//! per formula.

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

/// How many filled paths of the scene are in the text color: a formula's glyphs and rules.
#[cfg(feature = "math")]
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

/// The fixtures with a formula in each of the new positions.
const VALID: [&str; 6] = [
    "latex_inline_seq_title",
    "latex_inline_seq_group",
    "latex_inline_seq_frames",
    "latex_inline_title",
    "latex_inline_json",
    "latex_inline_yaml",
];

#[cfg(not(feature = "math"))]
const ERRORS: [&str; 5] = [
    "latex_inline_fail_seq_title",
    "latex_inline_fail_seq_group",
    "latex_inline_fail_seq_frames",
    "latex_inline_fail_title",
    "latex_inline_fail_json",
];

#[cfg(feature = "math")]
mod math {
    use super::*;
    use sirene_core::ascii::{Charset, Options};
    use sirene_core::diagram::Diagram;

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

    /// The formula in a title is set in a title as tall as the formula, with the text of the
    /// title beside it, and the diagram still fits the scene.
    #[test]
    fn a_sequence_title_with_a_formula_is_in_the_scene() {
        let plain = scene("@startuml\ntitle Area of the curve\nA -> B : go\n@enduml\n");
        let with = scene(&fixture("valid", "latex_inline_seq_title"));
        assert!(formula_paths(&with) > formula_paths(&plain));
        assert!(texts(&with).contains(&"Area"), "{:?}", texts(&with));
        assert!(texts(&with).contains(&"of the curve"), "{:?}", texts(&with));
        // The integral is taller than a text line, so the title makes the scene taller.
        assert!(
            with.height > plain.height,
            "{} {}",
            with.height,
            plain.height
        );
        for e in &with.elements {
            if let Primitive::Text { center, .. } = &e.primitive {
                assert!(center.x >= 0.0 && center.x <= with.width, "{center:?}");
            }
        }
    }

    #[test]
    fn a_graph_title_with_a_formula_is_in_the_scene_and_in_text() {
        let src = fixture("valid", "latex_inline_title");
        let (title, _) = match diagram::lower(&src).diagram {
            Diagram::Graph(g) => (g.title.clone(), g),
            other => panic!("not a graph: {other:?}"),
        };
        assert!(title.is_some_and(|t| t.contains("<latex>x^2</latex>")));
        let s = scene(&src);
        assert!(formula_paths(&s) >= 2, "{}", formula_paths(&s));
        assert!(texts(&s).contains(&"Graph"), "{:?}", texts(&s));
        let rows = ascii(&src);
        let first = rows.lines().next().expect("a title row");
        assert!(first.contains("Graph"), "{rows}");
        // The title is above the drawing and centered on it.
        assert!(rows.lines().any(|l| l.contains('A')), "{rows}");
    }

    /// A Mermaid title with `$$` stays text.
    #[test]
    fn a_mermaid_title_stays_text() {
        let src = "---\ntitle: $$x^2$$\n---\nflowchart TD\n  A --> B\n";
        let s = scene(src);
        assert!(texts(&s).contains(&"$$x^2$$"), "{:?}", texts(&s));
    }

    #[test]
    fn a_group_title_frames_and_dividers_draw_formulas_and_grow_their_rows() {
        let plain = scene(
            "@startuml\nA -> B : go\nalt x\n  B -> A : big\nelse y\n  B -> A : small\nend\n== part ==\n... after ...\nA -> B : again\n@enduml\n",
        );
        let with = scene(&fixture("valid", "latex_inline_seq_frames"));
        assert!(formula_paths(&with) > formula_paths(&plain));
        // A fraction in a divider and a delta in a delay are taller than the text lines.
        assert!(
            with.height > plain.height,
            "{} {}",
            with.height,
            plain.height
        );
        let t = texts(&with);
        assert!(t.contains(&"alt"), "{t:?}");
        assert!(t.contains(&"part"), "{t:?}");

        let group = scene(&fixture("valid", "latex_inline_seq_group"));
        assert!(formula_paths(&group) > 0);
        assert!(texts(&group).contains(&"Group"), "{:?}", texts(&group));
    }

    #[test]
    fn a_tall_group_title_makes_room_for_itself() {
        let height = |title: &str| {
            scene(&format!(
                "@startuml\nbox \"{title}\"\nparticipant A\nend box\nA -> A : go\n@enduml\n"
            ))
            .height
        };
        assert!(height(r"<latex>\frac{\frac{a}{b}}{\frac{c}{d}}</latex>") > height("x"));
        let rows = ascii(
            "@startuml\nbox \"<latex>\\frac{1}{2}</latex>\"\nparticipant A\nend box\nA -> A : go\n@enduml\n",
        );
        assert!(rows.contains('1') && rows.contains('2'), "{rows}");
        assert!(!rows.contains("latex>"), "{rows}");
        // The title's rows are above the header and do not overwrite it.
        assert!(rows.contains('A'), "{rows}");
    }

    #[test]
    fn frame_labels_are_rows_in_text_and_keep_the_frame_aligned() {
        let rows = ascii(&fixture("valid", "latex_inline_seq_frames"));
        assert!(rows.contains("alt"), "{rows}");
        assert!(rows.contains("big") && rows.contains("small"), "{rows}");
        // Every row has the same width: the formulas' rows did not shift the frame.
        let widths: Vec<usize> = rows
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().count())
            .collect();
        let widest = widths.iter().copied().max().unwrap_or(0);
        assert!(
            widths.iter().filter(|&&w| w + 40 < widest).count() < widths.len() / 2,
            "{rows}"
        );
    }

    #[test]
    fn json_and_yaml_values_draw_formulas() {
        for name in ["latex_inline_json", "latex_inline_yaml"] {
            let src = fixture("valid", name);
            let s = scene(&src);
            assert!(formula_paths(&s) >= 2, "{name}: {}", formula_paths(&s));
            assert!(texts(&s).contains(&"plain"), "{name}: {:?}", texts(&s));
        }
    }

    /// A failing formula is drawn as its source and has one error on its body.
    #[test]
    fn a_failing_formula_has_one_error_on_its_body_and_draws_its_source() {
        for (name, text) in [
            ("latex_inline_fail_seq_title", r"\foo"),
            ("latex_inline_fail_seq_group", r"\foo"),
            ("latex_inline_fail_title", r"\foo"),
            ("latex_inline_fail_json", "{x"),
        ] {
            let src = fixture("errors", name);
            let lowered = diagram::lower(&src);
            let [d] = lowered.diagnostics.as_slice() else {
                panic!("{name}: one diagnostic, got {:?}", lowered.diagnostics);
            };
            assert_eq!(d.severity, Severity::Error, "{name}");
            let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
            // An error at the end of the body is an empty range there.
            assert!(
                text.contains(at) && (at == text || at.is_empty()),
                "{name}: {at:?}"
            );
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            assert!(
                texts(&s).iter().any(|t| t.contains("<latex>")),
                "{name}: {:?}",
                texts(&s)
            );
        }
    }

    /// Each failing formula of the frame positions is one error on its own line.
    #[test]
    fn failing_frame_labels_are_each_diagnosed_on_their_line() {
        let src = fixture("errors", "latex_inline_fail_seq_frames");
        let d = diagram::lower(&src).diagnostics;
        assert_eq!(d.len(), 4, "{d:?}");
        assert!(d.iter().all(|d| d.severity == Severity::Error), "{d:?}");
        let lines: Vec<usize> = d.iter().map(|d| d.line_col(&src).0).collect();
        assert_eq!(lines, [3, 5, 8, 9], "{d:?}");
        let s = scene(&src);
        let t = texts(&s);
        for formula in [
            r"<latex>a + \foo{x}</latex>",
            r"<latex>\baz{x}</latex>",
            r"<latex>\qux</latex>",
        ] {
            assert!(t.iter().any(|x| x.contains(formula)), "{formula}: {t:?}");
        }
    }

    #[test]
    fn a_title_diagnostic_is_not_taken_by_an_equal_label_formula() {
        let src = "@startuml\ntitle <latex>\\foo</latex>\nclass A\nnote right of A : <latex>\\foo</latex>\n@enduml\n";
        let d = diagram::lower(src).diagnostics;
        assert_eq!(d.len(), 2, "{d:?}");
        let mut lines: Vec<usize> = d.iter().map(|d| d.line_col(src).0).collect();
        lines.sort_unstable();
        assert_eq!(lines, [2, 4], "{d:?}");
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    /// One warning per formula, and the tags stay text.
    #[test]
    fn tags_stay_literal_with_one_warning_per_formula() {
        for name in VALID {
            let src = fixture("valid", name);
            let formulas = src.matches("<latex>").count() + src.matches("<math>").count();
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
