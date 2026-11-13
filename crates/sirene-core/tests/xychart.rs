//! Mermaid XY charts (MXY-*): the fixture corpus with its diagnostics and drawings (LANGS-6),
//! and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Segment, Theme};
use sirene_core::xychart::{self, SeriesKind, XAxis, XyChart};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xychart");
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
            assert_eq!(xychart::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::XyChart, "{name}");
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

fn lowered(src: &str) -> (XyChart, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::XyChart(c) => (c, d),
        other => panic!("not an XY chart: {other:?}"),
    }
}

fn chart(src: &str) -> XyChart {
    let (c, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    c
}

/// MXY-1, MXY-2: the orientation, axes, and series.
#[test]
fn series() {
    let c = chart(
        "xychart-beta horizontal\n  title \"Sales\"\n  accTitle: at\n  accDescr: ad\n  x-axis Month [jan, \"feb x\"]\n  y-axis \"Revenue\" 0 --> 100\n  bar [1, 2.5]\n  line \"avg\" [5 \"hi\", .5]\n",
    );
    assert!(c.horizontal);
    assert_eq!(c.title.as_deref(), Some("Sales"));
    assert_eq!(c.acc_title.as_deref(), Some("at"));
    assert_eq!(c.acc_descr.as_deref(), Some("ad"));
    assert_eq!(c.x_title.as_deref(), Some("Month"));
    assert_eq!(
        c.x_axis,
        XAxis::Categories(vec!["jan".into(), "feb x".into()])
    );
    assert_eq!(c.y_title.as_deref(), Some("Revenue"));
    assert_eq!(c.y_range, Some((0.0, 100.0)));
    assert_eq!(c.series.len(), 2);
    assert_eq!(
        (c.series[0].kind, c.series[0].name.as_deref()),
        (SeriesKind::Bar, None)
    );
    assert_eq!(c.series[0].values, [1.0, 2.5]);
    assert_eq!(
        (c.series[1].kind, c.series[1].name.as_deref()),
        (SeriesKind::Line, Some("avg"))
    );
    assert_eq!(c.series[1].values, [5.0, 0.5]);
    assert_eq!(c.series[1].labels, [Some("hi".to_owned()), None]);

    // Defaults: vertical, positions numbered from 1, values spanning the data.
    let c = chart("---\ntitle: Front\n---\nxychart vertical\n  line [+1.3, -.34, 2]\n");
    assert!(!c.horizontal);
    assert_eq!(c.title.as_deref(), Some("Front"));
    assert_eq!(c.x_labels(), ["1", "2", "3"]);
    assert_eq!(c.y_bounds(), (-0.34, 2.0));
    // A numeric x-axis spreads each series across it; a later axis replaces an earlier one.
    let c = chart("xychart\n  x-axis [a]\n  x-axis T 0 --> 10\n  bar [1, 2, 3]\n");
    assert_eq!(
        c.x_axis,
        XAxis::Range {
            min: 0.0,
            max: 10.0
        }
    );
    assert_eq!(c.x_labels(), ["0", "5", "10"]);
}

/// MXY-2: a reversed range is a warning and turned around; categories on the y-axis are an
/// error; values past the categories are a warning and not drawn.
#[test]
fn errors() {
    let (c, d) =
        lowered("xychart\n  y-axis 9 --> 1\n  y-axis T [a]\n  bar [1, 2, 3]\n  x-axis [a, b]\n");
    assert_eq!(
        d,
        [
            (
                Severity::Warning,
                "the range runs down from 9 to 1; drawing it from 1 to 9".to_owned()
            ),
            (
                Severity::Error,
                "the y-axis takes a range (`min --> max`), not categories".to_owned()
            ),
            (
                Severity::Warning,
                "this `bar` has 3 values for 2 categories; the last 1 are not drawn".to_owned()
            ),
        ]
    );
    assert_eq!(c.y_range, Some((1.0, 9.0)));
    assert_eq!(c.y_title.as_deref(), Some("T"));
    assert_eq!(c.positions(), 2);
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
    assert_eq!(errors("xychart\n  bar [1, x]\n"), ["expected a number"]);
    assert_eq!(
        errors("xychart\n  pie\n"),
        ["expected `line`, `bar`, `x-axis`, `y-axis`, or `title`"]
    );
    assert!(errors("xychart\n").is_empty());
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

fn at(t: &[(String, Pt)], want: &str) -> Pt {
    t.iter()
        .find(|(text, _)| text == want)
        .map(|&(_, p)| p)
        .unwrap_or_else(|| panic!("no {want} in {t:?}"))
}

/// The filled rects in paint order, as (x, y, right, bottom).
fn bars(s: &Scene) -> Vec<(f32, f32, f32, f32)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path {
                segments,
                fill: Some(_),
                ..
            } if segments.len() == 5 => {
                let pts: Vec<Pt> = segments
                    .iter()
                    .filter_map(|s| match s {
                        Segment::Move(p) | Segment::Line(p) => Some(*p),
                        _ => None,
                    })
                    .collect();
                let xs = pts.iter().map(|p| p.x);
                let ys = pts.iter().map(|p| p.y);
                Some((
                    xs.clone().fold(f32::MAX, f32::min),
                    ys.clone().fold(f32::MAX, f32::min),
                    xs.fold(f32::MIN, f32::max),
                    ys.fold(f32::MIN, f32::max),
                ))
            }
            _ => None,
        })
        .collect()
}

/// MXY-3: bars up from the axis by value, categories along the bottom, ticks up the side, and a
/// legend for named series; a horizontal chart turns it all a quarter.
#[test]
fn drawing() {
    let src = "xychart\n  title T\n  x-axis X [a, b, c]\n  y-axis Y 0 --> 10\n  bar \"n\" [5, 10, 0]\n  line [1 \"lo\", 2, 3]\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let (a, b, c) = (at(&t, "a"), at(&t, "b"), at(&t, "c"));
    assert!(a.x < b.x && b.x < c.x && (a.y - c.y).abs() < 0.01);
    let (zero, ten) = (at(&t, "0"), at(&t, "10"));
    assert!(zero.x < a.x && ten.y < zero.y && zero.y < a.y);
    assert!(at(&t, "T").y < at(&t, "n").y && at(&t, "n").y < at(&t, "Y").y);
    assert!(at(&t, "X").y > a.y);
    assert!(at(&t, "lo").x - a.x < 0.01);
    // The bars: b twice as tall as a, c empty, all up from the axis.
    let r = bars(&s);
    assert_eq!(r.len(), 3, "{r:?}");
    // Values past the last category neither draw nor stretch the value axis.
    let (c, _) = lowered("xychart\n  x-axis [a, b]\n  bar [1, 2, 1000]\n");
    assert_eq!(c.y_bounds(), (1.0, 2.0));
    // On a numeric axis each series spans the whole range, whatever its length.
    let spread =
        diagram::lower("xychart\n  x-axis 0 --> 10\n  bar [1, 2, 3, 4, 5]\n  bar [1, 5]\n")
            .diagram
            .scene(&Approx, &Theme::LIGHT);
    let r2 = bars(&spread);
    let mid = |k: usize| (r2[k].0 + r2[k].2) / 2.0;
    assert_eq!(r2.len(), 7, "{r2:?}");
    assert!((mid(0) - mid(5)).abs() < 0.01 && (mid(4) - mid(6)).abs() < 0.01);
    let h = |k: usize| r[k].3 - r[k].1;
    assert!((h(1) - 2.0 * h(0)).abs() < 0.01 && h(2) < 0.01);
    assert!((r[0].3 - zero.y).abs() < 0.01);
    for (_, p) in &t {
        assert!(p.x > 0.0 && p.x < s.width && p.y > 0.0 && p.y < s.height);
    }

    let s = diagram::lower(&src.replace("xychart", "xychart horizontal"))
        .diagram
        .scene(&Approx, &Theme::DARK);
    let t = texts(&s);
    let (a, b, zero, ten) = (at(&t, "a"), at(&t, "b"), at(&t, "0"), at(&t, "10"));
    assert!(a.y < b.y && a.x < zero.x && zero.x < ten.x && zero.y > b.y);
    let r = bars(&s);
    let w = |k: usize| r[k].2 - r[k].0;
    assert!((w(1) - 2.0 * w(0)).abs() < 0.01);
}
