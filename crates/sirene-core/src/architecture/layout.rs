//! Places an architecture diagram (MARCH-3). Each group, and the diagram, is a grid: an edge
//! `a:R -- L:b` puts `b` one column right of `a`, `a:T -- B:b` one row above, and an edge between
//! an across side and an up-or-down side one diagonal step, as Mermaid's positions do. A group is
//! placed in its parent's grid as one item. Units are the caller's: pixels or characters.

use std::collections::{HashMap, VecDeque};

use super::{Architecture, Side};

/// A box by its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(self) -> f64 {
        self.y + self.h
    }

    pub fn center(self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// The middle of `side`.
    fn port(self, side: Side) -> (f64, f64) {
        let (cx, cy) = self.center();
        match side {
            Side::Left => (self.x, cy),
            Side::Right => (self.right(), cy),
            Side::Top => (cx, self.y),
            Side::Bottom => (cx, self.bottom()),
        }
    }
}

/// A point.
pub type Xy = (f64, f64);

/// A node's box size and the size of its label below.
type NodeSize = (Xy, Xy);

/// What `layout` needs to know about sizes.
pub struct Sizes<'a> {
    /// A node's box, and the room under it for its label (width, height).
    pub node: &'a dyn Fn(usize) -> NodeSize,
    /// A group's title: its size, at the top.
    pub title: &'a dyn Fn(usize) -> (f64, f64),
    /// Between grid cells.
    pub gap_x: f64,
    pub gap_y: f64,
    /// Inside a group's border.
    pub pad: f64,
}

/// Where everything goes, with the top-left of the whole at the origin.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    /// Each node's box (without its label).
    pub nodes: Vec<Rect>,
    pub groups: Vec<Rect>,
    /// Each edge's path from end to end, and the middle of its longest run for a label.
    pub edges: Vec<(Vec<Xy>, Xy)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Cell {
    Node(usize),
    Group(usize),
}

/// The grid step from an item to its neighbor, leaving by `from` and entering by `to`.
fn step(from: Side, to: Side) -> (i32, i32) {
    let dx = if from.is_x() {
        if from == Side::Right { 1 } else { -1 }
    } else if to.is_x() {
        if to == Side::Left { 1 } else { -1 }
    } else {
        0
    };
    let dy = if !from.is_x() {
        if from == Side::Bottom { 1 } else { -1 }
    } else if !to.is_x() {
        if to == Side::Top { 1 } else { -1 }
    } else {
        0
    };
    (dx, dy)
}

/// A group's contents laid out: its size and each item's box inside it.
struct Local {
    w: f64,
    h: f64,
    items: Vec<(Cell, Rect)>,
}

/// What is directly in each container (a group, or `None` for the diagram), in source order.
type Within = HashMap<Option<usize>, Vec<usize>>;

fn within(m: &Within, container: Option<usize>) -> &[usize] {
    m.get(&container).map_or(&[], Vec::as_slice)
}

struct Ctx<'a> {
    arch: &'a Architecture,
    s: &'a Sizes<'a>,
    nodes_in: Within,
    groups_in: Within,
    /// Each edge is in the innermost container that holds both of its ends.
    ports_in: Within,
    /// Each `align` is in every container that holds one of its members.
    aligns_in: Within,
}

impl<'a> Ctx<'a> {
    fn new(arch: &'a Architecture, s: &'a Sizes<'a>) -> Self {
        let mut ctx = Ctx {
            arch,
            s,
            nodes_in: Within::new(),
            groups_in: Within::new(),
            ports_in: Within::new(),
            aligns_in: Within::new(),
        };
        for (n, &parent) in arch.node_parent.iter().enumerate() {
            ctx.nodes_in.entry(parent).or_default().push(n);
        }
        for (g, &parent) in arch.group_parent.iter().enumerate() {
            if parent != Some(g) {
                ctx.groups_in.entry(parent).or_default().push(g);
            }
        }
        // The containers around a node, innermost first, ending at the diagram.
        let around = |n: usize| {
            let mut chain = vec![arch.node_parent[n]];
            while let Some(g) = chain[chain.len() - 1] {
                if chain.len() > arch.group_parent.len() {
                    break;
                }
                chain.push(arch.group_parent[g]);
            }
            chain
        };
        for (i, p) in arch.ports.iter().enumerate() {
            let to = around(p.to);
            if let Some(&shared) = around(p.from).iter().find(|c| to.contains(c)) {
                ctx.ports_in.entry(shared).or_default().push(i);
            }
        }
        for (i, al) in arch.aligns.iter().enumerate() {
            for &n in &al.nodes {
                let list = ctx.aligns_in.entry(arch.node_parent[n]).or_default();
                if list.last() != Some(&i) {
                    list.push(i);
                }
            }
        }
        ctx
    }

    /// The item directly in `container` that holds node `n`, if any.
    fn item_in(&self, container: Option<usize>, n: usize) -> Option<Cell> {
        let mut cell = Cell::Node(n);
        let mut parent = self.arch.node_parent[n];
        let mut guard = 0;
        while parent != container {
            let g = parent?;
            cell = Cell::Group(g);
            parent = self.arch.group_parent[g];
            guard += 1;
            if guard > self.arch.group_parent.len() {
                return None;
            }
        }
        Some(cell)
    }

    fn layout(&self, container: Option<usize>, done: &mut HashMap<usize, Local>) -> Local {
        let a = self.arch;
        let mut cells: Vec<Cell> = Vec::new();
        cells.extend(
            within(&self.nodes_in, container)
                .iter()
                .map(|&n| Cell::Node(n)),
        );
        cells.extend(
            within(&self.groups_in, container)
                .iter()
                .map(|&g| Cell::Group(g)),
        );
        // Each item's own size; a cell's label room below.
        let mut sizes: HashMap<Cell, NodeSize> = HashMap::new();
        for &c in &cells {
            let size = match c {
                Cell::Node(n) => (self.s.node)(n),
                Cell::Group(g) => {
                    let local = self.layout(Some(g), done);
                    let size = ((local.w, local.h), (0.0, 0.0));
                    done.insert(g, local);
                    size
                }
            };
            sizes.insert(c, size);
        }
        // Neighbors by edges inside this container.
        type Step = (i32, i32);
        let mut next: HashMap<Cell, Vec<(Cell, Step)>> = HashMap::new();
        for &i in within(&self.ports_in, container) {
            let p = &a.ports[i];
            let (Some(x), Some(y)) = (
                self.item_in(container, p.from),
                self.item_in(container, p.to),
            ) else {
                continue;
            };
            if x == y {
                continue;
            }
            let (dx, dy) = step(p.from_side, p.to_side);
            next.entry(x).or_default().push((y, (dx, dy)));
            next.entry(y).or_default().push((x, (-dx, -dy)));
        }
        let mut at: HashMap<Cell, (i32, i32)> = HashMap::new();
        let mut taken: HashMap<(i32, i32), Cell> = HashMap::new();
        let free = |taken: &HashMap<(i32, i32), Cell>, mut p: (i32, i32), d: (i32, i32)| {
            while taken.contains_key(&p) {
                p = (p.0 + d.0, p.1 + d.1);
            }
            p
        };
        // The column right of every one taken so far.
        let mut col = 0;
        for &start in &cells {
            if at.contains_key(&start) {
                continue;
            }
            let p = free(&taken, (col, 0), (1, 0));
            at.insert(start, p);
            taken.insert(p, start);
            col = col.max(p.0 + 1);
            let mut queue = VecDeque::from([start]);
            while let Some(c) = queue.pop_front() {
                let here = at[&c];
                for &(n, d) in next.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
                    if at.contains_key(&n) {
                        continue;
                    }
                    let p = free(&taken, (here.0 + d.0, here.1 + d.1), d);
                    at.insert(n, p);
                    taken.insert(p, n);
                    col = col.max(p.0 + 1);
                    queue.push_back(n);
                }
            }
        }
        // `align`: a row shares the first member's row, a column its column, in order.
        for &i in within(&self.aligns_in, container) {
            let al = &a.aligns[i];
            let members: Vec<Cell> = al
                .nodes
                .iter()
                .filter(|&&n| a.node_parent[n] == container)
                .map(|&n| Cell::Node(n))
                .collect();
            let Some(&first) = members.first() else {
                continue;
            };
            let base = at[&first];
            let mut prev = base;
            for &m in &members[1..] {
                taken.remove(&at[&m]);
                let mut p = at[&m];
                if al.row {
                    p.1 = base.1;
                    p.0 = p.0.max(prev.0 + 1);
                } else {
                    p.0 = base.0;
                    p.1 = p.1.max(prev.1 + 1);
                }
                let d = if al.row { (1, 0) } else { (0, 1) };
                let p = free(&taken, p, d);
                at.insert(m, p);
                taken.insert(p, m);
                prev = p;
            }
        }
        // Columns and rows in use, each as wide and tall as its largest item.
        let mut cols: Vec<i32> = at.values().map(|p| p.0).collect();
        let mut rows: Vec<i32> = at.values().map(|p| p.1).collect();
        cols.sort_unstable();
        cols.dedup();
        rows.sort_unstable();
        rows.dedup();
        // Boxes in a row share a middle line, with the labels hanging below.
        let mut col_w = vec![0.0f64; cols.len()];
        let mut row_box = vec![0.0f64; rows.len()];
        let mut row_label = vec![0.0f64; rows.len()];
        let index = |v: &[i32], x: i32| v.binary_search(&x).unwrap_or(0);
        for (&c, &(x, y)) in &at {
            let ((w, h), (lw, lh)) = sizes[&c];
            let (i, j) = (index(&cols, x), index(&rows, y));
            col_w[i] = col_w[i].max(w.max(lw));
            row_box[j] = row_box[j].max(h);
            row_label[j] = row_label[j].max(lh);
        }
        let row_h: Vec<f64> = row_box.iter().zip(&row_label).map(|(b, l)| b + l).collect();
        let (pad, title) = match container {
            Some(g) => (self.s.pad, (self.s.title)(g)),
            None => (0.0, (0.0, 0.0)),
        };
        let top = pad + title.1;
        let mut xs = Vec::with_capacity(cols.len());
        let mut x = pad;
        for w in &col_w {
            xs.push(x);
            x += w + self.s.gap_x;
        }
        let mut ys = Vec::with_capacity(rows.len());
        let mut y = top;
        for h in &row_h {
            ys.push(y);
            y += h + self.s.gap_y;
        }
        let grid_w = if cols.is_empty() {
            0.0
        } else {
            x - self.s.gap_x - pad
        };
        let grid_h = if rows.is_empty() {
            0.0
        } else {
            y - self.s.gap_y - top
        };
        let items = cells
            .iter()
            .map(|&c| {
                let (x, y) = at[&c];
                let (i, j) = (index(&cols, x), index(&rows, y));
                let ((w, h), _) = sizes[&c];
                let r = Rect {
                    x: xs[i] + (col_w[i] - w) / 2.0,
                    y: ys[j] + (row_box[j] - h) / 2.0,
                    w,
                    h,
                };
                (c, r)
            })
            .collect();
        Local {
            w: (grid_w + 2.0 * pad).max(title.0 + 2.0 * pad),
            h: grid_h + top + pad,
            items,
        }
    }
}

fn assign(local: &Local, origin: (f64, f64), done: &HashMap<usize, Local>, placed: &mut Placed) {
    for &(c, r) in &local.items {
        let r = Rect {
            x: r.x + origin.0,
            y: r.y + origin.1,
            ..r
        };
        match c {
            Cell::Node(n) => placed.nodes[n] = r,
            Cell::Group(g) => {
                placed.groups[g] = r;
                if let Some(inner) = done.get(&g) {
                    assign(inner, (r.x, r.y), done, placed);
                }
            }
        }
    }
}

/// Places every node and group of `arch`, and routes its edges.
pub fn layout(arch: &Architecture, s: &Sizes) -> Placed {
    let ctx = Ctx::new(arch, s);
    let mut done = HashMap::new();
    let root = ctx.layout(None, &mut done);
    let mut placed = Placed {
        width: root.w,
        height: root.h,
        nodes: vec![Rect::default(); arch.chart.nodes.len()],
        groups: vec![Rect::default(); arch.chart.subgraphs.len()],
        edges: Vec::new(),
    };
    assign(&root, (0.0, 0.0), &done, &mut placed);
    placed.edges = arch
        .ports
        .iter()
        .map(|p| route(arch, s, &placed, p))
        .collect();
    placed
}

/// An edge's path: out of one side, with one or two turns, into the other.
fn route(
    arch: &Architecture,
    s: &Sizes,
    placed: &Placed,
    p: &super::Port,
) -> (Vec<(f64, f64)>, (f64, f64)) {
    let end = |n: usize, side: Side, group: bool| -> (f64, f64) {
        let node = placed.nodes[n];
        match arch.node_parent[n].filter(|_| group) {
            Some(g) => {
                let r = placed.groups[g];
                let (cx, cy) = node.center();
                match side {
                    Side::Left => (r.x, cy),
                    Side::Right => (r.right(), cy),
                    Side::Top => (cx, r.y),
                    Side::Bottom => (cx, r.bottom()),
                }
            }
            // Below a node is its label; the edge starts under it.
            None if side == Side::Bottom => {
                let (_, (_, lh)) = (s.node)(n);
                let (x, y) = node.port(side);
                (x, y + lh)
            }
            None => node.port(side),
        }
    };
    let a = end(p.from, p.from_side, p.from_group);
    let b = end(p.to, p.to_side, p.to_group);
    let points = match (p.from_side.is_x(), p.to_side.is_x()) {
        (true, true) if a.1 == b.1 => vec![a, b],
        (true, true) => {
            let mx = (a.0 + b.0) / 2.0;
            vec![a, (mx, a.1), (mx, b.1), b]
        }
        (false, false) if a.0 == b.0 => vec![a, b],
        (false, false) => {
            let my = (a.1 + b.1) / 2.0;
            vec![a, (a.0, my), (b.0, my), b]
        }
        (true, false) => vec![a, (b.0, a.1), b],
        (false, true) => vec![a, (a.0, b.1), b],
    };
    let mid = points
        .windows(2)
        .max_by(|x, y| {
            let len = |s: &[(f64, f64)]| (s[1].0 - s[0].0).abs() + (s[1].1 - s[0].1).abs();
            len(x).total_cmp(&len(y))
        })
        .map_or(a, |s| ((s[0].0 + s[1].0) / 2.0, (s[0].1 + s[1].1) / 2.0));
    (points, mid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{self, Diagram};

    fn placed(src: &str) -> (Architecture, Placed) {
        let Diagram::Architecture(a) = diagram::lower(src).diagram else {
            panic!("not an architecture diagram")
        };
        let sizes = Sizes {
            node: &|_| ((10.0, 10.0), (0.0, 0.0)),
            title: &|_| (0.0, 2.0),
            gap_x: 4.0,
            gap_y: 4.0,
            pad: 1.0,
        };
        let p = layout(&a, &sizes);
        (a, p)
    }

    #[test]
    fn steps_follow_the_sides() {
        assert_eq!(step(Side::Right, Side::Left), (1, 0));
        assert_eq!(step(Side::Left, Side::Right), (-1, 0));
        assert_eq!(step(Side::Bottom, Side::Top), (0, 1));
        assert_eq!(step(Side::Top, Side::Bottom), (0, -1));
        assert_eq!(step(Side::Top, Side::Left), (1, -1));
        assert_eq!(step(Side::Right, Side::Top), (1, 1));
    }

    #[test]
    fn edges_place_neighbors() {
        let (_, p) =
            placed("architecture-beta\nservice a\nservice b\nservice c\na:R -- L:b\nb:B -- T:c\n");
        let (a, b, c) = (p.nodes[0], p.nodes[1], p.nodes[2]);
        assert_eq!((a.x, a.y), (0.0, 0.0));
        assert_eq!((b.x, b.y), (14.0, 0.0));
        assert_eq!((c.x, c.y), (14.0, 14.0));
        assert_eq!(p.edges[0].0, [(10.0, 5.0), (14.0, 5.0)]);
        assert_eq!((p.width, p.height), (24.0, 24.0));
    }

    #[test]
    fn groups_hold_their_items() {
        let (_, p) = placed(
            "architecture-beta\ngroup g\nservice a in g\nservice b in g\nservice c\n\
             a:R -- L:b\nc:L -- R:b{group}\n",
        );
        let g = p.groups[0];
        for i in [0, 1] {
            let r = p.nodes[i];
            assert!(r.x >= g.x && r.right() <= g.right() && r.y >= g.y + 2.0);
        }
        assert!(p.nodes[2].x > g.right());
        // The edge ends at the group's side, level with `b`.
        let path = &p.edges[1].0;
        assert_eq!(path.last().unwrap().0, g.right());
    }

    #[test]
    fn align_makes_a_row() {
        let (_, p) = placed(
            "architecture-beta\nservice a\nservice b\nservice c\na:B -- T:b\na:B -- T:c\n\
             align row b c\n",
        );
        assert_eq!(p.nodes[1].y, p.nodes[2].y);
        assert!(p.nodes[1].x < p.nodes[2].x);
    }
}
