//! XY charts as text (MXY-4): a row per x position, with a bar for each bar series and a marker
//! for each line series, scaled across the value axis.

use super::{Charset, Options, width};
use crate::xychart::{SeriesKind, XyChart, format_number};

/// Cells across the value axis.
const FIELD: usize = 24;
// The ordinary table repeats every column's width for every position. Switch to a
// series-oriented outline before that expansion becomes large.
const TABLE_BUDGET: usize = 1_000_000;
const MAX_TITLE_PADDING: usize = 256;

/// Eighth blocks, from one eighth to seven.
const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A bar for `share` (0 to 1) of `FIELD` cells, padded to `FIELD`.
fn bar(share: f64, charset: Charset) -> String {
    let cells = share.clamp(0.0, 1.0) * FIELD as f64;
    let mut out = String::new();
    match charset {
        Charset::Unicode | Charset::UnicodeMath => {
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
        Charset::Unicode | Charset::UnicodeMath => '●',
        Charset::Ascii => '*',
    });
    out.push_str(&" ".repeat(FIELD - 1 - at));
    out
}

/// Renders `chart` as text (MXY-4).
pub fn render(chart: &XyChart, options: &Options) -> String {
    let n = chart.positions();
    let (lo, hi) = chart.y_bounds();
    let share = |v: f64| crate::xychart::fraction(v, lo, hi);
    let labels = chart.x_labels();
    let label_w = labels
        .iter()
        .map(|l| width(l))
        .chain(chart.x_title.as_deref().map(width))
        .max()
        .unwrap_or(0) as usize;
    // Estimate the table before building its n × series cells. Widths are display
    // cells; multiplying by four also bounds UTF-8 bytes for the repeated padding.
    let mut row_width = label_w;
    for s in &chart.series {
        let mut value_w = 0usize;
        for (i, v) in s.values.iter().take(n).enumerate() {
            let mut w = width(&format_number(*v)) as usize;
            if let Some(Some(label)) = s.labels.get(i) {
                w = w.saturating_add(1).saturating_add(width(label) as usize);
            }
            value_w = value_w.max(w);
        }
        let header_w = s.name.as_deref().map(width).unwrap_or(20) as usize;
        row_width = row_width
            .saturating_add(2)
            .saturating_add(header_w.max(FIELD.saturating_add(1).saturating_add(value_w)));
    }
    if row_width
        .saturating_mul(n.saturating_add(1))
        .saturating_mul(4)
        > TABLE_BUDGET
    {
        return render_compact(chart, &labels, lo, hi, options);
    }
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
            .take(n)
            .enumerate()
            .map(|(i, v)| match s.labels.get(i).and_then(Option::as_ref) {
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
        out.push_str(&" ".repeat((((w - width(t)).max(0) / 2) as usize).min(MAX_TITLE_PADDING)));
        out.push_str(t);
        out.push_str("\n\n");
    }
    for r in rows {
        out.push_str(&r);
        out.push('\n');
    }
    out
}

/// An outline whose work is proportional to the supplied labels and values, rather than
/// the product of positions and series. Position numbers relate the two lists.
fn render_compact(
    chart: &XyChart,
    labels: &[String],
    lo: f64,
    hi: f64,
    options: &Options,
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if let Some(title) = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let _ = writeln!(out, "{title}\n");
    }
    if let Some(title) = &chart.x_title {
        let _ = writeln!(out, "{title}:");
    }
    for (i, label) in labels.iter().enumerate() {
        let _ = writeln!(out, "{}: {label}", i + 1);
    }
    let (mut lines, mut bars) = (0, 0);
    for series in &chart.series {
        let kind = match series.kind {
            SeriesKind::Line => {
                lines += 1;
                format!("line {lines}")
            }
            SeriesKind::Bar => {
                bars += 1;
                format!("bar {bars}")
            }
        };
        let _ = writeln!(out, "\n{}:", series.name.as_deref().unwrap_or(&kind));
        for (i, &value) in series.values.iter().take(labels.len()).enumerate() {
            let share = crate::xychart::fraction(value, lo, hi);
            let field = match series.kind {
                SeriesKind::Line => marker(share, options.charset),
                SeriesKind::Bar => bar(share, options.charset),
            };
            let _ = write!(out, "{}: {field} {}", i + 1, format_number(value));
            if let Some(Some(label)) = series.labels.get(i) {
                let _ = write!(out, " {label}");
            }
            out.push('\n');
        }
    }
    let span = format!("{} to {}", format_number(lo), format_number(hi));
    let _ = writeln!(
        out,
        "\n{}",
        match &chart.y_title {
            Some(title) => format!("{title}: {span}"),
            None => span,
        }
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_sparse_table_uses_compact_outline() {
        use crate::xychart::{Series, XAxis};
        let long_label = "x".repeat(10_000);
        let mut categories = vec![long_label.clone()];
        categories.extend((1..1000).map(|i| format!("category{i}")));
        let chart = XyChart {
            x_axis: XAxis::Categories(categories),
            series: (0..1000)
                .map(|i| Series {
                    kind: SeriesKind::Line,
                    name: Some(format!("series{i}")),
                    values: vec![1.0],
                    labels: vec![Some("point".into())],
                })
                .collect(),
            ..XyChart::default()
        };
        let rendered = render(&chart, &Options::default());
        assert_eq!(rendered.matches(&long_label).count(), 1);
        assert_eq!(rendered.matches("point").count(), 1000);
        assert!(rendered.contains("series999:"));
        assert!(
            rendered.len() < 200_000,
            "output was {} bytes",
            rendered.len()
        );
    }

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
