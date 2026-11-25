//! Timing diagrams as text (TIMING-5).

use std::collections::HashMap;

use super::canvas::{Canvas, Line, glyph};
use super::mathtext::{Rows, tag_rows};
use super::{Options, width};
use crate::timing::{Constraint, Levels, SignalKind, Timing, Value};

/// Width of the time axis, in cells.
const W: i32 = 60;
/// Lines an analog signal takes.
const ANALOG_LINES: i32 = 5;

struct Row {
    /// The first line of the signal's waveform.
    y: i32,
    h: i32,
    /// The line of the signal's constraints.
    constraint_y: Option<i32>,
    /// Lines between one robust level and the next, and the offset of the level's line in them:
    /// a state label of several lines is given room (1 and 0 for plain text).
    step: i32,
    mid: i32,
    /// Lines above the band taken by its change notes.
    notes: i32,
}

/// A label as lines, and the line its baseline is on: a formula's rows, or the text as one line.
struct Lab {
    rows: Vec<String>,
    base: usize,
    /// Whether it holds a formula.
    math: bool,
}

impl Lab {
    fn new(formulas: &Rows, text: &str, tags: bool) -> Lab {
        match tag_rows(formulas, text, tags) {
            Some((rows, base)) => Lab {
                rows,
                base,
                math: true,
            },
            None => Lab {
                rows: vec![text.to_owned()],
                base: 0,
                math: false,
            },
        }
    }

    /// A note: its lines, or a formula's rows.
    fn note(formulas: &Rows, text: &str, tags: bool) -> Lab {
        match tag_rows(formulas, text, tags) {
            Some((rows, base)) => Lab {
                rows,
                base,
                math: true,
            },
            None => Lab {
                rows: text.lines().map(str::to_owned).collect(),
                base: 0,
                math: false,
            },
        }
    }

    fn width(&self) -> i32 {
        self.rows.iter().map(|r| width(r)).max().unwrap_or(0)
    }

    /// Lines above the baseline line, for a formula.
    fn above(&self) -> i32 {
        if self.math { self.base as i32 } else { 0 }
    }

    /// Lines below the baseline line, for a formula.
    fn below(&self) -> i32 {
        if self.math {
            (self.rows.len() - 1 - self.base) as i32
        } else {
            0
        }
    }

    /// The lines it takes: its rows.
    fn len(&self) -> i32 {
        self.rows.len() as i32
    }

    /// Draws every line with its baseline line on `y`, the left of each at `x`.
    fn draw(&self, c: &mut Canvas, x: i32, y: i32) {
        let top = y - self.base as i32;
        for (i, r) in self.rows.iter().enumerate() {
            c.text(x, top + i as i32, r.trim_end());
        }
    }
}

/// The lines above and below the baseline line that `labs` need, and at least 0.
fn around<'a>(labs: impl Iterator<Item = &'a Lab>) -> (i32, i32) {
    labs.fold((0, 0), |(a, b), l| (a.max(l.above()), b.max(l.below())))
}

/// Renders `timing` as text (TIMING-5).
pub fn render(timing: &Timing, options: &Options) -> String {
    let title = timing
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let formulas = Rows::new(options.charset);
    let tags = timing.tags;
    let lab = |text: &str| Lab::new(&formulas, text, tags);
    let note_lab = |text: &str| Lab::note(&formulas, text, tags);
    let title_rows =
        title.map(|t| tag_rows(&formulas, t, tags).map_or_else(|| vec![t.to_owned()], |r| r.0));
    let Some(range) = timing.range() else {
        return title_rows.map_or_else(String::new, |r| {
            r.iter().map(|l| format!("{l}\n")).collect::<String>()
        });
    };
    let (t0, t1) = range;
    let span = (t1 - t0).max(1e-9);
    // A column of signal names, then one of robust state labels.
    let levels: Vec<Levels> = timing.signals.iter().map(|s| s.levels()).collect();
    let names: Vec<Lab> = timing.signals.iter().map(|s| lab(&s.label)).collect();
    let state_labs: Vec<Vec<Lab>> = timing
        .signals
        .iter()
        .zip(&levels)
        .map(|(s, lv)| {
            if matches!(
                s.kind,
                SignalKind::Robust | SignalKind::Concise | SignalKind::Rectangle
            ) {
                lv.names.iter().map(|l| lab(lv.label(l))).collect()
            } else {
                Vec::new()
            }
        })
        .collect();
    let mut name_w = 0;
    let mut state_w = 0;
    for (i, s) in timing.signals.iter().enumerate() {
        name_w = name_w.max(names[i].width());
        if s.kind == SignalKind::Robust {
            for l in &state_labs[i] {
                state_w = state_w.max(l.width());
            }
        }
    }
    let index: HashMap<&str, usize> = timing
        .signals
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut constraints: HashMap<&str, Vec<&Constraint>> = HashMap::new();
    for k in &timing.constraints {
        if let Some(id) = k.signal.as_deref() {
            constraints.entry(id).or_default().push(k);
        }
    }
    let states_end = name_w + 1 + state_w;
    let x0 = states_end + if state_w > 0 { 2 } else { 1 };
    let col = |t: f64| {
        x0 + ((t - t0) / span * f64::from(W))
            .round()
            .clamp(0.0, f64::from(W)) as i32
    };

    // Layout: a label taller than a line gives the line it is on room above and below.
    let highlight_labs: Vec<Lab> = timing.highlights.iter().map(|h| lab(&h.label)).collect();
    let (hl_above, hl_below) = around(highlight_labs.iter());
    let mut y = 0;
    let highlight_y = y + hl_above;
    if !timing.highlights.is_empty() {
        y += 1 + hl_above + hl_below;
    }
    let above_notes: Vec<Vec<Lab>> = timing
        .signals
        .iter()
        .map(|s| {
            s.notes
                .iter()
                .filter(|n| n.above)
                .map(|n| note_lab(&n.text))
                .collect()
        })
        .collect();
    let below_notes: Vec<Vec<Lab>> = timing
        .signals
        .iter()
        .map(|s| {
            s.notes
                .iter()
                .filter(|n| !n.above)
                .map(|n| note_lab(&n.text))
                .collect()
        })
        .collect();
    let change_labs: Vec<Vec<Option<Lab>>> = timing
        .signals
        .iter()
        .map(|s| {
            s.changes
                .iter()
                .map(|c| c.note.as_deref().map(note_lab))
                .collect()
        })
        .collect();
    let constraint_labs: Vec<Vec<Lab>> = timing
        .signals
        .iter()
        .map(|s| {
            constraints
                .get(s.id.as_str())
                .into_iter()
                .flatten()
                .map(|k| lab(&k.label))
                .collect()
        })
        .collect();
    let layout = |extra: &[i32]| -> (Vec<Row>, i32) {
        let mut y = y;
        let mut rows = Vec::new();
        for (i, (s, lv)) in timing.signals.iter().zip(&levels).enumerate() {
            for n in &above_notes[i] {
                y += n.len();
            }
            let any_note = s.changes.iter().any(|c| c.note.is_some());
            // A change note is a line, or as many as its formula has.
            let notes = if any_note {
                change_labs[i]
                    .iter()
                    .flatten()
                    .map(|l| if l.math { l.len() } else { 1 })
                    .fold(1, i32::max)
            } else {
                0
            };
            y += notes;
            let (up, down) = around(state_labs[i].iter());
            let (step, mid) = if s.kind == SignalKind::Robust {
                (up.max(down) * 2 + 1, up.max(down))
            } else {
                (1, 0)
            };
            let inner = state_labs[i].iter().map(Lab::len).max().unwrap_or(1);
            let h = match s.kind {
                SignalKind::Robust => lv.len().max(2) as i32 * step,
                SignalKind::Binary | SignalKind::Clock => 2,
                SignalKind::Concise | SignalKind::Rectangle => 3.max(inner + 2),
                SignalKind::Analog => ANALOG_LINES,
            }
            .max(names[i].len());
            let row_y = y;
            y += h;
            let (c_up, c_down) = around(constraint_labs[i].iter());
            let constraint_y = constraints.contains_key(s.id.as_str()).then(|| {
                y += c_up + 1 + c_down;
                y - 1 - c_down
            });
            for n in &below_notes[i] {
                y += n.len();
            }
            rows.push(Row {
                y: row_y,
                h,
                constraint_y,
                step,
                mid,
                notes,
            });
            // A blank line between signals.
            y += 1 + extra[i];
        }
        (rows, y)
    };
    let mut gaps = vec![0; timing.signals.len()];
    let (mut rows, mut y) = layout(&gaps);
    // A message's label sits in the lines between its two signals, so a formula of more rows than
    // there are grows the gap below the upper signal.
    let ends = |m: &crate::timing::Message| -> Option<(usize, usize)> {
        let (&a, &b) = (index.get(m.from.as_str())?, index.get(m.to.as_str())?);
        (a != b).then_some((a, b))
    };
    let message_labs: Vec<Lab> = timing.messages.iter().map(|m| lab(&m.label)).collect();
    let mut grown = false;
    for (m, l) in timing.messages.iter().zip(&message_labs) {
        if let (true, Some((a, b))) = (l.math, ends(m).map(|(f, t)| (f.min(t), f.max(t)))) {
            let room = rows[b].y - rows[a].y - rows[a].h;
            if room < l.len() {
                gaps[a] = gaps[a].max(l.len() - room);
                grown = true;
            }
        }
    }
    if grown {
        (rows, y) = layout(&gaps);
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
    let free_labs: Vec<Lab> = free.iter().map(|k| lab(&k.label)).collect();
    let mut free_ys = Vec::new();
    for l in &free_labs {
        free_ys.push(y + l.above());
        y += l.above() + 1 + l.below();
    }

    let ticks = timing.ticks(range);
    let mut right = x0 + W + 1;
    for (_, l) in &ticks {
        right = right.max(x0 + W + width(l));
    }
    let mut c = Canvas::new(right + 40, y.max(1));

    // Highlights: a double rule over their span, the caption on it.
    for (h, l) in timing.highlights.iter().zip(&highlight_labs) {
        let (a, b) = (col(h.from.min(h.to)), col(h.from.max(h.to)));
        c.hline(a, b, highlight_y, Line::Double);
        if !h.label.is_empty() {
            let w = l.width();
            l.draw(&mut c, ((a + b) / 2 - w / 2).max(a), highlight_y);
        }
    }

    for (i, ((s, row), lv)) in timing.signals.iter().zip(&rows).zip(&levels).enumerate() {
        let above_total: i32 = above_notes[i].iter().map(Lab::len).sum();
        let mut ny = row.y - row.notes - above_total;
        for n in &above_notes[i] {
            for r in &n.rows {
                c.text(x0, ny, r.trim_end());
                ny += 1;
            }
        }
        for (ch, l) in s.changes.iter().zip(&change_labs[i]) {
            let Some(l) = l else { continue };
            if l.math {
                l.draw(&mut c, col(ch.at), row.y - l.len() + l.base as i32);
            } else if let Some(first) = l.rows.first() {
                c.text(col(ch.at), row.y - 1, first);
            }
        }
        names[i].draw(&mut c, 0, row.y + names[i].base as i32);
        let segs = timing.segments(s, range);
        match s.kind {
            SignalKind::Concise | SignalKind::Rectangle => {
                let (top, bot) = (row.y, row.y + row.h - 1);
                let inner = row.h - 2;
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
                            let room = b - a - 1;
                            if room > 0 {
                                if let Value::State(v) = &g.value {
                                    let l = lab(lv.label(v));
                                    if l.math {
                                        if l.width() <= room {
                                            let at = a + 1 + (room - l.width()) / 2;
                                            let first = top + 1 + (inner - l.len()) / 2;
                                            l.draw(&mut c, at, first + l.base as i32);
                                        }
                                        continue;
                                    }
                                }
                                let slashes;
                                let label = match &g.value {
                                    Value::State(v) => lv.label(v),
                                    _ => {
                                        slashes = "/".repeat(room as usize);
                                        &slashes
                                    }
                                };
                                let text: String = fit(label, room);
                                let at = a + 1 + (room - width(&text)) / 2;
                                c.text(at, top + 1 + (inner - 1) / 2, &text);
                            }
                        }
                    }
                }
            }
            _ => {
                let n = row.h;
                let (lo, hi) = s.analog_scale();
                let level = |v: &str| -> Option<i32> {
                    if s.kind == SignalKind::Analog {
                        let f = ((v.trim().parse::<f64>().ok()? - lo) / (hi - lo)).clamp(0.0, 1.0);
                        return Some(((1.0 - f) * f64::from(n - 1)).round() as i32);
                    }
                    lv.level(v).map(|i| i as i32 * row.step + row.mid)
                };
                if s.kind == SignalKind::Robust {
                    for (k, l) in state_labs[i].iter().enumerate() {
                        l.draw(
                            &mut c,
                            states_end - l.width(),
                            row.y + k as i32 * row.step + row.mid,
                        );
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
            for (k, l) in constraints
                .get(s.id.as_str())
                .into_iter()
                .flatten()
                .zip(&constraint_labs[i])
            {
                constraint(&mut c, col(k.from), col(k.to), cy, l);
            }
        }
        let (_, c_down) = around(constraint_labs[i].iter());
        let (c_up, _) = around(constraint_labs[i].iter());
        let mut ny = row.y + row.h + row.constraint_y.map_or(0, |_| c_up + 1 + c_down);
        for n in &below_notes[i] {
            for r in &n.rows {
                c.text(x0, ny, r.trim_end());
                ny += 1;
            }
        }
    }

    // Messages: a dotted line down (or up) at the target moment into the target signal.
    for (m, l) in timing.messages.iter().zip(&message_labs) {
        let Some((fi, ti)) = ends(m) else { continue };
        let x = col(m.to_at);
        let (f, t) = (&rows[fi.min(ti)], &rows[fi.max(ti)]);
        let (start, end, down) = (f.y + f.h, t.y - 1, fi < ti);
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
            if l.math {
                // Centered in the lines between the two signals, which were grown to hold it.
                let first = start + (end - start + 1 - l.len()) / 2;
                l.draw(&mut c, x + 2, first.max(start) + l.base as i32);
            } else {
                c.text(x + 2, (start + end) / 2, &m.label);
            }
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
    for ((k, l), &fy) in free.iter().zip(&free_labs).zip(&free_ys) {
        constraint(&mut c, col(k.from), col(k.to), fy, l);
    }

    let body = c.render(options.charset);
    let body = body.trim_end_matches('\n');
    let mut out = String::new();
    if let Some(rows) = title_rows {
        let total = body.lines().map(width).max().unwrap_or(0);
        for t in rows {
            let pad = ((total - width(&t)) / 2).max(0);
            out.push_str(&" ".repeat(pad as usize));
            out.push_str(t.trim_end());
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(body);
    out.push('\n');
    out
}

/// `label` cut to `room` columns.
fn fit(label: &str, room: i32) -> String {
    let mut out = String::new();
    let mut used = 0;
    let mut buf = [0; 4];
    for ch in label.chars() {
        used += width(ch.encode_utf8(&mut buf));
        if used > room {
            break;
        }
        out.push(ch);
    }
    out
}

/// A two-headed span from `a` to `b` on line `y`, its label over the middle: each line of a
/// formula label is padded a space each side, its baseline line on `y`.
fn constraint(c: &mut Canvas, a: i32, b: i32, y: i32, label: &Lab) {
    let (a, b) = (a.min(b), a.max(b));
    c.hline(a, b, y, Line::Light);
    c.arrow(a, y, glyph('◄', '<'));
    c.arrow(b, y, glyph('►', '>'));
    if !label.rows.iter().all(String::is_empty) {
        let w = label.width() + 2;
        let at = if w < b - a {
            a + 1 + (b - a - 1 - w) / 2
        } else {
            b + 1
        };
        let top = y - label.base as i32;
        for (i, r) in label.rows.iter().enumerate() {
            let padded = width(r);
            let text = format!(
                " {r}{} ",
                " ".repeat((label.width() - padded).max(0) as usize)
            );
            c.text(at, top + i as i32, &text);
        }
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
