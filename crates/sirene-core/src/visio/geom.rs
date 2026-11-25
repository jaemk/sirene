//! Geometry rows to paths in the shape's own frame, in inches, y up (VISIO-7). Curves are cubic
//! Beziers; splines and NURBS are sampled to lines.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::limits;
use super::model::{Row, Section};

pub(super) type P = (f64, f64);

/// One drawing operation of a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Op {
    Move(P),
    Line(P),
    Cubic(P, P, P),
}

/// One subpath: starts with a `Move`; `closed` when it ends where it began.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Sub {
    pub ops: Vec<Op>,
    pub closed: bool,
}

impl Sub {
    /// The end point of the path.
    pub(super) fn end(&self) -> Option<P> {
        self.ops.last().map(|op| match *op {
            Op::Move(p) | Op::Line(p) | Op::Cubic(_, _, p) => p,
        })
    }

    pub(super) fn start(&self) -> Option<P> {
        match self.ops.first() {
            Some(Op::Move(p)) => Some(*p),
            _ => None,
        }
    }
}

/// A geometry section drawn: its flags and subpaths.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Geom {
    pub no_fill: bool,
    pub no_line: bool,
    pub no_show: bool,
    pub subs: Vec<Sub>,
}

fn val(cells: &super::model::Cells, name: &str) -> Option<f64> {
    let v: f64 = cells.get(name)?.v.trim().parse().ok()?;
    v.is_finite().then_some(v)
}

fn flag(section: &Section, name: &str) -> bool {
    val(&section.cells, name).is_some_and(|v| v != 0.0)
}

fn near(a: P, b: P) -> bool {
    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
}

/// Numbers in the arguments of a formula like `POLYLINE(0, 0, 1, 2)`, or `None` when an argument
/// is not a plain number (formulas are never evaluated).
fn formula_args(text: &str) -> Option<Vec<f64>> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let inner = text.get(open + 1..close)?;
    let mut out = Vec::new();
    for arg in inner.split(',') {
        if out.len() >= limits::MAX_FORMULA_POINTS * 4 {
            return None;
        }
        let v: f64 = arg.trim().parse().ok()?;
        if !v.is_finite() {
            return None;
        }
        out.push(v);
    }
    Some(out)
}

fn formula<'a>(row: &'a Row, cell: &str) -> Option<&'a str> {
    let c = row.cells.get(cell)?;
    Some(c.f.as_deref().filter(|f| f.contains('(')).unwrap_or(&c.v))
}

/// The points of an elliptical arc from `p0` to `p1` through `pc`, on an ellipse whose major axis
/// is at angle `phi` and whose major to minor ratio is `ratio`, as cubic Beziers (VISIO-7).
pub(super) fn arc_through(p0: P, p1: P, pc: P, phi: f64, ratio: f64) -> Option<Vec<Op>> {
    if !(ratio > 0.0 && ratio <= 1000.0) || !phi.is_finite() {
        return None;
    }
    let (s, c) = phi.sin_cos();
    let to_circle = |p: P| (c * p.0 + s * p.1, ratio * (-s * p.0 + c * p.1));
    let from_circle = |q: P| {
        let (x, y) = (q.0, q.1 / ratio);
        (c * x - s * y, s * x + c * y)
    };
    let (a, b, m) = (to_circle(p0), to_circle(p1), to_circle(pc));
    let d = 2.0 * (a.0 * (m.1 - b.1) + m.0 * (b.1 - a.1) + b.0 * (a.1 - m.1));
    if d.abs() < 1e-12 {
        return None;
    }
    let sq = |p: P| p.0 * p.0 + p.1 * p.1;
    let ux = (sq(a) * (m.1 - b.1) + sq(m) * (b.1 - a.1) + sq(b) * (a.1 - m.1)) / d;
    let uy = (sq(a) * (b.0 - m.0) + sq(m) * (a.0 - b.0) + sq(b) * (m.0 - a.0)) / d;
    let r = ((a.0 - ux).hypot(a.1 - uy)).max(0.0);
    if !r.is_finite() || r < 1e-12 {
        return None;
    }
    let angle = |p: P| (p.1 - uy).atan2(p.0 - ux);
    let norm = |t: f64| t.rem_euclid(TAU);
    let t0 = angle(a);
    let to_mid = norm(angle(m) - t0);
    let to_end = norm(angle(b) - t0);
    let total = if to_mid < to_end {
        to_end
    } else {
        to_end - TAU
    };
    let segments = ((total.abs() / FRAC_PI_2).ceil() as usize).clamp(1, 4);
    let step = total / segments as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let at = |t: f64| (ux + r * t.cos(), uy + r * t.sin());
    let tangent = |t: f64| (-r * t.sin(), r * t.cos());
    let mut ops = Vec::new();
    for i in 0..segments {
        let (ta, tb) = (t0 + step * i as f64, t0 + step * (i + 1) as f64);
        let (pa, pb) = (at(ta), at(tb));
        let (da, db) = (tangent(ta), tangent(tb));
        let c1 = (pa.0 + k * da.0, pa.1 + k * da.1);
        let c2 = (pb.0 - k * db.0, pb.1 - k * db.1);
        let end = if i + 1 == segments {
            p1
        } else {
            from_circle(pb)
        };
        ops.push(Op::Cubic(from_circle(c1), from_circle(c2), end));
    }
    Some(ops)
}

/// A full ellipse from its center and the end points of two conjugate axes, as four cubics.
fn ellipse(center: P, u: P, v: P) -> Vec<Op> {
    let k = 0.552_284_749_830_793_4;
    let pt = |cu: f64, cv: f64| {
        (
            center.0 + u.0 * cu + v.0 * cv,
            center.1 + u.1 * cu + v.1 * cv,
        )
    };
    vec![
        Op::Move(pt(1.0, 0.0)),
        Op::Cubic(pt(1.0, k), pt(k, 1.0), pt(0.0, 1.0)),
        Op::Cubic(pt(-k, 1.0), pt(-1.0, k), pt(-1.0, 0.0)),
        Op::Cubic(pt(-1.0, -k), pt(-k, -1.0), pt(0.0, -1.0)),
        Op::Cubic(pt(k, -1.0), pt(1.0, -k), pt(1.0, 0.0)),
    ]
}

/// Points of the clamped uniform B-spline of `degree` over `points` with `weights`, sampled.
fn bspline(points: &[P], weights: &[f64], degree: usize) -> Vec<P> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    let degree = degree.clamp(1, n - 1);
    let m = n + degree + 1;
    let mut knots = vec![0.0; m];
    let inner = n - degree;
    for (i, k) in knots.iter_mut().enumerate() {
        *k = if i <= degree {
            0.0
        } else if i >= n {
            1.0
        } else {
            (i - degree) as f64 / inner as f64
        };
    }
    let samples = (16 * inner).clamp(8, limits::MAX_FORMULA_POINTS);
    let knot = |i: usize| knots.get(i).copied().unwrap_or(1.0);
    let mut out = Vec::with_capacity(samples + 1);
    for s in 0..=samples {
        let u = s as f64 / samples as f64;
        // The span k with knots[k] <= u < knots[k + 1]; the last span at u = 1.
        let k = (degree..n)
            .rev()
            .find(|&k| knot(k) <= u)
            .unwrap_or(degree)
            .min(n - 1);
        // De Boor's algorithm on homogeneous coordinates (x w, y w, w).
        let mut d: Vec<(f64, f64, f64)> = (0..=degree)
            .map(|j| {
                let idx = k + j - degree;
                let p = points.get(idx).copied().unwrap_or_default();
                let w = weights.get(idx).copied().unwrap_or(1.0);
                (p.0 * w, p.1 * w, w)
            })
            .collect();
        for r in 1..=degree {
            for j in (r..=degree).rev() {
                let i = j + k - degree;
                let span = knot(i + degree + 1 - r) - knot(i);
                let alpha = if span.abs() > 1e-12 {
                    (u - knot(i)) / span
                } else {
                    0.0
                };
                let (a, b) = (
                    d.get(j - 1).copied().unwrap_or_default(),
                    d.get(j).copied().unwrap_or_default(),
                );
                if let Some(slot) = d.get_mut(j) {
                    *slot = (
                        (1.0 - alpha) * a.0 + alpha * b.0,
                        (1.0 - alpha) * a.1 + alpha * b.1,
                        (1.0 - alpha) * a.2 + alpha * b.2,
                    );
                }
            }
        }
        if let Some(&(x, y, w)) = d.get(degree)
            && w.abs() > 1e-12
        {
            out.push((x / w, y / w));
        }
    }
    if let (Some(first), Some(&p0)) = (out.first_mut(), points.first()) {
        *first = p0;
    }
    if let (Some(last), Some(&pn)) = (out.last_mut(), points.last()) {
        *last = pn;
    }
    out
}

/// Builds the paths of one `Geometry` section of a shape `w` by `h` inches. `warn` is told of
/// anything skipped.
pub(super) fn build(section: &Section, w: f64, h: f64, warn: &mut dyn FnMut(&str)) -> Geom {
    let mut geom = Geom {
        no_fill: flag(section, "NoFill"),
        no_line: flag(section, "NoLine"),
        no_show: flag(section, "NoShow"),
        subs: Vec::new(),
    };
    let rows: Vec<&Row> = section.rows.iter().take(limits::MAX_ROWS).collect();
    let mut cur: Option<P> = None;
    let mut sub = Sub::default();
    let finish = |sub: &mut Sub, geom: &mut Geom| {
        let done = std::mem::take(sub);
        if done.ops.len() > 1 {
            let mut done = done;
            done.closed = matches!((done.start(), done.end()), (Some(a), Some(b)) if near(a, b));
            geom.subs.push(done);
        }
    };
    let mut i = 0;
    while let Some(row) = rows.get(i).copied() {
        i += 1;
        let x = val(&row.cells, "X");
        let y = val(&row.cells, "Y");
        let a = val(&row.cells, "A");
        let b = val(&row.cells, "B");
        let c = val(&row.cells, "C");
        let d = val(&row.cells, "D");
        match row.t.as_str() {
            "MoveTo" | "RelMoveTo" => {
                let rel = row.t.starts_with("Rel");
                let (Some(x), Some(y)) = (x, y) else { continue };
                finish(&mut sub, &mut geom);
                let p = if rel { (x * w, y * h) } else { (x, y) };
                sub.ops.push(Op::Move(p));
                cur = Some(p);
            }
            "LineTo" | "RelLineTo" => {
                let rel = row.t.starts_with("Rel");
                let (Some(x), Some(y), Some(_)) = (x, y, cur) else {
                    continue;
                };
                let p = if rel { (x * w, y * h) } else { (x, y) };
                sub.ops.push(Op::Line(p));
                cur = Some(p);
            }
            "ArcTo" => {
                let (Some(x), Some(y), Some(bulge), Some(from)) = (x, y, a, cur) else {
                    continue;
                };
                let to = (x, y);
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                let len = dx.hypot(dy);
                let ops = (len > 1e-9 && bulge.abs() > 1e-9)
                    .then(|| {
                        let mid = (
                            from.0 + dx / 2.0 - dy / len * bulge,
                            from.1 + dy / 2.0 + dx / len * bulge,
                        );
                        arc_through(from, to, mid, 0.0, 1.0)
                    })
                    .flatten();
                match ops {
                    Some(ops) => sub.ops.extend(ops),
                    None => sub.ops.push(Op::Line(to)),
                }
                cur = Some(to);
            }
            "EllipticalArcTo" => {
                let (Some(x), Some(y), Some(from)) = (x, y, cur) else {
                    continue;
                };
                let to = (x, y);
                let ops = match (a, b, c, d) {
                    (Some(a), Some(b), Some(c), Some(d)) => arc_through(from, to, (a, b), c, d),
                    _ => None,
                };
                match ops {
                    Some(ops) => sub.ops.extend(ops),
                    None => sub.ops.push(Op::Line(to)),
                }
                cur = Some(to);
            }
            "Ellipse" => {
                let (Some(x), Some(y), Some(a), Some(b), Some(c), Some(d)) = (x, y, a, b, c, d)
                else {
                    continue;
                };
                finish(&mut sub, &mut geom);
                let center = (x, y);
                sub.ops = ellipse(center, (a - x, b - y), (c - x, d - y));
                sub.closed = true;
                geom.subs.push(std::mem::take(&mut sub));
                cur = None;
            }
            "PolylineTo" => {
                let (Some(x), Some(y), Some(_)) = (x, y, cur) else {
                    continue;
                };
                let args = formula(row, "A").and_then(formula_args);
                match args {
                    Some(args) if args.len() >= 2 => {
                        let (xt, yt) = (
                            args.first().copied().unwrap_or(1.0),
                            args.get(1).copied().unwrap_or(1.0),
                        );
                        for pair in args.get(2..).unwrap_or(&[]).as_chunks::<2>().0 {
                            let [px, py] = *pair;
                            let px = if xt == 0.0 { px * w } else { px };
                            let py = if yt == 0.0 { py * h } else { py };
                            sub.ops.push(Op::Line((px, py)));
                        }
                    }
                    _ => warn("a PolylineTo formula is not plain numbers; its points are skipped"),
                }
                sub.ops.push(Op::Line((x, y)));
                cur = Some((x, y));
            }
            "SplineStart" => {
                let (Some(x), Some(y), Some(from)) = (x, y, cur) else {
                    continue;
                };
                let degree = d.map_or(3, |d| d.clamp(1.0, 25.0) as usize);
                let mut points = vec![from, (x, y)];
                while let Some(next) = rows.get(i).filter(|r| r.t == "SplineKnot") {
                    i += 1;
                    if let (Some(kx), Some(ky)) = (val(&next.cells, "X"), val(&next.cells, "Y")) {
                        points.push((kx, ky));
                    }
                }
                let sampled = bspline(&points, &[], degree);
                sub.ops.extend(sampled.iter().skip(1).map(|p| Op::Line(*p)));
                cur = points.last().copied();
            }
            "SplineKnot" => {}
            "NURBSTo" => {
                let (Some(x), Some(y), Some(from)) = (x, y, cur) else {
                    continue;
                };
                let args = formula(row, "E").and_then(formula_args);
                let Some(args) = args.filter(|a| a.len() >= 6) else {
                    warn("a NURBSTo formula is not plain numbers; drawn as a line");
                    sub.ops.push(Op::Line((x, y)));
                    cur = Some((x, y));
                    continue;
                };
                let degree = args.get(1).copied().unwrap_or(3.0).clamp(1.0, 25.0) as usize;
                let (xt, yt) = (
                    args.get(2).copied().unwrap_or(1.0),
                    args.get(3).copied().unwrap_or(1.0),
                );
                let mut points = vec![from];
                let mut weights = vec![d.unwrap_or(1.0)];
                for quad in args.get(4..).unwrap_or(&[]).as_chunks::<4>().0 {
                    let [px, py, _, weight] = *quad;
                    let px = if xt == 0.0 { px * w } else { px };
                    let py = if yt == 0.0 { py * h } else { py };
                    points.push((px, py));
                    weights.push(weight);
                }
                points.push((x, y));
                weights.push(b.unwrap_or(1.0));
                let sampled = bspline(&points, &weights, degree);
                sub.ops.extend(sampled.iter().skip(1).map(|p| Op::Line(*p)));
                cur = Some((x, y));
            }
            "CubBezTo" | "RelCubBezTo" => {
                let k = if row.t.starts_with("Rel") {
                    (w, h)
                } else {
                    (1.0, 1.0)
                };
                let (Some(x), Some(y), Some(a), Some(b), Some(c), Some(d), Some(_)) =
                    (x, y, a, b, c, d, cur)
                else {
                    continue;
                };
                let to = (x * k.0, y * k.1);
                sub.ops
                    .push(Op::Cubic((a * k.0, b * k.1), (c * k.0, d * k.1), to));
                cur = Some(to);
            }
            "QuadBezTo" | "RelQuadBezTo" => {
                let k = if row.t.starts_with("Rel") {
                    (w, h)
                } else {
                    (1.0, 1.0)
                };
                let (Some(x), Some(y), Some(a), Some(b), Some(from)) = (x, y, a, b, cur) else {
                    continue;
                };
                let (to, q) = ((x * k.0, y * k.1), (a * k.0, b * k.1));
                let lift = |p: P| (p.0 + 2.0 / 3.0 * (q.0 - p.0), p.1 + 2.0 / 3.0 * (q.1 - p.1));
                sub.ops.push(Op::Cubic(lift(from), lift(to), to));
                cur = Some(to);
            }
            "InfiniteLine" => {
                let (Some(x), Some(y), Some(a), Some(b)) = (x, y, a, b) else {
                    continue;
                };
                let (dx, dy) = (a - x, b - y);
                let len = dx.hypot(dy);
                if len < 1e-9 {
                    continue;
                }
                finish(&mut sub, &mut geom);
                let far = limits::MAX_PAGE_INCHES;
                let (ux, uy) = (dx / len * far, dy / len * far);
                sub.ops = vec![Op::Move((x - ux, y - uy)), Op::Line((x + ux, y + uy))];
                geom.subs.push(std::mem::take(&mut sub));
                cur = None;
            }
            _ => {}
        }
    }
    finish(&mut sub, &mut geom);
    geom
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visio::model::{Cell, Cells};

    fn row(t: &str, cells: &[(&str, f64)]) -> Row {
        let cells: Cells = cells
            .iter()
            .map(|(n, v)| {
                (
                    (*n).to_owned(),
                    Cell {
                        v: v.to_string(),
                        f: None,
                    },
                )
            })
            .collect();
        Row {
            t: t.to_owned(),
            ix: 0,
            cells,
            ..Row::default()
        }
    }

    fn section(rows: Vec<Row>) -> Section {
        Section {
            n: "Geometry".into(),
            rows,
            ..Section::default()
        }
    }

    fn end_of(op: Op) -> P {
        match op {
            Op::Move(p) | Op::Line(p) | Op::Cubic(_, _, p) => p,
        }
    }

    #[test]
    fn rectangle_is_one_closed_subpath() {
        let g = build(
            &section(vec![
                row("MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row("LineTo", &[("X", 2.0), ("Y", 0.0)]),
                row("LineTo", &[("X", 2.0), ("Y", 1.0)]),
                row("LineTo", &[("X", 0.0), ("Y", 0.0)]),
            ]),
            2.0,
            1.0,
            &mut |_| {},
        );
        assert_eq!(g.subs.len(), 1);
        assert!(g.subs[0].closed);
    }

    #[test]
    fn relative_rows_scale_by_the_shape_size() {
        let g = build(
            &section(vec![
                row("RelMoveTo", &[("X", 0.0), ("Y", 0.5)]),
                row("RelLineTo", &[("X", 1.0), ("Y", 0.5)]),
            ]),
            4.0,
            2.0,
            &mut |_| {},
        );
        assert_eq!(g.subs[0].ops, [Op::Move((0.0, 1.0)), Op::Line((4.0, 1.0))]);
        assert!(!g.subs[0].closed);
    }

    #[test]
    fn circular_arc_through_a_semicircle() {
        // From (0,0) to (2,0) with the arc's midpoint 1 to the left of the chord direction
        // (above, y up): a semicircle of radius 1 about (1,0).
        let ops = arc_through((0.0, 0.0), (2.0, 0.0), (1.0, -1.0), 0.0, 1.0).unwrap();
        assert_eq!(ops.len(), 2);
        let mid = end_of(ops[0]);
        assert!(
            (mid.0 - 1.0).abs() < 1e-9 && (mid.1 + 1.0).abs() < 1e-9,
            "{mid:?}"
        );
        assert_eq!(end_of(ops[1]), (2.0, 0.0));
        // Every control point of a quarter circle sits at the standard distance.
        if let Op::Cubic(c1, _, _) = ops[0] {
            assert!((c1.0 - 0.0).abs() < 1e-9);
            assert!((c1.1 + 0.552_284_749_830_793_4).abs() < 1e-9, "{c1:?}");
        }
    }

    #[test]
    fn arcto_bulge_sign_picks_the_side() {
        let up = build(
            &section(vec![
                row("MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row("ArcTo", &[("X", 2.0), ("Y", 0.0), ("A", 1.0)]),
            ]),
            2.0,
            1.0,
            &mut |_| {},
        );
        // Positive bulge is to the left of the chord direction (+x), so up in y.
        let mid = end_of(up.subs[0].ops[1]);
        assert!(
            (mid.0 - 1.0).abs() < 1e-9 && (mid.1 - 1.0).abs() < 1e-9,
            "{mid:?}"
        );
        let flat = build(
            &section(vec![
                row("MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row("ArcTo", &[("X", 2.0), ("Y", 0.0), ("A", 0.0)]),
            ]),
            2.0,
            1.0,
            &mut |_| {},
        );
        assert_eq!(flat.subs[0].ops[1], Op::Line((2.0, 0.0)));
    }

    #[test]
    fn elliptical_arc_with_a_ratio() {
        // The upper half of an ellipse with semi-axes 2 (x) and 1 (y): a ratio of 2 and angle 0.
        let ops = arc_through((-2.0, 0.0), (2.0, 0.0), (0.0, 1.0), 0.0, 2.0).unwrap();
        let mid = end_of(ops[0]);
        assert!(
            (mid.0).abs() < 1e-9 && (mid.1 - 1.0).abs() < 1e-9,
            "{mid:?}"
        );
        assert_eq!(end_of(*ops.last().unwrap()), (2.0, 0.0));
    }

    #[test]
    fn collinear_arc_points_fail() {
        assert!(arc_through((0.0, 0.0), (2.0, 0.0), (1.0, 0.0), 0.0, 1.0).is_none());
        assert!(arc_through((0.0, 0.0), (2.0, 0.0), (1.0, 1.0), 0.0, 0.0).is_none());
        assert!(arc_through((0.0, 0.0), (2.0, 0.0), (1.0, 1.0), f64::NAN, 1.0).is_none());
    }

    #[test]
    fn ellipse_row_is_four_cubics() {
        let g = build(
            &section(vec![row(
                "Ellipse",
                &[
                    ("X", 1.0),
                    ("Y", 1.0),
                    ("A", 3.0),
                    ("B", 1.0),
                    ("C", 1.0),
                    ("D", 2.0),
                ],
            )]),
            2.0,
            2.0,
            &mut |_| {},
        );
        let sub = &g.subs[0];
        assert!(sub.closed);
        assert_eq!(sub.ops.len(), 5);
        assert_eq!(sub.ops[0], Op::Move((3.0, 1.0)));
        assert_eq!(end_of(sub.ops[1]), (1.0, 2.0));
        assert_eq!(end_of(sub.ops[2]), (-1.0, 1.0));
    }

    #[test]
    fn polyline_reads_literal_points() {
        let mut r = row("PolylineTo", &[("X", 3.0), ("Y", 0.0)]);
        r.cells.insert(
            "A".into(),
            Cell {
                v: "No Formula".into(),
                f: Some("POLYLINE(1, 1, 1, 1, 2, 0)".into()),
            },
        );
        let g = build(
            &section(vec![row("MoveTo", &[("X", 0.0), ("Y", 0.0)]), r]),
            1.0,
            1.0,
            &mut |_| {},
        );
        assert_eq!(
            g.subs[0].ops,
            [
                Op::Move((0.0, 0.0)),
                Op::Line((1.0, 1.0)),
                Op::Line((2.0, 0.0)),
                Op::Line((3.0, 0.0))
            ]
        );
    }

    #[test]
    fn polyline_with_a_reference_warns_and_draws_to_the_end() {
        let mut r = row("PolylineTo", &[("X", 3.0), ("Y", 0.0)]);
        r.cells.insert(
            "A".into(),
            Cell {
                v: String::new(),
                f: Some("POLYLINE(0, 0, Width*0.5, 1)".into()),
            },
        );
        let mut warned = Vec::new();
        let g = build(
            &section(vec![row("MoveTo", &[("X", 0.0), ("Y", 0.0)]), r]),
            1.0,
            1.0,
            &mut |m| warned.push(m.to_owned()),
        );
        assert_eq!(warned.len(), 1);
        assert_eq!(g.subs[0].ops.last(), Some(&Op::Line((3.0, 0.0))));
    }

    #[test]
    fn spline_ends_at_its_first_and_last_control_points() {
        let g = build(
            &section(vec![
                row("MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row("SplineStart", &[("X", 1.0), ("Y", 2.0), ("D", 3.0)]),
                row("SplineKnot", &[("X", 3.0), ("Y", 2.0)]),
                row("SplineKnot", &[("X", 4.0), ("Y", 0.0)]),
            ]),
            4.0,
            2.0,
            &mut |_| {},
        );
        let ops = &g.subs[0].ops;
        assert!(ops.len() > 10);
        assert_eq!(end_of(*ops.last().unwrap()), (4.0, 0.0));
        // The curve stays inside the control polygon's hull.
        for op in ops {
            let p = end_of(*op);
            assert!(
                p.0 >= -1e-9 && p.0 <= 4.0 + 1e-9 && p.1 >= -1e-9 && p.1 <= 2.0 + 1e-9,
                "{p:?}"
            );
        }
    }

    #[test]
    fn nurbs_reads_its_formula() {
        let mut r = row(
            "NURBSTo",
            &[
                ("X", 2.0),
                ("Y", 0.0),
                ("A", 1.0),
                ("B", 1.0),
                ("C", 0.0),
                ("D", 1.0),
            ],
        );
        r.cells.insert(
            "E".into(),
            Cell {
                v: String::new(),
                f: Some("NURBS(1, 2, 1, 1, 1, 2, 0.5, 1)".into()),
            },
        );
        let g = build(
            &section(vec![row("MoveTo", &[("X", 0.0), ("Y", 0.0)]), r]),
            2.0,
            2.0,
            &mut |_| {},
        );
        let ops = &g.subs[0].ops;
        assert!(ops.len() > 8);
        assert_eq!(end_of(*ops.last().unwrap()), (2.0, 0.0));
        // A quadratic through (0,0), (1,2), (2,0) peaks at y = 1 at the middle.
        let peak = ops.iter().map(|o| end_of(*o).1).fold(0.0, f64::max);
        assert!((peak - 1.0).abs() < 0.01, "{peak}");
    }

    #[test]
    fn beziers_and_infinite_lines() {
        let g = build(
            &section(vec![
                row("MoveTo", &[("X", 0.0), ("Y", 0.0)]),
                row(
                    "QuadBezTo",
                    &[("X", 3.0), ("Y", 0.0), ("A", 1.5), ("B", 3.0)],
                ),
                row(
                    "CubBezTo",
                    &[
                        ("X", 6.0),
                        ("Y", 0.0),
                        ("A", 4.0),
                        ("B", 1.0),
                        ("C", 5.0),
                        ("D", 1.0),
                    ],
                ),
                row(
                    "InfiniteLine",
                    &[("X", 0.0), ("Y", 1.0), ("A", 1.0), ("B", 1.0)],
                ),
            ]),
            6.0,
            3.0,
            &mut |_| {},
        );
        assert_eq!(g.subs.len(), 2);
        assert_eq!(
            g.subs[0].ops[1],
            Op::Cubic((1.0, 2.0), (2.0, 2.0), (3.0, 0.0))
        );
        assert_eq!(
            g.subs[0].ops[2],
            Op::Cubic((4.0, 1.0), (5.0, 1.0), (6.0, 0.0))
        );
        assert_eq!(g.subs[1].ops[0], Op::Move((-limits::MAX_PAGE_INCHES, 1.0)));
    }

    #[test]
    fn rows_without_a_current_point_or_cells_are_skipped() {
        let g = build(
            &section(vec![
                row("LineTo", &[("X", 1.0), ("Y", 1.0)]),
                row("MoveTo", &[("X", 0.0)]),
                row("ArcTo", &[("X", 1.0), ("Y", 1.0), ("A", 1.0)]),
                row("Bogus", &[]),
            ]),
            1.0,
            1.0,
            &mut |_| {},
        );
        assert!(g.subs.is_empty());
    }

    #[test]
    fn flags() {
        let mut s = section(vec![]);
        s.cells.insert(
            "NoFill".into(),
            Cell {
                v: "1".into(),
                f: None,
            },
        );
        s.cells.insert(
            "NoShow".into(),
            Cell {
                v: "0".into(),
                f: None,
            },
        );
        let g = build(&s, 1.0, 1.0, &mut |_| {});
        assert!(g.no_fill && !g.no_line && !g.no_show);
    }

    #[test]
    fn formula_arguments() {
        assert_eq!(
            formula_args("POLYLINE(0, 1, 2.5)"),
            Some(vec![0.0, 1.0, 2.5])
        );
        assert_eq!(formula_args("POLYLINE(0, Width)"), None);
        assert_eq!(formula_args("no parens"), None);
        assert_eq!(formula_args("F(NaN)"), None);
    }
}
