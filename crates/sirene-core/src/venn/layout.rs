//! Circle placement for venn diagrams (MVENN-3): a port of venn.js, the layout Mermaid uses.
//!
//! Each set is a circle whose area is its size. Circles are placed greedily so that each pair
//! overlaps by its overlap's size, refined with Nelder-Mead, rotated and mirrored into a standard
//! orientation, and scaled to fit the drawing. A region's label goes at the point inside it
//! farthest from its edges.

use std::collections::HashMap;
use std::f64::consts::PI;

const SMALL: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

impl Circle {
    fn center(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

/// One arc of a region's outline: along `circle` from `p2` to `p1`, clockwise on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub circle: Circle,
    pub p1: Point,
    pub p2: Point,
    /// Whether it is the larger of the two arcs between its ends.
    pub large: bool,
    width: f64,
}

/// A set or overlap to lay out: the set names and the region's size.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    pub sets: Vec<String>,
    pub size: f64,
}

/// A laid-out region.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// The outline: one whole circle, or arcs from circle to circle.
    pub arcs: Vec<Arc>,
    /// Where its label goes; `None` when the circles do not all overlap.
    pub text: Option<Point>,
    /// Its circles, in the order of its sets.
    pub circles: Vec<Circle>,
}

fn distance(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// The area of a circular segment of `width` in a circle of radius `r`.
fn segment_area(r: f64, width: f64) -> f64 {
    r * r * (1.0 - width / r).acos() - (r - width) * (width * (2.0 * r - width)).sqrt()
}

/// The overlap of two circles of radii `r1` and `r2` whose centers are `d` apart.
fn circle_overlap(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        return PI * r1.min(r2) * r1.min(r2);
    }
    let w1 = r1 - (d * d - r2 * r2 + r1 * r1) / (2.0 * d);
    let w2 = r2 - (d * d - r1 * r1 + r2 * r2) / (2.0 * d);
    segment_area(r1, w1) + segment_area(r2, w2)
}

/// Where two circles cross: none when apart or one contains the other.
fn circle_circle_intersection(p1: Circle, p2: Circle) -> Vec<Point> {
    let d = distance(p1.center(), p2.center());
    let (r1, r2) = (p1.radius, p2.radius);
    if d >= r1 + r2 || d <= (r1 - r2).abs() {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).sqrt();
    let x0 = p1.x + a * (p2.x - p1.x) / d;
    let y0 = p1.y + a * (p2.y - p1.y) / d;
    let rx = -(p2.y - p1.y) * (h / d);
    let ry = -(p2.x - p1.x) * (h / d);
    vec![
        Point {
            x: x0 + rx,
            y: y0 - ry,
        },
        Point {
            x: x0 - rx,
            y: y0 + ry,
        },
    ]
}

fn center_of(points: &[Point]) -> Point {
    let n = points.len() as f64;
    Point {
        x: points.iter().map(|p| p.x).sum::<f64>() / n,
        y: points.iter().map(|p| p.y).sum::<f64>() / n,
    }
}

/// The area common to all `circles`, and the arcs outlining it.
fn intersection_area(circles: &[Circle]) -> (f64, Vec<Arc>) {
    struct Inner {
        p: Point,
        parents: [usize; 2],
        angle: f64,
    }
    let mut inner = Vec::new();
    for i in 0..circles.len() {
        for j in i + 1..circles.len() {
            for p in circle_circle_intersection(circles[i], circles[j]) {
                if circles
                    .iter()
                    .all(|c| distance(p, c.center()) < c.radius + SMALL)
                {
                    inner.push(Inner {
                        p,
                        parents: [i, j],
                        angle: 0.0,
                    });
                }
            }
        }
    }
    let mut arc_area = 0.0;
    let mut polygon_area = 0.0;
    let mut arcs = Vec::new();
    if inner.len() > 1 {
        let points: Vec<Point> = inner.iter().map(|i| i.p).collect();
        let center = center_of(&points);
        for i in &mut inner {
            i.angle = (i.p.x - center.x).atan2(i.p.y - center.y);
        }
        inner.sort_by(|a, b| b.angle.total_cmp(&a.angle));
        let mut k2 = inner.len() - 1;
        for k1 in 0..inner.len() {
            let (p1, p2) = (&inner[k1], &inner[k2]);
            polygon_area += (p2.p.x + p1.p.x) * (p1.p.y - p2.p.y);
            let mid = Point {
                x: (p1.p.x + p2.p.x) / 2.0,
                y: (p1.p.y + p2.p.y) / 2.0,
            };
            let mut arc: Option<Arc> = None;
            for &parent in &p1.parents {
                if !p2.parents.contains(&parent) {
                    continue;
                }
                let c = circles[parent];
                let a1 = (p1.p.x - c.x).atan2(p1.p.y - c.y);
                let a2 = (p2.p.x - c.x).atan2(p2.p.y - c.y);
                let mut diff = a2 - a1;
                if diff < 0.0 {
                    diff += 2.0 * PI;
                }
                let a = a2 - diff / 2.0;
                let edge = Point {
                    x: c.x + c.radius * a.sin(),
                    y: c.y + c.radius * a.cos(),
                };
                let width = distance(mid, edge).min(c.radius * 2.0);
                if arc.is_none_or(|arc| arc.width > width) {
                    arc = Some(Arc {
                        circle: c,
                        p1: p1.p,
                        p2: p2.p,
                        large: width > c.radius,
                        width,
                    });
                }
            }
            if let Some(arc) = arc {
                arc_area += segment_area(arc.circle.radius, arc.width);
                arcs.push(arc);
                k2 = k1;
            }
        }
    } else if let Some(&first) = circles.first() {
        // Disjoint, or the smallest circle inside all the others.
        let smallest = circles
            .iter()
            .skip(1)
            .fold(first, |s, &c| if c.radius < s.radius { c } else { s });
        let disjoint = circles
            .iter()
            .any(|c| distance(c.center(), smallest.center()) > (smallest.radius - c.radius).abs());
        if !disjoint {
            arc_area = smallest.radius * smallest.radius * PI;
            arcs.push(Arc {
                circle: smallest,
                p1: Point {
                    x: smallest.x,
                    y: smallest.y + smallest.radius,
                },
                p2: Point {
                    x: smallest.x - SMALL,
                    y: smallest.y + smallest.radius,
                },
                large: true,
                width: smallest.radius * 2.0,
            });
        }
    }
    (arc_area + polygon_area / 2.0, arcs)
}

/// A root of `f` between `a` and `b`, where `f` changes sign.
fn bisect(f: impl Fn(f64) -> f64, mut a: f64, b: f64) -> f64 {
    let (fa, fb) = (f(a), f(b));
    if fa == 0.0 {
        return a;
    }
    if fb == 0.0 {
        return b;
    }
    let mut delta = b - a;
    for _ in 0..100 {
        delta /= 2.0;
        let mid = a + delta;
        let fm = f(mid);
        if fm * fa >= 0.0 {
            a = mid;
        }
        if delta.abs() < 1e-10 || fm == 0.0 {
            return mid;
        }
    }
    a + delta
}

/// The distance between circles of radii `r1` and `r2` that overlap by `overlap`.
fn distance_from_overlap(r1: f64, r2: f64, overlap: f64) -> f64 {
    if r1.min(r2) * r1.min(r2) * PI <= overlap + SMALL {
        return (r1 - r2).abs();
    }
    bisect(|d| circle_overlap(r1, r2, d) - overlap, 0.0, r1 + r2)
}

/// Minimizes `f` from `x0` with the downhill simplex method, as fmin does.
fn nelder_mead(
    f: impl Fn(&[f64]) -> f64,
    x0: Vec<f64>,
    max_iterations: usize,
    min_error_delta: f64,
    min_tolerance: f64,
) -> Vec<f64> {
    let (rho, chi, psi, sigma) = (1.0, 2.0, -0.5, 0.5);
    let n = x0.len();
    if n == 0 {
        return x0;
    }
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    let fx0 = f(&x0);
    simplex.push((x0.clone(), fx0));
    for i in 0..n {
        let mut p = x0.clone();
        p[i] = if p[i] != 0.0 { p[i] * 1.05 } else { 0.001 };
        let fx = f(&p);
        simplex.push((p, fx));
    }
    let weighted = |w1: f64, v1: &[f64], w2: f64, v2: &[f64]| -> Vec<f64> {
        v1.iter().zip(v2).map(|(a, b)| w1 * a + w2 * b).collect()
    };
    let order = |a: &(Vec<f64>, f64), b: &(Vec<f64>, f64)| {
        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
    };
    for _ in 0..max_iterations {
        simplex.sort_by(order);
        let max_diff = (0..n)
            .map(|i| (simplex[0].0[i] - simplex[1].0[i]).abs())
            .fold(0.0, f64::max);
        if (simplex[0].1 - simplex[n].1).abs() < min_error_delta && max_diff < min_tolerance {
            break;
        }
        let centroid: Vec<f64> = (0..n)
            .map(|i| simplex[..n].iter().map(|s| s.0[i]).sum::<f64>() / n as f64)
            .collect();
        let worst = simplex[n].clone();
        let reflected = weighted(1.0 + rho, &centroid, -rho, &worst.0);
        let reflected_fx = f(&reflected);
        if reflected_fx < simplex[0].1 {
            let expanded = weighted(1.0 + chi, &centroid, -chi, &worst.0);
            let expanded_fx = f(&expanded);
            simplex[n] = if expanded_fx < reflected_fx {
                (expanded, expanded_fx)
            } else {
                (reflected, reflected_fx)
            };
        } else if reflected_fx >= simplex[n - 1].1 {
            let reduce = if reflected_fx > worst.1 {
                let contracted = weighted(1.0 + psi, &centroid, -psi, &worst.0);
                let fx = f(&contracted);
                if fx < worst.1 {
                    simplex[n] = (contracted, fx);
                    false
                } else {
                    true
                }
            } else {
                let contracted = weighted(1.0 - psi * rho, &centroid, psi * rho, &worst.0);
                let fx = f(&contracted);
                if fx < reflected_fx {
                    simplex[n] = (contracted, fx);
                    false
                } else {
                    true
                }
            };
            if reduce {
                let best = simplex[0].0.clone();
                for s in &mut simplex[1..] {
                    s.0 = weighted(1.0 - sigma, &best, sigma, &s.0);
                    s.1 = f(&s.0);
                }
            }
        } else {
            simplex[n] = (reflected, reflected_fx);
        }
    }
    simplex.sort_by(order);
    simplex.swap_remove(0).0
}

/// An area by circle index.
struct Overlap {
    sets: Vec<usize>,
    size: f64,
}

/// How far the overlaps of `circles` are from the sizes asked for.
fn loss(circles: &[Circle], overlaps: &[Overlap]) -> f64 {
    overlaps
        .iter()
        .filter(|a| a.sets.len() > 1)
        .map(|a| {
            let overlap = if a.sets.len() == 2 {
                let (l, r) = (circles[a.sets[0]], circles[a.sets[1]]);
                circle_overlap(l.radius, r.radius, distance(l.center(), r.center()))
            } else {
                let cs: Vec<Circle> = a.sets.iter().map(|&i| circles[i]).collect();
                intersection_area(&cs).0
            };
            (overlap - a.size).powi(2)
        })
        .sum()
}

/// The pairs `overlaps` leave out, as overlaps of size 0, so unrelated sets are kept apart.
fn add_missing(names: &[String], overlaps: &mut Vec<Overlap>) {
    let mut ids: Vec<usize> = (0..names.len()).collect();
    ids.sort_by(|&a, &b| names[a].cmp(&names[b]));
    let has = |overlaps: &[Overlap], a: usize, b: usize| {
        overlaps
            .iter()
            .any(|o| o.sets.len() == 2 && (o.sets == [a, b] || o.sets == [b, a]))
    };
    for i in 0..ids.len() {
        for j in i + 1..ids.len() {
            if !has(overlaps, ids[i], ids[j]) {
                overlaps.push(Overlap {
                    sets: vec![ids[i], ids[j]],
                    size: 0.0,
                });
            }
        }
    }
}

/// Places circles one at a time, most overlapped first, each where its pairwise overlaps with
/// those already placed come closest to their sizes.
fn greedy(sizes: &[f64], overlaps: &[Overlap]) -> Vec<Circle> {
    let mut circles: Vec<Circle> = sizes
        .iter()
        .map(|&s| Circle {
            x: 1e10,
            y: 1e10,
            radius: (s / PI).sqrt(),
        })
        .collect();
    let pairs: Vec<Overlap> = overlaps
        .iter()
        .filter(|o| o.sets.len() == 2)
        .map(|o| Overlap {
            sets: o.sets.clone(),
            size: o.size,
        })
        .collect();
    // Each set's overlaps: (other set, size, weight).
    let mut set_overlaps: Vec<Vec<(usize, f64, f64)>> = vec![Vec::new(); sizes.len()];
    for p in &pairs {
        let (l, r) = (p.sets[0], p.sets[1]);
        // Circles inside others should not be placed early.
        let weight = if p.size + SMALL >= sizes[l].min(sizes[r]) {
            0.0
        } else {
            1.0
        };
        set_overlaps[l].push((r, p.size, weight));
        set_overlaps[r].push((l, p.size, weight));
    }
    let mut most: Vec<(usize, f64)> = set_overlaps
        .iter()
        .enumerate()
        .map(|(i, o)| (i, o.iter().map(|&(_, s, w)| s * w).sum()))
        .collect();
    most.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let Some(&(first, _)) = most.first() else {
        return circles;
    };
    let mut placed = vec![false; sizes.len()];
    circles[first].x = 0.0;
    circles[first].y = 0.0;
    placed[first] = true;
    for &(set, _) in &most[1..] {
        let mut overlap: Vec<(usize, f64, f64)> = set_overlaps[set]
            .iter()
            .copied()
            .filter(|o| placed[o.0])
            .collect();
        overlap.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let radius = circles[set].radius;
        let mut points = Vec::new();
        for (j, &(other, size, _)) in overlap.iter().enumerate() {
            let p1 = circles[other];
            let d1 = distance_from_overlap(radius, p1.radius, size);
            points.push(Point {
                x: p1.x + d1,
                y: p1.y,
            });
            points.push(Point {
                x: p1.x - d1,
                y: p1.y,
            });
            points.push(Point {
                x: p1.x,
                y: p1.y + d1,
            });
            points.push(Point {
                x: p1.x,
                y: p1.y - d1,
            });
            for &(other2, size2, _) in &overlap[j + 1..] {
                let p2 = circles[other2];
                let d2 = distance_from_overlap(radius, p2.radius, size2);
                points.extend(circle_circle_intersection(
                    Circle { radius: d1, ..p1 },
                    Circle { radius: d2, ..p2 },
                ));
            }
        }
        let mut best_loss = 1e50;
        let mut best = points.first().copied().unwrap_or(Point { x: 0.0, y: 0.0 });
        for p in points {
            circles[set].x = p.x;
            circles[set].y = p.y;
            let l = loss(&circles, &pairs);
            if l < best_loss {
                best_loss = l;
                best = p;
            }
        }
        circles[set].x = best.x;
        circles[set].y = best.y;
        placed[set] = true;
    }
    circles
}

/// The bounding box of `circles`: (min x, max x, min y, max y).
fn bounds(circles: &[Circle]) -> (f64, f64, f64, f64) {
    circles.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, x1, y0, y1), c| {
            (
                x0.min(c.x - c.radius),
                x1.max(c.x + c.radius),
                y0.min(c.y - c.radius),
                y1.max(c.y + c.radius),
            )
        },
    )
}

/// Groups of circles that overlap one another, by index.
fn clusters(circles: &[Circle]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..circles.len()).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            let root = find(parent, parent[i]);
            parent[i] = root;
        }
        parent[i]
    }
    for i in 0..circles.len() {
        for j in i + 1..circles.len() {
            let max = circles[i].radius + circles[j].radius;
            if distance(circles[i].center(), circles[j].center()) + 1e-10 < max {
                let (x, y) = (find(&mut parent, j), find(&mut parent, i));
                parent[x] = y;
            }
        }
    }
    let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
    for i in 0..circles.len() {
        let root = find(&mut parent, i);
        match out.iter_mut().find(|(r, _)| *r == root) {
            Some((_, members)) => members.push(i),
            None => out.push((root, vec![i])),
        }
    }
    out.into_iter().map(|(_, m)| m).collect()
}

/// Turns a cluster so the largest circle is at the origin, the next largest at `orientation`
/// from it, and the third on a fixed side. Sorts `cluster` largest first.
fn orientate(circles: &mut [Circle], cluster: &mut [usize], orientation: f64) {
    cluster.sort_by(|&a, &b| {
        circles[b]
            .radius
            .partial_cmp(&circles[a].radius)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let Some(&largest) = cluster.first() else {
        return;
    };
    let (lx, ly) = (circles[largest].x, circles[largest].y);
    for &i in cluster.iter() {
        circles[i].x -= lx;
        circles[i].y -= ly;
    }
    if cluster.len() == 2 {
        let (a, b) = (circles[cluster[0]], circles[cluster[1]]);
        if distance(a.center(), b.center()) < (b.radius - a.radius).abs() {
            circles[cluster[1]].x = a.x + a.radius - b.radius - 1e-10;
            circles[cluster[1]].y = a.y;
        }
    }
    if cluster.len() > 1 {
        let second = circles[cluster[1]];
        let rotation = second.x.atan2(second.y) - orientation;
        let (c, s) = (rotation.cos(), rotation.sin());
        for &i in cluster.iter() {
            let (x, y) = (circles[i].x, circles[i].y);
            circles[i].x = c * x - s * y;
            circles[i].y = s * x + c * y;
        }
    }
    if cluster.len() > 2 {
        let third = circles[cluster[2]];
        let mut angle = third.x.atan2(third.y) - orientation;
        while angle < 0.0 {
            angle += 2.0 * PI;
        }
        while angle > 2.0 * PI {
            angle -= 2.0 * PI;
        }
        if angle > PI {
            let second = circles[cluster[1]];
            let slope = second.y / (1e-10 + second.x);
            for &i in cluster.iter() {
                let c = circles[i];
                let d = (c.x + slope * c.y) / (1.0 + slope * slope);
                circles[i].x = 2.0 * d - c.x;
                circles[i].y = 2.0 * d * slope - c.y;
            }
        }
    }
}

/// Orientates each cluster and lays the clusters out in a grid, largest first. Returns the
/// circles' order after it, which later steps follow.
fn normalize(circles: &mut [Circle], orientation: f64) -> Vec<usize> {
    /// A cluster's circles, bounds, and bounding area.
    type Group = (Vec<usize>, (f64, f64, f64, f64), f64);
    let mut groups: Vec<Group> = clusters(circles)
        .into_iter()
        .map(|mut c| {
            orientate(circles, &mut c, orientation);
            let only: Vec<Circle> = c.iter().map(|&i| circles[i]).collect();
            let b = bounds(&only);
            let size = (b.1 - b.0) * (b.3 - b.2);
            (c, b, size)
        })
        .collect();
    groups.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let Some((first, mut ret, _)) = groups.first().cloned() else {
        return Vec::new();
    };
    let mut order = first;
    let spacing = (ret.1 - ret.0) / 50.0;
    let mut index = 1;
    while index < groups.len() {
        for (k, (right, bottom)) in [(true, false), (false, true), (true, true)]
            .into_iter()
            .enumerate()
        {
            let Some((cluster, b, _)) = groups.get(index + k) else {
                continue;
            };
            let x_offset = if right {
                ret.1 - b.0 + spacing
            } else {
                let centreing = (b.1 - b.0) / 2.0 - (ret.1 - ret.0) / 2.0;
                ret.1 - b.1 + centreing.min(0.0)
            };
            let y_offset = if bottom {
                ret.3 - b.2 + spacing
            } else {
                let centreing = (b.3 - b.2) / 2.0 - (ret.3 - ret.2) / 2.0;
                ret.3 - b.3 + centreing.min(0.0)
            };
            for &i in cluster {
                circles[i].x += x_offset;
                circles[i].y += y_offset;
                order.push(i);
            }
        }
        index += 3;
        let placed: Vec<Circle> = order.iter().map(|&i| circles[i]).collect();
        ret = bounds(&placed);
    }
    order
}

/// Scales and centers `circles` to fit `width` by `height` inside `padding`.
fn scale(circles: &mut [Circle], width: f64, height: f64, padding: f64) {
    let (w, h) = (width - 2.0 * padding, height - 2.0 * padding);
    let (x0, x1, y0, y1) = bounds(circles);
    if x1 == x0 || y1 == y0 {
        return;
    }
    let s = (h / (y1 - y0)).min(w / (x1 - x0));
    let x_offset = (w - (x1 - x0) * s) / 2.0;
    let y_offset = (h - (y1 - y0) * s) / 2.0;
    for c in circles {
        *c = Circle {
            radius: s * c.radius,
            x: padding + x_offset + (c.x - x0) * s,
            y: padding + y_offset + (c.y - y0) * s,
        };
    }
}

/// How far `p` is inside all of `interior` and outside all of `exterior`; negative when it
/// is not.
fn margin(p: Point, interior: &[Circle], exterior: &[Circle]) -> f64 {
    let mut m = interior[0].radius - distance(interior[0].center(), p);
    for c in &interior[1..] {
        m = m.min(c.radius - distance(c.center(), p));
    }
    for c in exterior {
        m = m.min(distance(c.center(), p) - c.radius);
    }
    m
}

/// The point inside `interior` and outside `exterior` farthest from their edges.
fn text_centre(interior: &[Circle], exterior: &[Circle]) -> Option<Point> {
    let mut points = Vec::new();
    for c in interior {
        let r = c.radius / 2.0;
        points.extend([
            (c.x, c.y),
            (c.x + r, c.y),
            (c.x - r, c.y),
            (c.x, c.y + r),
            (c.x, c.y - r),
        ]);
    }
    let (mut best, mut best_margin) = (*points.first()?, f64::NEG_INFINITY);
    for (i, &(x, y)) in points.iter().enumerate() {
        let m = margin(Point { x, y }, interior, exterior);
        if i == 0 || m >= best_margin {
            best = (x, y);
            best_margin = m;
        }
    }
    let solution = nelder_mead(
        |p| -margin(Point { x: p[0], y: p[1] }, interior, exterior),
        vec![best.0, best.1],
        500,
        1e-10,
        1e-10,
    );
    let ret = Point {
        x: solution[0],
        y: solution[1],
    };
    let valid = interior
        .iter()
        .all(|c| distance(ret, c.center()) <= c.radius)
        && exterior
            .iter()
            .all(|c| distance(ret, c.center()) >= c.radius);
    if valid {
        return Some(ret);
    }
    if interior.len() == 1 {
        return Some(interior[0].center());
    }
    let (_, arcs) = intersection_area(interior);
    match arcs.len() {
        0 => None,
        1 => Some(arcs[0].circle.center()),
        _ if !exterior.is_empty() => text_centre(interior, &[]),
        _ => Some(center_of(&arcs.iter().map(|a| a.p1).collect::<Vec<_>>())),
    }
}

/// Lays out `areas` in `width` by `height` with `padding` around them (MVENN-3). Sets of size 0
/// or less are left out, with every area that names them. Returns a region for each area, in
/// order, or `None` for those left out.
pub fn layout(areas: &[Area], width: f64, height: f64, padding: f64) -> Vec<Option<Region>> {
    let removed: Vec<&str> = areas
        .iter()
        .filter(|a| a.sets.len() == 1 && (a.size.is_nan() || a.size <= 0.0))
        .map(|a| a.sets[0].as_str())
        .collect();
    let kept = |a: &Area| !a.sets.iter().any(|s| removed.contains(&s.as_str()));
    // The sets in order of first appearance, sized by their last.
    let mut names: Vec<String> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut sizes: Vec<f64> = Vec::new();
    for a in areas.iter().filter(|a| kept(a) && a.sets.len() == 1) {
        match index.get(a.sets[0].as_str()) {
            Some(&i) => sizes[i] = a.size,
            None => {
                index.insert(&a.sets[0], names.len());
                names.push(a.sets[0].clone());
                sizes.push(a.size);
            }
        }
    }
    let lookup = |a: &Area| -> Option<Vec<usize>> {
        a.sets
            .iter()
            .map(|s| index.get(s.as_str()).copied())
            .collect()
    };
    let mut overlaps: Vec<Overlap> = areas
        .iter()
        .filter(|a| kept(a))
        .filter_map(|a| {
            Some(Overlap {
                sets: lookup(a)?,
                size: a.size.max(0.0),
            })
        })
        .collect();
    add_missing(&names, &mut overlaps);
    let mut circles = greedy(&sizes, &overlaps);
    let initial: Vec<f64> = circles.iter().flat_map(|c| [c.x, c.y]).collect();
    let radii: Vec<f64> = circles.iter().map(|c| c.radius).collect();
    let solution = nelder_mead(
        |v| {
            let current: Vec<Circle> = radii
                .iter()
                .enumerate()
                .map(|(i, &radius)| Circle {
                    x: v[2 * i],
                    y: v[2 * i + 1],
                    radius,
                })
                .collect();
            loss(&current, &overlaps)
        },
        initial,
        500,
        1e-6,
        1e-5,
    );
    for (i, c) in circles.iter_mut().enumerate() {
        c.x = solution[2 * i];
        c.y = solution[2 * i + 1];
    }
    let order = normalize(&mut circles, PI / 2.0);
    let mut ordered: Vec<Circle> = order.iter().map(|&i| circles[i]).collect();
    scale(&mut ordered, width, height, padding);
    for (k, &i) in order.iter().enumerate() {
        circles[i] = ordered[k];
    }
    // Circles that wholly contain each circle are not exterior to its regions.
    let mut containers: Vec<Vec<usize>> = vec![Vec::new(); circles.len()];
    for (k, &a) in order.iter().enumerate() {
        for &b in &order[k + 1..] {
            let (ca, cb) = (circles[a], circles[b]);
            let d = distance(ca.center(), cb.center());
            if d + cb.radius <= ca.radius + 1e-10 {
                containers[b].push(a);
            } else if d + ca.radius <= cb.radius + 1e-10 {
                containers[a].push(b);
            }
        }
    }
    areas
        .iter()
        .map(|a| {
            if !kept(a) {
                return None;
            }
            let sets = lookup(a)?;
            let interior: Vec<Circle> = order
                .iter()
                .filter(|i| sets.contains(i))
                .map(|&i| circles[i])
                .collect();
            let exterior: Vec<Circle> = order
                .iter()
                .filter(|i| !sets.contains(i) && !sets.iter().any(|&s| containers[s].contains(i)))
                .map(|&i| circles[i])
                .collect();
            let own: Vec<Circle> = sets.iter().map(|&i| circles[i]).collect();
            Some(Region {
                arcs: intersection_area(&own).1,
                text: text_centre(&interior, &exterior),
                circles: own,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(sets: &[&str], size: f64) -> Area {
        Area {
            sets: sets.iter().map(|s| s.to_string()).collect(),
            size,
        }
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn overlaps() {
        // Two unit circles one radius apart.
        let o = circle_overlap(1.0, 1.0, 1.0);
        assert!(close(o, 2.0 * PI / 3.0 - 3f64.sqrt() / 2.0, 1e-9), "{o}");
        assert_eq!(circle_overlap(1.0, 1.0, 2.0), 0.0);
        assert!(close(circle_overlap(2.0, 1.0, 0.5), PI, 1e-12));
        let d = distance_from_overlap(1.0, 1.0, o);
        assert!(close(d, 1.0, 1e-8), "{d}");
        let c = |x| Circle {
            x,
            y: 0.0,
            radius: 1.0,
        };
        let (a, arcs) = intersection_area(&[c(0.0), c(1.0)]);
        assert!(close(a, o, 1e-9), "{a}");
        assert_eq!(arcs.len(), 2);
        assert_eq!(intersection_area(&[c(0.0), c(5.0)]).1.len(), 0);
    }

    #[test]
    fn minimizes() {
        let x = nelder_mead(
            |v| (v[0] - 3.0).powi(2) + (v[1] + 1.0).powi(2),
            vec![0.0, 0.0],
            500,
            1e-10,
            1e-10,
        );
        assert!(close(x[0], 3.0, 1e-4) && close(x[1], -1.0, 1e-4), "{x:?}");
    }

    #[test]
    fn two_sets() {
        let regions = layout(
            &[
                area(&["A"], 10.0),
                area(&["B"], 10.0),
                area(&["A", "B"], 2.5),
            ],
            800.0,
            450.0,
            15.0,
        );
        let a = regions[0].as_ref().unwrap();
        let b = regions[1].as_ref().unwrap();
        let ab = regions[2].as_ref().unwrap();
        // Equal circles, side by side, centered, filling the height.
        let (ca, cb) = (a.circles[0], b.circles[0]);
        assert!(close(ca.radius, cb.radius, 1e-6));
        assert!(close(ca.y, 225.0, 1e-6) && close(cb.y, 225.0, 1e-6));
        assert!(close(ca.radius, 210.0, 1e-6), "{ca:?}");
        assert!(close((ca.x + cb.x) / 2.0, 400.0, 1e-6));
        assert!(ca.x < cb.x);
        // The overlap's share of a circle matches the sizes.
        let o = circle_overlap(ca.radius, cb.radius, (cb.x - ca.x).abs());
        assert!(close(o / (PI * ca.radius * ca.radius), 0.25, 1e-3), "{o}");
        // Labels inside their regions.
        let t = ab.text.unwrap();
        assert!(close(t.x, 400.0, 0.5) && close(t.y, 225.0, 0.5), "{t:?}");
        let t = a.text.unwrap();
        assert!(t.x < ca.x && distance(t, cb.center()) > cb.radius);
        assert_eq!(ab.arcs.len(), 2);
    }

    #[test]
    fn three_sets() {
        let regions = layout(
            &[
                area(&["A"], 10.0),
                area(&["B"], 10.0),
                area(&["C"], 10.0),
                area(&["A", "B", "C"], 10.0 / 9.0),
                area(&["A", "B"], 2.5),
                area(&["B", "C"], 2.5),
                area(&["A", "C"], 2.5),
            ],
            800.0,
            450.0,
            15.0,
        );
        assert!(regions.iter().all(Option::is_some));
        let abc = regions[3].as_ref().unwrap();
        let t = abc.text.unwrap();
        for c in &abc.circles {
            assert!(distance(t, c.center()) < c.radius);
        }
        assert_eq!(abc.arcs.len(), 3);
    }

    #[test]
    fn disjoint_and_removed() {
        let regions = layout(
            &[
                area(&["A"], 10.0),
                area(&["B"], 5.0),
                area(&["Z"], 0.0),
                area(&["A", "Z"], 1.0),
            ],
            800.0,
            450.0,
            15.0,
        );
        assert!(regions[2].is_none() && regions[3].is_none());
        let (a, b) = (regions[0].as_ref().unwrap(), regions[1].as_ref().unwrap());
        let (ca, cb) = (a.circles[0], b.circles[0]);
        assert!(distance(ca.center(), cb.center()) >= ca.radius + cb.radius - 1e-6);
    }

    #[test]
    fn subset_inside() {
        let regions = layout(
            &[
                area(&["A"], 10.0),
                area(&["B"], 2.0),
                area(&["A", "B"], 2.0),
            ],
            800.0,
            450.0,
            15.0,
        );
        let (a, b) = (regions[0].as_ref().unwrap(), regions[1].as_ref().unwrap());
        let (ca, cb) = (a.circles[0], b.circles[0]);
        assert!(distance(ca.center(), cb.center()) + cb.radius <= ca.radius + 1e-6);
        // A's label stays outside B.
        let t = a.text.unwrap();
        assert!(distance(t, cb.center()) >= cb.radius);
    }
}
