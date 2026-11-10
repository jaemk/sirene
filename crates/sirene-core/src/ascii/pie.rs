//! Pie charts as text (MPIE-4).

use super::{Charset, Options, width};
use crate::pie::{Pie, percent};

/// The length of a 100% bar, in cells.
const BAR: usize = 40;

/// Eighth blocks, from one eighth to seven.
const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A bar for `share` (0 to 1) of `BAR` cells, padded to `BAR`.
fn bar(share: f64, charset: Charset) -> String {
    let cells = share.clamp(0.0, 1.0) * BAR as f64;
    let mut out = String::new();
    match charset {
        Charset::Unicode => {
            let eighths = (cells * 8.0).round() as usize;
            out.extend(std::iter::repeat_n('█', eighths / 8));
            let rest = eighths % 8;
            if rest > 0 {
                out.push(EIGHTHS[rest - 1]);
            }
        }
        Charset::Ascii => out.extend(std::iter::repeat_n('#', cells.round() as usize)),
    }
    let n = out.chars().count();
    out.extend(std::iter::repeat_n(' ', BAR - n));
    out
}

/// Renders `pie` as text (MPIE-4).
pub fn render(pie: &Pie, options: &Options) -> String {
    let total = pie.total();
    let ordered = pie.ordered();
    let label_w = ordered.iter().map(|s| width(&s.label)).max().unwrap_or(0) as usize;
    let values: Vec<String> = ordered.iter().map(|s| s.value.to_string()).collect();
    let value_w = values.iter().map(String::len).max().unwrap_or(0);
    let mut rows = Vec::new();
    for (s, value) in ordered.iter().zip(&values) {
        let share = if total > 0.0 { s.value / total } else { 0.0 };
        let pad = label_w - width(&s.label) as usize;
        let mut row = format!(
            "{}{}  {}  {:>4}",
            s.label,
            " ".repeat(pad),
            bar(share, options.charset),
            format!("{}%", percent(s.value, total)),
        );
        if pie.show_data {
            row.push_str(&format!("  {value:>value_w$}"));
        }
        rows.push(row);
    }
    let table_w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
    let mut out = String::new();
    if let Some(t) = pie
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pad = (table_w - width(t)).max(0) / 2;
        out.push_str(&" ".repeat(pad as usize));
        out.push_str(t);
        out.push('\n');
        if !rows.is_empty() {
            out.push('\n');
        }
    }
    for r in rows {
        out.push_str(r.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars() {
        assert_eq!(bar(1.0, Charset::Unicode), "█".repeat(BAR));
        assert_eq!(bar(0.0, Charset::Ascii), " ".repeat(BAR));
        assert!(bar(0.5, Charset::Ascii).starts_with(&"#".repeat(20)));
        assert!(bar(0.0125, Charset::Unicode).starts_with('▌'));
        assert!(bar(1.0 / 80.0 + 0.025, Charset::Unicode).starts_with("█▌"));
    }
}
