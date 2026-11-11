//! Mermaid swimlane diagrams (MSWIM-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), the lanes lowering builds, and how the layout places them.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{self, Flowchart, Label, Measure, Node, Subgraph};
use sirene_core::layout::{self, Config, Rect, Size, Spacing};
use sirene_core::scene::{Approx, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/swimlane");
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
            assert_eq!(Kind::of(&src), Kind::Swimlane, "{name}");
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

fn lane_ids(chart: &Flowchart) -> Vec<&str> {
    chart.subgraphs.iter().map(|s| s.id.as_str()).collect()
}

/// MSWIM-1, MSWIM-2: the header marks the lanes; a flowchart has none.
#[test]
fn header() {
    let (chart, d) = lowered("swimlane-beta LR\nsubgraph a\n  x\nend\n");
    assert!(d.is_empty(), "{d:?}");
    assert!(chart.lanes);
    assert_eq!(chart.direction, flowchart::Direction::LeftRight);
    let (chart, _) = lowered("flowchart LR\nsubgraph a\n  x\nend\n");
    assert!(!chart.lanes);
    let (chart, _) = lowered("swimlane-beta\nA --> B\n");
    assert_eq!(chart.direction, flowchart::Direction::TopBottom);
}

/// MSWIM-2: nodes outside every lane share an untitled last lane; a lane emptied by a node
/// listed first elsewhere is dropped, but not one declared empty or one an edge names.
#[test]
fn lanes() {
    let (chart, _) = lowered("swimlane-beta\nsubgraph l1\n  a\nend\nb\na --> b\n");
    assert_eq!(lane_ids(&chart), ["l1", "__swimlane_default__"]);
    let default = &chart.subgraphs[1];
    assert_eq!(default.members, ["b"]);
    assert!(default.title.text.is_empty());

    let (chart, d) = lowered(
        "swimlane-beta LR\nsubgraph lane1[Performer A]\n  a[\"step one\"]\nend\n\
         subgraph phase1[Phase 1]\n  a\nend\n",
    );
    assert_eq!(lane_ids(&chart), ["lane1"]);
    assert_eq!(d, ["`a` already belongs to subgraph `lane1`"]);

    let (chart, d) = lowered("swimlane-beta LR\nsubgraph l1\n  a\nend\nsubgraph l2\nend\n");
    assert_eq!(lane_ids(&chart), ["l1", "l2"]);
    assert!(d.is_empty(), "{d:?}");

    let (chart, _) =
        lowered("swimlane-beta\nsubgraph l1\n  a\nend\nsubgraph l2\n  a\nend\na --> l2\n");
    assert_eq!(lane_ids(&chart), ["l1", "l2"]);
}

/// MSWIM-2: a subgraph in a lane is a plain box, with a warning.
#[test]
fn nested() {
    let (chart, d) = lowered(
        "swimlane-beta LR\nsubgraph lane1[Performer A]\n  subgraph ph1[Phase 1]\n    a\n  end\nend\n",
    );
    assert_eq!(
        d,
        [
            "subgraph `ph1` is nested in a lane; only top-level subgraphs are lanes, so it is \
          drawn as a plain box"
        ]
    );
    assert_eq!(chart.parent("ph1").map(|s| s.id.as_str()), Some("lane1"));
}

struct Fixed;

impl Measure for Fixed {
    fn node(&self, _: &Node) -> Size {
        Size::new(5, 3)
    }
    fn edge_label(&self, _: &Label) -> Option<Size> {
        None
    }
    fn subgraph_title(&self, s: &Subgraph) -> Size {
        if s.title.text.is_empty() {
            Size::default()
        } else {
            Size::new(4, 1)
        }
    }
    fn note(&self, _: &Label) -> Size {
        Size::new(5, 3)
    }
}

const SPACING: Spacing = Spacing {
    node_gap: 3,
    edge_gap: 0,
    rank_gap: 1,
    track_gap: 1,
    cluster_cross_pad: 2,
    cluster_main_pad: 1,
    port_inset: 1,
    loop_size: 2,
    loop_inset: 0,
};

fn place(src: &str) -> (Flowchart, layout::Layout) {
    let (chart, _) = lowered(src);
    let l = layout::layout(
        &chart.layout_graph(&Fixed),
        &Config {
            vertical: SPACING,
            horizontal: SPACING,
        },
    );
    (chart, l)
}

fn inside(a: Rect, b: Rect) -> bool {
    a.x > b.x && a.right() < b.right() && a.y > b.y && a.bottom() < b.bottom()
}

/// MSWIM-3: lanes are bands side by side across the flow, sharing borders and spanning the
/// whole diagram along it; each node sits in its lane; edges run between lanes.
#[test]
fn placement() {
    let src = "swimlane-beta TB\nsubgraph a[A]\n  x\n  z\nend\nsubgraph b[B]\n  y\nend\n\
               x --> y --> z\n";
    let (chart, l) = place(src);
    let (a, b) = (l.clusters[0], l.clusters[1]);
    assert_eq!((a.y, a.h), (0, l.size.h));
    assert_eq!((b.y, b.h), (0, l.size.h));
    assert_eq!(a.right(), b.x);
    assert_eq!(b.right() + 1, l.size.w);
    for (n, lane) in [("x", a), ("z", a), ("y", b)] {
        let i = chart.nodes.iter().position(|node| node.id == n).unwrap();
        assert!(inside(l.nodes[i], lane), "{n}");
    }
    // Three ranks down the lanes, whatever lane each is in.
    assert!(l.nodes[0].bottom() < l.nodes[2].y && l.nodes[2].bottom() < l.nodes[1].y);
    assert!(l.edges.iter().all(Option::is_some));
    assert_eq!(l.titles[0], Some(Rect::new(a.x + 2, 0, 4, 1)));

    // Left to right, the lanes stack downward.
    let (_, l) = place(&src.replace("TB", "LR"));
    let (a, b) = (l.clusters[0], l.clusters[1]);
    assert_eq!((a.x, a.w), (0, l.size.w));
    assert_eq!(a.bottom(), b.y);
    assert_eq!(l.titles[1], Some(Rect::new(1, b.y, 4, 1)));
}

/// MSWIM-3: an edge to a lane itself is not drawn.
#[test]
fn edges_to_lanes() {
    let (_, l) = place("swimlane-beta\nsubgraph a\n  x\nend\nsubgraph b\n  y\nend\nx --> b\n");
    assert_eq!(l.edges, [None]);
}
