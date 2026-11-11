//! Graphviz DOT (DOT-*): the fixture corpus with its diagnostics and drawings (LANGS-6), and
//! the graph model each construct lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{Arrow, Direction, Flowchart, Shape, Stroke};
use sirene_core::scene::{Approx, Color, Theme};
use sirene_core::{Severity, dot};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dot");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "dot"))
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
            assert_eq!(dot::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Dot, "{name}");
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

fn style<'a>(styles: &'a [sirene_core::flowchart::Style], property: &str) -> Option<&'a str> {
    styles
        .iter()
        .rev()
        .find(|s| s.property == property)
        .map(|s| s.value.as_str())
}

/// LANGS-1: a DOT header is DOT, whatever the case; Mermaid's `graph TD` is not.
#[test]
fn detection() {
    assert_eq!(Kind::of("digraph { a }"), Kind::Dot);
    assert_eq!(Kind::of("/* c */\nSTRICT DiGraph \"g\" {}"), Kind::Dot);
    assert_eq!(Kind::of("graph TD\n  A --> B\n"), Kind::Flowchart);
    assert_eq!(Kind::of("graph\n"), Kind::Flowchart);
}

/// DOT-2: nodes in first-mention order, labeled by their ID or `label` with its escapes; edge
/// chains; arrows in a digraph and none in a graph.
#[test]
fn nodes_and_edges() {
    let c = chart(
        "digraph G {\n  a -> b -> c\n  b [label=\"\\N of \\G\\nline\"]\n  \"x y\" -> a [label=\"\\E\"]\n}\n",
    );
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c", "x y"]);
    assert_eq!(c.nodes[1].label.text, "b of G\nline");
    assert_eq!(c.nodes[0].shape, Shape::Ellipse);
    let ends: Vec<_> = c
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str(), e.start, e.end))
        .collect();
    assert_eq!(
        ends,
        [
            ("a", "b", None, Some(Arrow::Point)),
            ("b", "c", None, Some(Arrow::Point)),
            ("x y", "a", None, Some(Arrow::Point)),
        ]
    );
    assert_eq!(c.edges[2].label.as_ref().unwrap().text, "x y->a");

    let c = chart("graph { a -- b }");
    assert_eq!((c.edges[0].start, c.edges[0].end), (None, None));
}

/// DOT-2: `node` and `edge` defaults apply to later statements in their scope only.
#[test]
fn defaults() {
    let c = chart(
        "digraph {\n  a\n  node [shape=box]\n  b\n  { node [shape=circle]; c; edge [style=dashed]; c -> d }\n  e\n  a -> e\n}\n",
    );
    let shapes: Vec<Shape> = c.nodes.iter().map(|n| n.shape).collect();
    assert_eq!(
        shapes,
        [
            Shape::Ellipse,
            Shape::Rect,
            Shape::Circle,
            Shape::Circle,
            Shape::Rect
        ]
    );
    let strokes: Vec<Stroke> = c.edges.iter().map(|e| e.stroke).collect();
    assert_eq!(strokes, [Stroke::Dotted, Stroke::Normal]);
}

/// DOT-2: an edge to a subgraph joins every node in it; `strict` merges repeated edges.
#[test]
fn subgraph_operands_and_strict() {
    let c = chart("digraph { a -> {b c} -> d }");
    let pairs: Vec<_> = c
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(pairs, [("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]);

    let c = chart("strict graph { a -- b; b -- a [color=red]; a -- c }");
    assert_eq!(c.edges.len(), 2);
    assert_eq!(style(&c.edges[0].styles, "stroke"), Some("#ff0000"));
    let c = chart("strict digraph { a -> b; b -> a }");
    assert_eq!(c.edges.len(), 2);
}

/// DOT-2, DOT-3: `cluster` subgraphs are titled clusters with their colors, nested as in the
/// source; other subgraphs draw nothing.
#[test]
fn clusters() {
    let c = chart(
        "digraph {\n  subgraph cluster_a {\n    label=\"A\"; style=filled; fillcolor=lightblue; pencolor=red\n    x\n    subgraph cluster_b { y }\n  }\n  subgraph plain { z }\n  x -> z\n}\n",
    );
    let subs: Vec<_> = c
        .subgraphs
        .iter()
        .map(|s| (s.id.as_str(), s.title.text.as_str(), s.members.clone()))
        .collect();
    assert_eq!(
        subs,
        [
            (
                "cluster_a",
                "A",
                vec!["x".to_owned(), "cluster_b".to_owned()]
            ),
            ("cluster_b", "", vec!["y".to_owned()]),
        ]
    );
    assert_eq!(style(&c.subgraphs[0].styles, "fill"), Some("#add8e6"));
    assert_eq!(style(&c.subgraphs[0].styles, "stroke"), Some("#ff0000"));
}

/// DOT-2: a node is a member of the innermost cluster that mentions it, wherever it is
/// mentioned first or last.
#[test]
fn cluster_membership() {
    let c = chart(
        "digraph {\n  x; y\n  subgraph cluster_a {\n    x; z\n    subgraph cluster_b { x; y }\n    y\n  }\n  subgraph cluster_c { z; w }\n  x -> y\n}\n",
    );
    let members: Vec<_> = c
        .subgraphs
        .iter()
        .map(|s| (s.id.as_str(), s.members.join(" ")))
        .collect();
    assert_eq!(
        members,
        [
            ("cluster_a", "z cluster_b".to_owned()),
            ("cluster_b", "x y".to_owned()),
            ("cluster_c", "w".to_owned()),
        ]
    );
}

/// DOT-2: Graphviz names nodes and subgraphs apart; a cluster named as a node is renamed.
#[test]
fn cluster_named_as_a_node() {
    let c = chart("digraph { subgraph cluster_a { cluster_a -> cluster_a_ } }");
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["cluster_a", "cluster_a_"]);
    assert_eq!(c.subgraphs[0].id, "cluster_a__");
    assert_eq!(c.subgraphs[0].members, ["cluster_a", "cluster_a_"]);
}

/// DOT-3: a cluster's `bgcolor` fills it; `style=filled` fills with `fillcolor`, then `color`,
/// then `bgcolor`. A node has neither `bgcolor` nor `pencolor`.
#[test]
fn cluster_fills() {
    let c = chart(
        "digraph {\n  subgraph cluster_a { bgcolor=yellow; a }\n  subgraph cluster_b { style=filled; bgcolor=yellow; color=red; b }\n  subgraph cluster_c { style=filled; bgcolor=yellow; c }\n  subgraph cluster_d { style=filled; d }\n}\n",
    );
    let fills: Vec<_> = c
        .subgraphs
        .iter()
        .map(|s| (style(&s.styles, "fill"), style(&s.styles, "stroke")))
        .collect();
    assert_eq!(
        fills,
        [
            (Some("#ffff00"), None),
            (Some("#ff0000"), Some("#ff0000")),
            (Some("#ffff00"), None),
            (Some("#d3d3d3"), None),
        ]
    );
    let (c, d) = lowered("digraph { a [bgcolor=red, pencolor=blue] }");
    assert!(c.nodes[0].styles.is_empty(), "{:?}", c.nodes[0].styles);
    assert_eq!(d.len(), 2, "{d:?}");
}

/// DOT-2: the escapes in a name are read where the name is a label.
#[test]
fn names_in_labels() {
    let c = chart(
        "digraph \"G\\nH\" {\n  \"a\\nb\" -> c [label=\"\\T to \\H\"]\n  d [label=\"\\\\N is \\N\"]\n  label=\"\\G\"\n}\n",
    );
    assert_eq!(c.nodes[0].id, "a\\nb");
    assert_eq!(c.nodes[0].label.text, "a\nb");
    assert_eq!(c.edges[0].label.as_ref().unwrap().text, "a\nb to c");
    assert_eq!(c.nodes[2].label.text, "\\N is d");
    assert_eq!(c.title.as_deref(), Some("G\nH"));
}

/// DOT-3: an HTML label is drawn as its text, a line per `<br/>` and per table row.
#[test]
fn html_labels() {
    let (c, d) = lowered(
        "digraph {\n  a [label=<<table>\n    <tr><td>x</td><td>y &amp; z</td></tr>\n    <tr><td>w</td></tr>\n  </table>>]\n  b [label=<one<br/>two>]\n  a -> b [label=<<i>e</i>>]\n}\n",
    );
    assert_eq!(
        d,
        [(
            Severity::Warning,
            "HTML labels are drawn as their text".to_owned()
        )]
    );
    assert_eq!(c.nodes[0].label.text, "x y & z\nw");
    assert_eq!(c.nodes[1].label.text, "one\ntwo");
    assert_eq!(c.edges[0].label.as_ref().unwrap().text, "e");
}

/// DOT-3: a blank graph label is no title; `minlen` is a whole number of ranks from 1 to 10.
#[test]
fn blank_labels_and_lengths() {
    for src in [
        "digraph { label=\"\"; a }",
        "digraph { label=\" \\n \"; a }",
        "digraph { label=<<b> </b>>; a }",
    ] {
        let lowered = diagram::lower(src);
        let c = lowered.diagram.into_graph();
        assert_eq!(c.title, None, "{src}");
        let out = ascii::render(&c, &Options::default());
        assert!(out.starts_with('╭'), "{src}\n{out}");
    }
    let c = chart(
        "digraph { a -> b [minlen=nan]; a -> c [minlen=99]; a -> d [minlen=\"-inf\"]; a -> e [minlen=2.6] }",
    );
    let lengths: Vec<u8> = c.edges.iter().map(|e| e.length).collect();
    assert_eq!(lengths, [1, 10, 1, 3]);
}

/// DOT-3: an attribute that many nodes or edges share is reported once.
#[test]
fn shared_attributes_warn_once() {
    let (_, d) = lowered(
        "digraph {\n  node [color=nope]\n  edge [color=nope, dir=sideways]\n  a -> b -> c -> a\n  d [color=nope]\n}\n",
    );
    let messages: Vec<&str> = d.iter().map(|(_, m)| m.as_str()).collect();
    assert_eq!(
        messages,
        [
            "unknown color `nope`; ignored",
            "unknown color `nope`; ignored",
            "unknown `dir` `sideways`: expected forward, back, both, or none",
            "unknown color `nope`; ignored",
        ]
    );
}

/// DOT-1: subgraphs nest 64 deep; a deeper one is an error, and neither it nor a wide stroke
/// breaks the drawing.
#[test]
fn limits() {
    let src = format!("digraph {{ a {}", "{ b -> ".repeat(50_000));
    let lowered = diagram::lower(&src);
    let messages: Vec<&str> = lowered
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages.contains(&"subgraphs nested more than 64 deep"),
        "{messages:?}"
    );
    let c = lowered.diagram.into_graph();
    assert_eq!(c.nodes.len(), 2);
    assert!(!ascii::render(&c, &Options::default()).is_empty());

    // No edge is drawn across a subgraph that was skipped.
    let src = format!(
        "digraph {{ {}a -> {{ b }} -> c{} }}",
        "{ ".repeat(64),
        " }".repeat(64)
    );
    let c = diagram::lower(&src).diagram.into_graph();
    let ids: Vec<&str> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a", "c"]);
    assert!(c.edges.is_empty(), "{:?}", c.edges);

    let lowered =
        diagram::lower("digraph { a -> b [penwidth=1000000000, dir=both]; c [penwidth=99999] }");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
    assert!(
        s.width < 1000.0 && s.height < 1000.0,
        "{}x{}",
        s.width,
        s.height
    );
}

/// DOT-3: graph, node, and edge attributes.
#[test]
fn attributes() {
    let src = "digraph {\n  rankdir=RL; label=\"T\"; bgcolor=black\n  a [shape=box, style=\"filled,rounded,dashed\", color=blue, fontcolor=\"#123456\", penwidth=3]\n  b [style=filled]\n  a -> b [dir=back, arrowtail=odiamond, headlabel=h, taillabel=t, minlen=3, style=bold, penwidth=2, fontcolor=red]\n  b -> a [dir=both, arrowhead=none, arrowtail=crow, style=invis]\n}\n";
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let theme = lowered.theme.apply(&Theme::LIGHT);
    assert_eq!(theme.background, Color::rgb(0));
    let c = lowered.diagram.into_graph();
    assert_eq!(c.direction, Direction::RightLeft);
    assert_eq!(c.title.as_deref(), Some("T"));
    let a = &c.nodes[0];
    assert_eq!(a.shape, Shape::Rounded);
    assert_eq!(style(&a.styles, "fill"), Some("#0000ff"));
    assert_eq!(style(&a.styles, "stroke"), Some("#0000ff"));
    assert_eq!(style(&a.styles, "color"), Some("#123456"));
    assert_eq!(style(&a.styles, "stroke-width"), Some("3px"));
    assert_eq!(style(&a.styles, "stroke-dasharray"), Some("5 5"));
    assert_eq!(style(&c.nodes[1].styles, "fill"), Some("#d3d3d3"));
    let e = &c.edges[0];
    assert_eq!((e.start, e.end), (Some(Arrow::HollowDiamond), None));
    assert_eq!(e.end_label.as_ref().unwrap().text, "h");
    assert_eq!(e.start_label.as_ref().unwrap().text, "t");
    assert_eq!(e.length, 3);
    assert_eq!(e.stroke, Stroke::Thick);
    assert_eq!(style(&e.styles, "stroke-width"), Some("2px"));
    assert_eq!(style(&e.styles, "color"), Some("#ff0000"));
    let e = &c.edges[1];
    assert_eq!((e.start, e.end), (Some(Arrow::OneMany), None));
    assert_eq!(e.stroke, Stroke::Invisible);
}

/// DOT-3: record labels are compartments: the first field is the title, each other field a
/// section of rows.
#[test]
fn records() {
    let c = chart(
        "digraph {\n  n [shape=record, label=\"{Name|<f> a\\lb\\l|{c|d}}\"]\n  m [shape=record, label=\"single\"]\n}\n",
    );
    let n = &c.nodes[0];
    assert_eq!(n.label.text, "Name");
    let rows: Vec<Vec<&str>> = n
        .sections
        .iter()
        .map(|s| s.iter().map(|r| r.cells[0].as_str()).collect())
        .collect();
    assert_eq!(rows, [vec!["a", "b"], vec!["c", "d"]]);
    assert_eq!(c.nodes[1].label.text, "single");
    assert!(c.nodes[1].sections.is_empty());
}

/// DOT-3, DOT-4: what is not drawn is a warning, once per name.
#[test]
fn warnings() {
    let (c, d) = lowered(
        "digraph {\n  splines=ortho; splines=line\n  { rank=same; a; b }\n  a:n -> b:s\n  a [label=<<i>x</i>>, shape=star2]\n  b [width=1, color=nope]\n  a -> b [arrowhead=zzz, weight=2]\n}\ndigraph two {}\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            w("the graph attribute `splines` is not supported; ignored"),
            w("`rank` is not supported yet; the nodes are ranked as usual"),
            w("ports are not supported yet; the edge joins the node"),
            w("HTML labels are drawn as their text"),
            w("the shape `star2` is not supported; drawn as an ellipse"),
            w("the node attribute `width` is not supported; ignored"),
            w("unknown color `nope`; ignored"),
            w("unknown arrow `zzz`; drawn as `normal`"),
            w("the edge attribute `weight` is not supported; ignored"),
            w("only the first graph in a file is drawn"),
        ]
    );
    assert_eq!(c.nodes[0].label.text, "x");
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
        errors("digraph { a -- b }"),
        ["`--` in a `digraph`: expected `->`"]
    );
    assert_eq!(errors("digraph {"), ["unclosed block: expected `}`"]);
    assert!(errors("digraph {}").is_empty());
}
