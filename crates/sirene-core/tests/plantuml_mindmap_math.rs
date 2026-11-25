//! `<latex>` and `<math>` formulas in PlantUML mindmap node text (LEMB-2, LEMB-5, PCREO-3): the
//! mindmap shares Mermaid's model, so the node text is read with PlantUML's tags and not `$$`. A
//! formula is drawn in the scene and as rows in text, a failing one is drawn as its source with
//! one diagnostic on its body, and with the `math` feature off each formula is one warning.

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};

const VALID: &str = "@startmindmap\n* Energy <latex>\\alpha</latex>\n** Area <math>pi r^2</math>\n*** a <latex>\\frac{a}{b}</latex> and <LaTeX>\\beta</LaTeX>\n@endmindmap\n";

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

/// How many filled paths without a stroke the scene has.
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

#[cfg(feature = "math")]
mod math {
    use super::*;

    const FAILING: &str = "@startmindmap\n* root\n** <latex>\\fooa</latex>\n** ok <latex>\\alpha</latex> and <latex>\\foob</latex>\n@endmindmap\n";

    #[test]
    fn formulas_are_drawn_in_scene_and_text() {
        let lowered = lower(VALID);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let plain = VALID
            .replace("<latex>", "")
            .replace("</latex>", "")
            .replace("<LaTeX>", "")
            .replace("</LaTeX>", "")
            .replace("<math>", "")
            .replace("</math>", "");
        let s = scene(VALID);
        assert!(filled(&s) > filled(&scene(&plain)));
        assert!(
            texts(&s).iter().all(|t| !t.contains('<')),
            "{:?}",
            texts(&s)
        );
        let text = ascii(VALID);
        assert!(!text.contains('<'), "{text}");
        assert!(text.contains('α') && text.contains('β'), "{text}");
        assert_ne!(text, ascii(&plain));
    }

    #[test]
    fn a_failing_formula_is_one_error_on_its_body_and_drawn_as_source() {
        let lowered = lower(FAILING);
        let errors: Vec<_> = lowered.diagnostics.iter().collect();
        assert_eq!(errors.len(), 2, "{errors:?}");
        for d in errors {
            assert_eq!(d.severity, Severity::Error);
            let at = &FAILING[usize::from(d.range.start())..usize::from(d.range.end())];
            assert!(at.starts_with("\\foo"), "{at:?}");
        }
        let text = ascii(FAILING);
        assert!(text.contains("<latex>\\fooa</latex>"), "{text}");
        assert!(text.contains("<latex>\\foob</latex>"), "{text}");
        assert!(text.contains('α'), "{text}");
        let s = scene(FAILING);
        assert!(
            texts(&s)
                .iter()
                .any(|t| t.contains("<latex>\\fooa</latex>")),
            "{:?}",
            texts(&s)
        );
    }

    /// `$$` is text in PlantUML, and an unclosed tag is text.
    #[test]
    fn dollars_and_unclosed_tags_stay_text() {
        let src = "@startmindmap\n* $$x$$\n** open <latex>x\n@endmindmap\n";
        let lowered = lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let text = ascii(src);
        assert!(
            text.contains("$$x$$") && text.contains("<latex>x"),
            "{text}"
        );
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    #[test]
    fn each_formula_is_one_warning_and_drawn_as_source() {
        let lowered = lower(VALID);
        assert_eq!(lowered.diagnostics.len(), 4, "{:?}", lowered.diagnostics);
        assert!(
            lowered
                .diagnostics
                .iter()
                .all(|d| d.severity == Severity::Warning)
        );
        let text = ascii(VALID);
        assert!(text.contains("<latex>\\alpha</latex>"), "{text}");
        assert!(
            texts(&scene(VALID)).iter().any(|t| t.contains("<latex>")),
            "{:?}",
            texts(&scene(VALID))
        );
    }
}
