//! Radar charts as text (MRADAR-4): a group per axis with a bar per curve, scaled from the
//! minimum to the maximum.

use super::{Charset, Options, width};
use crate::radar::Radar;
use crate::xychart::format_number;

/// Cells across a bar.
const FIELD: usize = 24;

/// Eighth blocks, from one eighth to seven.
const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A bar for `share` (0 to 1) of `FIELD` cells, padded to `FIELD`.
fn bar(share: f64, charset: Charset) -> String {
    let cells = share.clamp(0.0, 1.0) * FIELD as f64;
    let mut out = String::new();
    match charset {
        Charset::Unicode => {
            let eighths = (cells * 8.0).round() as usize;
            out.extend(std::iter::repeat_n('█', eighths / 8));
            if let Some(&e) = EIGHTHS.get((eighths % 8).wrapping_sub(1)) {
                out.push(e);
            }
        }
        Charset::Ascii => out.extend(std::iter::repeat_n('#', cells.round() as usize)),
    }
    let n = out.chars().count();
    out.extend(std::iter::repeat_n(' ', FIELD - n));
    out
}

fn pad(s: &str, w: i32) -> String {
    format!("{s}{}", " ".repeat((w - width(s)).max(0) as usize))
}

/// Renders `radar` as text (MRADAR-4).
pub fn render(radar: &Radar, options: &Options) -> String {
    let axis_w = radar
        .axes
        .iter()
        .map(|a| width(&a.label))
        .max()
        .unwrap_or(0);
    let curve_w = radar
        .curves
        .iter()
        .map(|c| width(&c.label))
        .max()
        .unwrap_or(0);
    let mut rows = Vec::new();
    if !radar.curves.is_empty() {
        for (i, a) in radar.axes.iter().enumerate() {
            for (k, c) in radar.curves.iter().enumerate() {
                let v = c.values.get(i).copied().unwrap_or(radar.min);
                let first = if k == 0 { a.label.as_str() } else { "" };
                let row = format!(
                    "{}  {}  {} {}",
                    pad(first, axis_w),
                    pad(&c.label, curve_w),
                    bar(radar.share(v), options.charset),
                    format_number(v)
                );
                rows.push(row);
            }
        }
    } else {
        rows.extend(radar.axes.iter().map(|a| a.label.clone()));
    }
    if !radar.axes.is_empty() {
        let (lo, hi) = radar.bounds();
        rows.push(String::new());
        rows.push(format!("{} to {}", format_number(lo), format_number(hi)));
    }

    let mut out = String::new();
    if let Some(t) = radar
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
        out.push_str(&" ".repeat(((w - width(t)).max(0) / 2) as usize));
        out.push_str(t);
        out.push_str("\n\n");
    }
    for r in rows {
        out.push_str(&r);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::radar::{Axis, Curve};

    #[test]
    fn groups() {
        let axis = |id: &str| Axis {
            id: id.into(),
            label: id.into(),
        };
        let r = Radar {
            axes: vec![axis("x"), axis("yy")],
            curves: vec![
                Curve {
                    id: "a".into(),
                    label: "a".into(),
                    values: vec![4.0, 2.0],
                },
                Curve {
                    id: "b".into(),
                    label: "bee".into(),
                    values: vec![0.0, 1.0],
                },
            ],
            ..Radar::default()
        };
        let out = render(
            &r,
            &Options {
                charset: Charset::Ascii,
            },
        );
        let full = "#".repeat(FIELD);
        let half = format!("{}{}", "#".repeat(FIELD / 2), " ".repeat(FIELD / 2));
        let quarter = format!("{}{}", "#".repeat(FIELD / 4), " ".repeat(3 * FIELD / 4));
        let none = " ".repeat(FIELD);
        assert_eq!(
            out,
            format!(
                "x   a    {full} 4\n    bee  {none} 0\nyy  a    {half} 2\n    bee  {quarter} 1\n\n0 to 4\n"
            )
        );
    }
}
