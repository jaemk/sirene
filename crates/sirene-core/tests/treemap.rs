//! Mermaid treemaps (MTREE-*): the fixture corpus with its diagnostics and drawings (LANGS-6),
//! and the model and layout each item builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::scene::{Approx, Color, Owner, Primitive, Pt, Scene, Theme};
use sirene_core::treemap::{self, Treemap};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/treemap");
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
            assert_eq!(treemap::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Treemap, "{name}");
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

fn lowered(src: &str) -> (Treemap, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Treemap(t) => (t, d),
        other => panic!("not a treemap: {other:?}"),
    }
}

/// MTREE-1, MTREE-2: indentation nests items under the last section above with less of it; a
/// leaf holds nothing, so an item indented under one goes beside it.
#[test]
fn model() {
    let (t, d) = lowered(
        "---\ntitle: T\nconfig:\n  treemap:\n    padding: 4\n    diagramPadding: 2\n    showValues: false\n    nodeWidth: 50\n    nodeHeight: 20\n    valueFormat: \".1f\"\n---\ntreemap-beta\naccTitle: at\naccDescr: ad\n\"A\"\n  \"B\"\n    'c': 1,500\n      \"d\", 2:::k\n  \"e\": 3\n\"f\": 4\nclassDef k fill:#f00\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(t.title.as_deref(), Some("T"));
    assert_eq!(
        (t.acc_title.as_deref(), t.acc_descr.as_deref()),
        (Some("at"), Some("ad"))
    );
    let shape: Vec<(&str, Option<f64>, Option<&str>)> = t
        .items
        .iter()
        .map(|i| {
            (
                i.name.as_str(),
                i.value,
                i.parent.map(|p| t.items[p].name.as_str()),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            ("A", None, None),
            ("B", None, Some("A")),
            ("c", Some(1500.0), Some("B")),
            ("d", Some(2.0), Some("B")),
            ("e", Some(3.0), Some("A")),
            ("f", Some(4.0), None),
        ]
    );
    assert_eq!(t.items[3].class.as_deref(), Some("k"));
    assert_eq!(t.items[3].styles.len(), 1);
    assert_eq!(
        (t.padding, t.diagram_padding, t.show_values),
        (4.0, 2.0, false)
    );
    assert_eq!((t.width, t.height), (500.0, 200.0));
    assert_eq!(t.format_value(2.0), "2.0");
    assert_eq!(t.values()[0], 1505.0);
    let (t, _) = lowered("treemap\n\"A\": 1\n");
    assert_eq!(
        (t.width, t.height, t.format_value(1234.0).as_str()),
        (1000.0, 400.0, "1,234")
    );
}

/// MTREE-2: a bad value leaves its item out; an empty section and an unknown class are warnings.
#[test]
fn errors() {
    let (t, d) = lowered("treemap\n\"E\"\n\"S\"\n  \"a\": 1.2.3\n  \"b\": 2:::x\n");
    let w = |m: &str| (Severity::Warning, m.to_owned());
    assert_eq!(
        d,
        [
            w(
                "section `E` holds no items, so it is not drawn; give it a value such as `\"E\": 1`, \
               or indent items under it"
            ),
            (
                Severity::Error,
                "`1.2.3` is not a number; ignoring `a`".to_owned()
            ),
            w("class `x` has no `classDef`; ignoring it"),
        ]
    );
    let names: Vec<&str> = t.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["E", "S", "b"]);
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
        errors("treemap\n\"A\": x\n"),
        ["expected a value such as `12`"]
    );
    assert!(errors("treemap-beta\n").is_empty());
}

fn texts(s: &Scene) -> Vec<(String, Pt, f32)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text {
                text, center, size, ..
            } => Some((text.clone(), *center, *size)),
            _ => None,
        })
        .collect()
}

/// MTREE-3: a section's name sits in its header above its leaves; the larger leaf gets the
/// larger box; a leaf takes its section's color.
#[test]
fn drawing() {
    let src = "treemap-beta\n\"S\"\n  \"big\": 30\n  \"small\": 10\n";
    let (t, _) = lowered(src);
    let cells = t.layout();
    let area = |i: usize| (cells[i].x1 - cells[i].x0) * (cells[i].y1 - cells[i].y0);
    assert!(area(1) > 2.5 * area(2) && area(1) < 3.5 * area(2));
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| t.iter().find(|(x, ..)| x == want).unwrap().clone();
    let (section, big, small) = (at("S"), at("big"), at("small"));
    assert!(section.1.y < big.1.y && section.1.y < small.1.y);
    assert!(big.2 >= small.2);
    assert!(t.iter().any(|(x, ..)| x == "40") && t.iter().any(|(x, ..)| x == "30"));
    let fills: Vec<Color> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path { fill: Some(f), .. } => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), 3);
    let rgb = |c: Color| (c.r, c.g, c.b);
    assert!(fills.iter().all(|&f| rgb(f) == rgb(fills[0])));
    assert!(fills[0].a > fills[1].a);
}
