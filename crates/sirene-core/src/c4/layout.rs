//! Places a C4 diagram (MC4-3) as Mermaid does: in the diagram and in each boundary, the
//! elements in rows of `shapes_in_row`, then the boundaries below in rows of
//! `boundaries_in_row`, all from the left. Units are the caller's: pixels or characters.

use super::C4;

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

/// What `layout` needs to know about sizes.
pub struct Sizes<'a> {
    /// An element's box.
    pub node: &'a dyn Fn(usize) -> (f64, f64),
    /// A boundary's heading: its size, at the top.
    pub title: &'a dyn Fn(usize) -> (f64, f64),
    /// Between items.
    pub gap_x: f64,
    pub gap_y: f64,
    /// Inside a boundary's border.
    pub pad: f64,
}

/// Where everything goes, with the top-left of the whole at the origin.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<Rect>,
    pub boundaries: Vec<Rect>,
}

struct Ctx<'a> {
    c4: &'a C4,
    s: &'a Sizes<'a>,
    placed: Placed,
}

impl Ctx<'_> {
    /// Places the contents of `container` with its top-left at `(x, y)`; returns their size.
    fn place(&mut self, container: Option<usize>, x: f64, y: f64) -> (f64, f64) {
        let c4 = self.c4;
        let nodes: Vec<usize> = (0..c4.chart.nodes.len())
            .filter(|&n| c4.node_parent[n] == container)
            .collect();
        let groups: Vec<usize> = (0..c4.chart.subgraphs.len())
            .filter(|&g| c4.boundary_parent[g] == container && Some(g) != container)
            .collect();
        let (mut w, mut cy) = (0.0f64, y);
        let mut first_row = true;
        for row in nodes.chunks(c4.shapes_in_row.max(1)) {
            if !first_row {
                cy += self.s.gap_y;
            }
            first_row = false;
            let mut cx = x;
            let mut h = 0.0f64;
            for (k, &n) in row.iter().enumerate() {
                if k > 0 {
                    cx += self.s.gap_x;
                }
                let (nw, nh) = (self.s.node)(n);
                self.placed.nodes[n] = Rect {
                    x: cx,
                    y: cy,
                    w: nw,
                    h: nh,
                };
                cx += nw;
                h = h.max(nh);
            }
            w = w.max(cx - x);
            cy += h;
        }
        for row in groups.chunks(c4.boundaries_in_row.max(1)) {
            if !first_row {
                cy += self.s.gap_y;
            }
            first_row = false;
            let mut cx = x;
            let mut h = 0.0f64;
            for (k, &g) in row.iter().enumerate() {
                if k > 0 {
                    cx += self.s.gap_x;
                }
                let (tw, th) = (self.s.title)(g);
                let pad = self.s.pad;
                let (iw, ih) = self.place(Some(g), cx + pad, cy + pad + th);
                let gw = (iw.max(tw) + 2.0 * pad).max(2.0 * pad);
                let gh = ih + th + 2.0 * pad;
                self.placed.boundaries[g] = Rect {
                    x: cx,
                    y: cy,
                    w: gw,
                    h: gh,
                };
                cx += gw;
                h = h.max(gh);
            }
            w = w.max(cx - x);
            cy += h;
        }
        (w, cy - y)
    }
}

/// Places every element and boundary of `c4`.
pub fn layout(c4: &C4, s: &Sizes) -> Placed {
    let mut ctx = Ctx {
        c4,
        s,
        placed: Placed {
            nodes: vec![Rect::default(); c4.chart.nodes.len()],
            boundaries: vec![Rect::default(); c4.chart.subgraphs.len()],
            ..Placed::default()
        },
    };
    let (w, h) = ctx.place(None, 0.0, 0.0);
    ctx.placed.width = w;
    ctx.placed.height = h;
    ctx.placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{self, Diagram};

    fn placed(src: &str) -> Placed {
        let Diagram::C4(c) = diagram::lower(src).diagram else {
            panic!("not a C4 diagram")
        };
        layout(
            &c,
            &Sizes {
                node: &|_| (10.0, 5.0),
                title: &|_| (4.0, 2.0),
                gap_x: 2.0,
                gap_y: 1.0,
                pad: 1.0,
            },
        )
    }

    #[test]
    fn elements_fill_rows() {
        let p = placed(
            "C4Context\nUpdateLayoutConfig($c4ShapeInRow=\"2\")\nPerson(a, A)\nPerson(b, B)\n\
             Person(c, C)\n",
        );
        let at: Vec<(f64, f64)> = p.nodes.iter().map(|r| (r.x, r.y)).collect();
        assert_eq!(at, [(0.0, 0.0), (12.0, 0.0), (0.0, 6.0)]);
        assert_eq!((p.width, p.height), (22.0, 11.0));
    }

    #[test]
    fn boundaries_go_below() {
        let p = placed(
            "C4Context\nPerson(a, A)\nBoundary(b, B) {\n  System(s, S)\n}\nBoundary(c, C) {\n}\n",
        );
        assert_eq!(
            p.boundaries[0],
            Rect {
                x: 0.0,
                y: 6.0,
                w: 12.0,
                h: 9.0
            }
        );
        assert_eq!((p.nodes[1].x, p.nodes[1].y), (1.0, 9.0));
        // An empty boundary is as wide as its title.
        assert_eq!(
            p.boundaries[1],
            Rect {
                x: 14.0,
                y: 6.0,
                w: 6.0,
                h: 4.0
            }
        );
    }
}
