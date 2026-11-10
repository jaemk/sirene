//! Mermaid quadrant charts (MQUAD-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::quadrant::{self, PointStyle, QuadrantChart};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme, parse_color};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quadrant");
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

fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(out, "{line}:{col}: {severity}: {}", d.message).unwrap();
    }
    out
}

/// Each fixture's diagnostics and drawing; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(quadrant::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Quadrant, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
                assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = diagram::lower(&src).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn lowered(src: &str) -> (QuadrantChart, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Quadrant(q) => (q, d),
        other => panic!("not a quadrant chart: {other:?}"),
    }
}

fn chart(src: &str) -> QuadrantChart {
    let (q, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    q
}

/// MQUAD-1, MQUAD-2: the title, axes, quadrant labels, and points.
#[test]
fn points() {
    let q = chart(
        "quadrantChart\n  title Reach\n  accTitle: at\n  accDescr: ad\n  x-axis Low --> High\n  y-axis \"Bottom\"\n  quadrant-2 Promote\n  quadrant-4 \"Improve\"\n  A: [0.3, 0.6]\n  \"B: c\": [1, 0]\n",
    );
    assert_eq!(q.title.as_deref(), Some("Reach"));
    assert_eq!(q.acc_title.as_deref(), Some("at"));
    assert_eq!(q.acc_descr.as_deref(), Some("ad"));
    assert_eq!(q.x_axis.low.as_deref(), Some("Low"));
    assert_eq!(q.x_axis.high.as_deref(), Some("High"));
    assert_eq!(q.y_axis.low.as_deref(), Some("Bottom"));
    assert_eq!(q.y_axis.high, None);
    assert_eq!(
        q.quadrants,
        [None, Some("Promote".into()), None, Some("Improve".into())]
    );
    let pts: Vec<(&str, f64, f64)> = q
        .points
        .iter()
        .map(|p| (p.name.as_str(), p.x, p.y))
        .collect();
    assert_eq!(pts, [("A", 0.3, 0.6), ("B: c", 1.0, 0.0)]);
    // Frontmatter gives the title when there is no `title`; a later statement replaces an
    // earlier one.
    let q = chart("---\ntitle: Front\n---\nquadrantChart\n  quadrant-1 a\n  quadrant-1 b\n");
    assert_eq!(q.title.as_deref(), Some("Front"));
    assert_eq!(q.quadrants[0].as_deref(), Some("b"));
}

/// MQUAD-2: a class's styles apply first and a point's own after, whichever comes first in the
/// source.
#[test]
fn styles() {
    let q = chart(
        "quadrantChart\n  A:::hot: [0.1, 0.1] color: #00f\n  B: [0.2, 0.2] radius: 8, stroke-color: red, stroke-width: 2px\n  classDef hot,cold color: #f00, radius: 9\n",
    );
    assert_eq!(q.points[0].class.as_deref(), Some("hot"));
    assert_eq!(
        q.points[0].style,
        PointStyle {
            radius: Some(9.0),
            color: parse_color("#00f"),
            ..PointStyle::default()
        }
    );
    assert_eq!(
        q.points[1].style,
        PointStyle {
            radius: Some(8.0),
            stroke_color: parse_color("red"),
            stroke_width: Some(2.0),
            ..PointStyle::default()
        }
    );
}

/// MQUAD-2: a coordinate that is not a number is an error and the point is skipped; one outside
/// 0 to 1 is clamped, and unknown styles and classes are ignored, each with a warning.
#[test]
fn errors() {
    let (q, d) = lowered(
        "quadrantChart\n  a: [x, 0]\n  b: [2, -1]\n  c: [0, 0] size: 3, color: nope, radius\n  d:::none: [0, 0]\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            (
                Severity::Error,
                "`x` is not a number; expected a value from 0 to 1".to_owned()
            ),
            w("`2` is outside 0 to 1; drawing it at 1"),
            w("`-1` is outside 0 to 1; drawing it at 0"),
            w("ignoring `radius`: expected `name: value`"),
            w(
                "ignoring `size`: a point's styles are `radius`, `color`, `stroke-color`, and `stroke-width`"
            ),
            w("ignoring `color: nope`: not a valid value"),
            w("no `classDef none`; drawing `d` with the default style"),
        ]
    );
    let pts: Vec<(&str, f64, f64)> = q
        .points
        .iter()
        .map(|p| (p.name.as_str(), p.x, p.y))
        .collect();
    assert_eq!(pts, [("b", 1.0, 0.0), ("c", 0.0, 0.0), ("d", 0.0, 0.0)]);
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(
        errors("quadrantChart\n  A\n"),
        ["expected `[x, y]` after the point's name"]
    );
    assert_eq!(
        errors("quadrantChart\n  : [0, 0]\n"),
        [
            "expected a point (`name: [x, y]`), `title`, `x-axis`, `y-axis`, `quadrant-1` to `quadrant-4`, or `classDef`"
        ]
    );
    assert!(errors("quadrantChart\n").is_empty());
}

fn texts(s: &Scene) -> Vec<(String, Pt)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.clone(), *center)),
            _ => None,
        })
        .collect()
}

/// MQUAD-3: quadrant labels in their quadrants, axis labels outside the square, and points where
/// their coordinates put them, named below.
#[test]
fn drawing() {
    let src = "quadrantChart\n  title T\n  x-axis Left --> Right\n  y-axis Bottom --> Top\n  quadrant-1 One\n  quadrant-2 Two\n  quadrant-3 Three\n  quadrant-4 Four\n  P: [0.25, 0.75]\n  Q: [1, 0]\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (one, two, three, four) = (at("One"), at("Two"), at("Three"), at("Four"));
    assert!(two.x < one.x && (one.y - two.y).abs() < 0.01);
    assert!(three.x < four.x && one.y < four.y && (three.x - two.x).abs() < 0.01);
    let (left, right, bottom, top) = (at("Left"), at("Right"), at("Bottom"), at("Top"));
    assert!(left.x < right.x && left.y > four.y && (left.y - right.y).abs() < 0.01);
    assert!(top.y < bottom.y && top.x < two.x && bottom.x < three.x);
    assert!(at("T").y < one.y);
    // The square spans the inner lines; points sit by their coordinates.
    let (p, q) = (at("P"), at("Q"));
    assert!((p.x - (two.x + one.x) / 2.0).abs() < 100.5);
    assert!(p.x < q.x && p.y < q.y && q.x > four.x);
    for (_, c) in &t {
        assert!(c.x > 0.0 && c.x < s.width && c.y > 0.0 && c.y < s.height);
    }
    // A point's style: its radius and fill.
    let s = diagram::lower("quadrantChart\n  A: [0.5, 0.5] radius: 20, color: #123456\n")
        .diagram
        .scene(&Approx, &Theme::DARK);
    let circle = s.elements.iter().find_map(|e| match &e.primitive {
        Primitive::Path {
            segments,
            fill: Some(f),
            ..
        } if *f == parse_color("#123456").unwrap() => Some(segments.clone()),
        _ => None,
    });
    match circle.as_deref() {
        Some(
            [
                sirene_core::scene::Segment::Move(a),
                sirene_core::scene::Segment::Arc { to, .. },
                ..,
            ],
        ) => {
            assert!((to.x - a.x - 40.0).abs() < 0.01)
        }
        other => panic!("no styled point: {other:?}"),
    }
    // Without points, quadrant labels sit in the middle of their quadrants.
    let s = diagram::lower("quadrantChart\n  quadrant-1 A\n  quadrant-4 B\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    assert!((t[1].1.y - t[0].1.y - 200.0).abs() < 0.01, "{t:?}");
}
