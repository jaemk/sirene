//! XY charts as text (MXY-4): a row per x position, with a bar for each bar series and a marker
//! for each line series, scaled across the value axis.

use super::{Charset, Options, width};
use crate::xychart::{SeriesKind, XyChart, format_number};

/// Cells across the value axis.
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

/// A marker at `share` (0 to 1) across `FIELD` cells.
fn marker(share: f64, charset: Charset) -> String {
    let at = (share.clamp(0.0, 1.0) * (FIELD - 1) as f64).round() as usize;
    let mut out = " ".repeat(at);
    out.push(match charset {
        Charset::Unicode => '●',
        Charset::Ascii => '*',
    });
    out.push_str(&" ".repeat(FIELD - 1 - at));
    out
}

/// Renders `chart` as text (MXY-4).
pub fn render(chart: &XyChart, options: &Options) -> String {
    let n = chart.positions();
    let (lo, hi) = chart.y_bounds();
    let share = |v: f64| (v - lo) / (hi - lo);
    let labels = chart.x_labels();
    let label_w = labels
        .iter()
        .map(|l| width(l))
        .chain(chart.x_title.as_deref().map(width))
        .max()
        .unwrap_or(0) as usize;
    // Each series' cells: the field and the value (and a line's point label).
    let mut columns: Vec<(String, Vec<String>)> = Vec::new();
    let (mut lines, mut bars) = (0, 0);
    for s in &chart.series {
        let kind = match s.kind {
            SeriesKind::Line => {
                lines += 1;
                format!("line {lines}")
            }
            SeriesKind::Bar => {
                bars += 1;
                format!("bar {bars}")
            }
        };
        let values: Vec<String> = s
            .values
            .iter()
            .zip(&s.labels)
            .take(n)
            .map(|(v, l)| match l {
                Some(l) => format!("{} {l}", format_number(*v)),
                None => format_number(*v),
            })
            .collect();
        let value_w = values.iter().map(|v| width(v)).max().unwrap_or(0) as usize;
        let cells = (0..n)
            .map(|i| {
                let Some(&v) = s.values.get(i) else {
                    return String::new();
                };
                let field = match s.kind {
                    SeriesKind::Bar => bar(share(v), options.charset),
                    SeriesKind::Line => marker(share(v), options.charset),
                };
                let value = &values[i];
                let pad = value_w - width(value) as usize;
                format!("{field} {value}{}", " ".repeat(pad))
            })
            .collect();
        columns.push((s.name.clone().unwrap_or(kind), cells));
    }
    let col_w: Vec<usize> = columns
        .iter()
        .map(|(h, cells)| {
            cells
                .iter()
                .map(|c| width(c))
                .chain([width(h)])
                .max()
                .unwrap_or(0) as usize
        })
        .collect();
    let row = |first: &str, cells: Vec<&str>| {
        let mut out = format!("{first}{}", " ".repeat(label_w - width(first) as usize));
        for (c, w) in cells.iter().zip(&col_w) {
            out.push_str("  ");
            out.push_str(c);
            out.push_str(&" ".repeat(w - width(c) as usize));
        }
        out.trim_end().to_owned()
    };
    let mut rows = Vec::new();
    if !columns.is_empty() {
        rows.push(row(
            chart.x_title.as_deref().unwrap_or(""),
            columns.iter().map(|(h, _)| h.as_str()).collect(),
        ));
    }
    for (i, l) in labels.iter().enumerate() {
        rows.push(row(l, columns.iter().map(|(_, c)| c[i].as_str()).collect()));
    }
    // The value axis's span under the table.
    let span = format!("{} to {}", format_number(lo), format_number(hi));
    rows.push(String::new());
    rows.push(match &chart.y_title {
        Some(t) => format!("{t}: {span}"),
        None => span,
    });

    let mut out = String::new();
    if let Some(t) = chart
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

    #[test]
    fn fields() {
        assert_eq!(bar(1.0, Charset::Unicode), "█".repeat(FIELD));
        assert_eq!(bar(0.0, Charset::Ascii), " ".repeat(FIELD));
        assert!(bar(0.5, Charset::Ascii).starts_with(&"#".repeat(12)));
        assert!(bar(1.0 / 48.0, Charset::Unicode).starts_with('▌'));
        assert_eq!(marker(0.0, Charset::Ascii).trim_end(), "*");
        assert_eq!(marker(1.0, Charset::Unicode).trim_start(), "●");
        assert_eq!(marker(2.0, Charset::Ascii).chars().count(), FIELD);
    }
}
