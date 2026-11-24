//! The 128 KaTeX screenshot inputs (`ss_data.yaml`) typeset and drawn as SVG (LMATH-13 item 3).
//!
//! Each input is set in the style KaTeX's screenshotter uses (display only where the yaml says
//! `display: 1`, text otherwise). It must typeset, draw something, draw every path inside the
//! formula's box (0.1 em slack), and match its line of the committed snapshot: the number of
//! paths, the ink's bounds, the formula's box, and a hash of the SVG text.

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
];

/// Inputs known to draw outside their box, with the reason. A drawing bug to fix, not a rule.
const KNOWN_ESCAPES: &[(&str, &str)] = &[];

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

/// The 64-bit FNV-1a hash of `text`.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// One line of the snapshot: the number of paths, the ink's bounds relative to the formula's
/// origin in em (left, top, right, bottom, rounded to 0.01), the formula's box in em (width,
/// ascent, descent), and the hash of the SVG, which changes with any change to what is drawn.
fn summarize(case: &Case, formula: &Formula, origin: Pt, scene: &Scene, svg: &str) -> String {
    let mut ink: Option<(f32, f32, f32, f32)> = None;
    for e in &scene.elements {
        if let Primitive::Path { segments, clip, .. } = &e.primitive
            && let Some(b) = bounds(segments, *clip)
        {
            ink = Some(match ink {
                None => b,
                Some(o) => (o.0.min(b.0), o.1.min(b.1), o.2.max(b.2), o.3.max(b.3)),
            });
        }
    }
    // Pixels from the origin to em, rounded; `+ 0.0` turns a rounded -0 into 0. The box is
    // already in em.
    let em = |v: f32, from: f32| ((v - from) / SIZE * 100.0).round() / 100.0 + 0.0;
    let em_box = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
    let ink = ink.map_or_else(
        || "none".to_owned(),
        |(x0, y0, x1, y1)| {
            format!(
                "[{:.2} {:.2} {:.2} {:.2}]",
                em(x0, origin.x),
                em(y0, origin.y),
                em(x1, origin.x),
                em(y1, origin.y)
            )
        },
    );
    format!(
        "{}: paths={} ink={} box=[{:.2} {:.2} {:.2}] svg={:016x}\n",
        case.name,
        scene.elements.len(),
        ink,
        em_box(formula.width),
        em_box(formula.ascent),
        em_box(formula.descent),
        fnv1a(svg)
    )
}

#[test]
fn there_are_128_screenshot_inputs() {
    assert_eq!(cases().len(), 128);
}

#[test]
fn screenshot_inputs_draw_inside_their_box_and_match_snapshots() {
    let mut problems = Vec::new();
    let mut failed = Vec::new();
    let mut summary = String::new();
    for case in cases() {
        let formula = match typeset(&case.source, case.style) {
            Ok(f) => f,
            Err(e) => {
                failed.push(case.name.clone());
                summary.push_str(&format!("{}: does not typeset\n", case.name));
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
        summary.push_str(&summarize(&case, &formula, origin, &scene, &out));
    }
    insta::assert_snapshot!("summary", summary);
    for (name, _) in EXPECTED_FAILURES {
        if !failed.iter().any(|f| f == name) {
            problems.push(format!(
                "{name}: listed as an expected failure but typesets"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
