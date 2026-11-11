//! Wardley maps as text (MWARD-5): the components plotted by name over the evolution stages,
//! or by a key listed below where a name does not fit, then the links, evolve trends,
//! pipelines, notes, annotations, and accelerators as lists.

use super::canvas::{Canvas, Line, glyph};
use super::quadrant::key;
use super::{Options, width};
use crate::wardley::{Flow, NodeKind, Wardley};

/// Least columns for each stage.
const MIN_STAGE: i32 = 16;
/// Rows in the plot.
const ROWS: i32 = 15;

/// Renders `map` as text (MWARD-5).
pub fn render(map: &Wardley, options: &Options) -> String {
    let stages = map.stage_names();
    let spans: Vec<f64> = if map.boundaries.len() == stages.len() {
        let mut prev = 0.0;
        map.boundaries
            .iter()
            .map(|&b| {
                let w = b - prev;
                prev = b;
                w
            })
            .collect()
    } else {
        vec![1.0 / stages.len() as f64; stages.len()]
    };
    // Columns inside the plot: wide enough for every stage's name in its span.
    let inner = stages
        .iter()
        .zip(&spans)
        .map(|(s, &f)| {
            let need = f64::from(width(s).max(MIN_STAGE - 2) + 2);
            (need / f.max(0.05)).ceil() as i32
        })
        .fold(MIN_STAGE * stages.len() as i32, i32::max);
    // The plot's left border is column 0; its bottom border is row ROWS.
    let (x0, xr) = (0, inner + 1);
    let mut canvas = Canvas::new(xr + 1, ROWS + 2);
    canvas.hline(x0, xr, ROWS, Line::Light);
    canvas.vline(x0, 0, ROWS, Line::Light);
    let col = |f: f64| x0 + 1 + (f * f64::from(inner - 1)).round() as i32;
    let mut edge = 0.0;
    for (i, (s, &f)) in stages.iter().zip(&spans).enumerate() {
        let (a, b) = (col(edge), col(edge + f));
        if i > 0 {
            canvas.vline(a, 0, ROWS - 1, Line::Dotted);
        }
        canvas.text((a + b - width(s)) / 2, ROWS + 1, s);
        edge += f;
    }

    // The cells holding text or a point, by row, as `(first, last)` columns.
    let mut taken: Vec<Vec<(i32, i32)>> = vec![Vec::new(); ROWS as usize];
    let cells: Vec<(i32, i32)> = map
        .nodes
        .iter()
        .map(|n| {
            let y = ((1.0 - n.y / 100.0) * f64::from(ROWS - 1)).round() as i32;
            // A pipeline's parent sits a row above its components, as it does when drawn.
            let y = y - i32::from(n.pipeline_parent);
            (col(n.x / 100.0), y.clamp(0, ROWS - 1))
        })
        .collect();
    for &(x, y) in &cells {
        taken[y as usize].push((x, x));
    }
    let mut keys = Vec::new();
    for (n, &(x, y)) in map.nodes.iter().zip(&cells) {
        let mut name = n.label.clone();
        let mut notes: Vec<&str> = n.strategy.map(|s| s.name()).into_iter().collect();
        if n.inertia {
            notes.push("inertia");
        }
        if !notes.is_empty() {
            name = format!("{name} ({})", notes.join(", "));
        }
        let w = width(&name);
        let row = &taken[y as usize];
        let fits = |a: i32| {
            let b = a + w - 1;
            a > x0 && b < xr && row.iter().all(|&(s, e)| b + 1 < s || a > e + 1)
        };
        let point = match (n.kind, n.pipeline_parent) {
            (NodeKind::Anchor, _) => glyph('◆', '@'),
            (_, true) => glyph('■', '#'),
            _ => glyph('●', '*'),
        };
        match [x + 2, x - 1 - w].into_iter().find(|&a| fits(a)) {
            Some(a) => {
                canvas.arrow(x, y, point);
                canvas.text(a, y, &name);
                taken[y as usize].push((a, a + w - 1));
            }
            None => {
                let k = key(keys.len());
                canvas.arrow(x, y, glyph(k, k));
                keys.push(format!("{k} {name}"));
            }
        }
    }

    let mut out = String::new();
    if let Some(t) = map
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pad = ((xr + 1 - width(t)) / 2).max(0) as usize;
        out.push_str(&" ".repeat(pad));
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str("Visibility\n");
    out.push_str(&canvas.render(options.charset));
    let label = "Evolution";
    out.push_str(&" ".repeat(((xr + 1 - width(label)) / 2).max(0) as usize));
    out.push_str(label);
    out.push('\n');

    let mut sections: Vec<(&str, Vec<String>)> = Vec::new();
    sections.push(("Key", keys));
    let name = |i: usize| map.nodes[i].label.as_str();
    let arrow = |dashed: bool, flow: Option<Flow>| {
        let line = if dashed { "-.-" } else { "--" };
        let (l, r) = match flow {
            Some(Flow::Forward) => ("", ">"),
            Some(Flow::Backward) => ("<", ""),
            Some(Flow::Both) => ("<", ">"),
            None => ("", ""),
        };
        format!("{l}{line}{r}")
    };
    sections.push((
        "Links",
        map.links
            .iter()
            .map(|l| {
                let mut s = format!(
                    "{} {} {}",
                    name(l.source),
                    arrow(l.dashed, l.flow),
                    name(l.target)
                );
                if let Some(t) = &l.label {
                    s.push_str(&format!(" ({t})"));
                }
                s
            })
            .collect(),
    ));
    sections.push((
        "Evolve",
        map.trends
            .iter()
            .map(|t| format!("{} -> {}", name(t.node), number(t.x)))
            .collect(),
    ));
    sections.push((
        "Pipelines",
        map.pipelines
            .iter()
            .map(|p| {
                let parts: Vec<&str> = p.components.iter().map(|&i| name(i)).collect();
                format!("{}: {}", name(p.parent), parts.join(", "))
            })
            .collect(),
    ));
    sections.push(("Notes", map.notes.iter().map(|n| n.text.clone()).collect()));
    let mut annotations: Vec<_> = map.annotations.iter().collect();
    annotations.sort_by_key(|a| a.number);
    sections.push((
        "Annotations",
        annotations
            .iter()
            .map(|a| format!("{}. {}", a.number, a.text))
            .collect(),
    ));
    sections.push((
        "Accelerators",
        map.accelerators.iter().map(|m| m.text.clone()).collect(),
    ));
    sections.push((
        "Deaccelerators",
        map.deaccelerators.iter().map(|m| m.text.clone()).collect(),
    ));
    for (heading, rows) in sections {
        if rows.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(heading);
        out.push_str(":\n");
        for r in rows {
            out.push_str("  ");
            out.push_str(r.trim_end());
            out.push('\n');
        }
    }
    out
}

/// A percentage as Mermaid writes it, from 0 to 1.
fn number(percent: f64) -> String {
    let v = (percent / 100.0 * 1000.0).round() / 1000.0;
    format!("{v}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(src: &str) -> String {
        let (map, d) = crate::wardley::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        render(&map, &Options::default())
    }

    #[test]
    fn plots_and_lists() {
        let out = draw(
            "wardley-beta\ntitle Tea\nanchor Business [0.95, 0.63]\n\
             component Cup [0.73, 0.78] (buy)\nBusiness -> Cup\nevolve Cup 0.9\n",
        );
        assert!(out.contains("◆ Business"), "{out}");
        assert!(out.contains("● Cup (buy)"), "{out}");
        for want in [
            "Genesis",
            "Custom Built",
            "Product",
            "Commodity",
            "Evolution",
        ] {
            assert!(out.contains(want), "{want}: {out}");
        }
        assert!(
            out.ends_with("Links:\n  Business -- Cup\n\nEvolve:\n  Cup -> 0.9\n"),
            "{out}"
        );
    }

    #[test]
    fn crowded_names_get_keys() {
        let out = draw(
            "wardley-beta\ncomponent Alpha [0.5, 0.5]\ncomponent Beta [0.5, 0.51]\n\
             component Gamma [0.5, 0.52]\n",
        );
        assert!(out.contains("Key:\n  1 "), "{out}");
    }
}
