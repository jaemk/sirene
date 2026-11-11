//! Mermaid state diagrams (MSTATE-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, NoteSide, Shape};
use sirene_core::scene::{self, Approx, Theme};
use sirene_core::{Severity, state};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/state");
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
            assert_eq!(state::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::State, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            // Every fixture draws.
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
            assert!(!s.elements.is_empty(), "{name}");
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

fn chart(src: &str) -> Flowchart {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    lowered.diagram.into_graph()
}

fn warnings(src: &str) -> Vec<String> {
    diagram::lower(src)
        .diagnostics
        .iter()
        .map(|d| {
            assert_eq!(d.severity, Severity::Warning, "{}", d.message);
            d.message.clone()
        })
        .collect()
}

fn shape(c: &Flowchart, id: &str) -> Shape {
    c.node(id).unwrap_or_else(|| panic!("no node {id}")).shape
}

/// MSTATE-1, MSTATE-2: states, descriptions, and transitions.
#[test]
fn states_and_transitions() {
    let c = chart(
        "stateDiagram-v2\n  s1\n  state \"Long name\" as s2\n  s3 : first\n  s3 : second\n  \
         s1 --> s2 : go\n  s2 --> s3\n",
    );
    let labels: Vec<_> = c
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.text.as_str()))
        .collect();
    assert_eq!(
        labels,
        [("s1", "s1"), ("s2", "Long name"), ("s3", "first\nsecond")]
    );
    assert!(c.nodes.iter().all(|n| n.shape == Shape::Rounded));
    assert_eq!(c.edges.len(), 2);
    assert_eq!(
        (c.edges[0].from.as_str(), c.edges[0].to.as_str()),
        ("s1", "s2")
    );
    assert_eq!(c.edges[0].label.as_ref().unwrap().text, "go");
    assert_eq!(c.edges[0].end, Some(Arrow::Point));
    assert_eq!(c.edges[0].start, None);
    assert!(c.edges[1].label.is_none());
    assert_eq!(c.direction, Direction::TopBottom);
}

/// MSTATE-2: `[*]` is a start state as a source and an end state as a target, one of each per
/// composite state.
#[test]
fn start_and_end_states() {
    let c = chart(
        "stateDiagram-v2\n  [*] --> A\n  A --> [*]\n  state A {\n    [*] --> x\n    x --> [*]\n  }\n",
    );
    let pseudo: Vec<_> = c
        .nodes
        .iter()
        .filter(|n| n.label.text.is_empty())
        .map(|n| n.shape)
        .collect();
    assert_eq!(
        pseudo,
        [
            Shape::SmallCircle,
            Shape::FramedCircle,
            Shape::SmallCircle,
            Shape::FramedCircle
        ]
    );
    let a = c.subgraph("A").expect("A is a composite");
    assert_eq!(a.members.len(), 3);
    assert!(a.members.contains(&"x".to_owned()));
    // Edges to A attach to the cluster.
    assert!(c.edges.iter().any(|e| e.to == "A"));
    assert!(c.node("A").is_none());
}

#[test]
fn pseudo_states() {
    let c = chart(
        "stateDiagram-v2\n  state f <<fork>>\n  state j <<join>>\n  state c <<choice>>\n  \
         f --> j\n  j --> c\n",
    );
    assert_eq!(shape(&c, "f"), Shape::Fork);
    assert_eq!(shape(&c, "j"), Shape::Fork);
    assert_eq!(shape(&c, "c"), Shape::Diamond);
    assert!(c.nodes.iter().all(|n| n.label.text.is_empty()));
}

/// A fork bar lies across the direction of flow.
#[test]
fn fork_bars_turn_with_the_direction() {
    let size = |src: &str| {
        let c = chart(src);
        let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
        let i = c.nodes.iter().position(|n| n.id == "f").unwrap();
        let (lo, hi) = s.bounds(scene::Owner::Node(i)).unwrap();
        (hi.x - lo.x, hi.y - lo.y)
    };
    let (w, h) = size("stateDiagram-v2\n  state f <<fork>>\n  a --> f\n");
    assert!(w > h, "{w}x{h}");
    let (w, h) = size("stateDiagram-v2\n  direction LR\n  state f <<fork>>\n  a --> f\n");
    assert!(h > w, "{w}x{h}");
}

/// MSTATE-2: regions are untitled sub-clusters of their composite state.
#[test]
fn regions() {
    let c = chart("stateDiagram-v2\n  state S {\n    a --> b\n    --\n    c\n    --\n    d\n  }\n");
    let s = c.subgraph("S").unwrap();
    assert_eq!(s.members.len(), 3);
    assert!(!s.region);
    for (k, m) in s.members.iter().enumerate() {
        let r = c.subgraph(m).unwrap_or_else(|| panic!("region {m}"));
        assert!(r.region);
        assert!(r.title.text.is_empty());
        let want: &[&str] = match k {
            0 => &["a", "b"],
            1 => &["c"],
            _ => &["d"],
        };
        assert_eq!(r.members, want);
    }
}

#[test]
fn notes() {
    let c = chart(
        "stateDiagram-v2\n  note left of A : one\n  note right of A\n    two\n    lines\n  end note\n  A --> B\n",
    );
    assert_eq!(c.notes.len(), 2);
    assert_eq!(c.notes[0].target, "A");
    assert_eq!(c.notes[0].side, NoteSide::Before);
    assert_eq!(c.notes[0].label.text, "one");
    assert_eq!(c.notes[1].side, NoteSide::After);
    assert_eq!(c.notes[1].label.text, "two\nlines");
    // Each note sits beside its state, on its side.
    let s = scene::flowchart(&c, &Approx, &Theme::LIGHT);
    let a = c.nodes.iter().position(|n| n.id == "A").unwrap();
    let (lo, hi) = s.bounds(scene::Owner::Node(a)).unwrap();
    let state = c.layout_graph(&CellsOf).nodes[a].size;
    assert!(
        hi.x - lo.x > state.w as f32 * 3.0,
        "the notes widen what A owns"
    );
}

/// A stand-in measure for layout graph sizes.
struct CellsOf;

impl sirene_core::flowchart::Measure for CellsOf {
    fn node(&self, _: &sirene_core::flowchart::Node) -> sirene_core::layout::Size {
        sirene_core::layout::Size::new(10, 10)
    }
    fn edge_label(&self, _: &sirene_core::flowchart::Label) -> Option<sirene_core::layout::Size> {
        None
    }
    fn subgraph_title(&self, _: &sirene_core::flowchart::Subgraph) -> sirene_core::layout::Size {
        sirene_core::layout::Size::default()
    }
    fn note(&self, _: &sirene_core::flowchart::Label) -> sirene_core::layout::Size {
        sirene_core::layout::Size::new(10, 10)
    }
}

#[test]
fn direction_and_styling() {
    let c = chart(
        "stateDiagram-v2\n  direction LR\n  classDef hot fill:#f00\n  a:::hot --> b\n  \
         class b hot\n  style a stroke:#000\n  state C {\n    direction BT\n    x\n  }\n  \
         class C hot\n",
    );
    assert_eq!(c.direction, Direction::LeftRight);
    assert_eq!(
        c.subgraph("C").unwrap().direction,
        Some(Direction::BottomTop)
    );
    assert_eq!(c.subgraph("C").unwrap().classes, ["hot"]);
    assert_eq!(c.node("a").unwrap().classes, ["hot"]);
    assert_eq!(c.node("b").unwrap().classes, ["hot"]);
    assert_eq!(c.node("a").unwrap().styles[0].property, "stroke");
    assert_eq!(c.class("hot").unwrap().styles[0].value, "#f00");
}

#[test]
fn metadata() {
    let c = chart(
        "---\ntitle: T\n---\nstateDiagram-v2\n  accTitle: at\n  accDescr: ad\n  %% comment\n  a\n",
    );
    assert_eq!(c.title.as_deref(), Some("T"));
    assert_eq!(c.acc_title.as_deref(), Some("at"));
    assert_eq!(c.acc_descr.as_deref(), Some("ad"));
}

/// MSTATE-4, LANGS-6: what sirene does not draw is a warning, never dropped silently.
#[test]
fn unsupported_is_a_warning() {
    let w = warnings("stateDiagram-v2\n  state S {\n    a\n    --\n    b\n  }\n  a --> b\n");
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("between concurrent regions"), "{w:?}");
    let c = diagram::lower("stateDiagram-v2\n  state S {\n    a\n    --\n    b\n  }\n  a --> b\n")
        .diagram
        .into_graph();
    assert!(c.edges.is_empty());

    for (src, want) in [
        (
            "stateDiagram-v2\n  click a \"x\"\n",
            "`click` is not supported",
        ),
        ("stateDiagram-v2\n  note \"x\" as N\n", "floating notes"),
        (
            "stateDiagram-v2\n  state h <<history>>\n",
            "`<<history>>` states",
        ),
        ("stateDiagram-v2\n  --\n", "only inside a composite"),
        (
            "stateDiagram-v2\n  state C {\n    x\n  }\n  note left of C : n\n",
            "notes on composite states",
        ),
        (
            "stateDiagram-v2\n  class nowhere c\n",
            "unknown state `nowhere`",
        ),
    ] {
        let w = warnings(src);
        assert!(w.iter().any(|m| m.contains(want)), "{src}: {w:?}");
    }
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone())
            .collect()
    };
    assert_eq!(errors("stateDiagram-v2\n  a --> b\n"), Vec::<String>::new());
    assert_eq!(
        errors("stateDiagram-v2\n  state C {\n  a\n"),
        ["unclosed composite state: expected `}`"]
    );
    assert_eq!(
        errors("stateDiagram-v2\n  a -->\n"),
        ["expected a state after `-->`"]
    );
    assert_eq!(
        errors("stateDiagram-v2\n  note left of a\n  x\n"),
        ["unclosed note: expected `end note`"]
    );
    assert_eq!(errors("stateDiagram-v2\n  }\n"), ["unexpected `}`"]);
}
