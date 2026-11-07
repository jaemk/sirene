//! Finding what is drawn at a point (DISPLA-10).

use super::shapes::flatten;
use super::{Owner, Primitive, Pt, Scene, Segment};

impl Scene {
    /// The node, edge, or cluster drawn topmost at `p`: inside a filled path, or within `slop`
    /// of a stroked one (plus half its width). Text and the diagram's own elements are ignored.
    pub fn hit(&self, p: Pt, slop: f32) -> Option<Owner> {
        self.elements.iter().rev().find_map(|e| {
            let Primitive::Path {
                segments,
                fill,
                stroke,
            } = &e.primitive
            else {
                return None;
            };
            if e.owner == Owner::Diagram {
                return None;
            }
            let parts = subpaths(segments);
            let inside = fill.is_some() && parts.iter().any(|(pts, _)| contains(pts, p));
            let near = stroke.is_some_and(|s| {
                let reach = slop + s.width / 2.0;
                parts
                    .iter()
                    .any(|(pts, closed)| near(pts, *closed, p, reach))
            });
            (inside || near).then_some(e.owner)
        })
    }

    /// The bounding box (top left, bottom right) of the paths drawn for `owner`.
    pub fn bounds(&self, owner: Owner) -> Option<(Pt, Pt)> {
        let mut points = self
            .elements
            .iter()
            .filter(|e| e.owner == owner)
            .filter_map(|e| match &e.primitive {
                Primitive::Path { segments, .. } => Some(flatten(segments)),
                Primitive::Text { .. } => None,
            })
            .flatten();
        let first = points.next()?;
        Some(points.fold((first, first), |(lo, hi), q| {
            (
                Pt::new(lo.x.min(q.x), lo.y.min(q.y)),
                Pt::new(hi.x.max(q.x), hi.y.max(q.y)),
            )
        }))
    }
}

/// The flattened subpaths of a path, each with whether it is closed.
fn subpaths(segments: &[Segment]) -> Vec<(Vec<Pt>, bool)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, s) in segments.iter().enumerate() {
        if matches!(s, Segment::Move(_)) && i > start {
            out.push(&segments[start..i]);
            start = i;
        }
    }
    out.push(&segments[start..]);
    out.into_iter()
        .map(|part| {
            let closed = matches!(part.last(), Some(Segment::Close));
            (flatten(part), closed)
        })
        .filter(|(pts, _)| !pts.is_empty())
        .collect()
}

/// Even-odd containment in the polygon `pts`.
fn contains(pts: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether `p` is within `reach` of the polyline `pts` (closed back to its start when
/// `closed`).
fn near(pts: &[Pt], closed: bool, p: Pt, reach: f32) -> bool {
    let wrap = closed.then(|| (pts[pts.len() - 1], pts[0]));
    let mut pairs = pts.windows(2).map(|w| (w[0], w[1])).chain(wrap);
    if pts.len() == 1 {
        return distance(pts[0], pts[0], p) <= reach;
    }
    pairs.any(|(a, b)| distance(a, b, p) <= reach)
}

/// Distance from `p` to the segment from `a` to `b`.
fn distance(a: Pt, b: Pt, p: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let (x, y) = (a.x + t * dx - p.x, a.y + t * dy - p.y);
    (x * x + y * y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart;
    use crate::scene::{Approx, Theme};

    fn scene(src: &str) -> Scene {
        super::super::flowchart(
            &flowchart::parse(src).lower().flowchart,
            &Approx,
            &Theme::LIGHT,
        )
    }

    fn center(s: &Scene, owner: Owner) -> Pt {
        let (lo, hi) = s.bounds(owner).unwrap();
        Pt::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0)
    }

    #[test]
    fn nodes_edges_and_clusters() {
        let s =
            scene("---\ntitle: T\n---\nflowchart TD\n    subgraph G\n        A --> B\n    end\n");
        assert_eq!(s.hit(center(&s, Owner::Node(0)), 0.0), Some(Owner::Node(0)));
        assert_eq!(s.hit(center(&s, Owner::Node(1)), 0.0), Some(Owner::Node(1)));
        // The middle of the edge between the two nodes.
        let a = s.bounds(Owner::Node(0)).unwrap();
        let b = s.bounds(Owner::Node(1)).unwrap();
        let mid = Pt::new(center(&s, Owner::Node(0)).x, (a.1.y + b.0.y) / 2.0);
        assert_eq!(s.hit(mid, 2.0), Some(Owner::Edge(0)));
        // Beside the edge, inside the cluster.
        let beside = Pt::new(mid.x + 12.0, mid.y);
        assert_eq!(s.hit(beside, 2.0), Some(Owner::Cluster(0)));
        // The title and empty space hit nothing.
        assert_eq!(s.hit(Pt::new(s.width / 2.0, 20.0), 2.0), None);
        assert_eq!(s.hit(Pt::new(1.0, s.height - 1.0), 2.0), None);
        assert_eq!(s.bounds(Owner::Diagram), None);
    }

    #[test]
    fn edge_labels_hit_their_edge() {
        let s = scene("flowchart LR\n    A -->|a long label| B\n");
        let label = s
            .elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == "a long label" => Some(*center),
                _ => None,
            })
            .unwrap();
        assert_eq!(s.hit(label, 0.0), Some(Owner::Edge(0)));
    }

    #[test]
    fn geometry() {
        let square = [
            Pt::new(0.0, 0.0),
            Pt::new(10.0, 0.0),
            Pt::new(10.0, 10.0),
            Pt::new(0.0, 10.0),
        ];
        assert!(contains(&square, Pt::new(5.0, 5.0)));
        assert!(!contains(&square, Pt::new(15.0, 5.0)));
        assert!(near(&square, true, Pt::new(-1.0, 5.0), 1.5));
        assert!(!near(&square[..3], false, Pt::new(-1.0, 5.0), 1.5));
        assert!(near(&square[..1], false, Pt::new(1.0, 0.0), 1.5));
        assert_eq!(
            distance(Pt::new(0.0, 0.0), Pt::new(0.0, 0.0), Pt::new(3.0, 4.0)),
            5.0
        );
        let parts = subpaths(&[
            Segment::Move(Pt::new(0.0, 0.0)),
            Segment::Line(Pt::new(1.0, 0.0)),
            Segment::Close,
            Segment::Move(Pt::new(5.0, 5.0)),
            Segment::Line(Pt::new(6.0, 5.0)),
        ]);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].1 && !parts[1].1);
    }
}
