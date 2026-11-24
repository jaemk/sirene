//! The 128 KaTeX screenshot inputs (`ss_data.yaml`) typeset and drawn as SVG (LMATH-13 item 3).
//!
//! Each input is set in the style KaTeX's screenshotter uses (display only where the yaml says
//! `display: 1`, text otherwise). It must typeset, draw something, draw every path inside the
//! formula's box (0.1 em slack), and match its committed snapshot.

#![cfg(feature = "math")]

use std::fs;
use std::path::Path;

use sirene_core::math::{Formula, MathStyle, typeset};
use sirene_core::scene::{Color, Element, Owner, Primitive, Pt, Rect, Scene, Segment, svg};

/// Pixels per em in the snapshots.
const SIZE: f32 = 16.0;
/// Margin around the formula's box, in pixels.
const PAD: f32 = 4.0;
/// How far a path may stray outside the formula's box, in em.
const SLACK: f32 = 0.1;

/// Inputs that are expected not to typeset, with the reason.
const EXPECTED_FAILURES: &[(&str, &str)] = &[
    (
        "UnsupportedCmds",
        "KaTeX rejects it too (`\\err` is undefined); the yaml marks it noThrow",
    ),
    (
        "SurrogatePairs",
        "mathematical alphanumeric characters are not laid out yet",
    ),
];

/// Inputs known to draw outside their box, with the reason. A drawing bug to fix, not a rule.
const KNOWN_ESCAPES: &[(&str, &str)] = &[(
    "CD",
    "a horizontal arrow in the diagram ends 0.1 to 0.2 em past the right edge of the box",
)];

struct Case {
    name: String,
    source: String,
    style: MathStyle,
}

fn cases() -> Vec<Case> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/math/katex-cases.json");
    let all: Vec<serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(path).expect("fixture exists"))
            .expect("fixture is json");
    let mut out = Vec::new();
    for case in &all {
        let Some(name) = case["src"]
            .as_str()
            .and_then(|s| s.strip_prefix("ss_data.yaml:"))
        else {
            continue;
        };
        let mut source = String::new();
        // KaTeX's `macros` setting, as definitions ahead of the input.
        if let Some(macros) = case["macros"].as_object() {
            for (name, body) in macros {
                source.push_str(&format!(r"\def{name}{{{}}}", body.as_str().unwrap_or("")));
            }
        }
        source.push_str(case["input"].as_str().expect("input is a string"));
        out.push(Case {
            name: name.to_owned(),
            source,
            style: if case["display"].as_bool().unwrap_or(false) {
                MathStyle::Display
            } else {
                MathStyle::Text
            },
        });
    }
    out
}

fn scene_for(formula: &Formula) -> (Scene, Pt) {
    let origin = Pt::new(PAD, PAD + formula.ascent * SIZE);
    let mut scene = Scene {
        width: formula.width * SIZE + 2.0 * PAD,
        height: (formula.ascent + formula.descent) * SIZE + 2.0 * PAD,
        background: Color::rgb(0xffffff),
        elements: vec![],
    };
    formula.draw_into(&mut scene, Owner::Diagram, origin, SIZE, Color::rgb(0));
    (scene, origin)
}

/// The bounds of the points of `segments` (control points included), cut to `clip` if any.
fn bounds(segments: &[Segment], clip: Option<Rect>) -> Option<(f32, f32, f32, f32)> {
    let mut pts = Vec::new();
    for s in segments {
        match s {
            Segment::Move(a) | Segment::Line(a) => pts.push(*a),
            Segment::Arc { to, .. } => pts.push(*to),
            Segment::Cubic { c1, c2, to } => pts.extend([*c1, *c2, *to]),
            _ => {}
        }
    }
    let first = pts.first()?;
    let mut b = (first.x, first.y, first.x, first.y);
    for p in &pts {
        b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
    }
    if let Some(c) = clip {
        b = (
            b.0.max(c.x),
            b.1.max(c.y),
            b.2.min(c.x + c.w),
            b.3.min(c.y + c.h),
        );
        if b.0 > b.2 || b.1 > b.3 {
            return None;
        }
    }
    Some(b)
}

/// Every path of `elements` that leaves the formula's box, as a message.
fn escapes(formula: &Formula, origin: Pt, elements: &[Element]) -> Vec<String> {
    let slack = SLACK * SIZE;
    let (left, right) = (origin.x - slack, origin.x + formula.width * SIZE + slack);
    let top = origin.y - formula.ascent * SIZE - slack;
    let bottom = origin.y + formula.descent * SIZE + slack;
    let mut out = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        let Primitive::Path { segments, clip, .. } = &e.primitive else {
            out.push(format!("element {i} is not a path"));
            continue;
        };
        let Some((x0, y0, x1, y1)) = bounds(segments, *clip) else {
            continue;
        };
        if x0 < left || x1 > right || y0 < top || y1 > bottom {
            out.push(format!(
                "element {i} spans x {x0:.2}..{x1:.2} y {y0:.2}..{y1:.2}, box is x {left:.2}..{right:.2} y {top:.2}..{bottom:.2}"
            ));
        }
    }
    out
}

#[test]
fn there_are_128_screenshot_inputs() {
    assert_eq!(cases().len(), 128);
}

#[test]
fn screenshot_inputs_draw_inside_their_box_and_match_snapshots() {
    let mut problems = Vec::new();
    let mut failed = Vec::new();
    for case in cases() {
        let formula = match typeset(&case.source, case.style) {
            Ok(f) => f,
            Err(e) => {
                failed.push(case.name.clone());
                if !EXPECTED_FAILURES.iter().any(|(n, _)| *n == case.name) {
                    problems.push(format!("{}: does not typeset: {e}", case.name));
                }
                continue;
            }
        };
        let (scene, origin) = scene_for(&formula);
        if scene.elements.is_empty() {
            problems.push(format!("{}: draws nothing", case.name));
        }
        let out_of_box = escapes(&formula, origin, &scene.elements);
        let known = KNOWN_ESCAPES.iter().any(|(n, _)| *n == case.name);
        if known && out_of_box.is_empty() {
            problems.push(format!(
                "{}: listed as escaping but stays inside",
                case.name
            ));
        }
        if !known {
            for m in out_of_box {
                problems.push(format!("{}: {m}", case.name));
            }
        }
        let out = svg::write(&scene);
        if out.is_empty() {
            problems.push(format!("{}: empty svg", case.name));
        }
        insta::assert_snapshot!(case.name.clone(), out, &case.source);
    }
    for (name, _) in EXPECTED_FAILURES {
        if !failed.iter().any(|f| f == name) {
            problems.push(format!(
                "{name}: listed as an expected failure but typesets"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
