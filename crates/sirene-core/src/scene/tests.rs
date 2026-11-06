use super::*;
use crate::flowchart::{self, Flowchart, Shape};

fn chart(src: &str) -> Flowchart {
    flowchart::parse(src).lower().flowchart
}

fn draw(src: &str) -> Scene {
    flowchart(&chart(src), &Approx, &Theme::LIGHT)
}

fn owned(s: &Scene, owner: Owner) -> Vec<&Primitive> {
    s.elements
        .iter()
        .filter(|e| e.owner == owner)
        .map(|e| &e.primitive)
        .collect()
}

fn texts(s: &Scene) -> Vec<&str> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            Primitive::Path { .. } => None,
        })
        .collect()
}

/// Distance from `p` to the closest side of `polygon`.
fn distance_to_outline(p: Pt, polygon: &[Pt]) -> f32 {
    let n = polygon.len();
    (0..n)
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            let t = if len2 == 0.0 {
                0.0
            } else {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
            };
            (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
        })
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn approx_metrics() {
    assert_eq!(Approx.width("abc", 10.0), 18.0);
    assert_eq!(Approx.width("日本", 10.0), 24.0);
}

#[test]
fn empty_diagram() {
    let s = draw("flowchart\n");
    assert!(s.elements.is_empty());
    assert_eq!((s.width, s.height), (32.0, 32.0));
    assert_eq!(s.background, Theme::LIGHT.background);
    let s = flowchart(&chart("flowchart\n"), &Approx, &Theme::DARK);
    assert_eq!(s.background, Theme::DARK.background);
    assert_eq!(Theme::default(), Theme::LIGHT);
}

#[test]
fn title_is_centered_above() {
    let s = draw("---\ntitle: A long diagram title\n---\nflowchart\nA\n");
    let Some(Primitive::Text { text, center, .. }) = owned(&s, Owner::Diagram).first().copied()
    else {
        panic!("no title")
    };
    assert_eq!(text, "A long diagram title");
    assert_eq!(center.x, s.width / 2.0);
    // The title is wider than the node, which is centered under it.
    let Primitive::Path { segments, .. } = owned(&s, Owner::Node(0))[0] else {
        panic!("no node")
    };
    let Segment::Move(p) = segments[0] else {
        panic!("path starts with a move")
    };
    assert!(p.x > 16.0 && p.y > center.y, "{p:?}");
    assert!(
        draw("---\ntitle: \"  \"\n---\nflowchart\nA\n")
            .elements
            .len()
            == 2
    );
}

/// Every edge's arrow tip lands on its target's outline, for every shape (DISPLA-7).
#[test]
fn edges_end_on_every_outline() {
    for dir in ["TD", "LR", "BT", "RL"] {
        for &shape in Shape::ALL.iter().filter(|&&s| s != Shape::Text) {
            let mut c = chart(&format!(
                "flowchart {dir}\nX --> A\nY --> A\nZ --> A\nA --> W\n"
            ));
            c.nodes[1].shape = shape;
            let s = flowchart(&c, &Approx, &Theme::LIGHT);
            let node = owned(&s, Owner::Node(1));
            let Some(Primitive::Path { segments, .. }) = node.first() else {
                panic!("{shape:?} draws nothing")
            };
            let outline = shapes::flatten(segments);
            for e in 0..3 {
                let Primitive::Path { segments, .. } = owned(&s, Owner::Edge(e))[0] else {
                    panic!("no arrowhead")
                };
                let Segment::Move(tip) = segments[0] else {
                    panic!("arrowhead starts with a move")
                };
                let d = distance_to_outline(tip, &outline);
                assert!(d < 1.0, "{dir} {shape:?} edge {e}: tip {tip:?} is {d} off");
            }
        }
    }
}

#[test]
fn shapes_without_labels() {
    for shape in [
        Shape::SmallCircle,
        Shape::FilledCircle,
        Shape::FramedCircle,
        Shape::Fork,
    ] {
        let mut c = chart("flowchart\nA[hidden]\n");
        c.nodes[0].shape = shape;
        let s = flowchart(&c, &Approx, &Theme::LIGHT);
        assert!(texts(&s).is_empty(), "{shape:?}");
    }
    let mut c = chart("flowchart\nA[shown]\n");
    c.nodes[0].shape = Shape::Text;
    let s = flowchart(&c, &Approx, &Theme::LIGHT);
    assert_eq!(texts(&s), ["shown"]);
}

#[test]
fn arrowheads() {
    let s = draw("flowchart LR\nA <--> B\nC o--o D\nE x--x F\nG --- H\n");
    let count = |e: usize, filled: bool| {
        owned(&s, Owner::Edge(e))
            .iter()
            .filter(|p| matches!(p, Primitive::Path { fill: Some(_), .. }) == filled)
            .count()
    };
    // Two filled heads plus the line.
    assert_eq!((count(0, true), count(0, false)), (2, 1));
    assert_eq!((count(1, true), count(1, false)), (2, 1));
    // Two stroked arms per cross plus the line.
    assert_eq!((count(2, true), count(2, false)), (0, 5));
    assert_eq!((count(3, true), count(3, false)), (0, 1));
}

#[test]
fn stroke_kinds() {
    let s = draw("flowchart LR\nA --> B\nC ==> D\nE -.-> F\nG ~~~ H\n");
    let line = |e: usize| {
        owned(&s, Owner::Edge(e)).into_iter().find_map(|p| match p {
            Primitive::Path {
                stroke: Some(st), ..
            } => Some(*st),
            _ => None,
        })
    };
    assert_eq!(line(0).map(|s| (s.width, s.dash)), Some((1.5, None)));
    assert_eq!(line(1).map(|s| (s.width, s.dash)), Some((3.5, None)));
    assert_eq!(
        line(2).map(|s| (s.width, s.dash)),
        Some((2.0, Some((3.0, 3.0))))
    );
    assert!(owned(&s, Owner::Edge(3)).is_empty());
}

#[test]
fn edge_labels() {
    let s = draw("flowchart LR\nA -->|two<br>lines| B\nC ~~~|hidden| D\n");
    let t = texts(&s);
    assert!(t.contains(&"two") && t.contains(&"lines"), "{t:?}");
    assert!(!t.contains(&"hidden"));
    let fills: Vec<_> = owned(&s, Owner::Edge(0))
        .into_iter()
        .filter_map(|p| match p {
            Primitive::Path {
                fill, stroke: None, ..
            } => *fill,
            _ => None,
        })
        .collect();
    assert!(fills.contains(&Theme::LIGHT.edge_label_fill));
}

#[test]
fn styles_apply() {
    let s = draw(
        "flowchart LR\n\
         classDef default fill:#eee\n\
         classDef hot stroke:#f00\n\
         A:::hot -->|go| B\n\
         style B fill:#0f0,stroke-width:3px,color:#00f,stroke-dasharray:4 2\n\
         linkStyle default stroke:#123456\n\
         linkStyle 0 color:#654321,stroke-width:4px\n\
         subgraph S [Box]\nC\nend\n\
         style S fill:#abcdef,color:#fedcba\n",
    );
    let body = |n: usize| match owned(&s, Owner::Node(n))[0] {
        Primitive::Path { fill, stroke, .. } => (*fill, *stroke),
        p => panic!("{p:?}"),
    };
    let (fill, stroke) = body(0);
    assert_eq!(fill, parse_color("#eee"));
    assert_eq!(stroke.map(|s| s.color), parse_color("#f00"));
    let (fill, stroke) = body(1);
    assert_eq!(fill, parse_color("#0f0"));
    let stroke = stroke.unwrap();
    assert_eq!((stroke.width, stroke.dash), (3.0, Some((4.0, 2.0))));
    assert_eq!(stroke.color, Theme::LIGHT.node_stroke);
    let color_of = |owner: Owner, wanted: &str| {
        owned(&s, owner).into_iter().find_map(|p| match p {
            Primitive::Text { text, color, .. } if text == wanted => Some(*color),
            _ => None,
        })
    };
    assert_eq!(color_of(Owner::Node(1), "B"), parse_color("#00f"));
    assert_eq!(color_of(Owner::Edge(0), "go"), parse_color("#654321"));
    assert_eq!(color_of(Owner::Cluster(0), "Box"), parse_color("#fedcba"));
    let edge_line = owned(&s, Owner::Edge(0)).into_iter().find_map(|p| match p {
        Primitive::Path {
            fill: None,
            stroke: Some(st),
            ..
        } => Some(*st),
        _ => None,
    });
    assert_eq!(
        edge_line.map(|s| (s.color, s.width)),
        Some((parse_color("#123456").unwrap(), 4.0))
    );
    let Primitive::Path { fill, .. } = owned(&s, Owner::Cluster(0))[0] else {
        panic!("no cluster body")
    };
    assert_eq!(*fill, parse_color("#abcdef"));
}

#[test]
fn nested_clusters_paint_outer_first() {
    let s = draw("flowchart TD\nsubgraph inner\nA\nend\nsubgraph outer\ninner\nend\nA --> outer\n");
    let first = s
        .elements
        .iter()
        .position(|e| e.owner == Owner::Cluster(1))
        .unwrap();
    let second = s
        .elements
        .iter()
        .position(|e| e.owner == Owner::Cluster(0))
        .unwrap();
    assert!(first < second);
}

#[test]
fn edges_to_clusters_end_on_their_border() {
    let s = draw("flowchart TD\nX --> S\nsubgraph S\nA\nend\n");
    let Primitive::Path { segments, .. } = owned(&s, Owner::Cluster(0))[0] else {
        panic!("no cluster")
    };
    let border = shapes::flatten(segments);
    let Primitive::Path { segments, .. } = owned(&s, Owner::Edge(0))[0] else {
        panic!("no edge")
    };
    let Segment::Move(tip) = segments[0] else {
        panic!("arrowhead starts with a move")
    };
    assert!(distance_to_outline(tip, &border) < 0.01, "{tip:?}");
}

#[test]
fn self_loops_draw() {
    let s = draw("flowchart TD\nA -->|again| A\n");
    assert_eq!(owned(&s, Owner::Edge(0)).len(), 4);
}

#[test]
fn flatten_arcs() {
    let p = |x, y| Pt::new(x, y);
    let arc = |radii, large, sweep, to| Segment::Arc {
        radii,
        large,
        sweep,
        to,
    };
    // A zero radius or a zero-length arc is a straight step.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(0.0, 5.0), false, true, p(10.0, 0.0)),
        arc(p(5.0, 5.0), false, true, p(10.0, 0.0)),
    ]);
    assert_eq!(pts, [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.0)]);
    // Radii too small to reach are scaled up, giving a half circle through (5, -5) when
    // sweeping clockwise from (0, 0) to (10, 0).
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(1.0, 1.0), false, true, p(10.0, 0.0)),
    ]);
    assert!(
        pts.iter()
            .any(|q| (q.x - 5.0).abs() < 0.01 && (q.y + 5.0).abs() < 0.01)
    );
    assert_eq!(*pts.last().unwrap(), p(10.0, 0.0));
    // Counterclockwise goes the other way.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(5.0, 5.0), false, false, p(10.0, 0.0)),
    ]);
    assert!(pts.iter().all(|q| q.y >= -0.01));
    // A large arc covers more than half the circle.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(10.0, 10.0), true, true, p(10.0, 0.0)),
    ]);
    assert!(pts.iter().any(|q| q.y < -15.0));
    // Cubic curves are sampled through their end.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        Segment::Cubic {
            c1: p(0.0, 10.0),
            c2: p(10.0, 10.0),
            to: p(10.0, 0.0),
        },
        Segment::Close,
    ]);
    assert_eq!(pts.len(), 9);
    assert_eq!(pts[4], p(5.0, 7.5));
}

#[test]
fn ray_hits() {
    let square = [
        Pt::new(0.0, 0.0),
        Pt::new(10.0, 0.0),
        Pt::new(10.0, 10.0),
        Pt::new(0.0, 10.0),
    ];
    let down = Pt::new(0.0, 1.0);
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(5.0, -5.0), down, 0.0),
        Some(Pt::new(5.0, 0.0))
    );
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(20.0, -5.0), down, 0.0),
        None
    );
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(5.0, -5.0), Pt::new(1.0, 0.0), 0.0),
        None
    );
}

#[test]
fn svg_details() {
    let scene = Scene {
        width: 10.5,
        height: -0.001,
        background: Color::rgb(0x102030).with_alpha(128),
        elements: vec![
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Path {
                    segments: vec![
                        Segment::Move(Pt::new(1.0, 2.0)),
                        Segment::Cubic {
                            c1: Pt::new(1.0, 3.0),
                            c2: Pt::new(2.0, 3.0),
                            to: Pt::new(2.0, 2.0),
                        },
                        Segment::Arc {
                            radii: Pt::new(1.0, 1.0),
                            large: true,
                            sweep: false,
                            to: Pt::new(1.0, 2.0),
                        },
                        Segment::Close,
                    ],
                    fill: None,
                    stroke: Some(Stroke {
                        width: 1.25,
                        color: Color::rgb(0xff0000),
                        dash: Some((2.0, 1.0)),
                    }),
                },
            },
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: "a<b>&\"c\"".into(),
                    center: Pt::new(0.333, 4.0),
                    size: 12.0,
                    color: Color::rgb(0),
                },
            },
        ],
    };
    assert_eq!(
        svg::write(&scene),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10.5\" height=\"0\" \
         viewBox=\"0 0 10.5 0\" font-family=\"trebuchet ms, verdana, arial, sans-serif\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#102030\" fill-opacity=\"0.5\"/>\n\
         <path d=\"M1 2 C1 3 2 3 2 2 A1 1 0 1 0 1 2 Z\" fill=\"none\" stroke=\"#ff0000\" \
         stroke-width=\"1.25\" stroke-dasharray=\"2 1\"/>\n\
         <text x=\"0.33\" y=\"4\" font-size=\"12\" text-anchor=\"middle\" \
         dominant-baseline=\"central\" fill=\"#000000\">a&lt;b&gt;&amp;&quot;c&quot;</text>\n\
         </svg>\n"
    );
}
