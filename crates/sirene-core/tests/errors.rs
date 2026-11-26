//! Error contracts at the public rendering boundaries (ERRORH-1..3).

use std::error::Error as _;

use sirene_core::ascii::{self, Options};
use sirene_core::diagram::Diagram;
use sirene_core::flowchart::{self, Measure, Node};
use sirene_core::layout::{self, EdgeSpec, Graph, Item, Size};
use sirene_core::scene::{self, Approx, Theme};

struct Cells;
impl Measure for Cells {
    fn node(&self, _: &Node) -> Size {
        Size::new(3, 3)
    }
    fn edge_label(&self, _: &flowchart::Label) -> Option<Size> {
        None
    }
    fn subgraph_title(&self, _: &flowchart::Subgraph) -> Size {
        Size::default()
    }
    fn note(&self, _: &flowchart::Label) -> Size {
        Size::default()
    }
}

#[test]
fn invalid_flowchart_returns_errors_from_every_rendering_boundary() {
    let mut chart = flowchart::parse("flowchart\nA --> B\n").lower().flowchart;
    chart.edges[0].to = "missing".to_owned();
    assert!(
        chart
            .layout_graph(&Cells)
            .unwrap_err()
            .report()
            .contains("missing")
    );
    assert!(
        ascii::render(&chart, &Options::UNICODE)
            .unwrap_err()
            .report()
            .contains("missing")
    );
    assert!(
        scene::flowchart(&chart, &Approx, &Theme::LIGHT)
            .unwrap_err()
            .report()
            .contains("missing")
    );
    let diagram = Diagram::Graph(chart);
    assert!(diagram.ascii(&Options::UNICODE).is_err());
    assert!(diagram.scene(&Approx, &Theme::LIGHT).is_err());
}

#[test]
fn collapsed_subgraph_does_not_hide_invalid_endpoints() {
    let mut chart = flowchart::parse(
        "flowchart\nsubgraph group\n A\nend\n A --> B\n group@{ view: collapsed }\n",
    )
    .lower()
    .flowchart;
    assert!(chart.collapsed().is_some());
    chart.subgraphs[0].members.push("missing".into());
    chart.edges[0].from = "missing".into();
    for error in [
        ascii::render(&chart, &Options::UNICODE).unwrap_err(),
        scene::flowchart(&chart, &Approx, &Theme::LIGHT).unwrap_err(),
    ] {
        assert!(error.report().contains("unknown edge endpoint `missing`"));
    }
}

#[test]
fn invalid_layout_endpoint_is_an_error() {
    let graph = Graph {
        edges: vec![EdgeSpec {
            from: Item::Node(0),
            to: Item::Node(1),
            min_len: 1,
            label: None,
            constraint: true,
            ports: [None; 2],
            anchors: [None; 2],
        }],
        ..Default::default()
    };
    assert!(
        layout::layout(&graph, &ascii::CONFIG)
            .unwrap_err()
            .report()
            .contains("edge 0")
    );
}

#[test]
fn boxed_causes_remain_inspectable() {
    let original = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access refused");
    let error = sirene_core::Error::with_boxed_source("save document", Box::new(original));
    let cause = error
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(error.report(), "save document: access refused");
}

#[cfg(feature = "png")]
#[test]
fn png_parser_error_keeps_its_original_cause() {
    let scene = scene::Scene {
        width: 0.0,
        height: 10.0,
        background: Theme::LIGHT.background,
        elements: Vec::new(),
    };
    let error = scene::png::write(&scene, Default::default()).unwrap_err();
    assert!(error.source().unwrap().is::<resvg::usvg::Error>());
    assert!(error.report().starts_with("cannot draw the diagram: "));
}
