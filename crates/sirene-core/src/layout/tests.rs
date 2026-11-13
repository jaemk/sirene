use super::*;

const SPACING: Spacing = Spacing {
    node_gap: 3,
    edge_gap: 1,
    rank_gap: 1,
    track_gap: 1,
    cluster_cross_pad: 2,
    cluster_main_pad: 1,
    port_inset: 1,
    loop_size: 2,
    loop_inset: 0,
};

const CONFIG: Config = Config {
    vertical: SPACING,
    horizontal: SPACING,
};

fn node(w: i32, h: i32) -> NodeSpec {
    NodeSpec {
        size: Size::new(w, h),
        parent: None,
    }
}

fn edge(from: usize, to: usize) -> EdgeSpec {
    EdgeSpec {
        from: Item::Node(from),
        to: Item::Node(to),
        min_len: 1,
        label: None,
        constraint: true,
        ports: [None; 2],
        anchors: [None; 2],
    }
}

fn pts(list: &[(i32, i32)]) -> Vec<Point> {
    list.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

#[test]
fn two_nodes_top_bottom() {
    let g = Graph {
        nodes: vec![node(5, 3), node(5, 3)],
        edges: vec![edge(0, 1)],
        ..Graph::default()
    };
    let l = layout(&g, &CONFIG);
    assert_eq!(l.nodes, [Rect::new(0, 0, 5, 3), Rect::new(0, 5, 5, 3)]);
    assert_eq!(l.size, Size::new(5, 8));
    let route = l.edges[0].as_ref().unwrap();
    assert_eq!(route.points, pts(&[(2, 2), (2, 5)]));
    assert_eq!(route.label, None);
}

#[test]
fn directions() {
    let mut g = Graph {
        nodes: vec![node(5, 3), node(5, 3)],
        edges: vec![edge(0, 1)],
        ..Graph::default()
    };
    g.direction = Direction::BottomTop;
    let l = layout(&g, &CONFIG);
    assert_eq!(l.nodes, [Rect::new(0, 5, 5, 3), Rect::new(0, 0, 5, 3)]);
    assert_eq!(l.edges[0].as_ref().unwrap().points, pts(&[(2, 5), (2, 2)]));

    g.direction = Direction::LeftRight;
    let l = layout(&g, &CONFIG);
    assert_eq!(l.nodes, [Rect::new(0, 0, 5, 3), Rect::new(7, 0, 5, 3)]);
    assert_eq!(l.size, Size::new(12, 3));
    assert_eq!(l.edges[0].as_ref().unwrap().points, pts(&[(4, 1), (7, 1)]));

    g.direction = Direction::RightLeft;
    let l = layout(&g, &CONFIG);
    assert_eq!(l.nodes, [Rect::new(7, 0, 5, 3), Rect::new(0, 0, 5, 3)]);
    assert_eq!(l.edges[0].as_ref().unwrap().points, pts(&[(7, 1), (4, 1)]));
}

fn cluster(parent: Option<usize>, direction: Option<Direction>) -> ClusterSpec {
    ClusterSpec {
        parent,
        direction,
        title: Size::default(),
        lane: false,
    }
}

fn member(parent: usize) -> NodeSpec {
    NodeSpec {
        size: Size::new(5, 3),
        parent: Some(parent),
    }
}

/// LAYOUT-3, Mermaid's "Limitation" example: an edge to the subgraph itself keeps its
/// direction, and an edge to a node inside it drops the direction.
#[test]
fn cluster_direction_survives_an_edge_to_the_cluster() {
    // Nodes 0-1 in cluster 0, 2-3 in cluster 1, 4 outside; every cluster is top to bottom.
    let g = |to_inner: bool| Graph {
        direction: Direction::LeftRight,
        nodes: vec![member(0), member(0), member(1), member(1), node(5, 3)],
        clusters: vec![
            cluster(None, Some(Direction::TopBottom)),
            cluster(None, Some(Direction::TopBottom)),
        ],
        edges: vec![
            edge(0, 1),
            edge(2, 3),
            EdgeSpec {
                to: Item::Cluster(0),
                ..edge(4, 0)
            },
            EdgeSpec {
                to: if to_inner {
                    Item::Node(2)
                } else {
                    Item::Cluster(1)
                },
                ..edge(4, 2)
            },
        ],
        ..Graph::default()
    };
    let stacked = |l: &Layout, a: usize, b: usize| l.nodes[a].x == l.nodes[b].x;
    let l = layout(&g(false), &CONFIG);
    assert!(stacked(&l, 0, 1) && stacked(&l, 2, 3), "{:?}", l.nodes);
    let l = layout(&g(true), &CONFIG);
    assert!(stacked(&l, 0, 1), "{:?}", l.nodes);
    assert!(!stacked(&l, 2, 3), "{:?}", l.nodes);
}

/// LAYOUT-3, Mermaid's "Direction in subgraphs" example: clusters linked to as a whole keep
/// theirs.
#[test]
fn nested_cluster_directions_are_kept() {
    // Cluster 0 is top to bottom and holds cluster 1 (right to left) and 2 (bottom to top).
    let g = Graph {
        direction: Direction::LeftRight,
        nodes: vec![
            member(1),
            member(1),
            member(2),
            member(2),
            node(5, 3),
            node(5, 3),
        ],
        clusters: vec![
            cluster(None, Some(Direction::TopBottom)),
            cluster(Some(0), Some(Direction::RightLeft)),
            cluster(Some(0), Some(Direction::BottomTop)),
        ],
        edges: vec![
            edge(0, 1),
            edge(2, 3),
            EdgeSpec {
                to: Item::Cluster(0),
                ..edge(4, 0)
            },
            EdgeSpec {
                from: Item::Cluster(0),
                to: Item::Node(5),
                ..edge(0, 0)
            },
            EdgeSpec {
                from: Item::Cluster(1),
                to: Item::Cluster(2),
                ..edge(0, 0)
            },
        ],
        ..Graph::default()
    };
    let l = layout(&g, &CONFIG);
    let (b1, b2) = (l.clusters[1], l.clusters[2]);
    // TOP is top to bottom: B1 above B2.
    assert!(b1.bottom() <= b2.y, "{b1:?} {b2:?}");
    // B1 runs right to left, B2 bottom to top.
    assert!(l.nodes[1].x < l.nodes[0].x && l.nodes[1].y == l.nodes[0].y);
    assert!(l.nodes[3].y < l.nodes[2].y && l.nodes[3].x == l.nodes[2].x);
}

/// An edge ending at a titled cluster does not run over the title, whatever the cluster's
/// direction.
#[test]
fn edge_to_titled_cluster_clears_the_title() {
    for direction in [
        None,
        Some(Direction::TopBottom),
        Some(Direction::BottomTop),
        Some(Direction::LeftRight),
        Some(Direction::RightLeft),
    ] {
        let mut c = cluster(None, direction);
        c.title = Size::new(1, 1);
        let g = Graph {
            nodes: vec![node(3, 3)],
            clusters: vec![c],
            edges: vec![EdgeSpec {
                to: Item::Cluster(0),
                ..edge(0, 0)
            }],
            ..Graph::default()
        };
        let l = layout(&g, &CONFIG);
        let title = l.titles[0].unwrap();
        let route = l.edges[0].as_ref().unwrap();
        assert!(
            route.points.iter().all(|&p| !title.contains(p)),
            "{direction:?}: {title:?} {:?}",
            route.points
        );
    }
}

/// An edge between a cluster and one of its descendants has no route.
#[test]
fn edge_to_own_descendant_is_unroutable() {
    let g = Graph {
        nodes: vec![member(0), member(0)],
        clusters: vec![cluster(None, None)],
        edges: vec![
            edge(0, 1),
            EdgeSpec {
                from: Item::Cluster(0),
                to: Item::Node(0),
                ..edge(0, 0)
            },
            EdgeSpec {
                from: Item::Node(1),
                to: Item::Cluster(0),
                ..edge(0, 0)
            },
        ],
        ..Graph::default()
    };
    let l = layout(&g, &CONFIG);
    assert!(l.edges[0].is_some());
    assert!(l.edges[1].is_none() && l.edges[2].is_none());
}

#[test]
fn bad_cluster_parents_are_top_level() {
    let cluster = |parent| ClusterSpec {
        parent: Some(parent),
        direction: None,
        title: Size::default(),
        lane: false,
    };
    // Cluster 0 and 1 name each other, 2 names itself, and 3 names a missing cluster.
    let g = Graph {
        nodes: vec![NodeSpec {
            size: Size::new(5, 3),
            parent: Some(1),
        }],
        clusters: vec![cluster(1), cluster(0), cluster(2), cluster(9)],
        ..Graph::default()
    };
    let l = layout(&g, &CONFIG);
    let inside = |a: Rect, b: Rect| a.x > b.x && a.right() < b.right() && a.y > b.y;
    assert!(inside(l.clusters[1], l.clusters[0]));
    assert!(inside(l.nodes[0], l.clusters[1]));
    for c in [2, 3] {
        assert!(!inside(l.clusters[c], l.clusters[0]));
    }
}

#[test]
fn simplify_keeps_turns_back() {
    assert_eq!(
        simplify(pts(&[(0, 5), (0, 3), (0, 0), (0, 4), (4, 4)])),
        pts(&[(0, 5), (0, 0), (0, 4), (4, 4)])
    );
    assert_eq!(
        simplify(pts(&[(0, 0), (2, 0), (2, 0), (5, 0)])),
        pts(&[(0, 0), (5, 0)])
    );
}

/// LAYOUT-4: grouped nodes share a rank; an edge between neighbors there is straight, and
/// others run through the gap below.
#[test]
fn flat_edges() {
    let g = Graph {
        nodes: vec![node(5, 3), node(5, 3), node(5, 3)],
        edges: vec![edge(0, 1), edge(0, 2)],
        same_rank: vec![vec![0, 1, 2]],
        ..Graph::default()
    };
    let l = layout(&g, &CONFIG);
    assert!(l.nodes.iter().all(|r| r.y == 0), "{:?}", l.nodes);
    let (a, b) = (l.nodes[0], l.nodes[1]);
    let straight = &l.edges[0].as_ref().unwrap().points;
    assert_eq!(*straight, pts(&[(a.right(), 1), (b.x, 1)]));
    let below = &l.edges[1].as_ref().unwrap().points;
    assert_eq!(below.len(), 4, "{below:?}");
    assert_eq!(below[0].y, a.bottom());
    assert!(below[1].y > a.bottom());
    // Unconstrained, the same edges leave the nodes in one rank too.
    let mut g = g;
    g.same_rank.clear();
    for e in &mut g.edges {
        e.constraint = false;
    }
    let l = layout(&g, &CONFIG);
    assert!(l.nodes.iter().all(|r| r.y == 0), "{:?}", l.nodes);
}

#[test]
fn empty_graph() {
    let l = layout(&Graph::default(), &CONFIG);
    assert_eq!(l, Layout::default());
}
