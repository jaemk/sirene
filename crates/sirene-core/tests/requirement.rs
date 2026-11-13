//! Mermaid requirement diagrams (MREQ-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, Stroke};
use sirene_core::scene::{Approx, Theme};
use sirene_core::{Severity, requirement};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/requirement");
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
            assert_eq!(requirement::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Requirement, "{name}");
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

fn rows(c: &Flowchart, i: usize) -> Vec<Vec<&str>> {
    c.nodes[i]
        .sections
        .iter()
        .flatten()
        .map(|r| r.cells.iter().map(String::as_str).collect())
        .collect()
}

/// MREQ-1, MREQ-2: each requirement and element, its stereotype, and its fields in order.
#[test]
fn definitions() {
    let c = chart(
        "requirementDiagram\n  DesignConstraint \"c 1\" {\n    verifyMethod: Analysis\n    RISK: low\n    text: \"a, b\"\n    id: 7\n  }\n  element e {\n    docref: d/r\n    type: t\n  }\n  element bare {\n  }\n",
    );
    let labels: Vec<&str> = c.nodes.iter().map(|n| n.label.text.as_str()).collect();
    assert_eq!(
        labels,
        [
            "«Design Constraint»\nc 1",
            "«Element»\ne",
            "«Element»\nbare"
        ]
    );
    assert_eq!(c.nodes[0].id, "c 1");
    assert_eq!(
        rows(&c, 0),
        [
            vec!["Id:", "7"],
            vec!["Text:", "a, b"],
            vec!["Risk:", "Low"],
            vec!["Verification:", "Analysis"],
        ]
    );
    assert_eq!(rows(&c, 1), [vec!["Type:", "t"], vec!["Doc Ref:", "d/r"]]);
    assert!(c.nodes[2].sections.is_empty());
    assert_eq!(c.direction, Direction::TopBottom);
}

/// MREQ-2: relationships run from source to destination in both forms; `contains` is a solid
/// line from a circle, the rest dashed arrows; each is labeled with its type.
#[test]
fn relationships() {
    let c = chart(
        "requirementDiagram\n  direction LR\n  element a {\n  }\n  element b {\n  }\n  a - satisfies -> b\n  b <- Contains - a\n",
    );
    assert_eq!(c.direction, Direction::LeftRight);
    let got: Vec<_> = c
        .edges
        .iter()
        .map(|e| {
            (
                e.from.as_str(),
                e.to.as_str(),
                e.stroke,
                e.start,
                e.end,
                e.label.as_ref().unwrap().text.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "a",
                "b",
                Stroke::Dotted,
                None,
                Some(Arrow::Point),
                "«satisfies»"
            ),
            (
                "a",
                "b",
                Stroke::Normal,
                Some(Arrow::Circle),
                None,
                "«contains»"
            ),
        ]
    );
}

/// MREQ-2: classes from `:::`, `class`, and `classDef`, and inline `style`.
#[test]
fn styling() {
    let c = chart(
        "requirementDiagram\n  element a:::x,y {\n  }\n  a:::z\n  class a w\n  style a fill:#f00\n  classDef x stroke:#00f\n",
    );
    assert_eq!(c.nodes[0].classes, ["x", "y", "z", "w"]);
    assert_eq!(c.nodes[0].styles[0].property, "fill");
    assert_eq!(c.classes[0].name, "x");
}

/// MREQ-2: bad field names and values are warnings and left out; an undefined name in a
/// relationship is drawn as a plain box, with a warning.
#[test]
fn errors() {
    let (c, d) = lowered(
        "requirementDiagram\n  requirement r {\n    risk: huge\n    type: x\n  }\n  element e {\n    id: 1\n  }\n  requirement r {\n    risk: low\n    type: y\n  }\n  e - traces -> ghost\n  e - traces -> ghost\n  class nobody k\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            w("ignoring risk `huge`: expected `Low`, `Medium`, or `High`"),
            w(
                "ignoring `type`: a requirement's fields are `id`, `text`, `risk`, and `verifymethod`"
            ),
            w("ignoring `id`: an element's fields are `type` and `docref`"),
            w("`r` is already defined; this definition is skipped"),
            w("`ghost` is not defined; drawing it as a plain box"),
            w("unknown requirement or element `nobody`"),
        ]
    );
    assert_eq!(c.nodes[2].label.text, "ghost");
    assert_eq!(c.edges.len(), 2);
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
        errors("requirementDiagram\n  a - b -> c\n"),
        [
            "expected a relationship type: `contains`, `copies`, `derives`, `satisfies`, `verifies`, `refines`, or `traces`"
        ]
    );
    assert_eq!(
        errors("requirementDiagram\n  element e {\n"),
        ["unclosed block: expected `}`"]
    );
    assert!(errors("requirementDiagram\n").is_empty());
}
