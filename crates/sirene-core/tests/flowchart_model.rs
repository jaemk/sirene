//! Tests of flowchart lowering (FLOWMO-*), plus snapshots of the model for every fixture.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::flowchart::{
    self, Arrow, Click, Direction, Edge, Flowchart, Label, Lowered, Shape, Stroke, Style,
};

fn lower(src: &str) -> Lowered {
    flowchart::parse(src).lower()
}

/// Lowers `src`, asserting no diagnostics.
fn chart(src: &str) -> Flowchart {
    let lowered = lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    lowered.flowchart
}

/// Diagnostic messages with severities, in order.
fn messages(src: &str) -> Vec<(Severity, String)> {
    lower(src)
        .diagnostics
        .into_iter()
        .map(|d| (d.severity, d.message))
        .collect()
}

fn style(p: &str, v: &str) -> Style {
    Style {
        property: p.into(),
        value: v.into(),
    }
}

fn plain(text: &str) -> Label {
    Label::plain(text)
}

fn edge_summary(e: &Edge) -> (String, String, Stroke, Option<Arrow>, Option<Arrow>, u8) {
    (
        e.from.clone(),
        e.to.clone(),
        e.stroke,
        e.start,
        e.end,
        e.length,
    )
}

// ---- FLOWMO-3 ----

#[test]
fn header_directions() {
    let cases = [
        ("flowchart", Direction::TopBottom),
        ("graph TD", Direction::TopBottom),
        ("graph TB", Direction::TopBottom),
        ("graph v", Direction::TopBottom),
        ("graph BT", Direction::BottomTop),
        ("graph ^", Direction::BottomTop),
        ("graph LR", Direction::LeftRight),
        ("graph >", Direction::LeftRight),
        ("graph RL", Direction::RightLeft),
        ("graph <", Direction::RightLeft),
    ];
    for (src, dir) in cases {
        assert_eq!(chart(src).direction, dir, "{src}");
    }
}

#[test]
fn top_level_direction_is_ignored() {
    let lowered = lower("flowchart LR\ndirection TB\n");
    assert_eq!(lowered.flowchart.direction, Direction::LeftRight);
    assert_eq!(lowered.diagnostics.len(), 1);
    assert_eq!(lowered.diagnostics[0].severity, Severity::Warning);
}

#[test]
fn titles() {
    let c = chart(
        "---\ntitle: Hello\n---\nflowchart\naccTitle: Acc  \naccDescr: first\naccDescr {\n  multi\n  line\n}\n",
    );
    assert_eq!(c.title.as_deref(), Some("Hello"));
    assert_eq!(c.acc_title.as_deref(), Some("Acc"));
    assert_eq!(c.acc_descr.as_deref(), Some("multi\n  line"));
    assert_eq!(
        chart("flowchart\naccTitle:\n").acc_title.as_deref(),
        Some("")
    );
    assert_eq!(chart("flowchart\n").title, None);
}

// ---- FLOWMO-4, FLOWMO-5, FLOWMO-6 ----

#[test]
fn nodes_in_first_mention_order() {
    let c = chart("flowchart\nB --> A\nC\nA --> B\n");
    let ids: Vec<_> = c.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["B", "A", "C"]);
    assert_eq!(c.node("A").unwrap().mentions.len(), 2);
    assert_eq!(c.node("A").unwrap().label, plain("A"));
    assert_eq!(c.node("C").unwrap().shape, Shape::Rect);
}

#[test]
fn last_shape_and_label_win() {
    let c = chart("flowchart\nA[one] --> B\nA(two)\nA\n");
    let a = c.node("A").unwrap();
    assert_eq!(a.shape, Shape::Rounded);
    assert_eq!(a.label, plain("two"));
}

#[test]
fn classic_shapes() {
    let src = "flowchart
a[t] --> b(t) --> c([t]) --> d[[t]] --> e[(t)] --> f((t)) --> g(((t))) --> h(-t-)
i>t] --> j{t} --> k{{t}} --> l[/t/] --> m[\\t\\] --> n[/t\\] --> o[\\t/]
";
    let c = chart(src);
    let shapes: Vec<_> = c.nodes.iter().map(|n| n.shape).collect();
    use Shape::*;
    assert_eq!(
        shapes,
        [
            Rect,
            Rounded,
            Stadium,
            FramedRect,
            Cylinder,
            Circle,
            DoubleCircle,
            Ellipse,
            Odd,
            Diamond,
            Hexagon,
            LeanRight,
            LeanLeft,
            TrapezoidBottom,
            TrapezoidTop
        ]
    );
    assert!(c.nodes.iter().all(|n| n.label == plain("t")));
}

#[test]
fn labels() {
    let c = chart(
        "flowchart
a[\"  quoted #quot;x#quot; \"]
b[\"`**md**`\"]
c[line one<br/>line two]
d[I #9829; you]
e[]
",
    );
    assert_eq!(c.node("a").unwrap().label, plain("quoted \"x\""));
    assert_eq!(
        c.node("b").unwrap().label,
        Label {
            text: "**md**".into(),
            markdown: true
        }
    );
    assert_eq!(c.node("c").unwrap().label, plain("line one\nline two"));
    assert_eq!(c.node("d").unwrap().label, plain("I \u{2665} you"));
    assert_eq!(c.node("e").unwrap().label, plain(""));
}

#[test]
fn multiline_label() {
    let c = chart("flowchart\na[first\n     second]\n");
    assert_eq!(c.node("a").unwrap().label, plain("first\nsecond"));
}

#[test]
fn class_shorthand() {
    let c = chart("flowchart\nA:::hot --> B:::cold\nA:::warm\n");
    assert_eq!(c.node("A").unwrap().classes, ["hot", "warm"]);
    assert_eq!(c.node("B").unwrap().classes, ["cold"]);
}

#[test]
fn end_node_is_skipped() {
    let lowered = lower("flowchart\nA --> end\n");
    assert_eq!(lowered.flowchart.nodes.len(), 1);
    assert!(lowered.flowchart.edges.is_empty());
    assert!(!lowered.diagnostics.is_empty());
}

#[test]
fn unclosed_shape_still_lowers() {
    let lowered = lower("flowchart\nA[open\nB --> C\n");
    assert_eq!(lowered.flowchart.node("A").unwrap().label, plain("open"));
    assert_eq!(lowered.flowchart.edges.len(), 1);
    assert_eq!(lowered.diagnostics.len(), 1);
}

// ---- FLOWMO-7 ----

#[test]
fn node_metadata() {
    let c = chart(
        "flowchart
A@{ shape: db, label: \"Store\", icon: x }
B@{ label: plain, labelType: text }
C[old]@{ label: \"\" }
D@{
  shape: hex
  label: two
}
",
    );
    let a = c.node("A").unwrap();
    assert_eq!(a.shape, Shape::Cylinder);
    assert_eq!(
        a.label,
        Label {
            text: "Store".into(),
            markdown: true
        }
    );
    assert_eq!(
        a.metadata,
        [
            ("shape".to_owned(), "db".to_owned()),
            ("label".to_owned(), "Store".to_owned()),
            ("icon".to_owned(), "x".to_owned())
        ]
    );
    assert_eq!(c.node("B").unwrap().label, plain("plain"));
    assert_eq!(c.node("C").unwrap().label, plain("old"));
    assert_eq!(c.node("D").unwrap().shape, Shape::Hexagon);
}

#[test]
fn metadata_merges_across_mentions() {
    let c = chart("flowchart\nA@{ a: 1, b: 2 }\nA@{ b: 3 }\n");
    assert_eq!(
        c.node("A").unwrap().metadata,
        [
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "3".to_owned())
        ]
    );
}

#[test]
fn bad_metadata() {
    let lowered = lower("flowchart\nA[x]@{ shape: Rect }\nB@{ shape: nope, junk }\n");
    let c = &lowered.flowchart;
    assert_eq!(c.node("A").unwrap().shape, Shape::Rect);
    assert_eq!(c.node("B").unwrap().shape, Shape::Rect);
    let msgs: Vec<_> = lowered
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        msgs,
        [
            "unknown shape `Rect`",
            "expected `key: value` in metadata, found `junk`",
            "unknown shape `nope`"
        ]
    );
    assert!(
        lowered
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Error)
    );
}

#[test]
fn edge_metadata() {
    let c = chart(
        "flowchart
A e1@--> B
e1@{ animate: true, animation: fast, curve: linear }
",
    );
    assert_eq!(c.nodes.len(), 2, "edge id must not create a node");
    let e = c.edge("e1").unwrap();
    assert!(e.animate);
    assert_eq!(e.animation.as_deref(), Some("fast"));
    assert_eq!(e.curve.as_deref(), Some("linear"));
    assert_eq!(e.metadata.len(), 3);

    let c = chart("flowchart\nA e1@--> B\ne1@{ animate: false }\ne1\n");
    assert!(!c.edge("e1").unwrap().animate);
    assert_eq!(c.nodes.len(), 2);
}

#[test]
fn subgraph_metadata() {
    let c = chart("flowchart\nsubgraph s\nA\nend\ns@{ view: collapsed }\n");
    assert_eq!(
        c.subgraph("s").unwrap().metadata,
        [("view".to_owned(), "collapsed".to_owned())]
    );
    assert!(c.node("s").is_none());
}

// ---- FLOWMO-8 ----

#[test]
fn strokes_and_arrows() {
    use Arrow::*;
    use Stroke::*;
    let cases = [
        ("-->", Normal, None, Some(Point), 1),
        ("---", Normal, None, None, 1),
        ("--x", Normal, None, Some(Cross), 1),
        ("--o", Normal, None, Some(Circle), 1),
        ("<-->", Normal, Some(Point), Some(Point), 1),
        ("x--x", Normal, Some(Cross), Some(Cross), 1),
        ("o--o", Normal, Some(Circle), Some(Circle), 1),
        ("<---", Normal, Some(Point), None, 1),
        ("---->", Normal, None, Some(Point), 3),
        ("----", Normal, None, None, 2),
        ("==>", Thick, None, Some(Point), 1),
        ("===", Thick, None, None, 1),
        ("<====>", Thick, Some(Point), Some(Point), 3),
        ("-.->", Dotted, None, Some(Point), 1),
        ("-.-", Dotted, None, None, 1),
        ("-...-x", Dotted, None, Some(Cross), 3),
        ("~~~", Invisible, None, None, 1),
        ("~~~~~", Invisible, None, None, 3),
        ("------------------->", Normal, None, Some(Point), 10),
    ];
    for (stroke, kind, start, end, length) in cases {
        let c = chart(&format!("flowchart\nA {stroke} B\n"));
        assert_eq!(
            edge_summary(&c.edges[0]),
            ("A".into(), "B".into(), kind, start, end, length),
            "{stroke}"
        );
    }
}

#[test]
fn inline_label_edges() {
    use Arrow::*;
    use Stroke::*;
    let cases = [
        ("-- text -->", Normal, None, Some(Point), 1),
        ("-- text --->", Normal, None, Some(Point), 2),
        ("<-- text -->", Normal, Some(Point), Some(Point), 1),
        ("x-- text --x", Normal, Some(Cross), Some(Cross), 1),
        ("== text ==>", Thick, None, Some(Point), 1),
        ("-. text .->", Dotted, None, Some(Point), 1),
        ("-. text ..->", Dotted, None, Some(Point), 2),
        ("-- text ---", Normal, None, None, 1),
    ];
    for (stroke, kind, start, end, length) in cases {
        let c = chart(&format!("flowchart\nA {stroke} B\n"));
        let e = &c.edges[0];
        assert_eq!(
            edge_summary(e),
            ("A".into(), "B".into(), kind, start, end, length),
            "{stroke}"
        );
        assert_eq!(e.label, Some(plain("text")), "{stroke}");
    }
}

#[test]
fn pipe_labels() {
    let c = chart("flowchart\nA -->|yes| B\nA -->|\"quoted\"| C\nA -->|| D\nA --> E\n");
    let labels: Vec<_> = c.edges.iter().map(|e| e.label.clone()).collect();
    assert_eq!(
        labels,
        [Some(plain("yes")), Some(plain("quoted")), None, None]
    );
}

#[test]
fn groups_make_cartesian_edges() {
    let c = chart("flowchart\nA & B --> C & D --> E\n");
    let pairs: Vec<_> = c
        .edges
        .iter()
        .map(|e| format!("{}{}", e.from, e.to))
        .collect();
    assert_eq!(pairs, ["AC", "AD", "BC", "BD", "CE", "DE"]);
}

#[test]
fn edge_ids() {
    let c = chart("flowchart\nA & B e1@--> C & D\n");
    let ids: Vec<_> = c.edges.iter().map(|e| e.id.as_deref()).collect();
    assert_eq!(ids, [None, None, Some("e1"), None]);
    assert_eq!(c.edge("e1").unwrap().from, "B");
    assert_eq!(c.edge("e1").unwrap().to, "C");

    let lowered = lower("flowchart\nA e1@--> B\nB e1@--> C\n");
    assert_eq!(lowered.flowchart.edges[1].id, None);
    assert_eq!(
        messages("flowchart\nA e1@--> B\nB e1@--> C\n"),
        [(
            Severity::Warning,
            "duplicate edge id `e1` is ignored".into()
        )]
    );
}

#[test]
fn incomplete_edges_are_dropped() {
    let lowered = lower("flowchart\nA -- open\nB -->\nC --> D\n");
    let pairs: Vec<_> = lowered
        .flowchart
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(pairs, [("C", "D")]);
}

#[test]
fn edge_range_covers_stroke_and_label() {
    let src = "flowchart\nA -->|x| B\n";
    let c = chart(src);
    let r = c.edges[0].range;
    assert_eq!(&src[usize::from(r.start())..usize::from(r.end())], "-->|x|");
}

// ---- FLOWMO-9, FLOWMO-10 ----

#[test]
fn subgraph_headers() {
    let c = chart(
        "flowchart
subgraph one
a
end
subgraph id2 [Title Two]
b
end
subgraph \"Quoted Title\"
c
end
subgraph \"Quoted\"
d
end
subgraph free text title
e
end
subgraph
f
end
",
    );
    let got: Vec<_> = c
        .subgraphs
        .iter()
        .map(|s| (s.id.as_str(), s.title.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("one", "one"),
            ("id2", "Title Two"),
            ("subGraph2", "Quoted Title"),
            ("Quoted", "Quoted"),
            ("subGraph4", "free text title"),
            ("subGraph5", "")
        ]
    );
}

#[test]
fn auto_ids_count_in_end_order() {
    let c = chart("flowchart\nsubgraph outer x\nsubgraph inner y\nA\nend\nend\n");
    let ids: Vec<_> = c.subgraphs.iter().map(|s| s.id.as_str()).collect();
    // Declaration order; the inner subgraph closes first so it is subGraph0.
    assert_eq!(ids, ["subGraph1", "subGraph0"]);
    assert_eq!(c.subgraphs[0].members, ["subGraph0"]);
}

#[test]
fn subgraph_direction() {
    let lowered =
        lower("flowchart LR\nsubgraph s\ndirection TB\ndirection BT\nA\nend\nsubgraph t\nB\nend\n");
    assert!(lowered.diagnostics.is_empty());
    let c = lowered.flowchart;
    assert_eq!(
        c.subgraph("s").unwrap().direction,
        Some(Direction::BottomTop)
    );
    assert_eq!(c.subgraph("t").unwrap().direction, None);
}

#[test]
fn nested_membership() {
    let c = chart(
        "flowchart
A --> B
subgraph outer
  B --> C
  subgraph inner
    C --> D
  end
  E
end
",
    );
    assert_eq!(c.subgraph("inner").unwrap().members, ["C", "D"]);
    assert_eq!(c.subgraph("outer").unwrap().members, ["B", "inner", "E"]);
    assert_eq!(c.parent("D").unwrap().id, "inner");
    assert_eq!(c.parent("inner").unwrap().id, "outer");
    assert!(c.parent("A").is_none());
}

#[test]
fn sibling_conflict_warns() {
    let src = "flowchart\nsubgraph one\nA\nend\nsubgraph two\nA\nB\nend\n";
    let lowered = lower(src);
    let c = &lowered.flowchart;
    assert_eq!(c.subgraph("one").unwrap().members, ["A"]);
    assert_eq!(c.subgraph("two").unwrap().members, ["B"]);
    assert_eq!(
        messages(src),
        [(
            Severity::Warning,
            "`A` already belongs to subgraph `one`".into()
        )]
    );
}

#[test]
fn repeated_subgraph_id_merges() {
    let c = chart("flowchart\nsubgraph s [First]\nA\nend\nsubgraph s [Second]\nB\nA\nend\n");
    assert_eq!(c.subgraphs.len(), 1);
    let s = c.subgraph("s").unwrap();
    assert_eq!(s.title, plain("First"));
    assert_eq!(s.members, ["A", "B"]);
}

#[test]
fn subgraph_is_not_its_own_member() {
    let c = chart("flowchart\nsubgraph s\ns --> A\nend\n");
    assert_eq!(c.subgraph("s").unwrap().members, ["A"]);
    assert!(c.node("s").is_none());
}

#[test]
fn node_with_subgraph_id_merges() {
    let c = chart(
        "flowchart
A --> s
s:::hot
style s fill:red
subgraph s
B
end
",
    );
    assert!(c.node("s").is_none());
    let s = c.subgraph("s").unwrap();
    assert_eq!(s.classes, ["hot"]);
    assert_eq!(s.styles, [style("fill", "red")]);
    assert_eq!(c.edges[0].to, "s");
}

// ---- FLOWMO-11, FLOWMO-12 ----

#[test]
fn class_defs() {
    let c = chart("flowchart\nclassDef a,b fill:#f9f,stroke:#333\nclassDef a color:red\n");
    let a = c.class("a").unwrap();
    assert_eq!(
        a.styles,
        [
            style("fill", "#f9f"),
            style("stroke", "#333"),
            style("color", "red")
        ]
    );
    assert_eq!(c.class("b").unwrap().styles.len(), 2);
    let names: Vec<_> = c.classes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

#[test]
fn class_statements() {
    let src = "flowchart
class A early
A e1@--> B
subgraph s
C
end
class A,e1,s,C hot
class nope hot
";
    let lowered = lower(src);
    let c = &lowered.flowchart;
    assert_eq!(c.node("A").unwrap().classes, ["hot"]);
    assert_eq!(c.edge("e1").unwrap().classes, ["hot"]);
    assert_eq!(c.subgraph("s").unwrap().classes, ["hot"]);
    assert_eq!(c.node("C").unwrap().classes, ["hot"]);
    assert_eq!(
        messages(src),
        [
            (Severity::Warning, "`class` refers to unknown id `A`".into()),
            (
                Severity::Warning,
                "`class` refers to unknown id `nope`".into()
            )
        ]
    );
}

#[test]
fn style_statements() {
    let src = "flowchart
A e1@--> B
subgraph s
C
end
style A fill:#f00,stroke-width:4px
style s fill:blue
style e1 stroke:green
style D font-family:a\\,b, bogus
";
    let lowered = lower(src);
    let c = &lowered.flowchart;
    assert_eq!(
        c.node("A").unwrap().styles,
        [style("fill", "#f00"), style("stroke-width", "4px")]
    );
    assert_eq!(c.subgraph("s").unwrap().styles, [style("fill", "blue")]);
    assert_eq!(c.edge("e1").unwrap().styles, [style("stroke", "green")]);
    assert_eq!(c.node("D").unwrap().styles, [style("font-family", "a,b")]);
    assert!(c.node("D").unwrap().mentions.is_empty());
    assert_eq!(
        messages(src),
        [
            (
                Severity::Warning,
                "`style` refers to unknown id `D`; creating a node".into()
            ),
            (
                Severity::Warning,
                "ignoring style declaration `bogus` without `:`".into()
            )
        ]
    );
}

// ---- FLOWMO-13 ----

#[test]
fn link_styles() {
    let src = "flowchart
A --> B --> C
linkStyle 0,1 stroke:red
linkStyle 1 interpolate basis stroke:blue
linkStyle 0 interpolate linear
linkStyle default interpolate step stroke:gray
linkStyle default stroke:black
linkStyle 2,99999999999999999999 stroke:none
";
    let lowered = lower(src);
    let c = &lowered.flowchart;
    assert_eq!(c.edges[0].styles, [style("stroke", "red")]);
    assert_eq!(c.edges[0].curve.as_deref(), Some("linear"));
    assert_eq!(c.edges[1].styles, [style("stroke", "blue")]);
    assert_eq!(c.edges[1].curve.as_deref(), Some("basis"));
    assert_eq!(c.default_curve.as_deref(), Some("step"));
    assert_eq!(c.default_edge_styles, [style("stroke", "black")]);
    assert_eq!(
        messages(src),
        [
            (
                Severity::Error,
                "no edge at index 2 (edges so far: 2)".into()
            ),
            (
                Severity::Error,
                "no edge at index 99999999999999999999 (edges so far: 2)".into()
            )
        ]
    );
}

#[test]
fn link_style_before_edge_is_an_error() {
    let lowered = lower("flowchart\nlinkStyle 0 stroke:red\nA --> B\n");
    assert!(lowered.flowchart.edges[0].styles.is_empty());
    assert_eq!(lowered.diagnostics.len(), 1);
}

// ---- FLOWMO-14 ----

#[test]
fn clicks() {
    let src = "flowchart
A & B & C & D & E & F
click A callback
click B call cb(\"x, y\", 2) \"tip B\"
click C href \"https://c.example\" \"tip C\" _blank
click D \"https://d.example\"
click E call cb()
click F call cb
click nope cb
";
    let lowered = lower(src);
    let c = &lowered.flowchart;
    let click = |id: &str| c.node(id).unwrap().click.clone().unwrap();
    let tip = |id: &str| c.node(id).unwrap().tooltip.clone();
    assert_eq!(
        click("A"),
        Click::Callback {
            name: "callback".into(),
            args: vec![]
        }
    );
    assert_eq!(
        click("B"),
        Click::Callback {
            name: "cb".into(),
            args: vec!["x, y".into(), "2".into()]
        }
    );
    assert_eq!(tip("B").as_deref(), Some("tip B"));
    assert_eq!(
        click("C"),
        Click::Link {
            url: "https://c.example".into(),
            target: Some("_blank".into())
        }
    );
    assert_eq!(tip("C").as_deref(), Some("tip C"));
    assert_eq!(
        click("D"),
        Click::Link {
            url: "https://d.example".into(),
            target: None
        }
    );
    assert_eq!(tip("D"), None);
    assert_eq!(
        click("E"),
        Click::Callback {
            name: "cb".into(),
            args: vec![]
        }
    );
    assert_eq!(
        click("F"),
        Click::Callback {
            name: "cb".into(),
            args: vec![]
        }
    );
    assert_eq!(
        messages(src),
        [(
            Severity::Warning,
            "`click` refers to unknown node `nope`".into()
        )]
    );
}

#[test]
fn incomplete_statements_are_skipped() {
    let lowered = lower(
        "flowchart
A
click A
click A href
classDef
classDef x
class A
style A
linkStyle
linkStyle 0
direction
subgraph s
direction
end
",
    );
    let c = &lowered.flowchart;
    assert!(c.node("A").unwrap().click.is_none());
    assert!(c.classes.is_empty());
    assert!(c.node("A").unwrap().styles.is_empty());
    assert_eq!(c.subgraph("s").unwrap().direction, None);
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
}

// ---- FLOWMO-2 ----

#[test]
fn diagnostics_are_ordered_by_position() {
    let lowered = lower("flowchart\nclick X cb\nA[open\n");
    let starts: Vec<_> = lowered
        .diagnostics
        .iter()
        .map(|d| d.range.start())
        .collect();
    let mut sorted = starts.clone();
    sorted.sort();
    assert_eq!(starts, sorted);
    assert_eq!(lowered.diagnostics.len(), 2);
}

// ---- fixture snapshots ----

fn dump(c: &Flowchart) -> String {
    let mut out = String::new();
    let styles = |s: &[Style]| {
        s.iter()
            .map(|s| format!("{}:{}", s.property, s.value))
            .collect::<Vec<_>>()
            .join(";")
    };
    let label = |l: &Label| format!("{:?}{}", l.text, if l.markdown { " md" } else { "" });
    writeln!(out, "direction {:?}", c.direction).unwrap();
    for (key, value) in [
        ("title", &c.title),
        ("accTitle", &c.acc_title),
        ("accDescr", &c.acc_descr),
        ("defaultCurve", &c.default_curve),
    ] {
        if let Some(v) = value {
            writeln!(out, "{key} {v:?}").unwrap();
        }
    }
    if !c.default_edge_styles.is_empty() {
        writeln!(out, "defaultEdgeStyle {}", styles(&c.default_edge_styles)).unwrap();
    }
    for n in &c.nodes {
        write!(out, "node {} {} {}", n.id, n.shape.name(), label(&n.label)).unwrap();
        if !n.classes.is_empty() {
            write!(out, " classes={}", n.classes.join(",")).unwrap();
        }
        if !n.styles.is_empty() {
            write!(out, " style={}", styles(&n.styles)).unwrap();
        }
        if let Some(click) = &n.click {
            write!(out, " click={click:?}").unwrap();
        }
        if let Some(t) = &n.tooltip {
            write!(out, " tooltip={t:?}").unwrap();
        }
        if !n.metadata.is_empty() {
            write!(out, " meta={:?}", n.metadata).unwrap();
        }
        writeln!(out).unwrap();
    }
    for (i, e) in c.edges.iter().enumerate() {
        write!(
            out,
            "edge {i} {} -> {} {:?} {:?}/{:?} len={}",
            e.from, e.to, e.stroke, e.start, e.end, e.length
        )
        .unwrap();
        if let Some(id) = &e.id {
            write!(out, " id={id}").unwrap();
        }
        if let Some(l) = &e.label {
            write!(out, " label={}", label(l)).unwrap();
        }
        if !e.classes.is_empty() {
            write!(out, " classes={}", e.classes.join(",")).unwrap();
        }
        if !e.styles.is_empty() {
            write!(out, " style={}", styles(&e.styles)).unwrap();
        }
        if let Some(curve) = &e.curve {
            write!(out, " curve={curve}").unwrap();
        }
        if e.animate {
            write!(out, " animate").unwrap();
        }
        if let Some(a) = &e.animation {
            write!(out, " animation={a}").unwrap();
        }
        writeln!(out).unwrap();
    }
    for s in &c.subgraphs {
        write!(
            out,
            "subgraph {} {} dir={:?} members=[{}]",
            s.id,
            label(&s.title),
            s.direction,
            s.members.join(",")
        )
        .unwrap();
        if !s.classes.is_empty() {
            write!(out, " classes={}", s.classes.join(",")).unwrap();
        }
        if !s.styles.is_empty() {
            write!(out, " style={}", styles(&s.styles)).unwrap();
        }
        if !s.metadata.is_empty() {
            write!(out, " meta={:?}", s.metadata).unwrap();
        }
        writeln!(out).unwrap();
    }
    for class in &c.classes {
        writeln!(out, "classDef {} {}", class.name, styles(&class.styles)).unwrap();
    }
    out
}

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/flowchart")
        .join(dir);
    let mut out: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect();
    out.sort();
    out
}

fn render(src: &str) -> String {
    let lowered = lower(src);
    let mut out = dump(&lowered.flowchart);
    out.push_str("--- diagnostics ---\n");
    for d in &lowered.diagnostics {
        writeln!(out, "{:?} {:?}: {}", d.severity, d.range, d.message).unwrap();
    }
    out
}

#[test]
fn model_snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            if dir == "valid" {
                let diagnostics = lower(&src).diagnostics;
                assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
            }
            insta::assert_snapshot!(format!("{dir}__{name}"), render(&src));
        }
    }
}
