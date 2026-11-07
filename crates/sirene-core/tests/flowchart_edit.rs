//! Tests of source edits (WYSIWY-8).

use proptest::prelude::*;
use sirene_core::Severity;
use sirene_core::flowchart::edit::{
    TextEdit, add_edge, add_node, delete, label_source, set_edge_label, set_node_label,
};
use sirene_core::flowchart::{self, Flowchart};

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
