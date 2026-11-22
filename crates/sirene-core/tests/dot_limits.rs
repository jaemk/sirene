//! DOT-6: bounded edge products and copied attributes and ports.

use sirene_core::dot;

fn side(prefix: &str, n: usize) -> String {
    (0..n)
        .map(|i| format!("{prefix}{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn limits(src: &str) -> (sirene_core::dot::Lowered, Vec<String>) {
    let l = dot::parse(src).lower();
    let messages = l
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("leaving out the rest"))
        .map(|d| d.message.clone())
        .collect();
    (l, messages)
}

#[test]
fn edge_product_is_truncated_once_and_later_nodes_are_read() {
    let a = side("a", 225);
    let b = side("b", 225);
    let c = side("c", 225);
    let src = format!("digraph {{ {{ {a} }} -> {{ {b} }} -> {{ {c} }}; x -> y; later; }}");
    let (out, warnings) = limits(&src);
    assert_eq!(out.flowchart.edges.len(), 100_000);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("100000 pending edges"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "later"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "y"));
    assert_eq!(out.flowchart.edges.first().unwrap().from, "a0");
    assert_eq!(out.flowchart.edges.first().unwrap().to, "b0");
}

#[test]
fn copied_default_and_own_attributes_have_a_shared_budget() {
    let text = "x".repeat(1_000_000);
    let targets = side("b", 40);
    let src = format!(
        "digraph {{ edge [label=\"{text}\"]; a -> {{ {targets} }} [headlabel=\"{text}\"]; x -> y; later; }}"
    );
    let (out, warnings) = limits(&src);
    assert!(!out.flowchart.edges.is_empty() && out.flowchart.edges.len() < 40);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("copies and expanded labels exceed 64 MiB"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "later"));
}

#[test]
fn copied_ports_are_budgeted() {
    let port = "p".repeat(1_000_000);
    let targets = side("b", 70);
    let src = format!("digraph {{ a:\"{port}\" -> {{ {targets} }}; x -> y; }}");
    let (out, warnings) = limits(&src);
    assert!(!out.flowchart.edges.is_empty() && out.flowchart.edges.len() < 70);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("copies and expanded labels exceed 64 MiB"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "y"));
}

#[test]
fn strict_directed_and_undirected_duplicates_still_merge() {
    for src in [
        "strict digraph { a -> b [label=one]; a -> b [label=two] }",
        "strict graph { a -- b [label=one]; b -- a [label=two] }",
    ] {
        let (out, warnings) = limits(src);
        assert!(warnings.is_empty());
        assert_eq!(out.flowchart.edges.len(), 1);
        assert_eq!(out.flowchart.edges[0].label.as_ref().unwrap().text, "two");
    }
}

#[test]
fn copied_endpoint_ids_are_budgeted() {
    let id = "a".repeat(1_000_000);
    let targets = side("b", 70);
    let src = format!("strict digraph {{ \"{id}\" -> {{ {targets} }}; x -> y; later; }}");
    let (out, warnings) = limits(&src);
    assert!(!out.flowchart.edges.is_empty() && out.flowchart.edges.len() < 70);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("copies and expanded labels exceed 64 MiB"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "later"));
}

#[test]
fn expanded_edge_labels_are_budgeted_before_allocation() {
    let graph = "g".repeat(10_000);
    let tail = "a".repeat(10_000);
    let label = r"\G\T\H\E".repeat(200);
    let targets = (0..12)
        .map(|i| format!("\"{}b{i}\"", "h".repeat(10_000)))
        .collect::<Vec<_>>()
        .join(" ");
    let src = format!(
        "digraph \"{graph}\" {{ \"{tail}\" -> {{ {targets} }} [label=\"{label}\"]; later; }}"
    );
    let (out, warnings) = limits(&src);
    assert!(!out.flowchart.edges.is_empty() && out.flowchart.edges.len() < 12);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("expanded labels exceed 64 MiB"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "later"));
}

#[test]
fn strict_undirected_reversed_edge_accounts_for_retained_orientation() {
    let long = "a".repeat(10_000);
    let label = r"\T".repeat(7_000);
    let src =
        format!("strict graph {{ \"{long}\" -- b; b -- \"{long}\" [label=\"{label}\"]; later; }}");
    let (out, warnings) = limits(&src);
    assert_eq!(out.flowchart.edges.len(), 1);
    assert!(out.flowchart.edges[0].label.is_none());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("expanded labels exceed 64 MiB"));
    assert!(out.flowchart.nodes.iter().any(|n| n.id == "later"));
}
