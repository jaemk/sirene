//! Mermaid use case diagrams (MUSE-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, Shape, Stroke};
use sirene_core::scene::{Approx, Theme};
use sirene_core::{Severity, usecase};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/usecase");
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
            assert_eq!(usecase::parse(&src).syntax().to_string(), src, "{name}");
            if name != "header" {
                assert_eq!(Kind::of(&src), Kind::UseCase, "{name}");
            }
            let lowered = usecase::parse(&src).lower();
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let lowered: Lowered = lowered.into();
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                if name != "empty" && name != "header" {
                    assert!(!s.elements.is_empty(), "{name}");
                }
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let chart = diagram::lower(&src).diagram.into_graph();
        let out = ascii::render(
            &chart,
            &Options {
                charset: Charset::Ascii,
            },
        );
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn lowered(src: &str) -> (Flowchart, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    (lowered.diagram.into_graph(), d)
}

fn chart(src: &str) -> Flowchart {
    let (c, d) = lowered(src);
    assert!(d.is_empty(), "{d:?}\n{src}");
    c
}

/// MUSE-2: declarations in any order; undeclared endpoints are ellipse use cases; quoted names
/// get generated ids; actors come first, then use cases, in first-mention order.
#[test]
fn declarations() {
    let c = chart(
        "usecase-beta\nCustomer --> Login\nactor Customer\nactor Admin(\"Main administrator\")\n\
         Login(\"Sign in\")\nReport[Generate report]\n\"Reset password\"\nAdmin --> Report\n",
    );
    assert_eq!(c.direction, Direction::LeftRight);
    let got: Vec<_> = c
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.text.as_str(), n.shape))
        .collect();
    assert_eq!(
        got,
        [
            ("Customer", "Customer", Shape::Actor),
            ("Admin", "Main administrator", Shape::Actor),
            ("Login", "Sign in", Shape::UseCase),
            ("Report", "Generate report", Shape::Rect),
            ("Reset_password", "Reset password", Shape::UseCase),
        ]
    );
}

/// MUSE-3: actor variants, business flags, and stereotypes above the label.
#[test]
fn metadata() {
    let c = chart(
        "usecase-beta\nactor H@{ type: hollow }\nactor W(\"Fan\")@{ type: awesome } <<Role>>\n\
         actor B@{ business: true }\nU(\"Do\")@{ business: true }\nH@{ type: normal }\n",
    );
    let shapes: Vec<_> = c.nodes.iter().map(|n| n.shape).collect();
    assert_eq!(
        shapes,
        [
            Shape::Actor,
            Shape::AwesomeActor,
            Shape::Actor,
            Shape::UseCase
        ]
    );
    assert_eq!(c.nodes[1].label.text, "«Role»\nFan");
    let business = |i: usize| {
        c.nodes[i]
            .metadata
            .contains(&("business".into(), "true".into()))
    };
    assert!(business(2) && business(3) && !business(0));
}

/// MUSE-4: each operator's markers, stroke, length, and label.
#[test]
fn relationships() {
    let c = chart(
        "usecase-beta\nactor A\nactor P\nA --> B\nB <--- A\nB -- C\nA --o B\nB o-- A\nA --x C\n\
         C x-- A\nA -- go ----> B\nB <-- back --- A\nA --|> P\nB ..> : Include C\n\
         C ..> : extend B\nA -- note --o C\n",
    );
    let got: Vec<_> = c
        .edges
        .iter()
        .map(|e| {
            (
                e.start,
                e.end,
                e.stroke,
                e.length,
                e.label.as_ref().map(|l| l.text.as_str()),
            )
        })
        .collect();
    let p = Some(Arrow::Point);
    let s = Stroke::Normal;
    assert_eq!(
        got,
        [
            (None, p, s, 1, None),
            (p, None, s, 2, None),
            (None, None, s, 1, None),
            (None, Some(Arrow::Circle), s, 1, None),
            (Some(Arrow::Circle), None, s, 1, None),
            (None, Some(Arrow::Cross), s, 1, None),
            (Some(Arrow::Cross), None, s, 1, None),
            (None, p, s, 3, Some("go")),
            (p, None, s, 2, Some("back")),
            (None, Some(Arrow::Triangle), s, 1, None),
            (None, p, Stroke::Dotted, 1, Some("«include»")),
            (None, p, Stroke::Dotted, 1, Some("«extend»")),
            (None, Some(Arrow::Circle), s, 1, Some("note")),
        ]
    );
}

/// MUSE-4, MUSE-7: edge ids take classes, styles, and animation metadata.
#[test]
fn edge_ids() {
    let c = chart(
        "usecase-beta\nA e1@--> B\nB e2@..> : include A\nclass e1 k\nstyle e2 stroke:#f00\n\
         e1@{ animation: slow }\ne2@{ animate: true }\n",
    );
    assert_eq!(c.edges[0].id.as_deref(), Some("e1"));
    assert_eq!(c.edges[0].classes, ["k"]);
    assert_eq!(c.edges[0].animation.as_deref(), Some("slow"));
    assert!(c.edges[0].animate && c.edges[1].animate);
    assert_eq!(c.edges[1].styles[0].property, "stroke");
}

/// MUSE-5, MUSE-6: notes, JSON tables, and system boundaries.
#[test]
fn notes_tables_boundaries() {
    let c = chart(
        "usecase-beta\nnote for U \"hi\"\nsystemBoundary \"Shop front\"\n  actor A\n  U\nend\n\
         Shop_front@{ type: package }\njson T@{ \"k\": [1, {\"x\": null}] }\nU --> T\n",
    );
    assert_eq!(c.notes.len(), 1);
    assert_eq!(c.notes[0].target, "U");
    let s = &c.subgraphs[0];
    assert_eq!(
        (s.id.as_str(), s.title.text.as_str()),
        ("Shop_front", "Shop front")
    );
    assert_eq!(s.members, ["A", "U"]);
    assert_eq!(s.metadata, [("type".to_owned(), "package".to_owned())]);
    let t = c.node("T").unwrap();
    assert_eq!(t.shape, Shape::Rect);
    let rows: Vec<Vec<&str>> = t.sections[0]
        .iter()
        .map(|r| r.cells.iter().map(String::as_str).collect())
        .collect();
    assert_eq!(rows, [vec!["k", "1"], vec!["k[1].x", "null"]]);
}

/// MUSE-7: `:::`, `class` with several classes, and `style` on nodes and boundaries.
#[test]
fn styling() {
    let c = chart(
        "usecase-beta\nactor A:::x\nB(\"b\"):::y,z\nsystemBoundary S\nend\nclass A,S p,q\n\
         style S fill:#fff\n",
    );
    assert_eq!(c.node("A").unwrap().classes, ["x", "p", "q"]);
    assert_eq!(c.node("B").unwrap().classes, ["y", "z"]);
    assert_eq!(c.subgraphs[0].classes, ["p", "q"]);
    assert_eq!(c.subgraphs[0].styles[0].property, "fill");
}

/// MUSE-8: Mermaid's rule violations are warnings; the diagram is still drawn.
#[test]
fn errors() {
    let (c, d) = lowered(
        "usecase-beta\nactor A\nactor I@{ icon: \"x\", type: hollow }\nR[r]@{ business: true }\n\
         A ..> : include B\nA --|> B\njson J@{ \"a\": 1 }\nJ --o B\nA --> C:::k\nnote for J \"n\"\n\
         class ghost k\nX@{ color: red }\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            w("actor `I` cannot combine `icon` with `type: hollow`"),
            w("rectangular use case `R` cannot be a business use case"),
            w("an include relationship joins two use cases"),
            w("a generalization joins two actors or two use cases"),
            w("a relationship with a JSON table takes only `-->`, `<--`, or `--`"),
            w(
                "ignoring `:::` on `C`: declare it (with a shape, metadata, or a stereotype) to \
                 give it classes"
            ),
            w("ignoring note for `J`: a note is for an actor or use case, not a JSON table"),
            w("unknown actor, use case, system boundary, or edge `ghost`"),
            w("ignoring metadata for unknown `X`"),
        ]
    );
    assert_eq!(c.edges.len(), 4);
}
