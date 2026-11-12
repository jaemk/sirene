//! Timing diagrams as text (TIMING-5).

use super::canvas::{Canvas, Line, glyph};
use super::{Options, width};
use crate::timing::{SignalKind, Timing, Value};

/// Width of the time axis, in cells.
const W: i32 = 60;
/// Lines an analog signal takes.
const ANALOG_LINES: i32 = 5;

struct Row {
    /// The first line of the signal's waveform.
    y: i32,
    h: i32,
    constraint_y: Option<i32>,
}

/// Renders `timing` as text (TIMING-5).
pub fn render(timing: &Timing, options: &Options) -> String {
    let title = timing
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let Some(range) = timing.range() else {
        return title.map_or_else(String::new, |t| format!("{t}\n"));
    };
    let (t0, t1) = range;
    let span = (t1 - t0).max(1e-9);
    // A column of signal names, then one of robust state labels.
    let mut name_w = 0;
    let mut state_w = 0;
    for s in &timing.signals {
        name_w = name_w.max(width(&s.label));
        if s.kind == SignalKind::Robust {
            for l in s.levels() {
                state_w = state_w.max(width(s.state_label(&l)));
            }
        }
    }
    let states_end = name_w + 1 + state_w;
    let x0 = states_end + if state_w > 0 { 2 } else { 1 };
    let col = |t: f64| {
        x0 + ((t - t0) / span * f64::from(W))
            .round()
            .clamp(0.0, f64::from(W)) as i32
    };

    // Layout.
    let mut y = 0;
    let highlight_y = y;
    if !timing.highlights.is_empty() {
        y += 1;
    }
    let mut rows = Vec::new();
    for s in &timing.signals {
        for n in s.notes.iter().filter(|n| n.above) {
            y += n.text.lines().count() as i32;
        }
        if s.changes.iter().any(|c| c.note.is_some()) {
            y += 1;
        }
        let h = match s.kind {
            SignalKind::Robust => s.levels().len().max(2) as i32,
            SignalKind::Binary | SignalKind::Clock => 2,
            SignalKind::Concise | SignalKind::Rectangle => 3,
            SignalKind::Analog => ANALOG_LINES,
        };
        let row_y = y;
        y += h;
        let constraint_y = timing
            .constraints
            .iter()
            .any(|c| c.signal.as_deref() == Some(s.id.as_str()))
            .then(|| {
                y += 1;
                y - 1
            });
        for n in s.notes.iter().filter(|n| !n.above) {
            y += n.text.lines().count() as i32;
        }
        rows.push(Row {
            y: row_y,
            h,
            constraint_y,
        });
        // A blank line between signals.
        y += 1;
    }
    let axis_y = y;
    if !timing.hide_axis {
        y += 2;
    }
    let free: Vec<_> = timing
        .constraints
        .iter()
        .filter(|c| c.signal.is_none())
        .collect();
    let free_y = y;
    y += free.len() as i32;

    let ticks = timing.ticks(range);
    let mut right = x0 + W + 1;
    for (_, l) in &ticks {
        right = right.max(x0 + W + width(l));
    }
    let mut c = Canvas::new(right + 40, y.max(1));

    // Highlights: a double rule over their span, the caption on it.
    for h in &timing.highlights {
        let (a, b) = (col(h.from.min(h.to)), col(h.from.max(h.to)));
        c.hline(a, b, highlight_y, Line::Double);
        if !h.label.is_empty() {
            let w = width(&h.label);
            c.text(((a + b) / 2 - w / 2).max(a), highlight_y, &h.label);
        }
    }

    for (s, row) in timing.signals.iter().zip(&rows) {
        let mut ny = row.y
            - s.changes.iter().any(|c| c.note.is_some()) as i32
            - s.notes
                .iter()
                .filter(|n| n.above)
                .map(|n| n.text.lines().count() as i32)
                .sum::<i32>();
        for n in s.notes.iter().filter(|n| n.above) {
            for l in n.text.lines() {
                c.text(x0, ny, l);
                ny += 1;
            }
        }
        for ch in &s.changes {
            if let Some(n) = &ch.note {
                c.text(col(ch.at), row.y - 1, n.lines().next().unwrap_or(""));
            }
        }
        c.text(0, row.y, &s.label);
        let segs = timing.segments(s, range);
        match s.kind {
            SignalKind::Concise | SignalKind::Rectangle => {
                let (top, mid, bot) = (row.y, row.y + 1, row.y + 2);
                for g in &segs {
                    let (a, b) = (col(g.start), col(g.end).min(x0 + W));
                    if b <= a {
                        continue;
                    }
                    match &g.value {
                        Value::Hidden => {}
                        Value::Undefined | Value::State(_) => {
                            c.hline(a, b, top, Line::Light);
                            c.hline(a, b, bot, Line::Light);
                            c.vline(a, top, bot, Line::Light);
                            c.vline(b, top, bot, Line::Light);
                            let label = match &g.value {
                                Value::State(v) => s.state_label(v).to_owned(),
                                _ => "/".repeat((b - a - 1).max(0) as usize),
                            };
                            let room = b - a - 1;
                            if room > 0 {
                                let text: String = fit(&label, room);
                                let at = a + 1 + (room - width(&text)) / 2;
                                c.text(at, mid, &text);
                            }
                        }
                    }
                }
            }
            _ => {
                let levels = s.levels();
                let n = row.h;
                let level = |v: &str| -> Option<i32> {
                    if s.kind == SignalKind::Analog {
                        let (lo, hi) = s.analog_scale();
                        let f = ((v.parse::<f64>().ok()? - lo) / (hi - lo)).clamp(0.0, 1.0);
                        return Some(((1.0 - f) * f64::from(n - 1)).round() as i32);
                    }
                    s.level(&levels, v).map(|i| i as i32)
                };
                if s.kind == SignalKind::Robust {
                    for (i, l) in levels.iter().enumerate() {
                        let label = s.state_label(l);
                        c.text(states_end - width(label), row.y + i as i32, label);
                    }
                }
                let mut last: Option<(i32, i32)> = None;
                for g in &segs {
                    let (a, b) = (col(g.start), col(g.end).min(x0 + W));
                    match &g.value {
                        Value::State(v) => {
                            let Some(l) = level(v) else {
                                last = None;
                                continue;
                            };
                            let ly = row.y + l;
                            if let Some((px, py)) = last
                                && px == a
                                && py != ly
                            {
                                c.vline(a, py.min(ly), py.max(ly), Line::Light);
                            }
                            if b > a {
                                c.hline(a, b, ly, Line::Light);
                            }
                            last = Some((b, ly));
                        }
                        Value::Hidden => last = None,
                        Value::Undefined => {
                            for k in 0..n {
                                c.hline(a, b, row.y + k, Line::Dotted);
                            }
                            last = None;
                        }
                    }
                }
            }
        }
        if let Some(cy) = row.constraint_y {
            for k in timing
                .constraints
                .iter()
                .filter(|k| k.signal.as_deref() == Some(s.id.as_str()))
            {
                constraint(&mut c, col(k.from), col(k.to), cy, &k.label);
            }
        }
        let mut ny = row.y + row.h + row.constraint_y.map_or(0, |_| 1);
        for n in s.notes.iter().filter(|n| !n.above) {
            for l in n.text.lines() {
                c.text(x0, ny, l);
                ny += 1;
            }
        }
    }

    // Messages: a dotted line down (or up) at the target moment into the target signal.
    for m in &timing.messages {
        let fi = timing.signals.iter().position(|s| s.id == m.from);
        let ti = timing.signals.iter().position(|s| s.id == m.to);
        let (Some(fi), Some(ti)) = (fi, ti) else {
            continue;
        };
        if fi == ti {
            continue;
        }
        let x = col(m.to_at);
        let (f, t) = (&rows[fi], &rows[ti]);
        let (start, end, down) = if fi < ti {
            (f.y + f.h, t.y - 1, true)
        } else {
            (t.y + t.h, f.y - 1, false)
        };
        if end < start {
            continue;
        }
        c.vline(x, start, end, Line::Dotted);
        if down {
            c.arrow(x, end, glyph('▼', 'v'));
        } else {
            c.arrow(x, start, glyph('▲', '^'));
        }
        if !m.label.is_empty() {
            c.text(x + 2, (start + end) / 2, &m.label);
        }
    }

    // The axis.
    if !timing.hide_axis {
        c.hline(x0, x0 + W, axis_y, Line::Light);
        let mut last_end = i32::MIN;
        for (t, label) in &ticks {
            let tx = col(*t);
            c.glyph(tx, axis_y, glyph('┬', '+'));
            let at = (tx - width(label) / 2).max(0);
            if at > last_end {
                last_end = c.text(at, axis_y + 1, label);
            }
        }
    }
    for (i, k) in free.iter().enumerate() {
        constraint(&mut c, col(k.from), col(k.to), free_y + i as i32, &k.label);
    }

    let body = c.render(options.charset);
    let body = body.trim_end_matches('\n');
    let mut out = String::new();
    if let Some(t) = title {
        let total = body.lines().map(width).max().unwrap_or(0);
        let pad = ((total - width(t)) / 2).max(0);
        out.push_str(&" ".repeat(pad as usize));
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str(body);
    out.push('\n');
    out
}

/// `label` cut to `room` columns.
fn fit(label: &str, room: i32) -> String {
    let mut out = String::new();
    for ch in label.chars() {
        if width(&format!("{out}{ch}")) > room {
            break;
        }
        out.push(ch);
    }
    out
}

/// A two-headed span from `a` to `b` on line `y`, its label over the middle.
fn constraint(c: &mut Canvas, a: i32, b: i32, y: i32, label: &str) {
    let (a, b) = (a.min(b), a.max(b));
    c.hline(a, b, y, Line::Light);
    c.arrow(a, y, glyph('◄', '<'));
    c.arrow(b, y, glyph('►', '>'));
    if !label.is_empty() {
        let text = format!(" {label} ");
        let w = width(&text);
        let at = if w < b - a {
            a + 1 + (b - a - 1 - w) / 2
        } else {
            b + 1
        };
        c.text(at, y, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting() {
        assert_eq!(fit("abcdef", 3), "abc");
        assert_eq!(fit("ab", 3), "ab");
    }
}
