//! Ishikawa diagrams as text (MISH-4): the effect, then an outline of its causes.

use super::{Charset, Options, width};
use crate::ishikawa::{Cause, Ishikawa};

/// The branch, last branch, continuing, and blank prefixes.
fn glyphs(charset: Charset) -> [&'static str; 4] {
    match charset {
        Charset::Unicode => ["├── ", "└── ", "│   ", "    "],
        Charset::Ascii => ["+-- ", "`-- ", "|   ", "    "],
    }
}

fn outline(causes: &[Cause], prefix: &str, g: &[&str; 4], rows: &mut Vec<String>) {
    for (i, c) in causes.iter().enumerate() {
        let last = i + 1 == causes.len();
        let (head, rest) = if last { (g[1], g[3]) } else { (g[0], g[2]) };
        let child = format!("{prefix}{rest}");
        for (k, l) in c.lines.iter().enumerate() {
            if k == 0 {
                rows.push(format!("{prefix}{head}{l}"));
            } else {
                rows.push(format!("{child}{l}"));
            }
        }
        outline(&c.children, &child, g, rows);
    }
}

/// Renders `chart` as text (MISH-4).
pub fn render(chart: &Ishikawa, options: &Options) -> String {
    let g = glyphs(options.charset);
    let mut rows = Vec::new();
    if let Some(effect) = &chart.effect {
        rows.extend(effect.lines.iter().cloned());
        outline(&effect.children, "", &g, &mut rows);
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
