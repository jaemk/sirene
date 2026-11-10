//! A port of d3-sankey's layout, which Mermaid uses (MSANKEY-3): nodes in columns by depth,
//! heights by value, and six passes that pull each node toward the nodes it links to.

use super::{Align, Sankey};

/// Passes that relax node positions, as in d3-sankey.
const ITERATIONS: usize = 6;

/// A node's box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeBox {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    /// The larger of what flows in and what flows out.
    pub value: f64,
    /// The column it sits in.
    pub layer: usize,
}

/// A link's band: its width, and the middle of its ends on the source and target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkBand {
    pub width: f32,
    pub y0: f32,
    pub y1: f32,
}

/// Where everything sits, in a `width` by `height` box from the origin.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub nodes: Vec<NodeBox>,
    /// One per link, in the model's order.
    pub links: Vec<LinkBand>,
    /// The node indices of each column, top to bottom.
    pub columns: Vec<Vec<usize>>,
}

struct State<'a> {
    s: &'a Sankey,
    nodes: Vec<NodeBox>,
    widths: Vec<f32>,
    /// Each node's outgoing and incoming links, by link index.
    out: Vec<Vec<usize>>,
    into: Vec<Vec<usize>>,
    py: f32,
    height: f32,
}

/// Steps from each node along `next` until none are left: a node's depth, or its height when
/// walking backward. Gives up past the node count, which only a cycle reaches.
fn levels(n: usize, starts: impl Fn(usize) -> Vec<usize>) -> Vec<usize> {
    let mut level = vec![0; n];
    let mut current: Vec<usize> = (0..n).collect();
    let mut x = 0;
    while !current.is_empty() && x <= n {
        let mut next = Vec::new();
        let mut seen = vec![false; n];
        for &i in &current {
            level[i] = x;
            for j in starts(i) {
                if !seen[j] {
                    seen[j] = true;
                    next.push(j);
                }
            }
        }
        x += 1;
        current = next;
    }
    level
}

impl Sankey {
    /// The layout in a `self.width` by `self.height` box (MSANKEY-3).
    pub fn layout(&self) -> Layout {
        let n = self.nodes.len();
        let mut out = vec![Vec::new(); n];
        let mut into = vec![Vec::new(); n];
        for (k, l) in self.links.iter().enumerate() {
            out[l.source].push(k);
            into[l.target].push(k);
        }
        let sum = |ls: &[usize]| ls.iter().map(|&k| self.links[k].value).sum::<f64>();
        let value: Vec<f64> = (0..n).map(|i| sum(&out[i]).max(sum(&into[i]))).collect();
        let depth = levels(n, |i| {
            out[i].iter().map(|&k| self.links[k].target).collect()
        });
        let height = levels(n, |i| {
            into[i].iter().map(|&k| self.links[k].source).collect()
        });

        // Columns.
        let x = depth.iter().max().map_or(1, |d| d + 1);
        let dx = self.node_width;
        let kx = if x > 1 {
            (self.width - dx) / (x - 1) as f32
        } else {
            0.0
        };
        let mut columns: Vec<Vec<usize>> = vec![Vec::new(); x];
        let mut nodes = Vec::with_capacity(n);
        for i in 0..n {
            let layer = match self.align {
                Align::Left => depth[i],
                Align::Right => (x - 1).saturating_sub(height[i]),
                Align::Justify => {
                    if out[i].is_empty() {
                        x - 1
                    } else {
                        depth[i]
                    }
                }
                Align::Center => {
                    if !into[i].is_empty() {
                        depth[i]
                    } else if !out[i].is_empty() {
                        out[i]
                            .iter()
                            .map(|&k| depth[self.links[k].target])
                            .min()
                            .unwrap_or(1)
                            .saturating_sub(1)
                    } else {
                        0
                    }
                }
            }
            .min(x - 1);
            columns[layer].push(i);
            let x0 = layer as f32 * kx;
            nodes.push(NodeBox {
                x0,
                x1: x0 + dx,
                y0: 0.0,
                y1: 0.0,
                value: value[i],
                layer,
            });
        }

        let longest = columns.iter().map(Vec::len).max().unwrap_or(1);
        let padding = self.padding();
        let py = if longest > 1 {
            padding.min(self.height / (longest - 1) as f32)
        } else {
            padding
        };
        let mut st = State {
            s: self,
            nodes,
            widths: vec![0.0; self.links.len()],
            out,
            into,
            py,
            height: self.height,
        };
        st.initialize(&columns);
        for i in 0..ITERATIONS {
            let alpha = 0.99f32.powi(i as i32);
            let beta = (1.0 - alpha).max((i + 1) as f32 / ITERATIONS as f32);
            st.relax_right_to_left(&mut columns, alpha, beta);
            st.relax_left_to_right(&mut columns, alpha, beta);
        }
        let links = st.link_breadths();
        Layout {
            nodes: st.nodes,
            links,
            columns,
        }
    }
}

impl State<'_> {
    fn initialize(&mut self, columns: &[Vec<usize>]) {
        let ky = columns
            .iter()
            .filter(|c| !c.is_empty())
            .map(|c| {
                let total: f64 = c.iter().map(|&i| self.nodes[i].value).sum();
                (self.height - (c.len() - 1) as f32 * self.py) / total as f32
            })
            .filter(|k| k.is_finite())
            .fold(f32::INFINITY, f32::min);
        let ky = if ky.is_finite() { ky.max(0.0) } else { 0.0 };
        for (k, l) in self.s.links.iter().enumerate() {
            self.widths[k] = l.value as f32 * ky;
        }
        for column in columns {
            let mut y = 0.0;
            for &i in column {
                let node = &mut self.nodes[i];
                node.y0 = y;
                node.y1 = y + node.value as f32 * ky;
                y = node.y1 + self.py;
            }
            let gap = (self.height - y + self.py) / (column.len() + 1) as f32;
            for (k, &i) in column.iter().enumerate() {
                self.nodes[i].y0 += gap * (k + 1) as f32;
                self.nodes[i].y1 += gap * (k + 1) as f32;
            }
            for &i in column {
                self.sort_links(i);
            }
        }
    }

    /// Orders a node's outgoing links by their targets' tops, and incoming by their sources'.
    fn sort_links(&mut self, i: usize) {
        let links = &self.s.links;
        let nodes = &self.nodes;
        self.out[i].sort_by(|&a, &b| {
            nodes[links[a].target]
                .y0
                .total_cmp(&nodes[links[b].target].y0)
                .then(a.cmp(&b))
        });
        self.into[i].sort_by(|&a, &b| {
            nodes[links[a].source]
                .y0
                .total_cmp(&nodes[links[b].source].y0)
                .then(a.cmp(&b))
        });
    }

    /// After moving node `i`, re-sorts the links of the nodes on its links' other ends.
    fn reorder_node_links(&mut self, i: usize) {
        let sources: Vec<usize> = self.into[i]
            .iter()
            .map(|&k| self.s.links[k].source)
            .collect();
        let targets: Vec<usize> = self.out[i]
            .iter()
            .map(|&k| self.s.links[k].target)
            .collect();
        let links = &self.s.links;
        for s in sources {
            let nodes = &self.nodes;
            self.out[s].sort_by(|&a, &b| {
                nodes[links[a].target]
                    .y0
                    .total_cmp(&nodes[links[b].target].y0)
                    .then(a.cmp(&b))
            });
        }
        for t in targets {
            let nodes = &self.nodes;
            self.into[t].sort_by(|&a, &b| {
                nodes[links[a].source]
                    .y0
                    .total_cmp(&nodes[links[b].source].y0)
                    .then(a.cmp(&b))
            });
        }
    }

    /// Where a link from `source` would meet the top of `target`, given its place among the
    /// source's links.
    fn target_top(&self, source: usize, target: usize) -> f32 {
        let links = &self.s.links;
        let mut y = self.nodes[source].y0 - (self.out[source].len() as f32 - 1.0) * self.py / 2.0;
        for &k in &self.out[source] {
            if links[k].target == target {
                break;
            }
            y += self.widths[k] + self.py;
        }
        for &k in &self.into[target] {
            if links[k].source == source {
                break;
            }
            y -= self.widths[k];
        }
        y
    }

    fn source_top(&self, source: usize, target: usize) -> f32 {
        let links = &self.s.links;
        let mut y = self.nodes[target].y0 - (self.into[target].len() as f32 - 1.0) * self.py / 2.0;
        for &k in &self.into[target] {
            if links[k].source == source {
                break;
            }
            y += self.widths[k] + self.py;
        }
        for &k in &self.out[source] {
            if links[k].target == target {
                break;
            }
            y -= self.widths[k];
        }
        y
    }

    fn shift(&mut self, i: usize, dy: f32) {
        self.nodes[i].y0 += dy;
        self.nodes[i].y1 += dy;
    }

    fn relax_left_to_right(&mut self, columns: &mut [Vec<usize>], alpha: f32, beta: f32) {
        for column in columns.iter_mut().skip(1) {
            for &t in column.iter() {
                let (mut y, mut w) = (0.0f32, 0.0f32);
                for &k in &self.into[t] {
                    let s = self.s.links[k].source;
                    let v = self.s.links[k].value as f32
                        * (self.nodes[t].layer as f32 - self.nodes[s].layer as f32);
                    y += self.target_top(s, t) * v;
                    w += v;
                }
                if w > 0.0 {
                    let dy = (y / w - self.nodes[t].y0) * alpha;
                    self.shift(t, dy);
                    self.reorder_node_links(t);
                }
            }
            self.sort_column(column);
            self.resolve_collisions(column, beta);
        }
    }

    fn relax_right_to_left(&mut self, columns: &mut [Vec<usize>], alpha: f32, beta: f32) {
        for c in (0..columns.len().saturating_sub(1)).rev() {
            for &s in &columns[c] {
                let (mut y, mut w) = (0.0f32, 0.0f32);
                for &k in &self.out[s] {
                    let t = self.s.links[k].target;
                    let v = self.s.links[k].value as f32
                        * (self.nodes[t].layer as f32 - self.nodes[s].layer as f32);
                    y += self.source_top(s, t) * v;
                    w += v;
                }
                if w > 0.0 {
                    let dy = (y / w - self.nodes[s].y0) * alpha;
                    self.shift(s, dy);
                    self.reorder_node_links(s);
                }
            }
            self.sort_column(&mut columns[c]);
            self.resolve_collisions(&columns[c], beta);
        }
    }

    fn sort_column(&self, column: &mut [usize]) {
        column.sort_by(|&a, &b| self.nodes[a].y0.total_cmp(&self.nodes[b].y0));
    }

    fn resolve_collisions(&mut self, column: &[usize], alpha: f32) {
        if column.is_empty() {
            return;
        }
        let mid = column.len() / 2;
        let subject = self.nodes[column[mid]];
        self.bottom_to_top(column, subject.y0 - self.py, mid as isize - 1, alpha);
        self.top_to_bottom(column, subject.y1 + self.py, mid + 1, alpha);
        self.bottom_to_top(column, self.height, column.len() as isize - 1, alpha);
        self.top_to_bottom(column, 0.0, 0, alpha);
    }

    /// Pushes nodes down from `i` so each starts at least at `y`.
    fn top_to_bottom(&mut self, column: &[usize], mut y: f32, from: usize, alpha: f32) {
        for &i in &column[from.min(column.len())..] {
            let dy = (y - self.nodes[i].y0) * alpha;
            if dy > 1e-6 {
                self.shift(i, dy);
            }
            y = self.nodes[i].y1 + self.py;
        }
    }

    /// Pushes nodes up from `i` so each ends at most at `y`.
    fn bottom_to_top(&mut self, column: &[usize], mut y: f32, from: isize, alpha: f32) {
        let mut k = from;
        while k >= 0 {
            let i = column[k as usize];
            let dy = (self.nodes[i].y1 - y) * alpha;
            if dy > 1e-6 {
                self.shift(i, -dy);
            }
            y = self.nodes[i].y0 - self.py;
            k -= 1;
        }
    }

    fn link_breadths(&self) -> Vec<LinkBand> {
        let mut bands: Vec<LinkBand> = self
            .widths
            .iter()
            .map(|&width| LinkBand {
                width,
                y0: 0.0,
                y1: 0.0,
            })
            .collect();
        for (i, node) in self.nodes.iter().enumerate() {
            let mut y = node.y0;
            for &k in &self.out[i] {
                bands[k].y0 = y + bands[k].width / 2.0;
                y += bands[k].width;
            }
            let mut y = node.y0;
            for &k in &self.into[i] {
                bands[k].y1 = y + bands[k].width / 2.0;
                y += bands[k].width;
            }
        }
        bands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sankey::Link;

    fn sankey(links: &[(usize, usize, f64)], n: usize) -> Sankey {
        Sankey {
            nodes: (0..n).map(|i| format!("n{i}")).collect(),
            links: links
                .iter()
                .map(|&(source, target, value)| Link {
                    source,
                    target,
                    value,
                })
                .collect(),
            ..Sankey::default()
        }
    }

    #[test]
    fn levels_and_columns() {
        // 0 -> 1 -> 2, and 3 -> 2: justified, 3 sits in the first column.
        let s = sankey(&[(0, 1, 5.0), (1, 2, 5.0), (3, 2, 2.0)], 4);
        let l = s.layout();
        let layers: Vec<usize> = l.nodes.iter().map(|n| n.layer).collect();
        assert_eq!(layers, [0, 1, 2, 0]);
        assert_eq!(l.columns.len(), 3);
        assert_eq!(l.nodes[2].value, 7.0);
        assert_eq!(l.nodes[0].x0, 0.0);
        assert_eq!(l.nodes[2].x1, s.width);
        // Right alignment moves 3 next to its target.
        let r = Sankey {
            align: Align::Right,
            ..s.clone()
        };
        assert_eq!(r.layout().nodes[3].layer, 1);
        let c = Sankey {
            align: Align::Center,
            ..s
        };
        assert_eq!(c.layout().nodes[3].layer, 1);
    }

    #[test]
    fn heights_follow_values() {
        let s = sankey(&[(0, 1, 30.0), (0, 2, 10.0)], 3);
        let l = s.layout();
        let h = |i: usize| l.nodes[i].y1 - l.nodes[i].y0;
        assert!((h(0) - h(1) - h(2)).abs() < 0.01);
        assert!((h(1) - 3.0 * h(2)).abs() < 0.01);
        // Inside the box, and the links fill their ends.
        for n in &l.nodes {
            assert!(n.y0 >= -0.01 && n.y1 <= s.height + 0.01, "{n:?}");
        }
        assert!((l.links[0].width + l.links[1].width - h(0)).abs() < 0.01);
        assert!(l.links[0].y0 < l.links[1].y0 || l.nodes[1].y0 > l.nodes[2].y0);
    }

    #[test]
    fn empty() {
        let l = Sankey::default().layout();
        assert!(l.nodes.is_empty() && l.links.is_empty());
    }
}
