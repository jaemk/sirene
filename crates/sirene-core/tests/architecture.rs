//! Mermaid architecture diagrams (MARCH-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model and placement each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::architecture::{self, Architecture, Item, Side};
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::flowchart::Arrow;
use sirene_core::scene::{Approx, Owner, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/architecture");
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
            assert_eq!(
                architecture::parse(&src).syntax().to_string(),
                src,
                "{name}"
            );
            assert_eq!(Kind::of(&src), Kind::Architecture, "{name}");
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

fn lowered(src: &str) -> (Architecture, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Architecture(a) => (a, d),
        other => panic!("not an architecture diagram: {other:?}"),
    }
}

/// MARCH-1, MARCH-2: groups nest, services carry an icon and label, junctions have neither,
/// and an edge records its sides, arrows, label, and `{group}` ends.
#[test]
fn model() {
    let (a, d) = lowered(
        "architecture-beta\ntitle T\naccTitle: at\ngroup outer(cloud)[Outer]\ngroup inner[Inner] in outer\nservice db(database)[\"The DB\"] in inner\nservice web(server)[Web]\njunction j in outer\nweb:R --> L:db{group}\ndb:B <-[sync]- T:j\nalign column web j\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let c = &a.chart;
    assert_eq!(c.title.as_deref(), Some("T"));
    assert_eq!(c.acc_title.as_deref(), Some("at"));
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["db", "web", "j"]);
    assert_eq!(c.nodes[0].label.text, "The DB");
    assert_eq!(
        a.items,
        [
            Item::Service {
                icon: Some("database".into())
            },
            Item::Service {
                icon: Some("server".into())
            },
            Item::Junction,
        ]
    );
    let groups: Vec<&str> = c.subgraphs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(groups, ["outer", "inner"]);
    assert_eq!(a.group_icons, [Some("cloud".into()), None]);
    assert_eq!(a.group_parent, [None, Some(0)]);
    assert_eq!(a.node_parent, [Some(1), None, Some(0)]);
    let p = a.ports[0];
    assert_eq!((p.from, p.to), (1, 0));
    assert_eq!((p.from_side, p.to_side), (Side::Right, Side::Left));
    assert!(!p.from_group && p.to_group);
    assert_eq!(c.edges[0].from, "web");
    assert_eq!(c.edges[0].to, "inner");
    assert_eq!(
        (c.edges[0].start, c.edges[0].end),
        (None, Some(Arrow::Point))
    );
    assert_eq!(
        (c.edges[1].start, c.edges[1].end),
        (Some(Arrow::Point), None)
    );
    assert_eq!(c.edges[1].label.as_ref().unwrap().text, "sync");
    assert_eq!(a.aligns.len(), 1);
    assert!(!a.aligns[0].row);
    assert_eq!(a.aligns[0].nodes, [1, 2]);
}

/// MARCH-2: unknown ids, repeats, and misused groups are drawn as near as they can be, with a
/// diagnostic.
#[test]
fn errors() {
    let (a, d) = lowered(
        "architecture-beta\nservice a in nope\nservice a\ngroup g\nservice b in g\na:R -- L:x\na:R -- L:g\na{group}:R -- L:b\nalign row a\n",
    );
    let e = |m: &str| (Severity::Error, m.to_owned());
    let w = |m: &str| (Severity::Warning, m.to_owned());
    assert_eq!(
        d,
        [
            e("no group `nope` declared above; drawing `a` outside any group"),
            e("`a` is already declared; ignoring this"),
            e("no service or junction `x`; ignoring this edge"),
            e(
                "`g` is a group; an edge ends at a service or junction, with `{group}` after it to end at its group"
            ),
            w("`a` is not in a group; drawing the edge from `a` itself"),
            w("`align` needs at least two services or junctions; ignoring it"),
        ]
    );
    assert_eq!(a.chart.nodes.len(), 2);
    assert_eq!(a.chart.edges.len(), 1);
    assert!(a.aligns.is_empty());
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
    assert_eq!(errors("architecture-beta\nservice\n"), ["expected an id"]);
    assert_eq!(
        errors("architecture-beta\nservice a(server\n"),
        ["expected `)` after the icon"]
    );
    assert_eq!(
        errors("architecture-beta\nservice a\nservice b\na:X -- L:b\n"),
        ["expected a side: `L`, `R`, `T`, or `B`"]
    );
    assert_eq!(
        errors("architecture-beta\nservice a\nservice b\na:R ~~ L:b\n"),
        ["expected an edge: `--`, `-->`, `<--`, or `<-->`"]
    );
    assert_eq!(
        errors("architecture-beta\nalign diagonal a b\n"),
        ["expected `row` or `column`"]
    );
    assert!(errors("architecture-beta\n").is_empty());
    assert!(errors("architecture\nservice a\n").is_empty());
}

/// MARCH-3: an edge's sides place its ends beside each other; a group holds its services; each
/// service has an icon with its label below.
#[test]
fn drawing() {
    let src = "architecture-beta\ngroup g[G]\nservice a(server)[A] in g\nservice b(database)[B] in g\nservice c[C]\na:R --> L:b\nb:B -- T:c\n";
    let lowered = diagram::lower(src);
    let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
    let center = |want: &str| {
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == want => Some(*center),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no text {want}"))
    };
    let (a, b, c) = (center("A"), center("B"), center("C"));
    assert!((a.y - b.y).abs() < 0.5 && a.x < b.x);
    // `c` is below the group that holds `b`.
    assert!(c.y > b.y);
    assert!(center("G").y < a.y);
    for n in 0..3 {
        assert!(
            s.elements.iter().any(|e| e.owner == Owner::Node(n)
                && matches!(&e.primitive, Primitive::Path { fill: Some(_), .. })),
            "node {n} has an icon"
        );
    }
    assert_eq!(
        s.elements
            .iter()
            .filter_map(|e| match e.owner {
                Owner::Edge(i) => Some(i),
                _ => None,
            })
            .max(),
        Some(1)
    );
}
