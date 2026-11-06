//! Layered layout of compound graphs into integer cells (LAYOUT-*).

mod order;
mod position;
mod rank;
mod route;

use std::collections::HashMap;

use crate::flowchart::Direction;
use position::{Link, Span};
use rank::RankEdge;
use route::Jog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

impl Size {
    pub fn new(w: i32, h: i32) -> Self {
        Self { w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Cells `x..x+w` by `y..y+h` (LAYOUT-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// The last column.
    pub fn right(&self) -> i32 {
        self.x + self.w - 1
    }

    /// The last row.
    pub fn bottom(&self) -> i32 {
        self.y + self.h - 1
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// Whether `p` lies on the outermost row or column.
    pub fn on_border(&self, p: Point) -> bool {
        self.contains(p)
            && (p.x == self.x || p.x == self.right() || p.y == self.y || p.y == self.bottom())
    }

    fn translate(self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// An edge endpoint or layout item: a node or a cluster, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Node(usize),
    Cluster(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSpec {
    pub size: Size,
    /// The cluster directly containing the node.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSpec {
    pub parent: Option<usize>,
    pub direction: Option<Direction>,
    /// Size of the title, whose rect starts on the top border row; empty for no title.
    pub title: Size,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSpec {
    pub from: Item,
    pub to: Item,
    /// Minimum length in ranks; 0 counts as 1.
    pub min_len: u32,
    pub label: Option<Size>,
}

/// Layout input. Endpoint and parent indexes must be in range.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    pub direction: Direction,
    pub nodes: Vec<NodeSpec>,
    pub clusters: Vec<ClusterSpec>,
    pub edges: Vec<EdgeSpec>,
}

/// Spacing for one orientation, in canonical terms: the cross axis runs along a rank and the
/// main axis runs from rank to rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spacing {
    /// Cross-axis gap between items in a rank.
    pub node_gap: i32,
    /// Cross-axis gap between two edge dummies.
    pub edge_gap: i32,
    /// Minimum main-axis gap between ranks. Edges span at least two ranks, so the gap between
    /// adjacent nodes is at least twice this.
    pub rank_gap: i32,
    /// Main-axis distance between tracks in a gap.
    pub track_gap: i32,
    pub cluster_cross_pad: i32,
    pub cluster_main_pad: i32,
    /// Ports keep this distance from a side's corners when the side is wide enough.
    pub port_inset: i32,
    /// How far a self-loop extends from its item.
    pub loop_size: i32,
    /// Distance of a self-loop's ends from the item's upstream and downstream sides.
    pub loop_inset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Spacing for top-bottom and bottom-top containers.
    pub vertical: Spacing,
    /// Spacing for left-right and right-left containers.
    pub horizontal: Spacing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// Orthogonal path from the source's border to the target's border.
    pub points: Vec<Point>,
    pub label: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    pub size: Size,
    pub nodes: Vec<Rect>,
    pub clusters: Vec<Rect>,
    /// Title rect per cluster, `None` when the title is empty.
    pub titles: Vec<Option<Rect>>,
    /// `None` for unroutable edges (LAYOUT-10).
    pub edges: Vec<Option<Route>>,
}

/// Lays out `graph`.
pub fn layout(graph: &Graph, config: &Config) -> Layout {
    Builder::new(graph, config).run()
}

fn horizontal(d: Direction) -> bool {
    matches!(d, Direction::LeftRight | Direction::RightLeft)
}

/// Converts between real and canonical sizes (they differ by a swap for horizontal directions).
fn swap_if(d: Direction, s: Size) -> Size {
    if horizontal(d) {
        Size::new(s.h, s.w)
    } else {
        s
    }
}

/// Maps a canonical cell of an area `h` cells tall into direction `d`.
fn to_real(d: Direction, h: i32, p: Point) -> Point {
    match d {
        Direction::TopBottom => p,
        Direction::BottomTop => Point::new(p.x, h - 1 - p.y),
        Direction::LeftRight => Point::new(p.y, p.x),
        Direction::RightLeft => Point::new(h - 1 - p.y, p.x),
    }
}

fn rect_to_real(d: Direction, h: i32, r: Rect) -> Rect {
    let a = to_real(d, h, Point::new(r.x, r.y));
    let b = to_real(d, h, Point::new(r.right(), r.bottom()));
    Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        (a.x - b.x).abs() + 1,
        (a.y - b.y).abs() + 1,
    )
}

/// Canonical side of a cluster: `Top` is upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Top,
    Bottom,
}

/// Where an edge crosses a cluster's border.
#[derive(Debug, Clone, Copy)]
struct Proxy {
    edge: usize,
    side: Side,
    /// The item inside the cluster the edge continues to; `None` when the edge ends at the
    /// cluster itself.
    inner: Option<Item>,
    /// Position of this route piece in the edge's full route.
    key: i32,
}

#[derive(Debug, Clone)]
enum Class {
    Unroutable,
    Loop,
    /// An edge between two items of the innermost scope holding both endpoints.
    Normal {
        /// Clusters strictly inside the scope that contain each endpoint, outermost first.
        from_chain: Vec<usize>,
        to_chain: Vec<usize>,
    },
}

#[derive(Debug, Default)]
struct Scope {
    items: Vec<Item>,
    /// `(edge, from item, to item)` for edges between items of this scope.
    edges: Vec<(usize, Item, Item)>,
    /// `(edge, item)` for self-loops on items of this scope.
    loops: Vec<(usize, Item)>,
    proxies: Vec<Proxy>,
    ranks: HashMap<Item, i32>,
}

/// A laid-out container in its own real coordinates.
#[derive(Debug, Default)]
struct Placed {
    size: Size,
    nodes: Vec<(usize, Rect)>,
    clusters: Vec<(usize, Rect)>,
    /// Route pieces, each from its upper end to its lower end.
    pieces: Vec<(usize, i32, Vec<Point>)>,
    labels: Vec<(usize, Rect)>,
    loops: Vec<(usize, Vec<Point>, Option<Rect>)>,
    title: Option<Rect>,
    /// Canonical cross offset of each border crossing from the block's anchor.
    ports: HashMap<(usize, Side), i32>,
}

struct Builder<'a> {
    graph: &'a Graph,
    config: &'a Config,
    cluster_parent: Vec<Option<usize>>,
    node_parent: Vec<Option<usize>>,
    /// Scope 0 is the diagram; scope `c + 1` is cluster `c`.
    scopes: Vec<Scope>,
    classes: Vec<Class>,
    directions: Vec<Direction>,
    reversed: Vec<bool>,
}

impl<'a> Builder<'a> {
    fn new(graph: &'a Graph, config: &'a Config) -> Self {
        let nc = graph.clusters.len();
        let mut cluster_parent: Vec<Option<usize>> = graph
            .clusters
            .iter()
            .map(|c| c.parent.filter(|&p| p < nc))
            .collect();
        for c in 0..nc {
            let mut current = cluster_parent[c];
            let mut steps = 0;
            while let Some(p) = current {
                steps += 1;
                if p == c || steps > nc {
                    cluster_parent[c] = None;
                    break;
                }
                current = cluster_parent[p];
            }
        }
        let node_parent = graph
            .nodes
            .iter()
            .map(|n| n.parent.filter(|&p| p < nc))
            .collect();
        Builder {
            graph,
            config,
            cluster_parent,
            node_parent,
            scopes: (0..=nc).map(|_| Scope::default()).collect(),
            classes: Vec::new(),
            directions: vec![graph.direction; nc + 1],
            reversed: vec![false; graph.edges.len()],
        }
    }

    fn parent(&self, item: Item) -> Option<usize> {
        match item {
            Item::Node(n) => self.node_parent[n],
            Item::Cluster(c) => self.cluster_parent[c],
        }
    }

    /// Clusters containing `item`, outermost first.
    fn path(&self, item: Item) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current = self.parent(item);
        while let Some(c) = current {
            path.push(c);
            current = self.cluster_parent[c];
        }
        path.reverse();
        path
    }

    fn run(mut self) -> Layout {
        self.structure();
        self.rank_scopes();
        self.add_proxies();
        let order = self.bottom_up();
        let mut placed: Vec<Option<Placed>> = (0..self.scopes.len()).map(|_| None).collect();
        for &s in &order {
            let p = self.place(s, &placed);
            placed[s] = Some(p);
        }
        let placed: Vec<Placed> = placed
            .into_iter()
            .map(|p| p.expect("every scope is placed"))
            .collect();
        self.compose(&order, &placed)
    }

    /// Classifies edges, fills scopes, and resolves directions (LAYOUT-2, LAYOUT-3).
    fn structure(&mut self) {
        for (n, _) in self.graph.nodes.iter().enumerate() {
            let s = self.node_parent[n].map_or(0, |c| c + 1);
            self.scopes[s].items.push(Item::Node(n));
        }
        for c in 0..self.graph.clusters.len() {
            let s = self.cluster_parent[c].map_or(0, |p| p + 1);
            self.scopes[s].items.push(Item::Cluster(c));
        }
        let mut crossed = vec![false; self.graph.clusters.len()];
        for (e, spec) in self.graph.edges.iter().enumerate() {
            let class = if spec.from == spec.to {
                let scope = self.parent(spec.from).map_or(0, |c| c + 1);
                self.scopes[scope].loops.push((e, spec.from));
                Class::Loop
            } else {
                let a = self.path(spec.from);
                let b = self.path(spec.to);
                let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
                let at =
                    |path: &[usize], end: Item| path.get(common).map_or(end, |&c| Item::Cluster(c));
                let (from, to) = (at(&a, spec.from), at(&b, spec.to));
                if from == to {
                    Class::Unroutable
                } else {
                    let scope = if common == 0 { 0 } else { a[common - 1] + 1 };
                    self.scopes[scope].edges.push((e, from, to));
                    let ends = [spec.from, spec.to].into_iter().filter_map(|i| match i {
                        Item::Cluster(c) => Some(c),
                        Item::Node(_) => None,
                    });
                    for c in a[common..]
                        .iter()
                        .copied()
                        .chain(b[common..].iter().copied())
                        .chain(ends)
                    {
                        crossed[c] = true;
                    }
                    Class::Normal {
                        from_chain: a[common..].to_vec(),
                        to_chain: b[common..].to_vec(),
                    }
                }
            };
            self.classes.push(class);
        }
        // Parents before children.
        let mut clusters: Vec<usize> = (0..self.graph.clusters.len()).collect();
        clusters.sort_by_key(|&c| self.path(Item::Cluster(c)).len());
        for c in clusters {
            let parent = self.cluster_parent[c].map_or(0, |p| p + 1);
            let own = self.graph.clusters[c].direction.filter(|_| !crossed[c]);
            self.directions[c + 1] = own.unwrap_or(self.directions[parent]);
        }
    }

    fn rank_scopes(&mut self) {
        for s in 0..self.scopes.len() {
            let scope = &self.scopes[s];
            let index: HashMap<Item, usize> = scope
                .items
                .iter()
                .enumerate()
                .map(|(i, &it)| (it, i))
                .collect();
            let edges: Vec<RankEdge> = scope
                .edges
                .iter()
                .map(|&(e, from, to)| RankEdge {
                    from: index[&from],
                    to: index[&to],
                    len: 2 * self.graph.edges[e].min_len.max(1) as i32,
                })
                .collect();
            let (reversed, ranks) = rank::rank(scope.items.len(), &edges);
            for (&(e, _, _), r) in scope.edges.iter().zip(reversed) {
                self.reversed[e] = r;
            }
            self.scopes[s].ranks = scope.items.iter().copied().zip(ranks).collect();
        }
    }

    /// Adds a proxy to every cluster an edge crosses (LAYOUT-7).
    fn add_proxies(&mut self) {
        for e in 0..self.graph.edges.len() {
            let Class::Normal {
                from_chain,
                to_chain,
                ..
            } = &self.classes[e]
            else {
                continue;
            };
            let spec = &self.graph.edges[e];
            let (upper, lower, upper_chain, lower_chain) = if self.reversed[e] {
                (spec.to, spec.from, to_chain.clone(), from_chain.clone())
            } else {
                (spec.from, spec.to, from_chain.clone(), to_chain.clone())
            };
            for (side, end, chain) in [
                (Side::Bottom, upper, upper_chain),
                (Side::Top, lower, lower_chain),
            ] {
                let sign = if side == Side::Bottom { -1 } else { 1 };
                for (i, &c) in chain.iter().enumerate() {
                    let inner = chain.get(i + 1).map_or(end, |&d| Item::Cluster(d));
                    self.scopes[c + 1].proxies.push(Proxy {
                        edge: e,
                        side,
                        inner: Some(inner),
                        key: sign * (i as i32 + 1),
                    });
                }
                if let Item::Cluster(c) = end {
                    self.scopes[c + 1].proxies.push(Proxy {
                        edge: e,
                        side,
                        inner: None,
                        key: 0,
                    });
                }
            }
        }
    }

    /// Scopes with children before parents; the diagram last.
    fn bottom_up(&self) -> Vec<usize> {
        let mut clusters: Vec<usize> = (0..self.graph.clusters.len()).collect();
        clusters.sort_by_key(|&c| std::cmp::Reverse(self.path(Item::Cluster(c)).len()));
        let mut order: Vec<usize> = clusters.into_iter().map(|c| c + 1).collect();
        order.push(0);
        order
    }

    fn place(&self, s: usize, placed: &[Option<Placed>]) -> Placed {
        Placer::new(self, s, placed).run()
    }

    /// Translates every scope into diagram coordinates and joins route pieces.
    fn compose(&self, order: &[usize], placed: &[Placed]) -> Layout {
        let mut offset = vec![(0, 0); self.scopes.len()];
        let mut nodes = vec![Rect::default(); self.graph.nodes.len()];
        let mut clusters = vec![Rect::default(); self.graph.clusters.len()];
        let mut titles = vec![None; self.graph.clusters.len()];
        let mut pieces: Vec<Vec<(i32, Vec<Point>)>> = vec![Vec::new(); self.graph.edges.len()];
        let mut labels = vec![None; self.graph.edges.len()];
        let mut loops = vec![None; self.graph.edges.len()];
        for &s in order.iter().rev() {
            let (dx, dy) = offset[s];
            let p = &placed[s];
            if let Some(c) = s.checked_sub(1) {
                titles[c] = p.title.map(|r| r.translate(dx, dy));
            }
            for &(n, r) in &p.nodes {
                nodes[n] = r.translate(dx, dy);
            }
            for &(c, r) in &p.clusters {
                clusters[c] = r.translate(dx, dy);
                offset[c + 1] = (dx + r.x, dy + r.y);
            }
            let shift = |pts: &[Point]| -> Vec<Point> {
                pts.iter().map(|q| Point::new(q.x + dx, q.y + dy)).collect()
            };
            for (e, key, pts) in &p.pieces {
                pieces[*e].push((*key, shift(pts)));
            }
            for &(e, r) in &p.labels {
                labels[e] = Some(r.translate(dx, dy));
            }
            for (e, pts, label) in &p.loops {
                loops[*e] = Some(Route {
                    points: shift(pts),
                    label: label.map(|r| r.translate(dx, dy)),
                });
            }
        }
        let edges = (0..self.graph.edges.len())
            .map(|e| match self.classes[e] {
                Class::Unroutable => None,
                Class::Loop => loops[e].take(),
                Class::Normal { .. } => {
                    let mut list = std::mem::take(&mut pieces[e]);
                    list.sort_by_key(|(key, _)| *key);
                    let mut points: Vec<Point> = Vec::new();
                    for (_, piece) in list {
                        points.extend(piece);
                    }
                    if self.reversed[e] {
                        points.reverse();
                    }
                    Some(Route {
                        points: simplify(points),
                        label: labels[e],
                    })
                }
            })
            .collect();
        Layout {
            size: placed[0].size,
            nodes,
            clusters,
            titles,
            edges,
        }
    }
}

/// Drops repeated points and the middle of straight runs.
fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            if (a.x == b.x && b.x == p.x) || (a.y == b.y && b.y == p.y) {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Item(Item),
    Dummy,
    Label,
    Proxy(Side),
    /// A cluster's title, in the border layer at the real top.
    Title,
}

#[derive(Debug, Clone, Copy)]
struct Elem {
    kind: Kind,
    layer: usize,
    /// Canonical cross and main sizes.
    cross: i32,
    main: i32,
    /// Extra cross-axis room on the far side, for self-loops.
    extra: i32,
    /// Main-axis size of the tallest self-loop label.
    loop_main: i32,
}

/// One edge's route through this scope: elements from the upper end to the lower end.
#[derive(Debug)]
struct Chain {
    edge: usize,
    key: i32,
    elems: Vec<usize>,
}

/// Lays out one scope in its canonical frame.
struct Placer<'a, 'b> {
    b: &'b Builder<'a>,
    s: usize,
    placed: &'b [Option<Placed>],
    dir: Direction,
    sp: Spacing,
    elems: Vec<Elem>,
    chains: Vec<Chain>,
    item_elem: HashMap<Item, usize>,
    /// Edge of each label element.
    label_edge: HashMap<usize, usize>,
    /// Proxy of each proxy element.
    proxy_of: HashMap<usize, Proxy>,
    layers: Vec<Vec<usize>>,
}

impl<'a, 'b> Placer<'a, 'b> {
    fn new(b: &'b Builder<'a>, s: usize, placed: &'b [Option<Placed>]) -> Self {
        let dir = b.directions[s];
        let sp = if horizontal(dir) {
            b.config.horizontal
        } else {
            b.config.vertical
        };
        Placer {
            b,
            s,
            placed,
            dir,
            sp,
            elems: Vec::new(),
            chains: Vec::new(),
            item_elem: HashMap::new(),
            label_edge: HashMap::new(),
            proxy_of: HashMap::new(),
            layers: Vec::new(),
        }
    }

    fn is_cluster(&self) -> bool {
        self.s != 0
    }

    fn elem(&mut self, kind: Kind, layer: usize, size: Size) -> usize {
        self.elems.push(Elem {
            kind,
            layer,
            cross: size.w,
            main: size.h,
            extra: 0,
            loop_main: 0,
        });
        self.elems.len() - 1
    }

    /// Canonical size of an item.
    fn item_size(&self, item: Item) -> Size {
        let real = match item {
            Item::Node(n) => self.b.graph.nodes[n].size,
            Item::Cluster(c) => self.placed[c + 1].as_ref().expect("child placed").size,
        };
        swap_if(self.dir, real)
    }

    fn label_size(&self, e: usize) -> Option<Size> {
        self.b.graph.edges[e].label.map(|l| swap_if(self.dir, l))
    }

    fn build(&mut self) -> usize {
        let b = self.b;
        let scope = &b.scopes[self.s];
        let base = usize::from(self.is_cluster());
        let max_rank = scope.ranks.values().copied().max().unwrap_or(-1);
        let item_layers = (max_rank + 1) as usize;
        let last = base + item_layers;
        for &item in &scope.items {
            let layer = base + scope.ranks[&item] as usize;
            let size = self.item_size(item);
            let el = self.elem(Kind::Item(item), layer, size);
            self.item_elem.insert(item, el);
        }
        for &(e, from, to) in &scope.edges {
            let (u, v) = if self.b.reversed[e] {
                (to, from)
            } else {
                (from, to)
            };
            let (eu, ev) = (self.item_elem[&u], self.item_elem[&v]);
            let (lu, lv) = (self.elems[eu].layer, self.elems[ev].layer);
            let mid = lu + (lv - lu) / 2;
            let mut elems = vec![eu];
            for layer in lu + 1..lv {
                let el = match self.label_size(e).filter(|_| layer == mid) {
                    Some(size) => {
                        let el = self.elem(Kind::Label, layer, size);
                        self.label_edge.insert(el, e);
                        el
                    }
                    None => self.elem(Kind::Dummy, layer, Size::new(1, 0)),
                };
                elems.push(el);
            }
            elems.push(ev);
            self.chains.push(Chain {
                edge: e,
                key: 0,
                elems,
            });
        }
        for &p in &scope.proxies {
            let layer = if p.side == Side::Top { 0 } else { last };
            let proxy = self.elem(Kind::Proxy(p.side), layer, Size::new(1, 1));
            self.proxy_of.insert(proxy, p);
            let Some(inner) = p.inner else {
                continue;
            };
            let ei = self.item_elem[&inner];
            let li = self.elems[ei].layer;
            let (first, range, end) = if p.side == Side::Top {
                (proxy, 1..li, ei)
            } else {
                (ei, li + 1..last, proxy)
            };
            let mut elems = vec![first];
            for layer in range {
                elems.push(self.elem(Kind::Dummy, layer, Size::new(1, 0)));
            }
            elems.push(end);
            self.chains.push(Chain {
                edge: p.edge,
                key: p.key,
                elems,
            });
        }
        for &(e, item) in &scope.loops {
            let label = self.label_size(e).unwrap_or_default();
            let el = &mut self.elems[self.item_elem[&item]];
            let room = if label.w > 0 { 1 + label.w } else { 0 };
            el.extra = el.extra.max(self.sp.loop_size + room);
            el.loop_main = el.loop_main.max(label.h);
        }
        if let Some(c) = self.cluster_spec() {
            let title = c.title;
            if title.w > 0 && !horizontal(self.dir) {
                let layer = if self.dir == Direction::TopBottom {
                    0
                } else {
                    last
                };
                self.elem(Kind::Title, layer, title);
            }
        }
        if self.is_cluster() { last + 1 } else { last }
    }

    /// `(upper, lower, chain)` for each consecutive pair in every chain.
    fn segments(&self) -> Vec<(usize, usize, usize)> {
        self.chains
            .iter()
            .enumerate()
            .flat_map(|(c, ch)| ch.elems.windows(2).map(move |w| (w[0], w[1], c)))
            .collect()
    }

    /// Initial order: a depth-first search from each element by layer.
    fn init_layers(&mut self, count: usize, segments: &[(usize, usize, usize)]) {
        let mut down = vec![Vec::new(); self.elems.len()];
        for &(u, v, _) in segments {
            down[u].push(v);
        }
        let mut roots: Vec<usize> = (0..self.elems.len()).collect();
        roots.sort_by_key(|&e| self.elems[e].layer);
        let mut seen = vec![false; self.elems.len()];
        self.layers = vec![Vec::new(); count];
        for root in roots {
            let mut stack = vec![root];
            while let Some(e) = stack.pop() {
                if std::mem::replace(&mut seen[e], true) {
                    continue;
                }
                self.layers[self.elems[e].layer].push(e);
                stack.extend(down[e].iter().rev());
            }
        }
    }

    /// Port offsets from the anchor for each segment's upper and lower end.
    fn ports(&self, segments: &[(usize, usize, usize)]) -> Vec<(i32, i32)> {
        let mut pos = vec![0usize; self.elems.len()];
        for layer in &self.layers {
            for (i, &e) in layer.iter().enumerate() {
                pos[e] = i;
            }
        }
        let mut offsets = vec![(0, 0); segments.len()];
        for (e, el) in self.elems.iter().enumerate() {
            let ports = match el.kind {
                Kind::Item(Item::Node(_)) => None,
                Kind::Item(Item::Cluster(c)) => {
                    Some(&self.placed[c + 1].as_ref().expect("child placed").ports)
                }
                _ => continue,
            };
            for (downward, side) in [(true, Side::Bottom), (false, Side::Top)] {
                let mut list: Vec<usize> = (0..segments.len())
                    .filter(|&i| {
                        let (u, v, _) = segments[i];
                        if downward { u == e } else { v == e }
                    })
                    .collect();
                list.sort_by_key(|&i| {
                    let (u, v, c) = segments[i];
                    (pos[if downward { v } else { u }], self.chains[c].edge)
                });
                let n = list.len() as i32;
                for (k, &i) in list.iter().enumerate() {
                    let edge = self.chains[segments[i].2].edge;
                    let off = ports
                        .and_then(|p| p.get(&(edge, side)).copied())
                        .unwrap_or_else(|| self.spread(el.cross, k as i32, n));
                    if downward {
                        offsets[i].0 = off;
                    } else {
                        offsets[i].1 = off;
                    }
                }
            }
        }
        offsets
    }

    /// Grows nodes along the cross axis until the ports on each side fit inside the corners
    /// (LAYOUT-7).
    fn fit_ports(&mut self, segments: &[(usize, usize, usize)]) {
        for (e, el) in self.elems.iter_mut().enumerate() {
            if let Kind::Item(Item::Node(_)) = el.kind {
                let down = segments.iter().filter(|s| s.0 == e).count();
                let up = segments.iter().filter(|s| s.1 == e).count();
                let need = down.max(up).max(1) as i32 + 2 * self.sp.port_inset;
                el.cross = el.cross.max(need);
            }
        }
    }

    /// Offset from the anchor of port `k` of `n` spread across a side `width` wide.
    fn spread(&self, width: i32, k: i32, n: i32) -> i32 {
        let inset = self.sp.port_inset;
        let usable = width - 2 * inset;
        inset + ((2 * k + 1) * usable) / (2 * n) - width / 2
    }

    fn run(mut self) -> Placed {
        let count = self.build();
        let segments = self.segments();
        self.init_layers(count, &segments);
        let pairs: Vec<(usize, usize)> = segments.iter().map(|&(u, v, _)| (u, v)).collect();
        order::order(&mut self.layers, &pairs, self.elems.len());
        self.fit_ports(&segments);
        let offsets = self.ports(&segments);
        let spans: Vec<Span> = (0..self.elems.len())
            .map(|e| {
                let el = self.elems[e];
                Span {
                    left: el.cross / 2,
                    right: el.cross - 1 - el.cross / 2 + el.extra,
                    line: matches!(el.kind, Kind::Dummy | Kind::Proxy(_)),
                    floating: el.kind == Kind::Title
                        || matches!(el.kind, Kind::Proxy(_))
                            && !segments.iter().any(|&(u, v, _)| u == e || v == e),
                }
            })
            .collect();
        let links: Vec<Link> = segments
            .iter()
            .zip(&offsets)
            .map(|(&(u, v, _), &(uo, vo))| Link {
                upper: u,
                lower: v,
                upper_off: uo,
                lower_off: vo,
            })
            .collect();
        let mut x = position::position(
            &self.layers,
            &spans,
            &links,
            self.sp.node_gap,
            self.sp.edge_gap,
        );

        // Cross-axis extent and padding.
        let lo = (0..x.len())
            .map(|e| x[e] - spans[e].left)
            .min()
            .unwrap_or(0);
        let hi = (0..x.len())
            .map(|e| x[e] + spans[e].right)
            .max()
            .unwrap_or(-1);
        let content = hi - lo + 1;
        let cluster = self.cluster_spec();
        // In a horizontal cluster the title runs along the main axis at the cross-axis start.
        let side_title = cluster
            .map(|c| c.title)
            .filter(|t| t.w > 0 && horizontal(self.dir));
        let (min_main, cross_start) = side_title.map_or((0, 0), |t| {
            (t.w + 2 * self.sp.cluster_main_pad, (t.h - 1).max(0))
        });
        let pad = if cluster.is_some() {
            self.sp.cluster_cross_pad
        } else {
            0
        };
        let width = content + 2 * pad + cross_start;
        let shift = pad + cross_start - lo;
        for a in &mut x {
            *a += shift;
        }

        // Main axis: layer heights, gaps sized by their tracks.
        let layers = self.layers.len();
        let mut heights: Vec<i32> = self
            .layers
            .iter()
            .map(|l| {
                l.iter()
                    .map(|&e| self.elems[e].main.max(self.elems[e].loop_main))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        if cluster.is_some() {
            // The border rows.
            heights[0] = heights[0].max(1);
            heights[layers - 1] = heights[layers - 1].max(1);
        }
        let port = |i: usize, upper: bool| {
            let (u, v, _) = segments[i];
            if upper {
                x[u] + offsets[i].0
            } else {
                x[v] + offsets[i].1
            }
        };
        let mut track_of = vec![None; segments.len()];
        let mut gaps = vec![0; layers.saturating_sub(1)];
        for (g, gap) in gaps.iter_mut().enumerate() {
            let in_gap: Vec<usize> = (0..segments.len())
                .filter(|&i| self.elems[segments[i].0].layer == g)
                .collect();
            let jogs: Vec<Jog> = in_gap
                .iter()
                .map(|&i| Jog {
                    top: port(i, true),
                    bottom: port(i, false),
                })
                .collect();
            let (tracks, k) = route::tracks(&jogs);
            for (&i, t) in in_gap.iter().zip(tracks) {
                track_of[i] = t;
            }
            let mut min = self.sp.rank_gap;
            if cluster.is_some() && (g == 0 || g + 2 == layers) {
                min = min.max(self.sp.cluster_main_pad);
            }
            let needed = if k > 0 {
                self.sp.track_gap * (k as i32 + 1) + 1
            } else {
                0
            };
            *gap = min.max(needed);
        }
        let total = heights.iter().sum::<i32>() + gaps.iter().sum::<i32>();
        if total < min_main && !gaps.is_empty() {
            let grow = min_main - total;
            let last = gaps.len() - 1;
            gaps[0] += grow / 2;
            gaps[last] += grow - grow / 2;
        }
        let mut tops = Vec::with_capacity(layers);
        let mut at = 0;
        for (l, h) in heights.iter().enumerate() {
            tops.push(at);
            at += h + gaps.get(l).copied().unwrap_or(0);
        }
        let height = at;

        // Element rows: (first, last) the route uses at the upstream and downstream sides.
        let rows: Vec<(i32, i32)> = self
            .elems
            .iter()
            .map(|el| {
                let (top, h) = (tops[el.layer], heights[el.layer]);
                let bottom = top + h.max(1) - 1;
                match el.kind {
                    Kind::Dummy => (top, bottom),
                    // Proxies and titles keep to the border row.
                    Kind::Proxy(Side::Top) => (top, top),
                    Kind::Proxy(Side::Bottom) => (bottom, bottom),
                    Kind::Title if el.layer == 0 => (top, top + el.main - 1),
                    Kind::Title => (bottom + 1 - el.main, bottom),
                    _ => {
                        let first = top + (h - el.main) / 2;
                        (first, first + el.main - 1)
                    }
                }
            })
            .collect();
        let rect = |e: usize| {
            let el = self.elems[e];
            Rect::new(x[e] - el.cross / 2, rows[e].0, el.cross, el.main)
        };

        let mut placed = Placed {
            size: swap_if(self.dir, Size::new(width, height)),
            ..Placed::default()
        };
        let real = |p: Point| to_real(self.dir, height, p);
        let real_rect = |r: Rect| rect_to_real(self.dir, height, r);

        for (e, el) in self.elems.iter().enumerate() {
            match el.kind {
                Kind::Item(Item::Node(n)) => placed.nodes.push((n, real_rect(rect(e)))),
                Kind::Item(Item::Cluster(c)) => placed.clusters.push((c, real_rect(rect(e)))),
                Kind::Label => placed
                    .labels
                    .push((self.label_edge[&e], real_rect(rect(e)))),
                Kind::Proxy(side) => {
                    let p = self.proxy_of[&e];
                    placed.ports.insert((p.edge, side), x[e] - width / 2);
                }
                Kind::Title => placed.title = Some(real_rect(rect(e))),
                Kind::Dummy => {}
            }
        }
        if let Some(t) = side_title {
            placed.title = Some(Rect::new(self.sp.cluster_main_pad, 0, t.w, t.h));
        }

        for (c, chain) in self.chains.iter().enumerate() {
            let mut points = Vec::new();
            for (i, &(u, v, _)) in segments.iter().enumerate() {
                if segments[i].2 != c {
                    continue;
                }
                let (xu, xv) = (port(i, true), port(i, false));
                points.push(real(Point::new(xu, rows[u].1)));
                if let Some(t) = track_of[i] {
                    let gap_start = tops[self.elems[u].layer] + heights[self.elems[u].layer];
                    let row = gap_start + self.sp.track_gap * (t as i32 + 1);
                    points.push(real(Point::new(xu, row)));
                    points.push(real(Point::new(xv, row)));
                }
                points.push(real(Point::new(xv, rows[v].0)));
            }
            placed.pieces.push((chain.edge, chain.key, points));
        }

        for &(e, item) in &self.b.scopes[self.s].loops {
            let el = self.item_elem[&item];
            let r = rect(el);
            let layer = self.elems[el].layer;
            let inset = self.sp.loop_inset.min((r.h - 1) / 2);
            let (t, bot, right) = (r.y + inset, r.bottom() - inset, r.right());
            let out = right + self.sp.loop_size;
            let points = [
                Point::new(right, t),
                Point::new(out, t),
                Point::new(out, bot),
                Point::new(right, bot),
            ]
            .map(real)
            .to_vec();
            let label = self.label_size(e).map(|l| {
                let y = tops[layer] + (heights[layer] - l.h) / 2;
                real_rect(Rect::new(out + 2, y, l.w, l.h))
            });
            placed.loops.push((e, points, label));
        }
        placed
    }

    fn cluster_spec(&self) -> Option<&'a ClusterSpec> {
        self.s.checked_sub(1).map(|c| &self.b.graph.clusters[c])
    }
}

#[cfg(test)]
mod tests;
