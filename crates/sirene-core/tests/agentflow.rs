//! Mermaid agentflow diagrams (MAGF-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and how `flow`, `global`, `connector`, shapes, and edges lower.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{self, Flowchart, Shape};
use sirene_core::scene::{Approx, Owner, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/agentflow");
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
    for dir in ["valid", "warnings"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(flowchart::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Agentflow, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
                assert!(
                    lowered
                        .diagnostics
                        .iter()
                        .all(|d| d.severity == Severity::Warning),
                    "{name}"
                );
            }
            for charset in [Charset::Unicode, Charset::Ascii] {
                let mut out = diagnostics(&src, &lowered);
                out.push('\n');
                out.push_str(&lowered.diagram.ascii(&Options { charset }));
                let kind = match charset {
                    Charset::Unicode => dir,
                    Charset::Ascii => "ascii",
                };
                insta::assert_snapshot!(format!("{kind}__{name}"), out, &src);
            }
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
            }
        }
    }
}

fn lowered(src: &str) -> (Flowchart, Vec<String>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect();
    (lowered.diagram.into_graph(), d)
}

fn shape(chart: &Flowchart, id: &str) -> Shape {
    chart.nodes.iter().find(|n| n.id == id).expect(id).shape
}

/// MAGF-1: the header, with or without a direction; a flowchart is not an agentflow.
#[test]
fn header() {
    let (chart, d) = lowered("agentflow-beta LR\n  a --> b\n");
    assert!(d.is_empty(), "{d:?}");
    assert!(chart.agentflow);
    assert_eq!(chart.direction, flowchart::Direction::LeftRight);
    let (chart, _) = lowered("agentflow-beta\n  a\n");
    assert_eq!(chart.direction, flowchart::Direction::TopBottom);
    let (chart, _) = lowered("flowchart\n  a\n");
    assert!(!chart.agentflow);
}

/// MAGF-2: `flow` containers in every header form; `global` members belong to no flow;
/// `subgraph` is an error.
#[test]
fn containers() {
    let (chart, d) = lowered(
        "agentflow-beta\n  flow a[\"A\"]\n    x\n  end\n  flow b\n    y\n  end\n  flow\n    z\n  end\n\
         flow c@{ view: collapsed }\n    w\n  end\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let titles: Vec<&str> = chart
        .subgraphs
        .iter()
        .map(|s| s.title.text.as_str())
        .collect();
    assert_eq!(titles, ["A", "", "", ""]);
    assert!(chart.subgraphs[3].is_collapsed());

    let (chart, d) =
        lowered("agentflow-beta\n  flow a\n    x\n    global\n      g\n    end\n  end\n");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(chart.subgraphs[0].members, ["x"]);
    assert!(chart.nodes.iter().any(|n| n.id == "g"));

    let (_, d) = lowered("agentflow-beta\n  subgraph s\n    x\n  end\n");
    assert!(d.iter().any(|m| m.contains("use `flow`")), "{d:?}");
    let (_, d) = lowered("agentflow-beta\n  flow a\n    x\n");
    assert!(d.iter().any(|m| m.contains("flow")), "{d:?}");
}

/// MAGF-2: a `connector` is a rounded node labeled by its title or its id; only `[...]`
/// titles it.
#[test]
fn connectors() {
    let (chart, d) = lowered("agentflow-beta\n  connector bus[\"Bus\"]\n  connector q\n");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(shape(&chart, "bus"), Shape::Rounded);
    let labels: Vec<&str> = chart.nodes.iter().map(|n| n.label.text.as_str()).collect();
    assert_eq!(labels, ["Bus", "q"]);
    let (_, d) = lowered("agentflow-beta\n  connector c{\"C\"}\n");
    assert_eq!(d.len(), 1, "{d:?}");
}

/// MAGF-2: open and collapsed flows are unfilled with a 0.75 stroke; a flowchart cluster is
/// filled.
#[test]
fn flow_style() {
    let bodies = |src: &str, owner: fn(&Owner) -> bool| -> Vec<(bool, f32)> {
        let lowered = diagram::lower(src);
        let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
        s.elements
            .iter()
            .filter(|e| owner(&e.owner))
            .filter_map(|e| match &e.primitive {
                Primitive::Path {
                    fill,
                    stroke: Some(stroke),
                    ..
                } => Some((fill.is_some(), stroke.width)),
                _ => None,
            })
            .take(1)
            .collect()
    };
    let cluster = |o: &Owner| matches!(o, Owner::Cluster(_));
    let node = |o: &Owner| matches!(o, Owner::Cluster(_) | Owner::Node(_));
    assert_eq!(
        bodies("agentflow-beta\n  flow f\n    a\n  end\n", cluster),
        [(false, 0.75)]
    );
    assert_eq!(
        bodies("flowchart\n  subgraph f\n    a\n  end\n", cluster),
        [(true, 1.0)]
    );
    assert_eq!(
        bodies(
            "agentflow-beta\n  flow f@{ view: collapsed }\n    a\n  end\n",
            node
        ),
        [(false, 0.75)]
    );
}

/// MAGF-3: only `-->`, `--x`, and `-.-` edges, with or without labels.
#[test]
fn edges() {
    let (chart, d) = lowered(
        "agentflow-beta\n  a --> b\n  b --x c\n  c -.- d\n  d -- go --> e\n  e -->|ok| f\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(chart.edges.len(), 5);
    for edge in ["==>", "-.->", "~~~", "---", "<-->"] {
        let (_, d) = lowered(&format!("agentflow-beta\n  a {edge} b\n"));
        assert!(
            d.iter().any(|m| m.contains("is not an agentflow edge")),
            "{edge}: {d:?}"
        );
    }
}

/// MAGF-4: shapes from delimiters and from `@{ shape }`, with the removed and unsupported ones
/// drawn rounded.
#[test]
fn shapes() {
    let (chart, d) = lowered(
        "agentflow-beta\n  a\n  b[B]\n  c(C)\n  d{D}\n  e{{E}}\n  f[[F]]\n  g[/G/]\n\
         h@{ shape: tool }\n  i@{ shape: input }\n  j@{ shape: refdoc }\n  k@{ shape: action }\n\
         l@{ shape: decision }\n",
    );
    assert!(d.is_empty(), "{d:?}");
    let want = [
        ("a", Shape::Rounded),
        ("b", Shape::Rounded),
        ("c", Shape::Rounded),
        ("d", Shape::Diamond),
        ("e", Shape::Hexagon),
        ("f", Shape::FramedRect),
        ("g", Shape::LeanRight),
        ("h", Shape::FramedRect),
        ("i", Shape::LeanRight),
        ("j", Shape::LinedDocument),
        ("k", Shape::Hexagon),
        ("l", Shape::Diamond),
    ];
    for (id, s) in want {
        assert_eq!(shape(&chart, id), s, "{id}");
    }

    let src = "agentflow-beta\n  a([A])\n  b@{ shape: cylinder }\n  c@{ shape: cloud }\n  d@{ shape: nope }\n";
    let lowered = diagram::lower(src);
    let severities: Vec<Severity> = lowered.diagnostics.iter().map(|d| d.severity).collect();
    assert_eq!(
        severities,
        [
            Severity::Error,
            Severity::Error,
            Severity::Warning,
            Severity::Error
        ]
    );
    let chart = lowered.diagram.into_graph();
    for id in ["a", "b", "c"] {
        assert_eq!(shape(&chart, id), Shape::Rounded, "{id}");
    }
}
