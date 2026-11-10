//! Gantt charts as text (MGANTT-6).

use unicode_width::UnicodeWidthChar;

use super::{Charset, Options, width};
use crate::gantt::time::DAY;
use crate::gantt::{Gantt, Time};

/// Width of the time axis, in cells.
const W: usize = 60;

/// A row of chart cells; a wide character's second cell is empty.
type Cells = Vec<String>;

fn blank() -> Cells {
    vec![" ".to_owned(); W]
}

/// Writes `text` into `cells` from `at`, stopping at `end`; returns where it stopped.
fn write(cells: &mut Cells, at: usize, end: usize, text: &str) -> usize {
    let mut c = at;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(1).max(1);
        if c + w > end.min(cells.len()) {
            break;
        }
        cells[c] = ch.to_string();
        for k in 1..w {
            cells[c + k] = String::new();
        }
        c += w;
    }
    c
}

/// `line` with `text` placed from column `at`, padding with spaces; text already there stays
/// unless overwritten.
fn place(line: &mut String, at: usize, text: &str) {
    let w = width(line) as usize;
    if w < at {
        line.push_str(&" ".repeat(at - w));
    }
    line.push_str(text);
}

/// Renders `gantt` as text (MGANTT-6).
pub fn render(gantt: &Gantt, options: &Options) -> String {
    let unicode = options.charset == Charset::Unicode;
    let g = |u: &str, a: &str| if unicode { u.to_owned() } else { a.to_owned() };
    let title = gantt
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let Some((t0, t1)) = gantt.range() else {
        return title.map_or_else(String::new, |t| format!("{t}\n"));
    };
    let span = (t1 - t0) as f64;
    let col = |t: Time| {
        ((t - t0) as f64 / span * W as f64)
            .round()
            .clamp(0.0, W as f64) as usize
    };
    let mark = |t: Time| col(t).min(W - 1);
    // The cells every task row starts from: excluded days, today, and `vert` lines.
    let mut base = blank();
    for (c, cell) in base.iter_mut().enumerate() {
        let tc = t0 + ((c as f64 + 0.5) / W as f64 * span) as i64;
        let day = t0 + (tc - t0).div_euclid(DAY) * DAY;
        if !gantt.excludes.is_empty() && gantt.is_excluded(day) {
            *cell = g("·", ".");
        }
    }
    if gantt.today_marker != crate::gantt::TodayMarker::Off && (t0..=t1).contains(&gantt.now) {
        base[mark(gantt.now)] = g("┆", ":");
    }
    let verts: Vec<(&str, usize)> = gantt
        .tasks()
        .filter(|t| t.tags.vert)
        .map(|t| (t.name.as_str(), mark(t.start)))
        .collect();
    for &(_, c) in &verts {
        base[c] = g("│", "|");
    }
    // Rows: a label and the chart cells, if any.
    let mut rows: Vec<(String, Option<Cells>)> = Vec::new();
    let mut last_section = None;
    for row in gantt.rows() {
        let section = &gantt.sections[row.section];
        let first = last_section != Some(row.section);
        last_section = Some(row.section);
        if !gantt.compact
            && first
            && let Some(name) = &section.name
        {
            rows.push((name.clone(), None));
        }
        let mut cells = base.clone();
        for &i in &row.tasks {
            let t = &section.tasks[i];
            if t.tags.milestone {
                cells[mark(t.start + (t.drawn_end - t.start) / 2)] = g("◆", "*");
                continue;
            }
            let a = col(t.start).min(W - 1);
            let b = col(t.drawn_end).max(a + 1).min(W);
            let fill = if t.tags.done {
                g("░", "-")
            } else if t.tags.active {
                g("▓", "=")
            } else {
                g("█", "#")
            };
            for cell in &mut cells[a..b] {
                *cell = fill.clone();
            }
        }
        if gantt.compact {
            // Names over their bars, or after them up to the next task.
            for &i in &row.tasks {
                let t = &section.tasks[i];
                let a = col(t.start).min(W - 1);
                let b = col(t.drawn_end).max(a + 1).min(W);
                let next = row
                    .tasks
                    .iter()
                    .map(|&j| col(section.tasks[j].start))
                    .filter(|&s| s >= b)
                    .min()
                    .unwrap_or(W);
                if t.tags.milestone {
                    write(&mut cells, (mark(t.start) + 2).min(W), next, &t.name);
                } else if (width(&t.name) as usize) <= b - a {
                    write(&mut cells, a, b, &t.name);
                } else {
                    write(&mut cells, (b + 1).min(W), next, &t.name);
                }
            }
            let label = if first {
                section.name.clone().unwrap_or_default()
            } else {
                String::new()
            };
            rows.push((label, Some(cells)));
        } else {
            let t = &section.tasks[row.tasks[0]];
            let marker = if t.tags.crit { "! " } else { "  " };
            let indent = if section.name.is_some() { marker } else { "" };
            let label = if section.name.is_none() && t.tags.crit {
                format!("! {}", t.name)
            } else {
                format!("{indent}{}", t.name)
            };
            rows.push((label, Some(cells)));
        }
    }
    let name_w = rows
        .iter()
        .map(|(l, _)| width(l) as usize)
        .max()
        .unwrap_or(0);
    let x = name_w + 2;
    let mut lines: Vec<String> = Vec::new();
    // `vert` names above the chart, on as many lines as they need.
    let mut vert_lines: Vec<String> = Vec::new();
    for &(name, c) in &verts {
        let at = x + c;
        match vert_lines.iter_mut().find(|l| (width(l) as usize) < at) {
            Some(l) => place(l, at, name),
            None => {
                let mut l = String::new();
                place(&mut l, at, name);
                vert_lines.push(l);
            }
        }
    }
    // The axis: a rule with a mark at each tick, and the labels that fit.
    let ticks: Vec<(usize, String)> = gantt
        .ticks()
        .into_iter()
        .map(|t| (col(t), gantt.label(t)))
        .collect();
    let rule = |tick: &str| -> String {
        let mut cells = vec![g("─", "-"); W + 1];
        for &(c, _) in &ticks {
            cells[c] = tick.to_owned();
        }
        format!("{}{}", " ".repeat(x), cells.concat())
    };
    let mut labels = String::new();
    for (c, label) in &ticks {
        let at = x + c;
        if labels.is_empty() || (width(&labels) as usize) < at {
            place(&mut labels, at, label);
        }
    }
    if gantt.top_axis {
        lines.push(labels.clone());
        lines.push(rule(&g("┴", "+")));
    }
    lines.extend(vert_lines);
    for (label, cells) in rows {
        let pad = name_w - width(&label) as usize;
        let mut line = format!("{label}{}", " ".repeat(pad));
        if let Some(cells) = cells {
            line.push_str("  ");
            line.push_str(&cells.concat());
        }
        lines.push(line);
    }
    lines.push(rule(&g("┬", "+")));
    lines.push(labels);
    let mut out = String::new();
    let total = lines.iter().map(|l| width(l.trim_end())).max().unwrap_or(0);
    if let Some(t) = title {
        let pad = (total - width(t)).max(0) / 2;
        out.push_str(&" ".repeat(pad as usize));
        out.push_str(t);
        out.push_str("\n\n");
    }
    for l in lines {
        out.push_str(l.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing() {
        let mut cells = blank();
        assert_eq!(write(&mut cells, 0, 4, "abcdef"), 4);
        assert_eq!(cells[..5].concat(), "abcd ");
        // A wide character takes two cells, and does not fit in one.
        assert_eq!(write(&mut cells, 10, 12, "中x"), 12);
        assert_eq!(cells[10..13].concat(), "中 ");
        assert_eq!(write(&mut cells, 20, 21, "中"), 20);
        let mut line = "ab".to_owned();
        place(&mut line, 4, "c");
        assert_eq!(line, "ab  c");
    }
}
