//! The treemap layout (MTREE-3), as d3's `treemap` with `treemapSquarify` lays it out for
//! Mermaid: items sorted by value, sections padded with room for a header, and every edge
//! rounded to a whole pixel.

use super::Treemap;

/// The space above a section's items, for its name.
pub const HEADER: f64 = 25.0;
/// The space inside a section's edges.
const SECTION_PADDING: f64 = 10.0;

/// An item's rectangle, from the drawing's top left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cell {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    /// Each edge pulled in, the sides meeting in the middle when they would cross.
    fn inset(self, left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        let (mut x0, mut y0, mut x1, mut y1) = (
            self.x0 + left,
            self.y0 + top,
            self.x1 - right,
            self.y1 - bottom,
        );
        if x1 < x0 {
            x0 = (x0 + x1) / 2.0;
            x1 = x0;
        }
        if y1 < y0 {
            y0 = (y0 + y1) / 2.0;
            y1 = y0;
        }
        Rect { x0, y0, x1, y1 }
    }
}

impl Treemap {
    /// Every item's cell, by index; a section's cell includes its header.
    pub fn layout(&self) -> Vec<Cell> {
        let values = self.values();
        let mut rects = vec![
            Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 0.0,
                y1: 0.0
            };
            self.items.len()
        ];
        let area = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: f64::from(self.width),
            y1: f64::from(self.height),
        };
        // The whole drawing is an unnamed section with no header and no edge padding.
        let roots = self.by_value(self.roots(), &values);
        let p = f64::from(self.padding) / 2.0;
        let inner = area.inset(-p, -p, -p, -p);
        let total: f64 = roots.iter().map(|&i| values[i]).sum();
        squarify(&roots, &values, total, inner, &mut rects);
        for &i in &roots {
            self.position(i, p, &values, &mut rects);
        }
        rects
            .into_iter()
            .map(|r| Cell {
                x0: r.x0.round() as f32,
                y0: r.y0.round() as f32,
                x1: r.x1.round() as f32,
                y1: r.y1.round() as f32,
            })
            .collect()
    }

    /// Pads item `i`'s tile by `p` on each side, then lays out its items inside.
    fn position(&self, i: usize, p: f64, values: &[f64], rects: &mut [Rect]) {
        let r = rects[i].inset(p, p, p, p);
        rects[i] = r;
        let item = &self.items[i];
        if item.children.is_empty() {
            return;
        }
        let q = f64::from(self.padding) / 2.0;
        let inner = r.inset(
            SECTION_PADDING - q,
            HEADER + SECTION_PADDING - q,
            SECTION_PADDING - q,
            SECTION_PADDING - q,
        );
        let children = self.by_value(item.children.iter().copied(), values);
        squarify(&children, values, values[i], inner, rects);
        for &c in &children {
            self.position(c, q, values, rects);
        }
    }
}

/// Tiles `r` with `ids` in rows of near-square cells, as d3's `treemapSquarify` with the golden
/// ratio.
fn squarify(ids: &[usize], values: &[f64], total: f64, r: Rect, out: &mut [Rect]) {
    let ratio = (1.0 + 5f64.sqrt()) / 2.0;
    let Rect {
        mut x0,
        mut y0,
        x1,
        y1,
    } = r;
    let mut value = total;
    let (mut i0, mut i1, n) = (0, 0, ids.len());
    while i0 < n {
        let (dx, dy) = (x1 - x0, y1 - y0);
        // The next item with a value starts the row.
        let mut sum;
        loop {
            sum = values[ids[i1]];
            i1 += 1;
            if sum != 0.0 || i1 >= n {
                break;
            }
        }
        let (mut min, mut max) = (sum, sum);
        let alpha = (dy / dx).max(dx / dy) / (value * ratio);
        let mut beta = sum * sum * alpha;
        let mut min_ratio = (max / beta).max(beta / min);
        // Add items while the row's worst aspect ratio holds or improves.
        while i1 < n {
            let v = values[ids[i1]];
            sum += v;
            min = min.min(v);
            max = max.max(v);
            beta = sum * sum * alpha;
            let new_ratio = (max / beta).max(beta / min);
            if new_ratio > min_ratio {
                sum -= v;
                break;
            }
            min_ratio = new_ratio;
            i1 += 1;
        }
        let row = &ids[i0..i1];
        if dx < dy {
            // A row across the top.
            let bottom = if value > 0.0 {
                y0 + dy * sum / value
            } else {
                y1
            };
            let k = if sum > 0.0 { (x1 - x0) / sum } else { 0.0 };
            let mut x = x0;
            for &id in row {
                let w = values[id] * k;
                out[id] = Rect {
                    x0: x,
                    y0,
                    x1: x + w,
                    y1: bottom,
                };
                x += w;
            }
            y0 = bottom;
        } else {
            // A column down the left.
            let right = if value > 0.0 {
                x0 + dx * sum / value
            } else {
                x1
            };
            let k = if sum > 0.0 { (y1 - y0) / sum } else { 0.0 };
            let mut y = y0;
            for &id in row {
                let h = values[id] * k;
                out[id] = Rect {
                    x0,
                    y0: y,
                    x1: right,
                    y1: y + h,
                };
                y += h;
            }
            x0 = right;
        }
        value -= sum;
        i0 = i1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treemap::Item;

    fn leaf(name: &str, v: f64, parent: Option<usize>) -> Item {
        Item {
            name: name.into(),
            value: Some(v),
            parent,
            ..Item::default()
        }
    }

    fn area(c: &Cell) -> f32 {
        (c.x1 - c.x0) * (c.y1 - c.y0)
    }

    #[test]
    fn areas_follow_values() {
        let t = Treemap {
            items: vec![
                leaf("a", 6.0, None),
                leaf("b", 3.0, None),
                leaf("c", 1.0, None),
            ],
            padding: 0.0,
            width: 100.0,
            height: 100.0,
            ..Treemap::default()
        };
        let cells = t.layout();
        let total: f32 = cells.iter().map(area).sum();
        assert!((total - 10_000.0).abs() < 1.0);
        assert!((area(&cells[0]) - 6000.0).abs() < 150.0);
        assert!((area(&cells[2]) - 1000.0).abs() < 150.0);
    }

    #[test]
    fn sections_hold_their_items_below_a_header() {
        let mut s = Item {
            name: "s".into(),
            children: vec![1, 2],
            ..Item::default()
        };
        s.value = None;
        let t = Treemap {
            items: vec![s, leaf("a", 1.0, Some(0)), leaf("b", 1.0, Some(0))],
            ..Treemap::default()
        };
        let c = t.layout();
        // The drawing has no edge padding of its own.
        assert_eq!(
            (c[0].x0, c[0].y0, c[0].x1, c[0].y1),
            (0.0, 0.0, 1000.0, 400.0)
        );
        for leaf in &c[1..] {
            assert!(leaf.y0 >= c[0].y0 + HEADER as f32);
            assert!(leaf.x0 >= c[0].x0 && leaf.x1 <= c[0].x1 && leaf.y1 <= c[0].y1);
        }
        assert!(c[1].x1 <= c[2].x0 || c[2].x1 <= c[1].x0 || c[1].y1 <= c[2].y0);
    }
}
