//! The grid of a block diagram (MBLOCK-3), as Mermaid places it: every cell of a group is as
//! large as its largest block (per column it spans), rows wrap after `columns`, and a group is
//! as large as its grid plus padding. Units are the caller's: pixels for the drawing, characters
//! for text.

use super::{Block, CellKind};

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
}

/// Where everything goes, with the top-left of the whole at the origin.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    /// Each node's box; `None` for one not in any grid.
    pub nodes: Vec<Option<Rect>>,
    /// Each group's box; `groups[0]`, the diagram, is the whole.
    pub groups: Vec<Rect>,
}

/// What `layout` needs to know about sizes.
pub struct Sizes<'a> {
    /// A node's own size.
    pub node: &'a dyn Fn(usize) -> (f64, f64),
    /// Extra room at the top of a group for its title.
    pub title: &'a dyn Fn(usize) -> f64,
    pub pad_x: f64,
    pub pad_y: f64,
}

/// A cell being placed: its size and center.
#[derive(Debug, Clone, Default)]
struct Item {
    kind: Option<CellKind>,
    span: usize,
    w: f64,
    h: f64,
    x: f64,
    y: f64,
    columns: Option<usize>,
    title: f64,
    children: Vec<Item>,
}

impl Item {
    fn is_space(&self) -> bool {
        self.kind == Some(CellKind::Space)
    }
}

fn tree(block: &Block, group: usize, s: &Sizes) -> Vec<Item> {
    block.groups[group]
        .cells
        .iter()
        .map(|c| {
            let mut item = Item {
                kind: Some(c.kind),
                span: c.width.max(1),
                ..Item::default()
            };
            match c.kind {
                CellKind::Node(n) => (item.w, item.h) = (s.node)(n),
                CellKind::Group(g) => {
                    item.columns = block.groups[g].columns;
                    item.title = (s.title)(g);
                    item.children = tree(block, g, s);
                }
                CellKind::Space => {}
            }
            item
        })
        .collect()
}

/// The column and row of the `position`th column filled, in a grid of `columns`.
fn grid(columns: Option<usize>, position: usize) -> (usize, usize) {
    match columns {
        None => (position, 0),
        Some(c) => (position % c, position / c),
    }
}

/// How many of a cell's columns fit in its row.
fn filled(columns: Option<usize>, position: usize, span: usize) -> usize {
    match columns {
        Some(c) => span.min(c - position % c),
        None => span,
    }
}

/// The columns across a group's grid.
fn across(item: &Item) -> usize {
    let total: usize = item.children.iter().map(|c| c.span).sum();
    match item.columns {
        Some(c) if c < total => c,
        _ => total.max(1),
    }
}

/// Sizes a group and everything in it, as Mermaid's `setBlockSizes`: a group that comes out
/// smaller than `sibling` grows to it, and its cells grow with it.
fn size(item: &mut Item, sibling: (f64, f64), s: &Sizes) {
    if item.w == 0.0 {
        (item.w, item.h) = sibling;
    }
    if item.children.is_empty() {
        return;
    }
    for c in &mut item.children {
        size(c, (0.0, 0.0), s);
    }
    let (mut max_w, mut max_h) = (0.0f64, 0.0f64);
    for c in item.children.iter().filter(|c| !c.is_space()) {
        max_w = max_w.max(c.w / c.span as f64);
        max_h = max_h.max(c.h);
    }
    let span_w = |w: f64, span: usize| w * span as f64 + s.pad_x * (span as f64 - 1.0);
    for c in &mut item.children {
        c.w = span_w(max_w, c.span);
        c.h = max_h;
    }
    for c in &mut item.children {
        size(c, (max_w, max_h), s);
    }
    let x_size = across(item) as f64;
    let total: usize = item.children.iter().map(|c| c.span).sum();
    let y_size = (total as f64 / x_size).ceil();
    let mut width = x_size * (max_w + s.pad_x) + s.pad_x;
    let mut height = y_size * (max_h + s.pad_y) + s.pad_y + item.title;
    if width < sibling.0 {
        width = sibling.0;
        height = height.max(sibling.1);
        let cw = (width - x_size * s.pad_x - s.pad_x) / x_size;
        let ch = (height - item.title - y_size * s.pad_y - s.pad_y) / y_size;
        for c in &mut item.children {
            c.w = span_w(cw, c.span);
            c.h = ch;
        }
    }
    if width < item.w {
        width = item.w;
        let cw = (width - x_size * s.pad_x - s.pad_x) / x_size;
        for c in &mut item.children {
            c.w = span_w(cw, c.span);
        }
    }
    item.w = width;
    item.h = height;
    // Groups resized above place their own cells again.
    for c in &mut item.children {
        if !c.children.is_empty() {
            regrow(c, s);
        }
    }
}

/// Spreads a group's cells over a box it was stretched to.
fn regrow(item: &mut Item, s: &Sizes) {
    let x_size = across(item) as f64;
    let total: usize = item.children.iter().map(|c| c.span).sum();
    let y_size = (total as f64 / x_size).ceil();
    let cw = (item.w - x_size * s.pad_x - s.pad_x) / x_size;
    let ch = (item.h - item.title - y_size * s.pad_y - s.pad_y) / y_size;
    let span_w = |w: f64, span: usize| w * span as f64 + s.pad_x * (span as f64 - 1.0);
    for c in &mut item.children {
        if cw > c.w / c.span as f64 {
            c.w = span_w(cw, c.span);
        }
        c.h = c.h.max(ch);
        if !c.children.is_empty() {
            regrow(c, s);
        }
    }
}

/// Places a group's cells in rows, as Mermaid's `layoutBlocks`.
fn place(item: &mut Item, left: f64, s: &Sizes) {
    let columns = item.columns;
    let mut rows: Vec<f64> = Vec::new();
    let mut at = 0;
    for c in &item.children {
        let (_, row) = grid(columns, at);
        if rows.len() <= row {
            rows.resize(row + 1, 0.0);
        }
        rows[row] = rows[row].max(c.h);
        at += filled(columns, at, c.span);
    }
    let mut offsets = Vec::with_capacity(rows.len());
    let mut offset = 0.0;
    for h in &rows {
        offsets.push(offset);
        offset += h + s.pad_y;
    }
    let top = item.y - item.h / 2.0 + item.title;
    let (mut x, mut row) = (left, 0);
    let mut at = 0;
    for c in &mut item.children {
        let (_, r) = grid(columns, at);
        if r != row {
            row = r;
            x = left;
        }
        c.x = x + s.pad_x + c.w / 2.0;
        x = c.x + c.w / 2.0;
        c.y = top + offsets[r] + rows[r] / 2.0 + s.pad_y;
        if !c.children.is_empty() {
            let l = c.x - c.w / 2.0;
            place(c, l, s);
        }
        at += filled(columns, at, c.span);
    }
}

fn collect(item: &Item, nodes: &mut [Option<Rect>], groups: &mut [Rect]) {
    for c in &item.children {
        let r = Rect {
            x: c.x - c.w / 2.0,
            y: c.y - c.h / 2.0,
            w: c.w,
            h: c.h,
        };
        match c.kind {
            Some(CellKind::Node(n)) => nodes[n] = Some(r),
            Some(CellKind::Group(g)) => groups[g] = r,
            _ => {}
        }
        collect(c, nodes, groups);
    }
}

fn bounds(item: &Item, b: &mut Option<(f64, f64, f64, f64)>) {
    for c in &item.children {
        let (x0, y0, x1, y1) = (
            c.x - c.w / 2.0,
            c.y - c.h / 2.0,
            c.x + c.w / 2.0,
            c.y + c.h / 2.0,
        );
        *b = Some(match *b {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
        bounds(c, b);
    }
}

/// Places every block and group of `block`.
pub fn layout(block: &Block, s: &Sizes) -> Placed {
    let mut root = Item {
        columns: block.groups.first().and_then(|g| g.columns),
        children: tree(block, 0, s),
        ..Item::default()
    };
    size(&mut root, (0.0, 0.0), s);
    place(&mut root, -s.pad_x, s);
    let mut b = None;
    bounds(&root, &mut b);
    let (x0, y0, x1, y1) = b.unwrap_or_default();
    let mut nodes = vec![None; block.chart.nodes.len()];
    let mut groups = vec![Rect::default(); block.groups.len()];
    collect(&root, &mut nodes, &mut groups);
    for r in nodes.iter_mut().flatten().chain(groups.iter_mut().skip(1)) {
        r.x -= x0;
        r.y -= y0;
    }
    let (width, height) = (x1 - x0, y1 - y0);
    groups[0] = Rect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: height,
    };
    Placed {
        width,
        height,
        nodes,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{self, Diagram};

    fn placed(src: &str, w: f64) -> (Block, Placed) {
        let Diagram::Block(b) = diagram::lower(src).diagram else {
            panic!("not a block diagram")
        };
        let sizes = Sizes {
            node: &|_| (w, 10.0),
            title: &|_| 0.0,
            pad_x: 2.0,
            pad_y: 1.0,
        };
        let p = layout(&b, &sizes);
        (b, p)
    }

    #[test]
    fn rows_wrap_after_columns() {
        let (_, p) = placed("block-beta\ncolumns 2\na b c\n", 6.0);
        let r = |i: usize| p.nodes[i].unwrap();
        assert_eq!((r(0).x, r(0).y), (0.0, 0.0));
        assert_eq!((r(1).x, r(1).y), (8.0, 0.0));
        assert_eq!((r(2).x, r(2).y), (0.0, 11.0));
        assert_eq!((p.width, p.height), (14.0, 21.0));
    }

    #[test]
    fn spans_and_spaces_take_columns() {
        let (_, p) = placed("block-beta\ncolumns 3\na:2 b\nspace c:2\n", 6.0);
        let r = |i: usize| p.nodes[i].unwrap();
        assert_eq!((r(0).x, r(0).w), (0.0, 14.0));
        assert_eq!(r(1).x, 16.0);
        assert_eq!((r(2).x, r(2).y, r(2).w), (8.0, 11.0, 14.0));
    }

    #[test]
    fn groups_hold_their_blocks() {
        let (b, p) = placed("block-beta\na\nblock:g\n  b c\nend\n", 6.0);
        let g = p.groups[1];
        assert_eq!(b.groups[1].cells.len(), 2);
        for i in [1, 2] {
            let r = p.nodes[i].unwrap();
            assert!(r.x >= g.x && r.right() <= g.right() && r.y >= g.y && r.bottom() <= g.bottom());
        }
        // The lone block grows to the group's size beside it.
        assert_eq!(p.nodes[0].unwrap().w, g.w);
    }
}
