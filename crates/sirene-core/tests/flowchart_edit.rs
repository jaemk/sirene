//! Tests of source edits (WYSIWY-8).

use proptest::prelude::*;
use sirene_core::Severity;
use sirene_core::flowchart::edit::{
    Line, MAX_LENGTH, TextEdit, add_edge, add_node, apply_class, delete, fresh_class_name, group,
    keep_together, label_source, move_edge, move_node, move_to_subgraph, remove_class,
    reverse_edge, set_class_style, set_direction, set_edge_label, set_edge_line, set_link_style,
    set_node_label, set_node_shape, ungroup,
};
use sirene_core::flowchart::{self, Arrow, Direction, Flowchart, Shape, Stroke};

fn chart(src: &str) -> Flowchart {
    flowchart::parse(src).lower().flowchart
}

fn errors(src: &str) -> usize {
    flowchart::parse(src)
        .lower()
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// `src` after `edit`, which must exist.
fn applied(src: &str, edit: Option<TextEdit>) -> String {
    edit.expect("an edit").apply(src)
}

#[test]
fn text_edits() {
    let e = TextEdit::between("abcdef", "abXYef").unwrap();
    assert_eq!(
        e,
        TextEdit {
            range: 2..4,
            text: "XY".into()
        }
    );
    assert_eq!(e.apply("abcdef"), "abXYef");
    assert_eq!(TextEdit::between("same", "same"), None);
    // Repeated characters: the prefix wins and the suffix never overlaps it.
    let e = TextEdit::between("aaa", "aaaa").unwrap();
    assert_eq!(
        e,
        TextEdit {
            range: 3..3,
            text: "a".into()
        }
    );
    let e = TextEdit::between("abab", "ab").unwrap();
    assert_eq!(
        e,
        TextEdit {
            range: 2..4,
            text: String::new()
        }
    );
    // Ranges fall on character boundaries.
    let e = TextEdit::between("aé", "aè").unwrap();
    assert_eq!(
        e,
        TextEdit {
            range: 1..3,
            text: "è".into()
        }
    );
}

#[test]
fn label_sources() {
    assert_eq!(label_source("Hello, world!", false), "Hello, world!");
    assert_eq!(label_source("  café  ", false), "café");
    assert_eq!(label_source("a-b", false), "\"a-b\"");
    assert_eq!(label_source("", false), "\"\"");
    assert_eq!(
        label_source("say \"hi\" #1 <b>", false),
        "\"say #quot;hi#quot; #35;1 #lt;b>\""
    );
    assert_eq!(label_source(" one \n two ", false), "\"one<br>two\"");
    assert_eq!(label_source("tab\there", false), "tab here");
    assert_eq!(label_source("**bold**", true), "\"`**bold**`\"");
}

#[test]
fn node_labels() {
    let src = "flowchart LR\n    %% keep me\n    A[Old] --> B\n    A\n";
    assert_eq!(
        applied(src, set_node_label(src, "A", "New one")),
        "flowchart LR\n    %% keep me\n    A[New one] --> B\n    A\n"
    );
    // A node without a shape gets one after its first mention.
    assert_eq!(
        applied(src, set_node_label(src, "B", "x|y")),
        "flowchart LR\n    %% keep me\n    A[Old] --> B[\"x|y\"]\n    A\n"
    );
    // The last mention with a shape sets the label.
    let src = "flowchart\n    A[one] --> B\n    A((two))\n";
    assert_eq!(
        applied(src, set_node_label(src, "A", "three")),
        "flowchart\n    A[one] --> B\n    A((three))\n"
    );
    // Quoted and markdown labels.
    let src = "flowchart\n    A[ \"old\" ]\n    B[\"`**old**`\"]\n";
    assert_eq!(
        applied(src, set_node_label(src, "A", "new")),
        "flowchart\n    A[new]\n    B[\"`**old**`\"]\n"
    );
    assert_eq!(
        applied(src, set_node_label(src, "B", "*new*")),
        "flowchart\n    A[ \"old\" ]\n    B[\"`*new*`\"]\n"
    );
    // No change, no such node, and metadata labels.
    assert_eq!(set_node_label(src, "A", "old"), None);
    assert_eq!(set_node_label(src, "Z", "x"), None);
    let src = "flowchart\n    A@{ label: \"meta\" }\n";
    assert_eq!(set_node_label(src, "A", "x"), None);
    // Metadata without a label keeps its shape.
    let src = "flowchart\n    A@{ shape: hex }\n";
    let out = applied(src, set_node_label(src, "A", "x"));
    let a = chart(&out).nodes[0].clone();
    assert_eq!(
        (a.label.text.as_str(), a.shape),
        ("x", chart(src).nodes[0].shape),
        "{out}"
    );
    assert_eq!(errors(&out), 0, "{out}");
}

#[test]
fn edge_labels() {
    let src = "flowchart\n    A -->|old| B\n";
    assert_eq!(
        applied(src, set_edge_label(src, 0, "new")),
        "flowchart\n    A -->|new| B\n"
    );
    assert_eq!(
        applied(src, set_edge_label(src, 0, " ")),
        "flowchart\n    A --> B\n"
    );
    let src = "flowchart\n    A -- old --> B -.-> C\n    C e1@<==> D\n";
    assert_eq!(
        applied(src, set_edge_label(src, 0, "new")),
        "flowchart\n    A -->|new| B -.-> C\n    C e1@<==> D\n"
    );
    assert_eq!(
        applied(src, set_edge_label(src, 0, "")),
        "flowchart\n    A --> B -.-> C\n    C e1@<==> D\n"
    );
    assert_eq!(
        applied(src, set_edge_label(src, 1, "dots")),
        "flowchart\n    A -- old --> B -.->|dots| C\n    C e1@<==> D\n"
    );
    assert_eq!(
        applied(src, set_edge_label(src, 2, "both")),
        "flowchart\n    A -- old --> B -.-> C\n    C e1@<==>|both| D\n"
    );
    assert_eq!(set_edge_label(src, 1, ""), None);
    assert_eq!(set_edge_label(src, 9, "x"), None);
}

#[test]
fn canonical_strokes() {
    for stroke in [
        "-->", "---", "--x", "--o", "<-->", "x--x", "o--o", "==>", "===", "-.->", "-.-", "-..->",
        "~~~", "~~~~", "---->", "=====>",
    ] {
        let src = format!("flowchart\n    A {stroke}|label| B\n");
        let before = chart(&src).edges[0].clone();
        let after = applied(&src, set_edge_label(&src, 0, ""));
        let edge = &chart(&after).edges[0];
        assert_eq!(edge.label, None, "{after}");
        assert_eq!(
            (edge.stroke, edge.start, edge.end, edge.length),
            (before.stroke, before.start, before.end, before.length),
            "{src} -> {after}"
        );
    }
}

#[test]
fn node_shapes() {
    let src = "flowchart\n    A[Start] --> B\n    A\n";
    assert_eq!(
        applied(src, set_node_shape(src, "A", Shape::Diamond)),
        "flowchart\n    A{Start} --> B\n    A\n"
    );
    // A node without delimiters gets them, with its label, after its first mention.
    assert_eq!(
        applied(src, set_node_shape(src, "B", Shape::Stadium)),
        "flowchart\n    A[Start] --> B([B])\n    A\n"
    );
    // Unquoted text that is not plain words is quoted; quoted text is kept.
    let src = "flowchart\n    A[x-y] --> B[\"q\"]\n";
    assert_eq!(
        applied(src, set_node_shape(src, "A", Shape::Rounded)),
        "flowchart\n    A(\"x-y\") --> B[\"q\"]\n"
    );
    assert_eq!(
        applied(src, set_node_shape(src, "B", Shape::TrapezoidTop)),
        "flowchart\n    A[x-y] --> B[\\\"q\"/]\n"
    );
    // Named shapes go in metadata, which a classic shape then drops.
    let src = "flowchart\n    A[Doc] --> B\n";
    let named = applied(src, set_node_shape(src, "A", Shape::Document));
    assert_eq!(named, "flowchart\n    A[Doc]@{ shape: doc } --> B\n");
    assert_eq!(
        applied(&named, set_node_shape(&named, "A", Shape::Cloud)),
        "flowchart\n    A[Doc]@{ shape: cloud } --> B\n"
    );
    assert_eq!(
        applied(&named, set_node_shape(&named, "A", Shape::Hexagon)),
        "flowchart\n    A{{Doc}} --> B\n"
    );
    let src = "flowchart\n    A@{ label: \"x\", shape: doc }\n    B@{ label: \"y\" }\n    C@{}\n";
    assert_eq!(
        applied(src, set_node_shape(src, "A", Shape::Rect)),
        "flowchart\n    A[x]@{ label: \"x\" }\n    B@{ label: \"y\" }\n    C@{}\n"
    );
    assert_eq!(
        applied(src, set_node_shape(src, "B", Shape::Bolt)),
        "flowchart\n    A@{ label: \"x\", shape: doc }\n    B@{ shape: bolt, label: \"y\" }\n    C@{}\n"
    );
    assert_eq!(
        applied(src, set_node_shape(src, "C", Shape::Bolt)),
        "flowchart\n    A@{ label: \"x\", shape: doc }\n    B@{ label: \"y\" }\n    C@{ shape: bolt }\n"
    );
    // Every shape round-trips and keeps the label.
    let src = "flowchart\n    A[\"Hi there\"] --> B\n";
    for &shape in Shape::ALL {
        let out = match set_node_shape(src, "A", shape) {
            Some(e) => e.apply(src),
            None => src.to_owned(),
        };
        let a = chart(&out).nodes[0].clone();
        assert_eq!(
            (a.shape, a.label.text.as_str()),
            (shape, "Hi there"),
            "{out}"
        );
        assert_eq!(errors(&out), 0, "{out}");
    }
    assert_eq!(set_node_shape(src, "A", Shape::Rect), None);
    assert_eq!(set_node_shape(src, "Z", Shape::Rect), None);
}

#[test]
fn edge_lines() {
    let line = |stroke, start, end, length| Line {
        stroke,
        start,
        end,
        length,
    };
    let src = "flowchart\n    A -- yes --> B -.-> C\n    C e1@==>|go| D\n";
    assert_eq!(
        applied(
            src,
            set_edge_line(src, 0, line(Stroke::Dotted, None, Some(Arrow::Point), 1))
        ),
        "flowchart\n    A -.->|yes| B -.-> C\n    C e1@==>|go| D\n"
    );
    assert_eq!(
        applied(
            src,
            set_edge_line(
                src,
                2,
                line(Stroke::Normal, Some(Arrow::Point), Some(Arrow::Point), 3)
            )
        ),
        "flowchart\n    A -- yes --> B -.-> C\n    C e1@<---->|go| D\n"
    );
    assert_eq!(
        applied(
            src,
            set_edge_line(src, 1, line(Stroke::Invisible, None, None, 1))
        ),
        "flowchart\n    A -- yes --> B ~~~ C\n    C e1@==>|go| D\n"
    );
    let e = chart(src).edges[1].clone();
    let longest = applied(
        src,
        set_edge_line(
            src,
            1,
            Line {
                length: 99,
                ..Line::of(&e)
            },
        ),
    );
    assert_eq!(chart(&longest).edges[1].length, MAX_LENGTH);
    assert_eq!(set_edge_line(src, 1, Line::of(&e)), None);
    assert_eq!(
        set_edge_line(
            src,
            1,
            Line {
                length: 0,
                ..Line::of(&e)
            }
        ),
        None
    );
    assert_eq!(set_edge_line(src, 9, Line::of(&e)), None);
}

#[test]
fn directions() {
    // The header's direction is replaced, or added.
    let src = "flowchart TD\n    A --> B\n";
    assert_eq!(
        applied(src, set_direction(src, None, Direction::LeftRight)),
        "flowchart LR\n    A --> B\n"
    );
    assert_eq!(set_direction(src, None, Direction::TopBottom), None);
    assert_eq!(
        set_direction("graph TB\n", None, Direction::TopBottom),
        None
    );
    assert_eq!(
        applied(
            "graph\nA\n",
            set_direction("graph\nA\n", None, Direction::RightLeft)
        ),
        "graph RL\nA\n"
    );

    // A subgraph's last `direction` is replaced; without one, one is added after the header.
    let src = "flowchart TD\n    subgraph s [S]\n        A --> B\n    end\n";
    let out = applied(src, set_direction(src, Some("s"), Direction::LeftRight));
    assert_eq!(
        out,
        "flowchart TD\n    subgraph s [S]\n        direction LR\n        A --> B\n    end\n"
    );
    assert_eq!(
        chart(&out).subgraphs[0].direction,
        Some(Direction::LeftRight)
    );
    assert_eq!(
        applied(&out, set_direction(&out, Some("s"), Direction::BottomTop)),
        "flowchart TD\n    subgraph s [S]\n        direction BT\n        A --> B\n    end\n"
    );
    assert_eq!(set_direction(&out, Some("s"), Direction::LeftRight), None);
    assert_eq!(
        set_direction(&out, Some("nope"), Direction::LeftRight),
        None
    );
    // An empty subgraph indents one level in.
    let src = "flowchart TD\n  subgraph s\n  end\n";
    assert_eq!(
        applied(src, set_direction(src, Some("s"), Direction::RightLeft)),
        "flowchart TD\n  subgraph s\n      direction RL\n  end\n"
    );
}

#[test]
fn class_styles() {
    let src = "flowchart TD\n    A --> B\n";
    // A new class gets a `classDef`; later properties go in it, replaced in place.
    let out = applied(src, set_class_style(src, "hot", "fill", Some("#f00")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    classDef hot fill:#f00\n"
    );
    let out = applied(&out, set_class_style(&out, "hot", "stroke", Some("#900")));
    let out = applied(&out, set_class_style(&out, "hot", "fill", Some("#ff0000")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    classDef hot fill:#ff0000,stroke:#900\n"
    );
    assert_eq!(chart(&out).classes[0].styles.len(), 2);
    assert_eq!(set_class_style(&out, "hot", "stroke", Some("#900")), None);
    // Removing properties; the last one takes the statement with it.
    let out = applied(&out, set_class_style(&out, "hot", "fill", None));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    classDef hot stroke:#900\n"
    );
    assert_eq!(
        applied(&out, set_class_style(&out, "hot", "stroke", None)),
        src
    );
    assert_eq!(set_class_style(src, "cold", "fill", None), None);
    // A `classDef` naming several classes is left alone; an unparsable declaration is kept.
    let shared = "flowchart TD\n    classDef a,b fill:#fff\n    classDef a color:red,junk\n";
    assert_eq!(
        applied(shared, set_class_style(shared, "a", "stroke", Some("#000"))),
        "flowchart TD\n    classDef a,b fill:#fff\n    classDef a color:red,stroke:#000,junk\n"
    );
    // Names not taken.
    assert_eq!(fresh_class_name(src), "style1");
    assert_eq!(
        fresh_class_name("flowchart TD\n    classDef style1 fill:#fff\n    style2\n"),
        "style3"
    );
}

#[test]
fn link_styles() {
    let src = "flowchart TD\n    A --> B --> C\n";
    // An unstyled edge gets a `linkStyle`; later properties go in it, replaced in place.
    let out = applied(src, set_link_style(src, 1, "stroke", Some("#f00")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B --> C\n    linkStyle 1 stroke:#f00\n"
    );
    let out = applied(&out, set_link_style(&out, 1, "stroke-width", Some("3px")));
    let out = applied(&out, set_link_style(&out, 1, "stroke", Some("#00f")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B --> C\n    linkStyle 1 stroke:#00f,stroke-width:3px\n"
    );
    assert_eq!(chart(&out).edges[1].styles.len(), 2);
    assert!(chart(&out).edges[0].styles.is_empty());
    assert_eq!(set_link_style(&out, 1, "stroke", Some("#00f")), None);
    // Removing properties; the last one takes the statement with it.
    let out = applied(&out, set_link_style(&out, 1, "stroke", None));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B --> C\n    linkStyle 1 stroke-width:3px\n"
    );
    assert_eq!(
        applied(&out, set_link_style(&out, 1, "stroke-width", None)),
        src
    );
    assert_eq!(set_link_style(src, 0, "stroke", None), None);
    assert_eq!(set_link_style(src, 2, "stroke", Some("red")), None);

    // A shared statement drops the index; the edge gets its own with every property, and
    // keeps its curve.
    let shared = "flowchart TD\n    A --> B --> C\n    linkStyle 0,1 interpolate step color:red\n";
    let out = applied(shared, set_link_style(shared, 1, "stroke", Some("#000")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B --> C\n    linkStyle 0 interpolate step color:red\n    \
         linkStyle 1 interpolate step color:red,stroke:#000\n"
    );
    assert_eq!(chart(&out).edges[0].styles, chart(shared).edges[0].styles);
    assert_eq!(chart(&out).edges[1].curve.as_deref(), Some("step"));

    // The last statement naming only the edge is rewritten with the styles in effect (a later
    // `linkStyle` replaces earlier ones); an earlier one loses its styles, keeping its curve,
    // and a later one is removed.
    let many = "flowchart TD\n    A --> B\n    linkStyle 0 interpolate basis color:red\n    \
                linkStyle 0 stroke:#111\n    linkStyle 0,0 color:blue\n";
    assert_eq!(
        applied(many, set_link_style(many, 0, "stroke-width", Some("2px"))),
        "flowchart TD\n    A --> B\n    linkStyle 0 interpolate basis\n    \
         linkStyle 0 color:blue,stroke-width:2px\n"
    );
    // Curve only: the styles go after it, and come out again leaving the curve.
    let curve = "flowchart TD\n    A --> B\n    linkStyle 0 interpolate basis\n";
    let out = applied(curve, set_link_style(curve, 0, "stroke", Some("red")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    linkStyle 0 interpolate basis stroke:red\n"
    );
    assert_eq!(
        applied(&out, set_link_style(&out, 0, "stroke", None)),
        curve
    );
    // `linkStyle default` stays.
    let default = "flowchart TD\n    A --> B\n    linkStyle default stroke:#999\n";
    assert_eq!(
        applied(default, set_link_style(default, 0, "stroke", Some("red"))),
        "flowchart TD\n    A --> B\n    linkStyle default stroke:#999\n    linkStyle 0 stroke:red\n"
    );
    for s in [&out, shared, many, default] {
        assert_eq!(errors(s), 0, "{s}");
    }
}

#[test]
fn applying_classes() {
    let src = "flowchart TD\n    A:::hot --> B\n    C\n    class A,B,C hot\n    class C cold\n";
    assert_eq!(apply_class(src, &["A", "B"], "hot"), None);
    assert_eq!(apply_class(src, &["Z"], "hot"), None);
    assert_eq!(
        applied(src, apply_class(src, &["A", "B"], "cold")),
        format!("{src}    class A,B cold\n")
    );
    // Removing takes ids out of `class` lists and drops `:::` shorthands.
    let out = applied(src, remove_class(src, &["A", "C"], "hot"));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    C\n    class B hot\n    class C cold\n"
    );
    assert!(chart(&out).node("A").unwrap().classes.is_empty());
    let out = applied(&out, remove_class(&out, &["C"], "cold"));
    assert_eq!(out, "flowchart TD\n    A --> B\n    C\n    class B hot\n");
    assert_eq!(remove_class(&out, &["A"], "hot"), None);
}

#[test]
fn keeping_together() {
    let src = "flowchart LR\n    A\n    B\n";
    let out = keep_together(src, "A", "B").apply(src);
    assert_eq!(out, "flowchart LR\n    A\n    B\n    A ~~~ B\n");
    assert_eq!(chart(&out).edges[0].stroke, Stroke::Invisible);
}

#[test]
fn grouping() {
    let src = "flowchart TD\n    A --> B\n    B --> C\n";
    let (id, edit) = group(src, &["A", "B"], "Front end").unwrap();
    assert_eq!(id, "group1");
    let out = edit.apply(src);
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    B --> C\n    subgraph group1 [Front end]\n        A\n        B\n    end\n"
    );
    let c = chart(&out);
    assert_eq!(c.subgraphs[0].members, ["A", "B"]);
    assert_eq!(c.subgraphs[0].title.text, "Front end");
    assert_eq!(errors(&out), 0);

    // Nodes in one subgraph are grouped inside it; the next id skips used ones.
    let (id, edit) = group(&out, &["A"], "x[y]").unwrap();
    assert_eq!(id, "group2");
    let nested = edit.apply(&out);
    assert_eq!(
        nested,
        "flowchart TD\n    A --> B\n    B --> C\n    subgraph group1 [Front end]\n        A\n        B\n        subgraph group2 [\"x[y]\"]\n            A\n        end\n    end\n"
    );
    let c = chart(&nested);
    let g1 = c.subgraph("group1").unwrap();
    assert_eq!(g1.members, ["B", "group2"]);
    assert_eq!(c.subgraph("group2").unwrap().members, ["A"]);

    // Nodes in different subgraphs, missing nodes, and nothing are not grouped.
    assert_eq!(group(&out, &["A", "C"], "x"), None);
    assert_eq!(group(src, &["A", "Z"], "x"), None);
    assert_eq!(group(src, &[], "x"), None);
}

#[test]
fn ungrouping() {
    let src = "flowchart TD\n    subgraph s [S]\n        direction LR\n        A --> B\n        subgraph t\n            C\n        end\n    end\n    style s fill:#eee\n    class s,A warm\n    class s cool\n";
    let out = applied(src, ungroup(src, "s"));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    subgraph t\n        C\n    end\n    class A warm\n"
    );
    assert_eq!(chart(&out).subgraph("s"), None);
    assert_eq!(chart(&out).subgraph("t").unwrap().members, ["C"]);

    // Edges at the subgraph, a missing subgraph, and a one-line block are left alone.
    assert_eq!(
        ungroup(
            "flowchart TD\n    subgraph s\n        A\n    end\n    s --> B\n",
            "s"
        ),
        None
    );
    assert_eq!(ungroup(src, "nope"), None);
    assert_eq!(ungroup("flowchart TD\n    subgraph s; A; end\n", "s"), None);
}

#[test]
fn moving_into_subgraphs() {
    let src = "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n    end\n";
    // From the top level: a bare mention goes in.
    let out = applied(src, move_to_subgraph(src, "A", Some("s")));
    assert_eq!(
        out,
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n        A\n    end\n"
    );
    assert_eq!(chart(&out).subgraph("s").unwrap().members, ["C", "A"]);
    // Out again: its lone statements move to the end of the document.
    let back = applied(&out, move_to_subgraph(&out, "A", None));
    assert_eq!(
        back,
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n    end\n    A\n"
    );
    assert_eq!(chart(&back).subgraph("s").unwrap().members, ["C"]);

    // Nothing to do, a missing node or subgraph, and a node in a chain inside a subgraph.
    assert_eq!(move_to_subgraph(src, "A", None), None);
    assert_eq!(move_to_subgraph(src, "Z", Some("s")), None);
    assert_eq!(move_to_subgraph(src, "A", Some("nope")), None);
    let chain = "flowchart TD\n    subgraph s\n        A --> B\n    end\n    subgraph t\n    end\n";
    assert_eq!(move_to_subgraph(chain, "A", Some("t")), None);
    // A target whose `end` follows a statement on its line.
    let tight = "flowchart TD\n    X\n    subgraph t\n        Y; end\n";
    assert_eq!(
        applied(tight, move_to_subgraph(tight, "X", Some("t"))),
        "flowchart TD\n    X\n    subgraph t\n        Y; \n        X\n    end\n"
    );
}

#[test]
fn moving_statements() {
    let src = "flowchart TD\n    A --> B\n    C --> D\n    E\n    linkStyle 0 stroke:red\n    linkStyle 1 stroke:blue\n";
    // C --> D moves before A --> B; the link styles follow their edges.
    let out = applied(src, move_edge(src, 1, false));
    assert_eq!(
        out,
        "flowchart TD\n    C --> D\n    A --> B\n    E\n    linkStyle 1 stroke:red\n    linkStyle 0 stroke:blue\n"
    );
    assert_eq!(chart(&out).nodes[0].id, "C");
    // And back, by node.
    assert_eq!(applied(&out, move_node(&out, "A", false)), src);
    assert_eq!(applied(src, move_node(src, "A", true)), out);
    // E swaps with the edge before it, which keeps its index; nothing moves past the ends or a
    // `linkStyle`.
    assert_eq!(
        applied(src, move_node(src, "E", false)),
        "flowchart TD\n    A --> B\n    E\n    C --> D\n    linkStyle 0 stroke:red\n    linkStyle 1 stroke:blue\n"
    );
    assert_eq!(move_node(src, "A", false), None);
    assert_eq!(move_node(src, "E", true), None);
    assert_eq!(move_node(src, "Z", true), None);
    assert_eq!(move_edge(src, 9, true), None);
    // A subgraph swaps as a whole.
    let src = "flowchart TD\n    A\n    subgraph s\n        B\n    end\n";
    assert_eq!(
        applied(src, move_node(src, "A", true)),
        "flowchart TD\n    subgraph s\n        B\n    end\n    A\n"
    );
}

#[test]
fn reversing_edges() {
    let src = "flowchart\n    A[Start] -->|go| B{Ok?}\n    B --> C --> D\n    E & F --> G\n";
    assert_eq!(
        applied(src, reverse_edge(src, 0)),
        "flowchart\n    B{Ok?} -->|go| A[Start]\n    B --> C --> D\n    E & F --> G\n"
    );
    // In a chain, the edge goes (as `delete` takes it, keeping where D is first mentioned) and
    // comes back reversed at the end.
    let out = applied(src, reverse_edge(src, 2));
    assert_eq!(
        out,
        "flowchart\n    A[Start] -->|go| B{Ok?}\n    B --> C\n    D\n    E & F --> G\n    D --> C\n"
    );
    let c = chart(&out);
    let ends: Vec<(&str, &str)> = c
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(
        ends,
        [("A", "B"), ("B", "C"), ("E", "G"), ("F", "G"), ("D", "C")]
    );
    // Shared strokes and ids in chains are left alone.
    assert_eq!(reverse_edge(src, 3), None);
    let src = "flowchart\n    A e1@--> B --> C\n";
    assert_eq!(reverse_edge(src, 0), None);
    assert_eq!(reverse_edge(src, 9), None);
}

#[test]
fn adding() {
    let src = "flowchart\n  A --> B\n";
    let (id, edit) = add_node(src, "New node");
    assert_eq!(id, "C");
    assert_eq!(edit.apply(src), "flowchart\n  A --> B\n  C[New node]\n");
    let (id, edit) = add_node("flowchart\r\n    A", "");
    assert_eq!(id, "B");
    assert_eq!(
        edit.apply("flowchart\r\n    A"),
        "flowchart\r\n    A\r\n    B\r\n"
    );
    assert_eq!(
        add_node("flowchart\n", "x").1.apply("flowchart\n"),
        "flowchart\n    A[x]\n"
    );

    let taken: String = ('A'..='Z').map(|c| format!("    {c}\n")).collect();
    let src = format!("flowchart\n{taken}    n1\n    subgraph n2\n    end\n    A n3@--> B\n");
    assert_eq!(add_node(&src, "").0, "n4");
    // Ids that statements refer to, without a node yet, are taken too.
    let src = "flowchart\n    A\n    style B fill:red\n    class C hot\n    click D cb\n";
    assert_eq!(add_node(src, "").0, "E");

    let src = "flowchart\n    subgraph s\n        A\n    end\n";
    assert_eq!(
        add_edge(src, "A", "s").apply(src),
        "flowchart\n    subgraph s\n        A\n    end\n    A --> s\n"
    );
}

#[test]
fn deleting_edges() {
    let src = "flowchart\n    A[a] --> B --> C\n    %% note\n";
    assert_eq!(
        applied(src, delete(src, &[], &[0])),
        "flowchart\n    A[a]\n    B --> C\n    %% note\n"
    );
    assert_eq!(
        applied(src, delete(src, &[], &[1])),
        "flowchart\n    A[a] --> B\n    C\n    %% note\n"
    );
    let src = "flowchart\n    A --> B\n    B --> C\n";
    assert_eq!(
        applied(src, delete(src, &[], &[0, 1])),
        "flowchart\n    A\n    B\n    C\n"
    );
    // Pieces that are first mentions stay, keeping node order.
    let src = "flowchart\n    A --> B\n    A --> C\n    B\n";
    assert_eq!(
        applied(src, delete(src, &[], &[0])),
        "flowchart\n    A\n    B\n    A --> C\n    B\n"
    );
    let src = "flowchart\n    A --> C\n    A --> B\n    B\n";
    assert_eq!(
        applied(src, delete(src, &[], &[1])),
        "flowchart\n    A --> C\n    B\n    B\n"
    );
    // Edges in one statement with `&` go together.
    let src = "flowchart\n    A & B --> C\n";
    assert_eq!(
        applied(src, delete(src, &[], &[1])),
        "flowchart\n    A & B\n    C\n"
    );
    assert_eq!(delete(src, &[], &[7]), None);
}

#[test]
fn deleting_nodes() {
    let src = "flowchart\n    A --> B --> C\n    B --> D\n";
    assert_eq!(
        applied(src, delete(src, &["B"], &[])),
        "flowchart\n    A\n    C\n    D\n"
    );
    let src = "flowchart\n    A & B --> C\n    B[b] --> D; A --> D\n";
    assert_eq!(
        applied(src, delete(src, &["B"], &[])),
        "flowchart\n    A --> C\n    D; A --> D\n"
    );
    // The second lone `D` goes, and its statement with the `;` before it.
    assert_eq!(
        applied(src, delete(src, &["A", "B"], &[])),
        "flowchart\n    C\n    D\n"
    );
    let src = "flowchart\n    X; A --> B; Y\n";
    assert_eq!(
        applied(src, delete(src, &["X", "Y"], &[])),
        "flowchart\n    A --> B\n"
    );
    assert_eq!(
        applied(src, delete(src, &["X", "A", "B"], &[])),
        "flowchart\n    Y\n"
    );
    // Membership: a lone mention in a subgraph stays even when mentioned outside it.
    let src = "flowchart\n    A --> B\n    subgraph s\n        A --> C\n    end\n";
    assert_eq!(
        applied(src, delete(src, &["C"], &[])),
        "flowchart\n    A --> B\n    subgraph s\n        A\n    end\n"
    );
    let src = "flowchart\n    A --> B\n    A\n";
    assert_eq!(applied(src, delete(src, &["A"], &[])), "flowchart\n    B\n");
    assert_eq!(delete(src, &["Z"], &[]), None);
}

#[test]
fn deleting_cleans_up_references() {
    let src = "flowchart\n    A --> B\n    B --> C\n    C --> A\n    class A,B hot\n    class A cold\n    style A fill:#f96\n    click A cb\n    style B fill:#f96\n    linkStyle 0,2 color:red\n    linkStyle 1 color:blue\n    linkStyle default color:green\n";
    let out = applied(src, delete(src, &["A"], &[]));
    assert_eq!(
        out,
        "flowchart\n    B\n    B --> C\n    class B hot\n    style B fill:#f96\n    linkStyle 0 color:blue\n    linkStyle default color:green\n"
    );
    // Edges keep their styles under the new indexes.
    let src = "flowchart\n    A --> B\n    B --> C\n    C --> D\n    linkStyle 2 color:red\n";
    let out = applied(src, delete(src, &[], &[0]));
    assert_eq!(chart(&out).edges[1].styles[0].value, "red");
    // An edge's `id@{ ... }` statement goes with it.
    let src = "flowchart\n    A e1@--> B\n    e1@{ animate: true }\n";
    assert_eq!(
        applied(src, delete(src, &[], &[0])),
        "flowchart\n    A\n    B\n"
    );
    // Also when the edge goes with a deleted node.
    let src = "flowchart\n    A e1@--> B\n    e1@{ animate: true }\n    class e1 hot\n";
    assert_eq!(applied(src, delete(src, &["A"], &[])), "flowchart\n    B\n");
}

#[test]
fn deleting_keeps_incomplete_tails_and_layout() {
    let src = "flowchart\n    A --> B -->\n";
    assert_eq!(
        applied(src, delete(src, &[], &[0])),
        "flowchart\n    A\n    B -->\n"
    );
    let src = "flowchart\n    A --> B --> X -->\n";
    assert_eq!(
        applied(src, delete(src, &["X"], &[])),
        "flowchart\n    A --> B\n"
    );
    let src = "flowchart\r\n\tA --> B;C --> D\r\n";
    assert_eq!(
        applied(src, delete(src, &["A"], &[])),
        "flowchart\r\n\tB;C --> D\r\n"
    );
    assert_eq!(
        applied(src, delete(src, &["C", "D"], &[])),
        "flowchart\r\n\tA --> B\r\n"
    );
}

/// The label text a parse gives back for `text` (WYSIWY-8).
fn normalized(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

const SHAPES: &[(&str, &str)] = &[
    ("[", "]"),
    ("(", ")"),
    ("([", "])"),
    ("[[", "]]"),
    ("[(", ")]"),
    ("((", "))"),
    ("(((", ")))"),
    (">", "]"),
    ("{", "}"),
    ("{{", "}}"),
    ("[/", "/]"),
    ("[\\", "\\]"),
    ("(-", "-)"),
];

fn label_text() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    let pieces = prop::sample::select(vec![
        "a", "Z", "0", " ", "\n", "\r", "\t", "\"", "`", "#", "#quot;", ";", "<", "<br>", ">",
        "[", "]", "(", ")", "{", "}", "|", "/", "\\", "-", "--", "==", "-.", ".", "&", "é", "🐟",
        "%%", ":::", "@{", "end",
    ]);
    prop_oneof![
        prop::collection::vec(pieces, 0..12).prop_map(|v| v.concat()),
        any::<String>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn node_labels_round_trip(
        text in label_text(),
        shape in prop::sample::select(SHAPES),
        markdown in any::<bool>(),
    ) {
        let (open, close) = shape;
        let old = if markdown { "\"`old`\"" } else { "old" };
        let src = format!("flowchart\n    A{open}{old}{close} --> B\n");
        let out = set_node_label(&src, "A", &text).map_or(src.clone(), |e| e.apply(&src));
        prop_assert_eq!(errors(&out), 0, "{}", out);
        let c = chart(&out);
        prop_assert_eq!(&c.nodes[0].label.text, &normalized(&text), "{}", out);
        prop_assert_eq!(c.nodes[0].label.markdown, markdown);
        prop_assert_eq!(c.edges.len(), 1);
    }

    #[test]
    fn edge_labels_round_trip(text in label_text(), inline in any::<bool>()) {
        let src = if inline {
            "flowchart\n    A -- old --> B\n"
        } else {
            "flowchart\n    A -->|old| B\n"
        };
        let out = set_edge_label(src, 0, &text).map_or(src.to_owned(), |e| e.apply(src));
        prop_assert_eq!(errors(&out), 0, "{}", out);
        let c = chart(&out);
        let want = normalized(&text);
        prop_assert_eq!(c.edges[0].label.as_ref().map(|l| l.text.clone()), (!want.is_empty()).then_some(want), "{}", out);
    }

    #[test]
    fn added_nodes_round_trip(text in label_text()) {
        let src = "flowchart\n    A --> B\n";
        let (id, edit) = add_node(src, &text);
        let out = edit.apply(src);
        prop_assert_eq!(errors(&out), 0, "{}", out);
        let c = chart(&out);
        let want = normalized(&text);
        let want = if want.is_empty() { id.clone() } else { want };
        prop_assert_eq!(&c.node(&id).unwrap().label.text, &want);
    }
}

/// Generated flowcharts with chains, `&` groups, labels, and a subgraph.
fn document() -> impl Strategy<Value = String> {
    let id = prop::sample::select(vec!["A", "B", "C", "D", "E"]);
    let node = (id.clone(), any::<bool>()).prop_map(|(id, shaped)| {
        if shaped {
            format!("{id}[{id} label]")
        } else {
            id.to_owned()
        }
    });
    let group = prop::collection::vec(node, 1..3).prop_map(|v| v.join(" & "));
    let edge = prop::sample::select(vec![" --> ", " -.-> ", " ==>|x| ", " -- y --> ", " --- "]);
    let chain =
        (group.clone(), prop::collection::vec((edge, group), 0..4)).prop_map(|(g, rest)| {
            let mut s = g;
            for (e, g) in rest {
                s.push_str(e);
                s.push_str(&g);
            }
            s
        });
    let stmt = prop_oneof![
        4 => chain,
        1 => id.clone().prop_map(|i| format!("class {i},C hot")),
        1 => id.clone().prop_map(|i| format!("style {i} fill:#f96")),
        1 => (0..6usize).prop_map(|n| format!("linkStyle {n},1 color:red")),
    ];
    (
        prop::collection::vec(stmt.clone(), 1..8),
        prop::collection::vec(stmt, 0..3),
    )
        .prop_map(|(top, nested)| {
            let mut s = "flowchart LR\n".to_owned();
            for t in top {
                s.push_str(&format!("    {t}\n"));
            }
            if !nested.is_empty() {
                s.push_str("    subgraph S\n");
                for t in nested {
                    s.push_str(&format!("        {t}\n"));
                }
                s.push_str("    end\n");
            }
            s
        })
}

fn edge_key(c: &Flowchart, i: usize) -> (String, String, Option<String>, Vec<String>) {
    let e = &c.edges[i];
    let styles = e.styles.iter().map(|s| s.value.clone()).collect();
    (
        e.from.clone(),
        e.to.clone(),
        e.label.as_ref().map(|l| l.text.clone()),
        styles,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Deleting keeps every other node (with its label, classes, styles, and subgraph) and every
    /// other edge (in order, with its label and styles).
    #[test]
    fn deleting_keeps_everything_else(
        src in document(),
        nodes in prop::collection::vec(prop::sample::select(vec!["A", "B", "C", "D", "E"]), 0..3),
        edges in prop::collection::vec(0..12usize, 0..3),
    ) {
        let before = chart(&src);
        let out = delete(&src, &nodes, &edges).map_or(src.clone(), |e| e.apply(&src));
        prop_assert!(errors(&out) <= errors(&src), "{}", out);
        let after = chart(&out);

        let dead_ranges: Vec<_> = edges.iter().filter_map(|&i| before.edges.get(i)).map(|e| e.range).collect();
        let kept_edges: Vec<_> = (0..before.edges.len())
            .filter(|&i| {
                let e = &before.edges[i];
                !dead_ranges.contains(&e.range)
                    && !nodes.contains(&e.from.as_str())
                    && !nodes.contains(&e.to.as_str())
            })
            .map(|i| edge_key(&before, i))
            .collect();
        let after_edges: Vec<_> = (0..after.edges.len()).map(|i| edge_key(&after, i)).collect();
        prop_assert_eq!(after_edges, kept_edges, "{}\n->\n{}", src, out);

        let kept: Vec<_> = before.nodes.iter().filter(|n| !nodes.contains(&n.id.as_str())).collect();
        let got: Vec<_> = after.nodes.iter().collect();
        let ids = |v: &[&flowchart::Node]| v.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        prop_assert_eq!(ids(&got), ids(&kept), "node order\n{}\n->\n{}", src, out);
        for (a, b) in got.iter().zip(&kept) {
            prop_assert_eq!(&a.id, &b.id);
            prop_assert_eq!(&a.label, &b.label, "{}\n->\n{}", src, out);
            prop_assert_eq!(&a.classes, &b.classes);
            prop_assert_eq!(&a.styles, &b.styles);
            prop_assert_eq!(
                after.parent(&a.id).map(|s| &s.id),
                before.parent(&b.id).map(|s| &s.id),
                "{}\n->\n{}", src, out
            );
        }
    }
}
