//! Venn diagrams as text (MVENN-5): each set and overlap on a line, with its text nodes under
//! it.

use super::{Options, width};
use crate::venn::Venn;

/// Renders `chart` as text (MVENN-5).
pub fn render(chart: &Venn, _options: &Options) -> String {
    let mut rows: Vec<String> = Vec::new();
    let mut listed: Vec<&[String]> = Vec::new();
    let heading = |sets: &[String]| -> String {
        let names: Vec<&str> = sets.iter().map(|s| chart.name(s)).collect();
        names.join(" & ")
    };
    let texts = |rows: &mut Vec<String>, sets: &[String]| {
        for t in chart.texts.iter().filter(|t| t.sets == sets) {
            rows.push(format!("  - {}", t.text()));
        }
    };
    for s in &chart.subsets {
        if listed.contains(&s.sets.as_slice()) {
            continue;
        }
        listed.push(&s.sets);
        let label = chart
            .subsets
            .iter()
            .rev()
            .filter(|o| o.sets == s.sets)
            .find_map(|o| o.label.as_deref());
        rows.push(match (s.sets.len(), label) {
            (1, _) => heading(&s.sets),
            (_, Some(l)) => format!("{}: {l}", heading(&s.sets)),
            (_, None) => heading(&s.sets),
        });
        texts(&mut rows, &s.sets);
    }
    // Text nodes in regions no statement declares.
    for t in &chart.texts {
        if !listed.contains(&t.sets.as_slice()) {
            listed.push(&t.sets);
            rows.push(heading(&t.sets));
            texts(&mut rows, &t.sets);
        }
    }
    let mut out = String::new();
    if let Some(t) = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
        let pad = (w - width(t)).max(0) / 2;
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
