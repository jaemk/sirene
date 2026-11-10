//! Sankey diagrams as text (MSANKEY-4): each node in column order with its value, and under it
//! a branch per outgoing link with a bar for its share of the largest link.

use super::{Charset, Options, width};
use crate::sankey::Sankey;

/// Cells across a bar.
pub(super) const FIELD: usize = 24;

/// Eighth blocks, from one eighth to seven.
const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A bar for `share` (0 to 1) of `FIELD` cells, at least a sliver when above 0.
pub(super) fn bar(share: f64, charset: Charset) -> String {
    let cells = share.clamp(0.0, 1.0) * FIELD as f64;
    let mut out = String::new();
    match charset {
        Charset::Unicode => {
            let eighths = ((cells * 8.0).round() as usize).max(usize::from(share > 0.0));
            out.extend(std::iter::repeat_n('█', eighths / 8));
            if let Some(&e) = EIGHTHS.get((eighths % 8).wrapping_sub(1)) {
                out.push(e);
            }
        }
        Charset::Ascii => {
            let n = (cells.round() as usize).max(usize::from(share > 0.0));
            out.extend(std::iter::repeat_n('#', n));
        }
    }
    out
}

fn pad(s: &str, w: i32) -> String {
    format!("{s}{}", " ".repeat((w - width(s)).max(0) as usize))
}

/// Renders `sankey` as text (MSANKEY-4).
pub fn render(sankey: &Sankey, options: &Options) -> String {
    let layout = sankey.layout();
    let (tee, elbow) = match options.charset {
        Charset::Unicode => ("├─ ", "└─ "),
        Charset::Ascii => ("+- ", "`- "),
    };
    let max = sankey.links.iter().map(|l| l.value).fold(0.0, f64::max);
    let target_w = sankey
        .links
        .iter()
        .map(|l| width(&sankey.nodes[l.target]))
        .max()
        .unwrap_or(0);
    let value_w = sankey
        .links
        .iter()
        .map(|l| width(&sankey.format_value(l.value)))
        .max()
        .unwrap_or(0);
    let mut rows = Vec::new();
    for column in &layout.columns {
        for &i in column {
            let n = &layout.nodes[i];
            if !rows.is_empty() {
                rows.push(String::new());
            }
            rows.push(format!(
                "{}  {}",
                sankey.nodes[i],
                sankey.format_value(n.value)
            ));
            // Outgoing links, top to bottom as drawn.
            let mut out: Vec<usize> = (0..sankey.links.len())
                .filter(|&k| sankey.links[k].source == i)
                .collect();
            out.sort_by(|&a, &b| layout.links[a].y0.total_cmp(&layout.links[b].y0));
            for (j, &k) in out.iter().enumerate() {
                let l = &sankey.links[k];
                let branch = if j + 1 == out.len() { elbow } else { tee };
                let value = sankey.format_value(l.value);
                let share = if max > 0.0 { l.value / max } else { 0.0 };
                let row = format!(
                    "{branch}{}  {}{value}  {}",
                    pad(&sankey.nodes[l.target], target_w),
                    " ".repeat((value_w - width(&value)).max(0) as usize),
                    bar(share, options.charset)
                );
                rows.push(row.trim_end().to_owned());
            }
        }
    }

    let mut out = String::new();
    if let Some(t) = sankey
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
    use crate::sankey::Link;

    #[test]
    fn branches() {
        let s = Sankey {
            nodes: vec!["a".into(), "bb".into(), "c".into()],
            links: vec![
                Link {
                    source: 0,
                    target: 1,
                    value: 8.0,
                },
                Link {
                    source: 0,
                    target: 2,
                    value: 0.01,
                },
            ],
            ..Sankey::default()
        };
        let out = render(
            &s,
            &Options {
                charset: Charset::Ascii,
            },
        );
        // The tiny link still gets a sliver of a bar.
        assert_eq!(
            out,
            format!(
                "a  8.01\n+- bb     8  {}\n`- c   0.01  #\n\nbb  8\n\nc  0.01\n",
                "#".repeat(FIELD)
            )
        );
    }
}
