//! Property tests for Graphviz DOT: the tree holds any source exactly (DOT-1), and any source
//! lowers to a well-formed graph model that draws (DOT-2).

mod common;

use common::{check_model, check_render};
use proptest::prelude::*;
use sirene_core::{Severity, TextSize, dot};

/// A graph header and a body of pieces biased toward DOT syntax, so the parser's and the
/// lowering's branches get exercised.
fn dot_ish() -> impl Strategy<Value = String> {
    let headers = prop::sample::select(vec![
        "digraph {",
        "graph {",
        "strict digraph G {",
        "strict graph {",
        "",
    ]);
    #[rustfmt::skip]
    let pieces = prop::sample::select(vec![
        "digraph", "graph", "strict ", "subgraph ", "cluster_a", "cluster_b", "cluster", "node",
        "edge", " ", " ", "\n", "\r\n", "\t", "{", "}", "[", "]", "=", ";", ",", ":", "+", "->",
        "--", "a", "b", "c", "1", "-1.5", ".", "-", "\"", "\\", "\\\"", "\"x y\"", "\"\"", "<",
        ">", "<b>", "</b>", "<br/>", "<<i>x</i>>", "//", "/*", "*/", "#", "label", "headlabel",
        "shape", "record", "box", "point", "\"{a|{b|c}|<p> d}\"", "style", "filled", "invis",
        "dashed", "bold", "color", "fillcolor", "red", "nope", "\"0.5 1 nan\"", "rank", "same",
        "minlen", "nan", "inf", "0", "99", "penwidth", "dir", "back", "both", "arrowhead",
        "arrowtail", "odiamond", "rankdir", "LR", "bgcolor", "|", "é", "🐟", "\"\\N\\l\"",
        "&amp;",
    ]);
    (headers, prop::collection::vec(pieces, 0..64)).prop_map(|(h, v)| format!("{h}{}", v.concat()))
}

fn id() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "a",
        "b",
        "c",
        "d",
        "1",
        "\"\"",
        "\"x\\ny\"",
        "cluster_a",
        "cluster_b",
        "é",
    ])
}

/// `key=value`, with values that fit the key or not.
fn attr() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    let keys = prop::sample::select(vec![
        "label", "headlabel", "taillabel", "shape", "style", "color", "fillcolor", "pencolor",
        "bgcolor", "fontcolor", "penwidth", "dir", "arrowhead", "arrowtail", "minlen", "rankdir",
        "rank", "width", "constraint", "compound", "lhead", "ltail", "ordering", "group",
    ]);
    #[rustfmt::skip]
    let values = prop::sample::select(vec![
        "x", "\"\"", "\"two\\llines\\l\"", "\"\\N \\G \\E \\T \\H\"", "<<b>x</b><br/>y>",
        "\"{a|{b|c}|<p> d}\"", "\"{{{{\"", "\"}|}\"", "box", "record", "Mrecord", "point",
        "circle", "plaintext", "nope", "\"filled,rounded\"", "\"dashed,bold\"", "invis", "dotted",
        "red", "\"#12345678\"", "\"0.5 1 1\"", "\"nan nan nan\"", "\"/x11/navy\"", "transparent",
        "0", "3", "99", "-1", "nan", "inf", "back", "both", "none", "forward", "odiamond", "crow",
        "LR", "BT", "same", "min", "max", "source", "sink", "false", "no", "true", "cluster_a", "cluster_b", "out", "in",
    ]);
    (keys, values).prop_map(|(k, v)| format!("{k}={v}"))
}

fn attrs() -> impl Strategy<Value = String> {
    prop::collection::vec(attr(), 0..4).prop_map(|a| {
        if a.is_empty() {
            String::new()
        } else {
            format!(" [{}]", a.join(", "))
        }
    })
}

fn statement(directed: bool) -> impl Strategy<Value = String> {
    let op = if directed { " -> " } else { " -- " };
    let simple = prop_oneof![
        (prop::collection::vec(id(), 1..4), attrs())
            .prop_map(move |(ids, a)| format!("{}{a}", ids.join(op))),
        (id(), prop::collection::vec(id(), 0..3), attrs())
            .prop_map(move |(from, to, a)| format!("{from}{op}{{ {} }}{a}", to.join(" "))),
        attr(),
        (prop::sample::select(vec!["node", "edge", "graph"]), attrs())
            .prop_filter("an attribute statement needs a list", |(_, a)| !a
                .is_empty())
            .prop_map(|(kw, a)| format!("{kw}{a}")),
    ];
    simple.prop_recursive(3, 24, 4, |inner| {
        (
            prop::sample::select(vec![
                "subgraph cluster_a ",
                "subgraph cluster_b ",
                "subgraph CLUSTER ",
                "subgraph s ",
                "",
            ]),
            prop::collection::vec(inner, 0..4),
        )
            .prop_map(|(head, body)| format!("{head}{{ {} }}", body.join("; ")))
    })
}

/// A well-formed graph.
fn document() -> impl Strategy<Value = String> {
    (any::<bool>(), any::<bool>()).prop_flat_map(|(strict, directed)| {
        prop::collection::vec(statement(directed), 0..10).prop_map(move |stmts| {
            let strict = if strict { "strict " } else { "" };
            let kw = if directed { "digraph" } else { "graph" };
            format!("{strict}{kw} G {{\n  {}\n}}\n", stmts.join("\n  "))
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn generated_graphs_lower_and_draw(src in document()) {
        let parse = dot::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        prop_assert!(parse.diagnostics().is_empty(), "{:?}\n{}", parse.diagnostics(), src);
        let lowered = parse.lower();
        prop_assert!(
            lowered.diagnostics.iter().all(|d| d.severity == Severity::Warning),
            "{:?}\n{}",
            lowered.diagnostics,
            src
        );
        check_model(&lowered.flowchart)?;
        check_render(&lowered.flowchart)?;
    }

    #[test]
    fn round_trips_arbitrary_text(src in any::<String>()) {
        let parse = dot::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src);
        check_model(&parse.lower().flowchart)?;
    }

    #[test]
    fn round_trips_dot_like_text(src in dot_ish()) {
        let parse = dot::parse(&src);
        prop_assert_eq!(parse.syntax().to_string(), src.clone());
        let lowered = parse.lower();
        for d in &lowered.diagnostics {
            prop_assert!(d.range.end() <= TextSize::of(src.as_str()));
        }
        check_model(&lowered.flowchart)?;
        check_render(&lowered.flowchart)?;
    }
}
