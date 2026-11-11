//! Collapsed flowchart subgraphs (FLOWMO-15): the folded chart, its text drawing, and the scene
//! owners that map back to the original chart.

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart::{self, Flowchart, Origin, Shape};
use sirene_core::scene::{self, Approx, Owner, Primitive, Theme};

const SRC: &str = "flowchart LR
  start --> a1
  subgraph one [Ingest]
    a1 --> a2
    subgraph inner
      a3
    end
    a2 --> a3
  end
  subgraph two [Report]
    b1 --> b2
  end
  a3 --> b1
  b2 --> done
  one@{ view: collapsed }
";

fn chart(src: &str) -> Flowchart {
    let lowered = flowchart::parse(src).lower();
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.flowchart
}

#[test]
fn folds_members_into_one_node() {
    let chart = chart(SRC);
    assert!(chart.subgraph("one").unwrap().is_collapsed());
    assert!(!chart.subgraph("two").unwrap().is_collapsed());
    // The model keeps everything.
    assert_eq!(chart.nodes.len(), 7);

    let c = chart.collapsed().expect("one subgraph is collapsed");
    let ids: Vec<&str> = c.chart.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["start", "one", "b1", "b2", "done"]);
    let one = c.chart.node("one").unwrap();
    assert_eq!(one.shape, Shape::Collapsed);
    assert_eq!(one.label.text, "Ingest");
    let one_index = chart.subgraphs.iter().position(|s| s.id == "one").unwrap();
    assert_eq!(c.nodes[1], Origin::Subgraph(one_index));
    assert_eq!(c.nodes[0], Origin::Node(0));

    let subgraphs: Vec<&str> = c.chart.subgraphs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(subgraphs, ["two"]);

    // Internal edges are dropped; crossing ones end at the collapsed node.
    let edges: Vec<(&str, &str)> = c
        .chart
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(
        edges,
        [
            ("start", "one"),
            ("b1", "b2"),
            ("one", "b1"),
            ("b2", "done")
        ]
    );
    assert_eq!(c.edges, [0, 3, 4, 5]);
}

#[test]
fn uncollapsed_charts_are_unchanged() {
    let chart = chart("flowchart LR\n  subgraph one\n    a --> b\n  end\n");
    assert!(chart.collapsed().is_none());
}

#[test]
fn nested_collapse_uses_the_outermost() {
    let chart = chart(
        "flowchart TB\n  subgraph outer\n    subgraph inner\n      a --> b\n    end\n  end\n  \
         c --> a\n  inner@{ view: collapsed }\n  outer@{ view: collapsed }\n",
    );
    let c = chart.collapsed().unwrap();
    let ids: Vec<&str> = c.chart.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["outer", "c"]);
    assert!(c.chart.subgraphs.is_empty());
    let edges: Vec<(&str, &str)> = c
        .chart
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(edges, [("c", "outer")]);
}

#[test]
fn empty_collapsed_subgraph_is_a_node_titled_by_its_id() {
    let chart = chart("flowchart LR\n  a\n  subgraph empty\n  end\n  empty@{ view: collapsed }\n");
    let c = chart.collapsed().unwrap();
    let node = c.chart.node("empty").unwrap();
    assert_eq!(node.label.text, "empty");
    assert_eq!(node.shape, Shape::Collapsed);
}

#[test]
fn text_drawing() {
    let text = ascii::render(
        &chart(SRC),
        &Options {
            charset: Charset::Unicode,
        },
    );
    let expected = "             ╭────────╮    ┌─ Report ───────────┐
┌───────┐    │ Ingest │    │  ┌────┐    ┌────┐  │    ┌──────┐
│ start ├───►│┄┄┄┄┄┄┄┄├────┼─►│ b1 ├───►│ b2 ├──┼───►│ done │
└───────┘    │   ···  │    │  └────┘    └────┘  │    └──────┘
             ╰────────╯    └────────────────────┘";
    assert_eq!(text.trim_end(), expected);
}

#[test]
fn scene_owners_map_to_the_original() {
    let chart = chart(SRC);
    let s = scene::flowchart(&chart, &Approx, &Theme::LIGHT);
    let one = chart.subgraphs.iter().position(|s| s.id == "one").unwrap();
    let texts: Vec<(Owner, &str)> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some((e.owner, text.as_str())),
            _ => None,
        })
        .collect();
    assert!(
        texts.contains(&(Owner::Cluster(one), "Ingest")),
        "{texts:?}"
    );
    let done = chart.nodes.iter().position(|n| n.id == "done").unwrap();
    assert!(texts.contains(&(Owner::Node(done), "done")), "{texts:?}");
    // Hidden nodes draw nothing.
    let a1 = chart.nodes.iter().position(|n| n.id == "a1").unwrap();
    assert!(s.elements.iter().all(|e| e.owner != Owner::Node(a1)));
    // Edges keep their original indices: edge 0 (start --> a1) is drawn, edge 1 is not.
    assert!(s.elements.iter().any(|e| e.owner == Owner::Edge(0)));
    assert!(s.elements.iter().all(|e| e.owner != Owner::Edge(1)));
}
