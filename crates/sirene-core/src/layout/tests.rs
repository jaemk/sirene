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

#[test]
fn bad_cluster_parents_are_top_level() {
    let cluster = |parent| ClusterSpec {
        parent: Some(parent),
        direction: None,
        title: Size::default(),
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
fn empty_graph() {
    let l = layout(&Graph::default(), &CONFIG);
    assert_eq!(l, Layout::default());
}
