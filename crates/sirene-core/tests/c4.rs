//! Mermaid C4 diagrams (MC4-*): the fixture corpus with its diagnostics and drawings (LANGS-6),
//! and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::c4::{self, Base, Body, C4};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Shape};
use sirene_core::scene::{Approx, Owner, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c4");
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
            assert_eq!(c4::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::C4, "{name}");
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

fn lowered(src: &str) -> (C4, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::C4(c) => (c, d),
        other => panic!("not a C4 diagram: {other:?}"),
    }
}

/// MC4-1, MC4-2: elements by macro, nested boundaries, arguments by position or name, and
/// relations with their direction, technology, and styles.
#[test]
fn model() {
    let (c, d) = lowered(
        "C4Container\ntitle T\nRel(u, api, \"Uses\", \"HTTPS\")\nPerson_Ext(u, \"User\", \"Someone\")\n\
         System_Boundary(sys, \"Sys\") {\n  Container(api, \"API\", $descr=\"Serves\", $techn=\"Go\")\n  \
         Deployment_Node(n, \"Node\", \"k8s\") {\n    ComponentQueue(q, Queue)\n  }\n}\n\
         SystemDb(db, DB)\nBiRel(api, db, \"Reads\")\nRel_Back(q, api, \"Feeds\")\n\
         UpdateElementStyle(u, $bgColor=\"grey\")\nUpdateRelStyle(api, db, $lineColor=\"red\")\n\
         UpdateLayoutConfig($c4ShapeInRow=\"2\", $c4BoundaryInRow=\"1\")\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let ch = &c.chart;
    assert_eq!(ch.title.as_deref(), Some("T"));
    let ids: Vec<&str> = ch.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["u", "api", "q", "db"]);
    let kinds: Vec<(Base, Body, bool)> = c
        .elements
        .iter()
        .map(|e| (e.base, e.body, e.external))
        .collect();
    assert_eq!(
        kinds,
        [
            (Base::Person, Body::Box, true),
            (Base::Container, Body::Box, false),
            (Base::Component, Body::Queue, false),
            (Base::System, Body::Db, false),
        ]
    );
    assert_eq!(c.elements[0].type_name(), "external_person");
    assert_eq!(c.elements[0].descr.as_deref(), Some("Someone"));
    assert_eq!(c.elements[0].colors.background.as_deref(), Some("grey"));
    assert_eq!(c.elements[1].techn.as_deref(), Some("Go"));
    assert_eq!(c.elements[1].descr.as_deref(), Some("Serves"));
    assert_eq!(ch.nodes[2].label.text, "Queue");
    assert_eq!(ch.nodes[3].shape, Shape::Cylinder);
    assert_eq!(c.node_parent, [None, Some(0), Some(1), None]);
    assert_eq!(c.boundary_parent, [None, Some(0)]);
    assert_eq!(c.boundaries[0].kind.as_deref(), Some("SYSTEM"));
    assert_eq!(c.boundaries[1].kind.as_deref(), Some("k8s"));
    assert!(c.boundaries[1].deployment && !c.boundaries[0].deployment);
    let rels: Vec<(&str, &str, Option<Arrow>)> = ch
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str(), e.start))
        .collect();
    assert_eq!(
        rels,
        [
            ("u", "api", None),
            ("api", "db", Some(Arrow::Point)),
            ("api", "q", None),
        ]
    );
    assert_eq!(c.rels[0].techn.as_deref(), Some("HTTPS"));
    assert_eq!(c.rels[1].colors.line.as_deref(), Some("red"));
    assert_eq!((c.shapes_in_row, c.boundaries_in_row), (2, 1));
}

/// MC4-2: repeats, unknown ids, and unsupported macros are drawn as near as they can be, with
/// a diagnostic.
#[test]
fn errors() {
    let (c, d) = lowered(
        "C4Context\nPerson(a)\nPerson(a, \"Again\")\nSystem()\nRel(a, nope, \"x\")\n\
         Person(p, P) {\n  System(s, S)\n}\nLay_R(a, p)\nUpdateLayoutConfig($c4ShapeInRow=\"x\")\n",
    );
    let e = |m: &str| (Severity::Error, m.to_owned());
    let w = |m: &str| (Severity::Warning, m.to_owned());
    assert_eq!(
        d,
        [
            e("`a` is already declared; ignoring this"),
            e("`System` needs an alias first"),
            e("no element or boundary `nope`; ignoring this"),
            w("`Person` is not a boundary; drawing what is inside it outside"),
            w("`Lay_R` is not supported; ignoring it"),
            w("`c4ShapeInRow` must be a whole number of at least 1; ignoring `x`"),
        ]
    );
    let ids: Vec<&str> = c.chart.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a", "p", "s"]);
    assert_eq!(c.chart.nodes[0].label.text, "a");
    assert!(c.chart.edges.is_empty());
    assert_eq!(c.shapes_in_row, 4);
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
        errors("C4Context\nPerson(a, \"A\n"),
        ["unclosed string: expected `\"`"]
    );
    assert_eq!(
        errors("C4Context\nBoundary(b, B) {\n"),
        ["unclosed boundary: expected `}`"]
    );
    assert!(errors("C4Dynamic\n").is_empty());
    assert!(errors("C4Component\nComponent(a, A)\n").is_empty());
}

/// MC4-3: elements fill rows from the left, boundaries go below and hold their elements, and
/// each element shows its type and label.
#[test]
fn drawing() {
    let src = "C4Context\nPerson(a, A)\nSystem(b, B)\nBoundary(g, G) {\n  System(c, C)\n}\nRel(a, b, \"Uses\")\n";
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
    // Rows share their tops; a person's label sits under its icon.
    let (pa, pb) = (center("<<person>>"), center("<<system>>"));
    assert!((pa.y - pb.y).abs() < 0.5 && a.x < b.x);
    assert!(c.y > a.y);
    assert!(center("G").y < c.y);
    assert!(center("<<person>>").y < a.y);
    assert!(s.elements.iter().any(|e| e.owner == Owner::Node(0)
        && matches!(&e.primitive, Primitive::Path { fill: Some(_), .. })));
    assert!(s.elements.iter().any(|e| matches!(e.owner, Owner::Edge(0))));
    // A long label between elements in a row spreads them apart to fit it.
    let spread = |label: &str| {
        let src = format!("C4Context\nSystem(a, A)\nSystem(b, B)\nRel(a, b, \"{label}\")\n");
        let s = diagram::lower(&src).diagram.scene(&Approx, &Theme::LIGHT);
        let x = |want: &str| {
            s.elements
                .iter()
                .find_map(|e| match &e.primitive {
                    Primitive::Text { text, center, .. } if text == want => Some(center.x),
                    _ => None,
                })
                .expect("text")
        };
        x("B") - x("A")
    };
    assert!(spread("Reads from and writes to the database") > spread("Uses") + 100.0);
    // A relation's label is drawn over the elements' text.
    let first_edge = s
        .elements
        .iter()
        .position(|e| matches!(e.owner, Owner::Edge(_)))
        .expect("an edge");
    assert!(
        s.elements[first_edge..]
            .iter()
            .all(|e| matches!(e.owner, Owner::Edge(_)))
    );
}
