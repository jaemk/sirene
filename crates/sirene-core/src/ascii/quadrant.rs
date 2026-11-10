//! Quadrant charts as text (MQUAD-4): a box split in four, with the points plotted by name, or
//! by a key listed below where a name does not fit.

use super::canvas::{Canvas, Line, glyph};
use super::{Options, width};
use crate::quadrant::QuadrantChart;

/// Least columns inside each quadrant.
const MIN_W: i32 = 20;
/// Rows inside each quadrant.
const ROWS: i32 = 5;

/// Renders `chart` as text (MQUAD-4).
pub fn render(chart: &QuadrantChart, options: &Options) -> String {
    let quadrants: Vec<&str> = chart
        .quadrants
        .iter()
        .map(|q| q.as_deref().unwrap_or(""))
        .collect();
    let qw = quadrants.iter().map(|q| width(q) + 2).fold(MIN_W, i32::max);
    let y_labels = chart.y_axis.placed();
    let x_labels = chart.x_axis.placed();
    let gutter = y_labels
        .iter()
        .map(|(t, _)| width(t) + 1)
        .max()
        .unwrap_or(0);
    // The box's columns and rows: borders, then the middle lines.
    let (x0, y0) = (gutter, 0);
    let (xm, xr) = (x0 + qw + 1, x0 + 2 * qw + 2);
    let (ym, yb) = (y0 + ROWS + 1, y0 + 2 * ROWS + 2);
    let rows = yb + 1 + i32::from(!x_labels.is_empty());
    let mut canvas = Canvas::new(xr + 1, rows);
    canvas.hline(x0, xr, y0, Line::Light);
    canvas.hline(x0, xr, yb, Line::Light);
    canvas.hline(x0, xr, ym, Line::Light);
    canvas.vline(x0, y0, yb, Line::Light);
    canvas.vline(xr, y0, yb, Line::Light);
    canvas.vline(xm, y0, yb, Line::Light);

    // The cells holding text or a point, by row, as `(first, last)` columns.
    let mut taken: Vec<Vec<(i32, i32)>> = vec![Vec::new(); rows as usize];
    let put = |canvas: &mut Canvas, taken: &mut Vec<Vec<(i32, i32)>>, x: i32, y: i32, s: &str| {
        canvas.text(x, y, s);
        taken[y as usize].push((x, x + width(s) - 1));
    };
    // Quadrant labels: on each quadrant's top row, or its middle when there are no points.
    let row = if chart.points.is_empty() {
        (ROWS + 1) / 2
    } else {
        1
    };
    for (k, q) in quadrants.iter().enumerate().filter(|(_, q)| !q.is_empty()) {
        let left = if matches!(k, 0 | 3) { xm + 1 } else { x0 + 1 };
        let top = if k < 2 { y0 } else { ym };
        put(
            &mut canvas,
            &mut taken,
            left + (qw - width(q)) / 2,
            top + row,
            q,
        );
    }
    // An axis label centers on its half, or on the middle line for a low label alone.
    let along = |f: f64, start: i32, mid: i32, n: i32| match f {
        f if f < 0.5 => start + 1 + n / 2,
        f if f > 0.5 => mid + 1 + n / 2,
        _ => mid,
    };
    for (t, f) in &x_labels {
        let center = along(*f, x0, xm, qw);
        canvas.text(center - width(t) / 2, yb + 1, t);
    }
    for (t, f) in &y_labels {
        let y = along(1.0 - f, y0, ym, ROWS);
        canvas.text(x0 - 1 - width(t), y, t);
    }
    let cells: Vec<(i32, i32)> = chart
        .points
        .iter()
        .map(|p| {
            (
                x0 + 1 + (p.x * f64::from(2 * qw)).round() as i32,
                y0 + 1 + ((1.0 - p.y) * f64::from(2 * ROWS)).round() as i32,
            )
        })
        .collect();
    for &(x, y) in &cells {
        taken[y as usize].push((x, x));
    }
    // Each name beside its point where it fits inside one quadrant clear of other text, else a
    // key in place of the point, listed below.
    let mut keys = Vec::new();
    for (p, &(x, y)) in chart.points.iter().zip(&cells) {
        let w = width(&p.name);
        let fits = |a: i32, taken: &[(i32, i32)]| {
            let b = a + w - 1;
            let inside = a > x0 && b < xr && (b < xm || a > xm);
            inside && y != ym && taken.iter().all(|&(s, e)| b + 1 < s || a > e + 1)
        };
        let row = &taken[y as usize];
        let at = [x + 2, x - 1 - w].into_iter().find(|&a| fits(a, row));
        match at {
            Some(a) => {
                canvas.arrow(x, y, glyph('●', '*'));
                put(&mut canvas, &mut taken, a, y, &p.name);
            }
            None => {
                let k = key(keys.len());
                canvas.arrow(x, y, glyph(k, k));
                keys.push(format!("{k} {}", p.name));
            }
        }
    }

    let mut out = String::new();
    if let Some(t) = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pad = (x0 + (xr - x0 + 1 - width(t)) / 2).max(0) as usize;
        out.push_str(&" ".repeat(pad));
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str(&canvas.render(options.charset));
    if !keys.is_empty() {
        out.push('\n');
        for k in keys {
            out.push_str(&" ".repeat(x0 as usize));
            out.push_str(&k);
            out.push('\n');
        }
    }
    out
}

/// The `n`th key for a point whose name does not fit: `1` to `9`, then `a` to `z`, then `#`.
fn key(n: usize) -> char {
    match n {
        0..9 => char::from(b'1' + n as u8),
        9..35 => char::from(b'a' + (n - 9) as u8),
        _ => '#',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadrant::{Axis, Point};

    #[test]
    fn axis_labels() {
        let a = |low: Option<&str>, high: Option<&str>| Axis {
            low: low.map(Into::into),
            high: high.map(Into::into),
        };
        assert_eq!(a(Some("L"), Some("H")).placed(), [("L", 0.25), ("H", 0.75)]);
        assert_eq!(a(Some("L"), None).placed(), [("L", 0.5)]);
        assert_eq!(a(None, Some("H")).placed(), [("H", 0.75)]);
        assert!(a(None, None).placed().is_empty());
    }

    #[test]
    fn names_go_left_near_the_right_edge() {
        let chart = QuadrantChart {
            points: vec![Point {
                name: "Edge".into(),
                x: 1.0,
                y: 1.0,
                class: None,
                style: Default::default(),
            }],
            ..Default::default()
        };
        let out = render(&chart, &Options::default());
        assert!(out.lines().nth(1).unwrap().ends_with("Edge ●│"), "{out}");
    }

    #[test]
    fn keys() {
        assert_eq!(key(0), '1');
        assert_eq!(key(8), '9');
        assert_eq!(key(9), 'a');
        assert_eq!(key(34), 'z');
        assert_eq!(key(35), '#');
    }

    #[test]
    fn crowded_names_get_keys() {
        let at = |name: &str, x: f64| Point {
            name: name.into(),
            x,
            y: 0.8,
            class: None,
            style: Default::default(),
        };
        let chart = QuadrantChart {
            points: vec![at("Alpha", 0.1), at("Beta", 0.15), at("Center", 0.5)],
            ..Default::default()
        };
        let out = render(&chart, &Options::default());
        let row = out.lines().nth(3).unwrap();
        // Alpha has Beta's point on its right and the border on its left.
        assert!(row.contains("1 ● Beta"), "{out}");
        assert!(out.ends_with("\n\n1 Alpha\n"), "{out}");
        // A point on the middle line keeps its name on that line's right.
        assert!(row.contains("● Center"), "{out}");
    }
}
