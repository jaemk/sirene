//! Quadrant charts as text (MQUAD-4): a box split in four, with the points plotted by name, or
//! by a key listed below where a name does not fit.

use super::canvas::{Canvas, Line, glyph};
use super::{Options, width};
use crate::quadrant::QuadrantChart;
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthChar;

/// Least columns inside each quadrant.
const MIN_W: i32 = 20;
/// Rows inside each quadrant.
const ROWS: i32 = 5;

/// Occupied intervals in one row, merged so lookup does not scan all points.
#[derive(Default)]
struct Taken(BTreeMap<i32, i32>);

impl Taken {
    fn insert(&mut self, mut start: i32, mut end: i32) {
        if let Some((&s, &e)) = self.0.range(..=start).next_back()
            && e.saturating_add(1) >= start
        {
            start = s;
            end = end.max(e);
            self.0.remove(&s);
        }
        while let Some((&s, &e)) = self.0.range(start..).next()
            && s <= end.saturating_add(1)
        {
            end = end.max(e);
            self.0.remove(&s);
        }
        self.0.insert(start, end);
    }

    fn clear(&self, start: i32, end: i32) -> bool {
        self.0
            .range(..=end)
            .next_back()
            .is_none_or(|(_, &e)| e < start)
    }
}

/// Keep oversized axis labels readable without making the entire chart as wide as the label.
fn wrapped_axis(out: &mut String, label: &str, columns: i32) {
    let mut line = String::new();
    let mut used = 0;
    for ch in label.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as i32;
        if used + w > columns && !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
            line.clear();
            used = 0;
        }
        line.push(ch);
        used += w;
    }
    if !line.is_empty() {
        out.push_str(&line);
        out.push('\n');
    }
}

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
    let along = |f: f64, start: i32, mid: i32, n: i32| match f {
        f if f < 0.5 => start + 1 + n / 2,
        f if f > 0.5 => mid + 1 + n / 2,
        _ => mid,
    };
    let x_extents: Vec<(i32, i32)> = x_labels
        .iter()
        .map(|(t, f)| {
            let start = along(*f, x0, xm, qw) - width(t) / 2;
            (start, start + width(t) - 1)
        })
        .collect();
    let separate_x = x_extents.iter().any(|&(a, b)| a < x0 || b > xr)
        || x_extents.windows(2).any(|w| w[0].1 >= w[1].0);
    let rows = yb + 1 + i32::from(!x_labels.is_empty() && !separate_x);
    let mut canvas = Canvas::new(xr + 1, rows);
    canvas.hline(x0, xr, y0, Line::Light);
    canvas.hline(x0, xr, yb, Line::Light);
    canvas.hline(x0, xr, ym, Line::Light);
    canvas.vline(x0, y0, yb, Line::Light);
    canvas.vline(xr, y0, yb, Line::Light);
    canvas.vline(xm, y0, yb, Line::Light);

    // The cells holding text or a point, by row, as `(first, last)` columns.
    let mut taken: Vec<Taken> = (0..rows).map(|_| Taken::default()).collect();
    let put = |canvas: &mut Canvas, taken: &mut Vec<Taken>, x: i32, y: i32, s: &str| {
        canvas.text(x, y, s);
        taken[y as usize].insert(x, x + width(s) - 1);
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
    if !separate_x {
        for ((t, _), &(start, _)) in x_labels.iter().zip(&x_extents) {
            canvas.text(start, yb + 1, t);
        }
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
        taken[y as usize].insert(x, x);
    }
    // Each name beside its point where it fits inside one quadrant clear of other text, else a
    // key in place of the point, listed below.
    let mut keys = Vec::new();
    for (p, &(x, y)) in chart.points.iter().zip(&cells) {
        let w = width(&p.name);
        let fits = |a: i32, taken: &Taken| {
            let b = a + w - 1;
            let inside = a > x0 && b < xr && (b < xm || a > xm);
            inside && y != ym && taken.clear(a - 1, b + 1)
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
    if separate_x {
        out.push('\n');
        for (label, _) in &x_labels {
            wrapped_axis(&mut out, label, xr - x0 + 1);
        }
    }
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
pub(super) fn key(n: usize) -> char {
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

    #[test]
    fn long_axis_labels_are_wrapped_separately_without_loss() {
        let low = "低".repeat(35);
        let high = "高".repeat(35);
        let chart = QuadrantChart {
            x_axis: Axis {
                low: Some(low.clone()),
                high: Some(high.clone()),
            },
            ..Default::default()
        };
        let out = render(&chart, &Options::default());
        let labels: String = out.lines().skip(14).collect();
        assert_eq!(labels, format!("{low}{high}"), "{out}");
        assert!(out.lines().all(|line| width(line) <= 43), "{out}");
    }

    #[test]
    fn taken_intervals_merge_and_check_gaps() {
        let mut taken = Taken::default();
        for (a, b) in [(10, 12), (4, 5), (7, 8), (6, 6), (20, 20), (10, 10)] {
            taken.insert(a, b);
        }
        assert_eq!(taken.0.len(), 3);
        assert!(!taken.clear(2, 4));
        assert!(!taken.clear(8, 11));
        assert!(taken.clear(13, 19));
        assert!(!taken.clear(20, 21));
    }

    #[test]
    fn long_y_label_does_not_indent_every_wrapped_x_line() {
        let chart = QuadrantChart {
            x_axis: Axis {
                low: Some("x".repeat(5_000)),
                high: None,
            },
            y_axis: Axis {
                low: Some("y".repeat(5_000)),
                high: None,
            },
            ..Default::default()
        };
        let out = render(&chart, &Options::default());
        assert!(out.len() < 100_000, "output grew to {} bytes", out.len());
        assert_eq!(out.chars().filter(|&ch| ch == 'x').count(), 5_000);
    }
}
