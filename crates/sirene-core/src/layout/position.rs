//! Cross-axis positions within ranks (LAYOUT-6).

const ROUNDS: usize = 4;

/// How hard a floating element pulls toward the center, against the elements with edges in
/// its layer: next to nothing, so a cluster title gives way to the border ports beside it.
const FLOATING: f64 = 1e-3;

/// Cross-axis extent of an element around its anchor.
#[derive(Debug, Clone, Copy)]
pub(super) struct Span {
    /// Cells left of the anchor.
    pub left: i32,
    /// Cells right of the anchor.
    pub right: i32,
    /// An edge dummy, label-less proxy, or other one-cell line element.
    pub line: bool,
    /// Has no edges in this container and is pulled toward the content center.
    pub floating: bool,
}

/// A segment between consecutive layers, with its port offsets from each end's anchor.
#[derive(Debug, Clone, Copy)]
pub(super) struct Link {
    pub upper: usize,
    pub lower: usize,
    pub upper_off: i32,
    pub lower_off: i32,
    /// Part of an edge kept straight before others, between two nodes of one group.
    pub heavy: bool,
}

/// How much harder a heavy link pulls, as in Graphviz.
const HEAVY: f64 = 100.0;

#[derive(Clone, Copy)]
enum Toward {
    Up,
    Down,
    Both,
}

struct Placer<'a> {
    spans: &'a [Span],
    links: &'a [Link],
    up: Vec<Vec<usize>>,
    down: Vec<Vec<usize>>,
    node_gap: i32,
    edge_gap: i32,
    x: Vec<f64>,
}

/// Returns each element's anchor. Elements in a layer keep their order and are separated by
/// `node_gap`, or `edge_gap` between two line elements.
pub(super) fn position(
    layers: &[Vec<usize>],
    spans: &[Span],
    links: &[Link],
    node_gap: i32,
    edge_gap: i32,
) -> Vec<i32> {
    let n = spans.len();
    let mut up = vec![Vec::new(); n];
    let mut down = vec![Vec::new(); n];
    for (i, l) in links.iter().enumerate() {
        up[l.lower].push(i);
        down[l.upper].push(i);
    }
    let mut p = Placer {
        spans,
        links,
        up,
        down,
        node_gap,
        edge_gap,
        x: vec![0.0; n],
    };
    for layer in layers {
        let mut at = 0;
        for (i, &e) in layer.iter().enumerate() {
            if i > 0 {
                at += p.sep(layer[i - 1], e);
            }
            p.x[e] = f64::from(at);
        }
    }
    for _ in 0..ROUNDS {
        for layer in layers.iter().skip(1) {
            p.place(layer, Toward::Up);
        }
        for layer in layers.iter().rev().skip(1) {
            p.place(layer, Toward::Down);
        }
    }
    for layer in layers {
        p.place(layer, Toward::Both);
    }
    let mut anchors = vec![0; n];
    for layer in layers {
        for (i, &e) in layer.iter().enumerate() {
            // Half up, so positions an integer apart round the same way on either side of 0.
            let mut a = (p.x[e] + 0.5).floor() as i32;
            if i > 0 {
                let prev = layer[i - 1];
                a = a.max(anchors[prev] + p.sep(prev, e));
            }
            anchors[e] = a;
        }
    }
    anchors
}

impl Placer<'_> {
    /// Minimum anchor distance between neighbors `a` (left) and `b`.
    fn sep(&self, a: usize, b: usize) -> i32 {
        let (sa, sb) = (self.spans[a], self.spans[b]);
        let gap = if sa.line && sb.line {
            self.edge_gap
        } else {
            self.node_gap
        };
        sa.right + sb.left + 1 + gap
    }

    fn weight(&self, l: &Link) -> f64 {
        let w = match (self.spans[l.upper].line, self.spans[l.lower].line) {
            (true, true) => 8.0,
            (false, false) => 1.0,
            _ => 2.0,
        };
        if l.heavy { w * HEAVY } else { w }
    }

    /// Midpoint of the extent of all non-floating elements.
    fn center(&self) -> f64 {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (e, s) in self.spans.iter().enumerate() {
            if !s.floating {
                lo = lo.min(self.x[e] - f64::from(s.left));
                hi = hi.max(self.x[e] + f64::from(s.right));
            }
        }
        if lo.is_finite() { (lo + hi) / 2.0 } else { 0.0 }
    }

    fn place(&mut self, layer: &[usize], toward: Toward) {
        if layer.is_empty() {
            return;
        }
        // Only layers with a floating element need the center, which costs a pass over all
        // elements.
        let center = if layer.iter().any(|&e| self.spans[e].floating) {
            self.center()
        } else {
            0.0
        };
        let mut desired = Vec::with_capacity(layer.len());
        let mut weights = Vec::with_capacity(layer.len());
        for &e in layer {
            let mut values = Vec::new();
            if matches!(toward, Toward::Up | Toward::Both) {
                for &i in &self.up[e] {
                    let l = &self.links[i];
                    let v = self.x[l.upper] + f64::from(l.upper_off - l.lower_off);
                    values.push((v, self.weight(l)));
                }
            }
            if matches!(toward, Toward::Down | Toward::Both) {
                for &i in &self.down[e] {
                    let l = &self.links[i];
                    let v = self.x[l.lower] + f64::from(l.lower_off - l.upper_off);
                    values.push((v, self.weight(l)));
                }
            }
            let (d, w) = if self.spans[e].floating {
                (center, FLOATING)
            } else if values.is_empty() {
                (self.x[e], 0.01)
            } else {
                let total = values.iter().map(|v| v.1).sum();
                (weighted_median(&mut values), total)
            };
            desired.push(d);
            weights.push(w);
        }
        let seps: Vec<f64> = layer
            .windows(2)
            .map(|w| f64::from(self.sep(w[0], w[1])))
            .collect();
        for (&e, x) in layer.iter().zip(project(&desired, &weights, &seps)) {
            self.x[e] = x;
        }
        self.compact(layer);
    }

    fn isolated(&self, e: usize) -> bool {
        !self.spans[e].floating && self.up[e].is_empty() && self.down[e].is_empty()
    }

    /// Packs elements without links against their neighbors, so they do not stay where earlier
    /// passes pushed them: toward the previous element, or the next one for a leading run.
    fn compact(&mut self, layer: &[usize]) {
        let lead = layer
            .iter()
            .position(|&e| !self.isolated(e))
            .unwrap_or(layer.len());
        for i in (0..lead.min(layer.len().saturating_sub(1))).rev() {
            self.x[layer[i]] = self.x[layer[i + 1]] - f64::from(self.sep(layer[i], layer[i + 1]));
        }
        for i in lead.max(1)..layer.len() {
            if self.isolated(layer[i]) {
                self.x[layer[i]] =
                    self.x[layer[i - 1]] + f64::from(self.sep(layer[i - 1], layer[i]));
            }
        }
    }
}

/// The weighted median of `(value, weight)` pairs; an exact tie averages the two middle values.
fn weighted_median(values: &mut [(f64, f64)]) -> f64 {
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let half = values.iter().map(|v| v.1).sum::<f64>() / 2.0;
    let mut acc = 0.0;
    for i in 0..values.len() - 1 {
        acc += values[i].1;
        if acc == half {
            return (values[i].0 + values[i + 1].0) / 2.0;
        }
        if acc > half {
            return values[i].0;
        }
    }
    values[values.len() - 1].0
}

/// Positions closest (weighted least squares) to `desired` with `x[i+1] - x[i] >= seps[i]`,
/// by pool-adjacent-violators on the separation-shifted targets.
fn project(desired: &[f64], weights: &[f64], seps: &[f64]) -> Vec<f64> {
    let mut offsets = vec![0.0; desired.len()];
    for i in 1..desired.len() {
        offsets[i] = offsets[i - 1] + seps[i - 1];
    }
    // (weighted sum, total weight, count)
    let mut blocks: Vec<(f64, f64, usize)> = Vec::new();
    for i in 0..desired.len() {
        blocks.push((weights[i] * (desired[i] - offsets[i]), weights[i], 1));
        while blocks.len() >= 2 {
            let b = blocks[blocks.len() - 1];
            let a = blocks[blocks.len() - 2];
            if a.0 / a.1 <= b.0 / b.1 {
                break;
            }
            blocks.pop();
            *blocks.last_mut().expect("two blocks") = (a.0 + b.0, a.1 + b.1, a.2 + b.2);
        }
    }
    let mut out = Vec::with_capacity(desired.len());
    for (sum, weight, count) in blocks {
        for _ in 0..count {
            out.push(sum / weight + offsets[out.len()]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(w: i32) -> Span {
        Span {
            left: w / 2,
            right: w - 1 - w / 2,
            line: false,
            floating: false,
        }
    }

    fn link(upper: usize, lower: usize) -> Link {
        Link {
            upper,
            lower,
            upper_off: 0,
            lower_off: 0,
            heavy: false,
        }
    }

    #[test]
    fn heavy_links_win() {
        // 0 and 1 above 2: 2 sits under the middle, or under 1 when its link is heavy.
        let layers = vec![vec![0, 1], vec![2]];
        let spans = [node(5), node(5), node(5)];
        let x = position(&layers, &spans, &[link(0, 2), link(1, 2)], 3, 1);
        assert_eq!(x[2] * 2, x[0] + x[1]);
        let heavy = Link {
            heavy: true,
            ..link(1, 2)
        };
        let x = position(&layers, &spans, &[link(0, 2), heavy], 3, 1);
        assert_eq!(x[2], x[1]);
    }

    #[test]
    fn projection_keeps_separation() {
        assert_eq!(project(&[0.0, 0.0], &[1.0, 1.0], &[4.0]), [-2.0, 2.0]);
        assert_eq!(project(&[0.0, 10.0], &[1.0, 1.0], &[4.0]), [0.0, 10.0]);
        assert_eq!(project(&[0.0, 0.0], &[3.0, 1.0], &[4.0]), [-1.0, 3.0]);
        assert!(project(&[], &[], &[]).is_empty());
    }

    #[test]
    fn medians() {
        assert_eq!(weighted_median(&mut [(4.0, 1.0)]), 4.0);
        assert_eq!(weighted_median(&mut [(10.0, 1.0), (0.0, 1.0)]), 5.0);
        assert_eq!(weighted_median(&mut [(0.0, 1.0), (10.0, 8.0)]), 10.0);
        assert_eq!(
            weighted_median(&mut [(0.0, 1.0), (1.0, 1.0), (9.0, 1.0)]),
            1.0
        );
    }

    #[test]
    fn parent_centers_over_children() {
        // 0 above 1 and 2.
        let layers = vec![vec![0], vec![1, 2]];
        let spans = [node(5), node(5), node(5)];
        let x = position(&layers, &spans, &[link(0, 1), link(0, 2)], 3, 1);
        assert_eq!(x[2] - x[1], 8);
        assert_eq!(x[0] * 2, x[1] + x[2]);
    }

    #[test]
    fn chain_is_straight_and_ports_align() {
        let layers = vec![vec![0], vec![1]];
        let spans = [node(7), node(4)];
        let l = Link {
            upper: 0,
            lower: 1,
            upper_off: 2,
            lower_off: -1,
            heavy: false,
        };
        let x = position(&layers, &spans, &[l], 3, 1);
        assert_eq!(x[0] + 2, x[1] - 1);
    }

    #[test]
    fn floating_elements_move_to_the_center() {
        let layers = vec![vec![0, 1, 2], vec![3]];
        let mut spans = vec![node(3); 4];
        spans[3].floating = true;
        spans[3].line = true;
        let x = position(&layers, &spans, &[], 3, 1);
        assert_eq!(x[3], x[1]);
    }

    #[test]
    fn line_elements_use_the_edge_gap() {
        let line = Span {
            left: 0,
            right: 0,
            line: true,
            floating: false,
        };
        let layers = vec![vec![0, 1], vec![2, 3]];
        let spans = [node(3), node(3), line, line];
        let x = position(&layers, &spans, &[link(0, 2), link(1, 3)], 3, 1);
        assert!(x[3] - x[2] >= 2);
        assert_eq!(x[1] - x[0], 6);
    }
}
