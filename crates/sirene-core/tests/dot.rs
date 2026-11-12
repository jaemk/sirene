//! Graphviz DOT (DOT-*): the fixture corpus with its diagnostics and drawings (LANGS-6), and
//! the graph model each construct lowers to.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::diagram::{self, Kind, Lowered};
use sirene_core::flowchart::{
    Arrow, Compass, Direction, EdgeOrder, Flowchart, RankEnd, Shape, Stroke,
};
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
            // Set in `cluster_a` before `cluster_b`, so inherited as in Graphviz.
            ("cluster_b", "A", vec!["y".to_owned()]),
        ]
    );
    assert_eq!(style(&c.subgraphs[0].styles, "fill"), Some("#add8e6"));
    assert_eq!(style(&c.subgraphs[0].styles, "stroke"), Some("#ff0000"));
}

/// DOT-2: a cluster starts with the graph attributes set around it before it, by `a=b` or
/// `graph [...]`, in clusters and plain subgraphs alike; later ones and its own override them.
#[test]
fn cluster_inherits_graph_attributes() {
    let c = chart(
        "digraph {\n  subgraph cluster_before { a }\n  label=\"T\"; graph [pencolor=red]\n  subgraph cluster_after { b }\n  subgraph cluster_own { label=\"own\"; c }\n  subgraph { pencolor=blue; subgraph cluster_inner { d } }\n  pencolor=green\n}\n",
    );
    let got: Vec<_> = c
        .subgraphs
        .iter()
        .map(|s| {
            (
                s.id.as_str(),
                s.title.text.as_str(),
                style(&s.styles, "stroke"),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("cluster_before", "", None),
            ("cluster_after", "T", Some("#ff0000")),
            ("cluster_own", "own", Some("#ff0000")),
            ("cluster_inner", "T", Some("#0000ff")),
        ]
    );
    assert_eq!(c.title.as_deref(), Some("T"));
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
    assert_eq!(lengths, [1, 10, 0, 3]);
}

/// DOT-2: `rank=same` puts a subgraph's nodes in one rank, `constraint=false` leaves an edge
/// out of the ranking, and `minlen=0` lets an edge's ends share a rank.
#[test]
fn ranks() {
    let c = chart("digraph { a -> b; a -> c; { rank=same; b; c } b -> c }");
    assert_eq!(c.same_rank, [vec!["b".to_owned(), "c".to_owned()]]);
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    // b and c side by side, the edge between them straight.
    let out = draw("digraph { a -> b; a -> c; { rank=same; b; c } b -> c }");
    assert!(out.contains("( b ├──►( c )"), "{out}");
    // Every node mentioned in the subgraph, edges included, and rank set after them.
    let c = chart("digraph { subgraph s { x -> y; z; rank=same } }");
    assert_eq!(c.same_rank, [vec!["x", "y", "z"]]);
    let out = draw("digraph { subgraph s { x -> y; rank=same } }");
    assert!(out.contains("( x ├──►( y )"), "{out}");
    // An edge left out of the ranking, and one of length 0.
    for attr in [
        "constraint=false",
        "constraint=no",
        "constraint=0",
        "minlen=0",
    ] {
        let out = draw(&format!("digraph {{ x -> y [{attr}] }}"));
        assert!(out.contains("( x ├──►( y )"), "{attr}\n{out}");
    }
    let c = chart("digraph { x -> y [constraint=yes]; x -> z [constraint=false] }");
    let constraint: Vec<bool> = c.edges.iter().map(|e| e.constraint).collect();
    assert_eq!(constraint, [true, false]);
    // An edge against the ranks of the others runs upward.
    let out = draw("digraph { a -> b -> c; c -> a [constraint=false] }");
    let (a, c) = (out.find("( a )").unwrap(), out.find("( c )").unwrap());
    assert!(a < c, "{out}");
}

#[test]
fn rank_warnings() {
    let messages =
        |src: &str| -> Vec<String> { lowered(src).1.into_iter().map(|(_, m)| m).collect() };
    assert_eq!(
        messages("digraph { rank=same; a }"),
        ["`rank` applies to subgraphs; ignored on the graph"]
    );
    assert!(messages("digraph { { rank=min; a; b } }").is_empty());
    assert_eq!(
        messages("digraph { { rank=up; a; b } }"),
        ["unknown `rank` `up`: expected same, min, max, source, or sink"]
    );
    assert_eq!(
        messages("digraph { a -> b [constraint=maybe] }"),
        ["`constraint` `maybe` is not a boolean; read as true"]
    );
    assert!(
        lowered("digraph { { rank=up; a; b } }")
            .0
            .same_rank
            .is_empty()
    );
    assert_eq!(
        messages("digraph { subgraph cluster_p { a } { rank=same; a; b; c } }"),
        ["`rank=same` nodes in different clusters do not share a rank"]
    );
    assert_eq!(
        messages("digraph { subgraph cluster_p { a } { rank=sink; a; b } }"),
        ["`rank=sink` nodes in different clusters do not share a rank"]
    );
}

/// DOT-3: with `compound=true`, `lhead` and `ltail` end an edge at a cluster holding its head
/// or tail.
#[test]
fn compound_edges() {
    let src = |extra: &str, attrs: &str| {
        format!(
            "digraph {{ {extra} subgraph cluster_p {{ a; b }} subgraph cluster_q {{ c }} \
             a -> c [{attrs}] }}"
        )
    };
    let ends = |c: &Flowchart| (c.edges[0].from.clone(), c.edges[0].to.clone());
    let c = chart(&src("compound=true;", "lhead=cluster_q"));
    assert_eq!(ends(&c), ("a".into(), "cluster_q".into()));
    let c = chart(&src("compound=yes;", "ltail=cluster_p, lhead=cluster_q"));
    assert_eq!(ends(&c), ("cluster_p".into(), "cluster_q".into()));
    // The route runs from border to border.
    let out = ascii::render(&c, &Options::default());
    assert!(out.contains("└───────┬───────┘"), "{out}");
    assert!(out.contains("▼\n    ┌───────┐"), "{out}");
    let messages = |src: &str| -> Vec<String> {
        let (c, d) = lowered(src);
        assert_eq!(ends(&c), ("a".into(), "c".into()), "{src}");
        d.into_iter().map(|(_, m)| m).collect()
    };
    assert_eq!(
        messages(&src("", "lhead=cluster_q")),
        ["`lhead` needs `compound=true` on the graph; ignored"]
    );
    assert_eq!(
        messages(&src("compound=true;", "lhead=cluster_z")),
        ["`lhead` `cluster_z` is not a cluster; ignored"]
    );
    assert_eq!(
        messages(&src("compound=true;", "ltail=cluster_q")),
        ["`ltail` `cluster_q` does not hold the edge's tail alone; ignored"]
    );
    assert_eq!(
        messages(&src("compound=false;", "lhead=cluster_q")),
        ["`lhead` needs `compound=true` on the graph; ignored"]
    );
}

/// DOT-3: `ordering=out` keeps a node's out-edges in source order along the rank below it, and
/// `ordering=in` its in-edges along the rank above.
#[test]
fn ordering() {
    let c = chart("digraph { ordering=out; a; b [ordering=in]; c [ordering=\"\"] }");
    assert_eq!(
        c.ordering,
        [
            ("a".to_owned(), EdgeOrder::Out),
            ("b".to_owned(), EdgeOrder::In)
        ]
    );
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    let before = |out: &str, x: &str, y: &str| {
        out.find(&format!("( {x} )")).unwrap() < out.find(&format!("( {y} )")).unwrap()
    };
    // Without `ordering`, c and b swap to untangle; with it, they keep source order.
    let src = "x -> b; a -> c; a -> b; y -> c";
    let out = draw(&format!("digraph {{ {src} }}"));
    assert!(before(&out, "b", "c"), "{out}");
    let out = draw(&format!("digraph {{ a [ordering=out]; {src} }}"));
    assert!(before(&out, "c", "b"), "{out}");
    // c -> a enters a left of b -> a, crossing it, since b and c keep their places.
    let src = "b -> x; c -> a; b -> a; c -> y";
    let out = draw(&format!("digraph {{ {src} }}"));
    assert!(before(&out, "b", "c") && !out.contains('┼'), "{out}");
    let out = draw(&format!("digraph {{ ordering=in; {src} }}"));
    assert!(before(&out, "b", "c") && out.contains('┼'), "{out}");
    assert_eq!(
        lowered("digraph { ordering=up; a; b }").1,
        [(
            Severity::Warning,
            "unknown `ordering` `up`: expected out or in".to_owned()
        )]
    );
}

/// DOT-2: a compass port puts an edge's end toward that side of the node, along the rank.
#[test]
fn ports() {
    let c = lowered(
        "digraph { a:e -> b; a:p:SW -> c; a:_ -> d:n; a -> e [tailport=w, headport=\"p:se\"] }",
    )
    .0;
    let ports: Vec<_> = c.edges.iter().map(|e| e.ports).collect();
    use Compass::*;
    assert_eq!(
        ports,
        [
            [Some(East), None],
            [Some(SouthWest), None],
            [None, Some(North)],
            [Some(West), Some(SouthEast)],
        ]
    );
    // A node's port beats the attribute.
    let c = chart("digraph { a:e -> b [tailport=w] }");
    assert_eq!(c.edges[0].ports, [Some(East), None]);
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    // a's ports run west to east, whichever way b and c lie: ports in the order the edges
    // take anyway change nothing, and the other order swaps the ends.
    for (dir, first, last) in [
        ("TB", "w", "e"),
        ("BT", "w", "e"),
        ("LR", "n", "s"),
        ("RL", "n", "s"),
    ] {
        let plain = draw(&format!("digraph {{ rankdir={dir}; a -> b; a -> c }}"));
        let along = draw(&format!(
            "digraph {{ rankdir={dir}; a:{first} -> b; a:{last} -> c }}"
        ));
        let against = draw(&format!(
            "digraph {{ rankdir={dir}; a:{last} -> b; a:{first} -> c }}"
        ));
        assert_eq!(plain, along, "{dir}");
        assert_ne!(plain, against, "{dir}\n{against}");
    }
    let messages =
        |src: &str| -> Vec<String> { lowered(src).1.into_iter().map(|(_, m)| m).collect() };
    assert_eq!(
        messages("digraph { a:p:up -> b }"),
        [
            "record ports are not supported yet; the edge joins the node",
            "unknown compass point `up`: expected n, ne, e, se, s, sw, w, nw, c, or _",
        ]
    );
    assert_eq!(
        messages("digraph { a -> b [headport=p] }"),
        ["record ports are not supported yet; the edge joins the node"]
    );
}

/// DOT-3: the edges between nodes of one `group` are kept straight before others.
#[test]
fn groups() {
    let c = chart("digraph { a [group=g]; b [group=h]; c [group=g]; d [group=\"\"] }");
    assert_eq!(c.groups, [vec!["a", "c"], vec!["b"]]);
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    // Whether the column through the middle of `lower` runs straight up to `upper`'s box.
    let straight = |out: &str, upper: &str, lower: &str| {
        let lines: Vec<Vec<char>> = out.lines().map(|l| l.chars().collect()).collect();
        let find = |node: &str| {
            let label: Vec<char> = format!("( {node} )").chars().collect();
            let row = lines
                .iter()
                .position(|l| l.windows(label.len()).any(|w| w == label.as_slice()))
                .unwrap();
            let col = lines[row]
                .windows(label.len())
                .position(|w| w == label.as_slice())
                .unwrap();
            (row, col + 2)
        };
        let ((top, _), (bottom, col)) = (find(upper), find(lower));
        (top + 1..bottom - 1).all(|r| lines[r].get(col).is_some_and(|c| "┬│▼".contains(*c)))
    };
    // A diamond centers a over b and c; grouped, the edges through b run straight.
    let src = "a -> b; a -> c; b -> d; c -> d";
    let out = draw(&format!("digraph {{ {src} }}"));
    assert!(!straight(&out, "a", "b"), "{out}");
    let out = draw(&format!(
        "digraph {{ node [group=g]; a; b; d; node [group=\"\"]; {src} }}"
    ));
    assert!(straight(&out, "a", "b"), "{out}");
}

/// DOT-2: `rank=min` and `max` put a subgraph's nodes in the first or last rank, and `source`
/// and `sink` in a rank of their own above or below every other node.
#[test]
fn end_ranks() {
    let c = chart("digraph { a -> b; { rank=min; c } { rank=SINK; d; e } }");
    assert_eq!(c.end_ranks.len(), 2);
    assert_eq!(c.end_ranks[0], (RankEnd::Min, vec!["c".to_owned()]));
    assert_eq!(
        c.end_ranks[1],
        (RankEnd::Sink, vec!["d".into(), "e".into()])
    );
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    let row = |out: &str, node: &str| {
        let i = out.find(&format!("( {node} )")).unwrap();
        out[..i].matches('\n').count()
    };
    // b joins a's rank as the first; the edge into it runs up from c.
    let out = draw("digraph { a -> b -> c; { rank=min; c } }");
    assert_eq!(row(&out, "a"), row(&out, "c"), "{out}");
    assert!(row(&out, "b") > row(&out, "c"), "{out}");
    // A source is alone above, a sink alone below.
    let out = draw("digraph { a -> b; x; { rank=source; b } }");
    assert!(row(&out, "b") < row(&out, "a"), "{out}");
    assert!(row(&out, "b") < row(&out, "x"), "{out}");
    let out = draw("digraph { a -> b; x -> y -> z; { rank=max; b } }");
    assert_eq!(row(&out, "b"), row(&out, "z"), "{out}");
    let out = draw("digraph { a -> b; x -> y -> z; { rank=sink; a } }");
    assert!(row(&out, "a") > row(&out, "z"), "{out}");
    assert!(row(&out, "b") < row(&out, "a"), "{out}");
}

/// LAYOUT-4: a flat edge between nodes that are not neighbors runs below the rank, and a
/// labeled one goes up to its label above the rank; between clusters it passes their borders.
#[test]
fn flat_edges() {
    let draw = |src: &str| ascii::render(&chart(src), &Options::default());
    let out = draw("digraph { {rank=same; a; b; c} a -> c }");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[1], "( a )   ( b )   ( c )", "{out}");
    assert_eq!(lines[3], "  │               ▲", "{out}");
    assert_eq!(lines[4], "  └───────────────┘", "{out}");
    // The label above the rank, the route reaching it.
    let out = draw("digraph { {rank=same; a; b} a -> b [label=go] }");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0].trim(), "go", "{out}");
    assert_eq!(lines[1].trim(), "│", "{out}");
    assert!(lines.last().unwrap().contains("╰───╯   ╰───╯"), "{out}");
    // Out of a cluster's bottom and into the other's.
    let out = draw(
        "digraph { subgraph cluster_p { a } subgraph cluster_q { b } a -> b [constraint=false] }",
    );
    assert!(out.contains("└───┼───┘   └───┼───┘"), "{out}");
    assert!(out.contains("    └───────────┘"), "{out}");
    let out = draw(
        "digraph { subgraph cluster_p { a } subgraph cluster_q { b } a -> b [constraint=false, label=x] }",
    );
    assert!(out.starts_with("          x\n          │\n"), "{out}");
    assert!(out.contains("┌───┼───┐   ┌───┼───┐"), "{out}");
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
        "digraph {\n  splines=ortho; splines=line\n  { rank=same; a; b }\n  a:p -> b:s\n  a [label=<<i>x</i>>, shape=star2]\n  b [width=1, color=nope]\n  a -> b [arrowhead=zzz, weight=2]\n}\ndigraph two {}\n",
    );
    let w = |s: &str| (Severity::Warning, s.to_owned());
    assert_eq!(
        d,
        [
            w("the graph attribute `splines` is not supported; ignored"),
            w("record ports are not supported yet; the edge joins the node"),
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

/// An unclosed string or HTML string runs to the end of the source and keeps all its text.
#[test]
fn unclosed_labels_keep_their_text() {
    for (src, text) in [
        ("digraph { a [label=<x é", "x é"),
        ("digraph { a [label=<x<b>é", "xé"),
        ("digraph { a [label=\"é", "é"),
        ("digraph { a [label=\"q\\\"", "q\""),
    ] {
        let (c, d) = lowered(src);
        assert!(!d.is_empty(), "{src}");
        assert_eq!(c.nodes[0].label.text, text, "{src}");
    }
}
