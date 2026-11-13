//! Mermaid radar charts (MRADAR-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::radar::{self, Graticule, Radar};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Segment, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/radar");
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
            assert_eq!(radar::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Radar, "{name}");
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

fn lowered(src: &str) -> (Radar, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Radar(r) => (r, d),
        other => panic!("not a radar chart: {other:?}"),
    }
}

/// MRADAR-1, MRADAR-2: axes with and without labels, listed and keyed values, options, and
/// frontmatter config.
#[test]
fn model() {
    let (r, d) = lowered(
        "---\ntitle: Front\nconfig:\n  radar:\n    width: 400\n    marginLeft: 10\n    axisLabelFactor: 1.2\n---\nradar-beta\n  accTitle: at\n  curve k[\"Keyed\"]{ y: 2, x: 1,\n    z: 3 }\n  axis x['E\\'x'], y\n  axis z\n  curve l{4, 5, 6}\n  graticule polygon, ticks 4\n  min -1, max 10\n  showLegend false\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(r.title.as_deref(), Some("Front"));
    assert_eq!(r.acc_title.as_deref(), Some("at"));
    let axes: Vec<(&str, &str)> = r
        .axes
        .iter()
        .map(|a| (a.id.as_str(), a.label.as_str()))
        .collect();
    assert_eq!(axes, [("x", "E'x"), ("y", "y"), ("z", "z")]);
    let curves: Vec<(&str, &[f64])> = r
        .curves
        .iter()
        .map(|c| (c.label.as_str(), c.values.as_slice()))
        .collect();
    assert_eq!(
        curves,
        [("Keyed", &[1.0, 2.0, 3.0][..]), ("l", &[4.0, 5.0, 6.0][..])]
    );
    assert_eq!(
        (r.graticule, r.ticks, r.min, r.max, r.show_legend),
        (Graticule::Polygon, 4, -1.0, Some(10.0), false)
    );
    assert_eq!((r.width, r.height, r.margins[3]), (400.0, 600.0, 10.0));
    assert_eq!(r.axis_label_factor, 1.2);
    // A `title` statement wins over frontmatter, and the defaults hold without options.
    let (r, _) = lowered("---\ntitle: Front\n---\nradar-beta\n  title Body\n");
    assert_eq!(r.title.as_deref(), Some("Body"));
    assert_eq!(
        (r.graticule, r.ticks, r.min, r.max, r.show_legend),
        (Graticule::Circle, 5, 0.0, None, true)
    );
}

/// MRADAR-2: value counts that miss the axes, unknown or repeated keys, bad options, and a
/// maximum at or under the minimum.
#[test]
fn errors() {
    let (r, d) = lowered(
        "---\nconfig:\n  radar:\n    curveTension: round\n---\nradar-beta\n  axis a, b, c\n  axis a[\"Again\"]\n  curve short{1, 2}\n  curve long{1, 2, 3, 4}\n  curve keyed{a: 1, z: 2, a: 3}\n  curve mixed{a: 1, 2}\n  graticule square\n  ticks 0\n  showLegend maybe\n  max -1\n",
    );
    let e = |m: &str| (Severity::Error, m.to_owned());
    assert_eq!(
        d,
        [
            (
                Severity::Warning,
                "ignoring `curveTension: round`: not a valid value".to_owned()
            ),
            e("axis `a` is already defined; ignoring this one"),
            e("`short` has 2 values for 3 axes; drawing the missing ones at the minimum"),
            e("`long` has 4 values for 3 axes; ignoring the extra ones"),
            e("`keyed` has no value for `b`, `c`; drawing it at the minimum"),
            e("`z` in `keyed` is not an axis; ignoring it"),
            e("`keyed` has two values for `a`; using the first"),
            e("`mixed` has no value for `b`, `c`; drawing it at the minimum"),
            e("expected `axis: value` in `mixed`, as its other values are"),
            e("expected `circle` or `polygon` after `graticule`, not `square`"),
            e("expected a whole number from 1 to 100 after `ticks`, not `0`"),
            e("expected `true` or `false` after `showLegend`, not `maybe`"),
            e("`max -1` is not above the minimum, 0; using the largest value"),
        ]
    );
    assert_eq!(r.axes.len(), 3);
    let values: Vec<&[f64]> = r.curves.iter().map(|c| c.values.as_slice()).collect();
    assert_eq!(
        values,
        [
            &[1.0, 2.0, 0.0][..],
            &[1.0, 2.0, 3.0][..],
            &[1.0, 0.0, 0.0][..],
            &[1.0, 0.0, 0.0][..],
        ]
    );
    assert_eq!((r.max, r.ticks, r.show_legend), (None, 5, true));
    assert_eq!(r.curve_tension, Radar::default().curve_tension);
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
        errors("radar-beta\n  axis a\n  curve c{1 2}\n"),
        ["expected `,` or `}`"]
    );
    assert!(errors("radar-beta\n").is_empty());
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

/// The points a curve passes through: each cubic's end.
fn curve_points(s: &Scene, k: usize) -> Vec<Pt> {
    let curves: Vec<&Vec<Segment>> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path { segments, .. }
                if segments.iter().any(|g| matches!(g, Segment::Cubic { .. }))
                    && segments.len() > 2
                    && !segments.iter().any(|g| matches!(g, Segment::Arc { .. })) =>
            {
                Some(segments)
            }
            _ => None,
        })
        .collect();
    curves[k]
        .iter()
        .filter_map(|g| match g {
            Segment::Cubic { to, .. } => Some(*to),
            _ => None,
        })
        .collect()
}

/// MRADAR-3: the first axis points up and the rest go clockwise; a value's distance from the
/// center is its share of the range; the legend names each curve.
#[test]
fn drawing() {
    let src = "radar-beta\n  axis n[\"North\"], e[\"East\"], s[\"South\"], w[\"West\"]\n  curve c[\"Cee\"]{10, 5, 0, 10}\n  max 10\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| t.iter().find(|(x, _)| x == want).map(|&(_, p)| p).unwrap();
    let (n, e, so, w) = (at("North"), at("East"), at("South"), at("West"));
    assert!(n.y < e.y && e.x > n.x && so.y > e.y && w.x < n.x);
    let center = Pt::new(n.x, e.y);
    let pts = curve_points(&s, 0);
    assert_eq!(pts.len(), 4);
    // Cubics end at the second point onward, closing at the first.
    let (east, south, west, north) = (pts[0], pts[1], pts[2], pts[3]);
    let full = center.y - north.y;
    assert!(full > 100.0);
    assert!((east.x - center.x - full / 2.0).abs() < 0.5);
    assert!((south.y - center.y).abs() < 0.5 && (south.x - center.x).abs() < 0.5);
    assert!((center.x - west.x - full).abs() < 0.5);
    assert!(t.iter().any(|(x, _)| x == "Cee"));

    let hidden = src.replace("max 10", "max 10\n  showLegend false");
    let s = diagram::lower(&hidden)
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert!(!texts(&s).iter().any(|(x, _)| x == "Cee"));
}
