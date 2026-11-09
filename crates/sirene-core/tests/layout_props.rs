//! Property tests of layout invariants on random compound graphs (LAYOUT-*).

use proptest::prelude::*;
use sirene_core::ascii;
use sirene_core::flowchart::Direction;
use sirene_core::layout::{
    ClusterSpec, Config, EdgeSpec, Graph, Item, Layout, NodeSpec, Point, Rect, Size, Spacing,
    layout,
};

const PIXELS: Config = Config {
    vertical: Spacing {
        node_gap: 20,
        edge_gap: 6,
        rank_gap: 20,
        track_gap: 6,
        cluster_cross_pad: 12,
        cluster_main_pad: 8,
        port_inset: 6,
        loop_size: 14,
        loop_inset: 6,
    },
    horizontal: Spacing {
        node_gap: 16,
        edge_gap: 6,
        rank_gap: 30,
        track_gap: 6,
        cluster_cross_pad: 8,
        cluster_main_pad: 12,
        port_inset: 6,
        loop_size: 14,
        loop_inset: 6,
    },
};

fn direction() -> impl Strategy<Value = Direction> {
    prop::sample::select(vec![
        Direction::TopBottom,
        Direction::BottomTop,
        Direction::LeftRight,
        Direction::RightLeft,
    ])
}

fn size() -> impl Strategy<Value = Size> {
    (3..14i32, 3..6i32).prop_map(|(w, h)| Size::new(w, h))
}

/// Graphs whose cluster `j` has a parent below `j`, so nesting is acyclic.
fn graph() -> impl Strategy<Value = Graph> {
    (0..5usize, 0..9usize).prop_flat_map(|(k, n)| {
        let clusters = (0..k)
            .map(|j| {
                (
                    prop::option::weighted(0.5, 0..j.max(1)),
                    prop::option::of(direction()),
                    prop::option::of((1..16i32, 1..3i32)),
                )
                    .prop_map(move |(parent, direction, title)| ClusterSpec {
                        parent: parent.filter(|&p| p < j),
                        direction,
                        title: title.map_or(Size::default(), |(w, h)| Size::new(w, h)),
                    })
            })
            .collect::<Vec<_>>();
        let nodes = prop::collection::vec(
            (size(), prop::option::weighted(0.6, 0..k.max(1))).prop_map(move |(size, parent)| {
                NodeSpec {
                    size,
                    parent: parent.filter(|&p| p < k),
                }
            }),
            n,
        );
        let item = move || -> BoxedStrategy<Item> {
            match (n, k) {
                (0, _) => (0..k).prop_map(Item::Cluster).boxed(),
                (_, 0) => (0..n).prop_map(Item::Node).boxed(),
                _ => prop_oneof![
                    4 => (0..n).prop_map(Item::Node),
                    1 => (0..k).prop_map(Item::Cluster),
                ]
                .boxed(),
            }
        };
        let edges = if n + k == 0 {
            Just(Vec::new()).boxed()
        } else {
            prop::collection::vec(
                (
                    item(),
                    item(),
                    0..4u32,
                    prop::option::weighted(0.3, (1..8i32, 1..3i32)),
                )
                    .prop_map(|(from, to, min_len, label)| EdgeSpec {
                        from,
                        to,
                        min_len,
                        label: label.map(|(w, h)| Size::new(w, h)),
                    }),
                0..12,
            )
            .boxed()
        };
        (direction(), clusters, nodes, edges).prop_map(|(direction, clusters, nodes, edges)| {
            Graph {
                direction,
                nodes,
                clusters,
                edges,
                notes: Vec::new(),
            }
        })
    })
}

fn ancestors(g: &Graph, item: Item) -> Vec<usize> {
    let mut out = Vec::new();
    let mut current = match item {
        Item::Node(n) => g.nodes[n].parent,
        Item::Cluster(c) => g.clusters[c].parent,
    };
    while let Some(c) = current {
        out.push(c);
        current = g.clusters[c].parent;
    }
    out
}

fn rect(l: &Layout, item: Item) -> Rect {
    match item {
        Item::Node(n) => l.nodes[n],
        Item::Cluster(c) => l.clusters[c],
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x <= b.right() && b.x <= a.right() && a.y <= b.bottom() && b.y <= a.bottom()
}

/// Strictly inside `outer`'s border.
fn inside(inner: Rect, outer: Rect) -> bool {
    inner.x > outer.x
        && inner.right() < outer.right()
        && inner.y > outer.y
        && inner.bottom() < outer.bottom()
}

fn cells(points: &[Point]) -> Vec<Point> {
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        let (mut at, to) = (w[0], w[1]);
        while at != to {
            at.x += (to.x - at.x).signum();
            at.y += (to.y - at.y).signum();
            out.push(at);
        }
    }
    out
}

fn check(g: &Graph, l: &Layout) -> Result<(), TestCaseError> {
    let bounds = Rect::new(0, 0, l.size.w, l.size.h);
    let items: Vec<Item> = (0..g.nodes.len())
        .map(Item::Node)
        .chain((0..g.clusters.len()).map(Item::Cluster))
        .collect();
    for (i, &a) in items.iter().enumerate() {
        let ra = rect(l, a);
        prop_assert!(
            ra.x >= 0 && ra.y >= 0 && ra.right() < l.size.w && ra.bottom() < l.size.h,
            "{:?} {:?} outside {:?}",
            a,
            ra,
            l.size
        );
        if let Item::Node(n) = a {
            prop_assert!(ra.w >= g.nodes[n].size.w && ra.h >= g.nodes[n].size.h);
        }
        for &b in &items[i + 1..] {
            let rb = rect(l, b);
            let nested = |x: Item, y: Item| match y {
                Item::Cluster(c) => ancestors(g, x).contains(&c),
                Item::Node(_) => false,
            };
            if nested(a, b) {
                prop_assert!(
                    inside(ra, rb),
                    "{:?} {:?} not inside {:?} {:?}",
                    a,
                    ra,
                    b,
                    rb
                );
            } else if nested(b, a) {
                prop_assert!(
                    inside(rb, ra),
                    "{:?} {:?} not inside {:?} {:?}",
                    b,
                    rb,
                    a,
                    ra
                );
            } else {
                prop_assert!(
                    !overlaps(ra, rb),
                    "{:?} {:?} overlaps {:?} {:?}",
                    a,
                    ra,
                    b,
                    rb
                );
            }
        }
    }
    for (c, spec) in g.clusters.iter().enumerate() {
        let title = l.titles[c];
        prop_assert_eq!(title.is_some(), spec.title.w > 0, "cluster {} title", c);
        let Some(t) = title else { continue };
        let r = l.clusters[c];
        prop_assert_eq!((t.w, t.h), (spec.title.w, spec.title.h));
        prop_assert_eq!(t.y, r.y, "cluster {} title starts on the top border", c);
        prop_assert!(
            t.x > r.x && t.right() < r.right() && t.bottom() < r.bottom(),
            "cluster {} title {:?} outside {:?}",
            c,
            t,
            r
        );
        for (n, &nr) in l.nodes.iter().enumerate() {
            prop_assert!(!overlaps(t, nr), "cluster {} title on node {}", c, n);
        }
        for (d, &cr) in l.clusters.iter().enumerate() {
            if d != c && ancestors(g, Item::Cluster(d)).contains(&c) {
                prop_assert!(!overlaps(t, cr), "cluster {} title on cluster {}", c, d);
            }
        }
        for (e, route) in l.edges.iter().enumerate() {
            for p in route.iter().flat_map(|r| cells(&r.points)) {
                prop_assert!(!t.contains(p), "edge {} crosses cluster {} title", e, c);
            }
        }
    }
    let labels: Vec<Rect> = l.edges.iter().flatten().filter_map(|r| r.label).collect();
    for (e, (spec, route)) in g.edges.iter().zip(&l.edges).enumerate() {
        let related = |x: Item, y: Item| match y {
            Item::Cluster(c) => x != y && ancestors(g, x).contains(&c),
            Item::Node(_) => false,
        };
        let unroutable = related(spec.from, spec.to) || related(spec.to, spec.from);
        prop_assert_eq!(route.is_none(), unroutable, "edge {} {:?}", e, spec);
        let Some(route) = route else { continue };
        let (from, to) = (rect(l, spec.from), rect(l, spec.to));
        let pts = &route.points;
        prop_assert!(pts.len() >= 2, "edge {} has {:?}", e, pts);
        prop_assert!(
            from.on_border(pts[0]),
            "edge {} starts at {:?} off {:?}",
            e,
            pts[0],
            from
        );
        prop_assert!(
            to.on_border(pts[pts.len() - 1]),
            "edge {} ends off {:?}: {:?}",
            e,
            to,
            pts
        );
        for w in pts.windows(2) {
            prop_assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "edge {} diagonal {:?}",
                e,
                pts
            );
        }
        prop_assert_eq!(
            route.label.is_some(),
            spec.label.is_some(),
            "edge {} label",
            e
        );
        if let (Some(r), Some(s)) = (route.label, spec.label) {
            prop_assert_eq!((r.w, r.h), (s.w, s.h));
            prop_assert!(r.x >= 0 && r.y >= 0 && r.right() < l.size.w && r.bottom() < l.size.h);
            for &it in &items {
                if let Item::Node(_) = it {
                    prop_assert!(!overlaps(r, rect(l, it)), "label of {} on {:?}", e, it);
                }
            }
        }
        for c in cells(pts) {
            prop_assert!(
                bounds.contains(c),
                "edge {} leaves the diagram at {:?}",
                e,
                c
            );
            for (n, &r) in l.nodes.iter().enumerate() {
                let endpoint = spec.from == Item::Node(n) || spec.to == Item::Node(n);
                if !endpoint {
                    prop_assert!(!r.contains(c), "edge {} crosses node {} at {:?}", e, n, c);
                }
            }
            for &r in &labels {
                if Some(r) != route.label {
                    prop_assert!(!r.contains(c), "edge {} crosses a label at {:?}", e, c);
                }
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn cell_layouts(g in graph()) {
        let l = layout(&g, &ascii::CONFIG);
        check(&g, &l)?;
        prop_assert_eq!(&l, &layout(&g, &ascii::CONFIG), "layout is deterministic");
    }

    #[test]
    fn pixel_layouts(g in graph()) {
        check(&g, &layout(&g, &PIXELS))?;
    }
}
