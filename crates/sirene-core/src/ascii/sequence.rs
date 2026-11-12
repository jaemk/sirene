//! Sequence diagrams as text (MSEQ-4).

use super::canvas::{Canvas, DOWN, Glyph, Line, UP, glyph};
use super::{Options, draw_box, draw_lines, width};
use crate::layout::Rect;
use crate::sequence::layout::{self, Placed, Sizes, Spacing};
use crate::sequence::{
    BlockKind, Event, Head, LineStyle, Message, Note, NoteSide, ParticipantKind, Sequence,
};

const SPACING: Spacing = Spacing {
    margin: 0.0,
    col_gap: 3.0,
    label_pad: 1.0,
    label_gap: 0.0,
    arrow_h: 1.0,
    row_gap: 1.0,
    self_w: 3.0,
    self_h: 1.0,
    // Lifelines sit mid-cell, so this leaves one blank cell beside a note.
    note_gap: 1.5,
    frame_pad: 2.0,
    frame_top: 0.0,
    divider_h: 0.0,
    frame_end: 1.0,
    bar_w: 0.0,
    group_pad: 2.0,
    // The title goes on the group's top border.
    group_title_h: 0.0,
};

fn c(v: f32) -> i32 {
    v.floor() as i32
}

fn text_w(lines: &[String]) -> i32 {
    lines.iter().map(|l| width(l)).max().unwrap_or(0)
}

/// A message's label, after its `autonumber` number.
fn label(m: &Message) -> Vec<String> {
    let mut lines = m.text.clone();
    if let Some(n) = m.number {
        match lines.first_mut() {
            Some(first) if !first.is_empty() => *first = format!("{n}. {first}"),
            Some(first) => *first = format!("{n}."),
            None => lines.push(format!("{n}.")),
        }
    }
    if lines.iter().all(String::is_empty) {
        lines.clear();
    }
    lines
}

/// A block's tab: its keyword and label; none for `rect`.
fn tab(kind: BlockKind, label: &str) -> String {
    if kind == BlockKind::Rect {
        return String::new();
    }
    match kind.tab(label) {
        (name, after) if after.is_empty() => name,
        (name, after) => format!("{name} {after}"),
    }
}

/// Rows a `||N||` space of `px` pixels takes: one per 20, at least one.
fn space_rows(px: Option<u32>) -> i32 {
    px.map_or(1, |n| (n.saturating_add(10) / 20).max(1) as i32)
}

fn divider(label: &str) -> String {
    if label.is_empty() {
        String::new()
    } else {
        format!("[{label}]")
    }
}

/// A header's width (odd, so it centers on its lifeline) and height.
fn header_size(kind: ParticipantKind, lines: &[String]) -> (i32, i32) {
    let (w, h) = match kind {
        ParticipantKind::Hidden => (1, 0),
        ParticipantKind::Actor => (text_w(lines).max(3), lines.len() as i32 + 3),
        _ => (text_w(lines) + 4, lines.len() as i32 + 2),
    };
    (w | 1, h)
}

fn head_glyph(head: Head, rightward: bool) -> Option<Glyph> {
    match (head, rightward) {
        (Head::None, _) => None,
        (Head::Filled, true) => Some(glyph('►', '>')),
        (Head::Filled, false) => Some(glyph('◄', '<')),
        (Head::Open, true) => Some(glyph('>', '>')),
        (Head::Open, false) => Some(glyph('<', '<')),
        (Head::Cross, _) => Some(glyph('×', 'x')),
    }
}

/// A note `w` cells wide, one blank cell from the lifelines in `cols` it is beside, or
/// reaching one cell past those it is over.
fn note_rect(note: &Note, cols: &[i32], w: i32, y: i32, h: i32) -> Rect {
    let first = note.participants.first().map_or(0, |&p| cols[p]);
    let last = note.participants.last().map_or(0, |&p| cols[p]);
    let (lo, hi) = (first.min(last), first.max(last));
    let (x, w) = match note.side {
        NoteSide::Left => (lo - 1 - w, w),
        NoteSide::Right => (lo + 2, w),
        NoteSide::Over if lo == hi => (lo - (w - 1) / 2, w),
        NoteSide::Over => {
            let span = hi - lo + 5;
            let nw = w.max(span);
            (lo - 2 - (nw - span) / 2, nw)
        }
    };
    Rect::new(x, y, w, h)
}

/// Draws a header with its top-left at `(x, y)`.
fn header(canvas: &mut Canvas, kind: ParticipantKind, lines: &[String], x: i32, y: i32, w: i32) {
    if kind == ParticipantKind::Hidden {
        return;
    }
    if kind == ParticipantKind::Actor {
        let mid = x + w / 2;
        canvas.clear(x, y, x + w - 1, y + 2 + lines.len() as i32);
        canvas.text(mid, y, "o");
        canvas.text(mid - 1, y + 1, "/|\\");
        canvas.text(mid - 1, y + 2, "/ \\");
        draw_lines(canvas, lines, x, w, y + 3);
    } else {
        // The lifeline joins the border, so only the inside is cleared.
        let r = Rect::new(x, y, w, lines.len() as i32 + 2);
        canvas.clear(r.x + 1, r.y + 1, r.right() - 1, r.bottom() - 1);
        draw_box(canvas, r, Line::Light);
        draw_lines(canvas, lines, x + 1, w - 2, y + 1);
    }
}

/// Renders `seq` as text (MSEQ-4).
pub fn render(seq: &Sequence, options: &Options) -> String {
    let headers: Vec<(i32, i32)> = seq
        .participants
        .iter()
        .map(|p| header_size(p.kind, &p.label))
        .collect();
    let labels: Vec<Vec<String>> = seq
        .events
        .iter()
        .map(|e| match e {
            Event::Message(m) => label(m),
            Event::Note(n) => n.text.clone(),
            Event::Start(kind, l) => vec![tab(*kind, l)],
            Event::Divider(l) => vec![divider(l)],
            Event::Ref { text, .. } => text.clone(),
            Event::Separator(t) if !t.is_empty() => vec![format!(" {t} ")],
            Event::Delay(Some(t)) if !t.is_empty() => vec![format!(" {t} ")],
            _ => Vec::new(),
        })
        .collect();
    let sizes = Sizes {
        headers: headers.iter().map(|&(w, h)| (w as f32, h as f32)).collect(),
        events: seq
            .events
            .iter()
            .zip(&labels)
            .map(|(e, l)| {
                let (w, h) = match e {
                    Event::Message(_) => (text_w(l), l.len() as i32),
                    Event::Note(_) => (text_w(l) + 4, l.len() as i32 + 2),
                    Event::Start(..) | Event::Divider(_) => {
                        (if text_w(l) > 0 { text_w(l) + 4 } else { 0 }, 1)
                    }
                    Event::Ref { .. } => ((text_w(l) + 4).max(7), l.len() as i32 + 2),
                    Event::Separator(_) => (if l.is_empty() { 0 } else { text_w(l) + 4 }, 1),
                    Event::Delay(_) => (text_w(l), 1),
                    Event::Space(px) => (0, space_rows(*px)),
                    _ => (0, 0),
                };
                (w as f32, h as f32)
            })
            .collect(),
        title_h: 0.0,
        groups: seq
            .groups
            .iter()
            .map(|g| g.title.as_deref().map_or(0, |t| width(t) + 4) as f32)
            .collect(),
    };
    let l = layout::layout(seq, &sizes, &SPACING);
    let header_h = c(l.header_h);
    let cols: Vec<i32> = l.x.iter().map(|&x| c(x)).collect();
    // A cell to spare: notes are placed by lifeline cell, which can differ from the layout's
    // by one.
    // Without footers, the lifelines' last row is the bottom one.
    let foot = i32::from(seq.hide_footbox);
    let mut canvas = Canvas::new(c(l.width) + 2, c(l.height) + foot);

    // Groups behind everything; their right and bottom edges are exclusive.
    let group_rect =
        |g: &layout::GroupBox| Rect::new(c(g.x0), c(g.y0), c(g.x1) - c(g.x0), c(g.y1) - c(g.y0));
    for g in &l.groups {
        draw_box(&mut canvas, group_rect(g), Line::Dotted);
    }

    // Lifelines, heavy where a participant is active and dotted through delays.
    let delays: Vec<(i32, i32)> = l
        .events
        .iter()
        .zip(&seq.events)
        .filter_map(|(place, e)| match (place, e) {
            (Placed::Band { y, h }, Event::Delay(_)) => Some((c(*y), c(y + h) - 1)),
            _ => None,
        })
        .collect();
    for (p, &x) in l.x.iter().enumerate() {
        if seq.participants[p].kind == ParticipantKind::Hidden {
            continue;
        }
        let col = c(x);
        let y0 = c(l.head_y[p]) + header_h - 1;
        let y1 = c(l.life_end[p]);
        for y in y0..=y1 {
            let active = l.bars.iter().any(|b| {
                b.participant == p && c(b.y0) <= y && y <= c(b.y1) && !(y == c(b.y1) && y == y1)
            });
            let line = if delays.iter().any(|&(d0, d1)| d0 <= y && y <= d1) {
                Line::Dotted
            } else if active {
                Line::Heavy
            } else {
                Line::Light
            };
            let bits = if y > y0 { UP } else { 0 } | if y < y1 { DOWN } else { 0 };
            canvas.add(col, y, bits, line);
        }
    }

    // Frames, inner ones last so their tabs show.
    for f in l.frames.iter().rev() {
        let Event::Start(kind, label) = &seq.events[f.event] else {
            continue;
        };
        let line = if *kind == BlockKind::Rect {
            Line::Dotted
        } else {
            Line::Light
        };
        let (x0, x1) = (c(f.x0), c(f.x1));
        let r = Rect::new(x0, c(f.y0), x1 - x0 + 1, c(f.y1) - c(f.y0) + 1);
        draw_box(&mut canvas, r, line);
        canvas.text(x0 + 2, r.y, &tab(*kind, label));
        for &(k, y) in &f.dividers {
            canvas.hline(x0, x1, c(y), Line::Dotted);
            if let Event::Divider(label) = &seq.events[k] {
                canvas.text(x0 + 2, c(y), &divider(label));
            }
        }
    }

    // Messages and notes.
    for (k, place) in l.events.iter().enumerate() {
        match (*place, &seq.events[k]) {
            (Placed::Message { x0, x1, y, label_y }, Event::Message(m)) => {
                let line = match m.arrow.line {
                    LineStyle::Solid => Line::Light,
                    LineStyle::Dotted => Line::Dotted,
                };
                let (x0, x1, row) = (c(x0), c(x1), c(y));
                let lines = &labels[k];
                if m.from == m.to {
                    let end = x0 + c(SPACING.self_w);
                    let bottom = row + c(SPACING.self_h);
                    canvas.hline(x0, end, row, line);
                    canvas.vline(end, row, bottom, line);
                    canvas.hline(x0, end, bottom, line);
                    if let Some(g) = head_glyph(m.arrow.head, false) {
                        canvas.arrow(x0 + 1, bottom, g);
                    }
                    for (i, t) in lines.iter().enumerate() {
                        canvas.text(x0 + 2, c(label_y) + i as i32, t);
                    }
                } else {
                    let rightward = x1 > x0;
                    let step = if rightward { 1 } else { -1 };
                    // Up to the target's lifeline, not into it; from the sender's when the
                    // arrow points both ways.
                    let start = if m.arrow.both { x0 + step } else { x0 };
                    let end = x1 - step;
                    canvas.hline(start.min(end), start.max(end), row, line);
                    if let Some(g) = head_glyph(m.arrow.head, rightward) {
                        canvas.arrow(x1 - step, row, g);
                    }
                    if m.arrow.both
                        && let Some(g) = head_glyph(m.arrow.head, !rightward)
                    {
                        canvas.arrow(x0 + step, row, g);
                    }
                    let left = x0.min(x1);
                    let w = (x1 - x0).abs();
                    draw_lines(&mut canvas, lines, left + 1, w - 1, c(label_y));
                }
            }
            (Placed::Note { y, h, .. }, Event::Note(note)) => {
                let r = note_rect(note, &cols, sizes.events[k].0 as i32, c(y), h as i32);
                canvas.clear(r.x, r.y, r.right(), r.bottom());
                draw_box(&mut canvas, r, Line::Light);
                draw_lines(&mut canvas, &labels[k], r.x + 1, r.w - 2, r.y + 1);
            }
            (Placed::Note { y, h, .. }, Event::Ref { participants, .. }) => {
                let over = Note {
                    side: NoteSide::Over,
                    participants: participants.clone(),
                    text: Vec::new(),
                };
                let r = note_rect(&over, &cols, sizes.events[k].0 as i32, c(y), h as i32);
                canvas.clear(r.x, r.y, r.right(), r.bottom());
                draw_box(&mut canvas, r, Line::Light);
                canvas.text(r.x + 2, r.y, "ref");
                draw_lines(&mut canvas, &labels[k], r.x + 1, r.w - 2, r.y + 1);
            }
            (Placed::Band { y, .. }, Event::Separator(_)) => {
                let w = c(l.width) + 1;
                canvas.hline(0, w - 1, c(y), Line::Double);
                draw_lines(&mut canvas, &labels[k], 0, w, c(y));
            }
            (Placed::Band { y, .. }, Event::Delay(_)) => {
                draw_lines(&mut canvas, &labels[k], 0, c(l.width) + 1, c(y));
            }
            _ => {}
        }
    }

    // Headers at the top (or where created) and again at the bottom.
    for (p, part) in seq.participants.iter().enumerate() {
        let (w, h) = headers[p];
        let x = c(l.x[p]) - w / 2;
        header(
            &mut canvas,
            part.kind,
            &part.label,
            x,
            c(l.head_y[p]) + header_h - h,
            w,
        );
        if l.destroyed[p] {
            canvas.arrow(c(l.x[p]), c(l.life_end[p]), glyph('×', 'X'));
        } else if !seq.hide_footbox {
            header(&mut canvas, part.kind, &part.label, x, c(l.footer_y), w);
        }
    }

    // Group titles on their top border.
    for g in &l.groups {
        if let Some(t) = &seq.groups[g.group].title {
            let r = group_rect(g);
            draw_lines(&mut canvas, &[format!(" {t} ")], r.x, r.w, r.y);
        }
    }

    let body = canvas.render(options.charset);
    match seq
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            let pad = (c(l.width) + 1 - width(t)).max(0) / 2;
            format!("{}{t}\n\n{body}", " ".repeat(pad as usize))
        }
        None => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let m = |text: &[&str], number| Message {
            from: 0,
            to: 1,
            arrow: crate::sequence::Arrow::parse("->>").unwrap(),
            text: text.iter().map(|s| (*s).to_owned()).collect(),
            activate: false,
            deactivate: false,
            number,
        };
        assert_eq!(label(&m(&["hi"], Some(3))), ["3. hi"]);
        assert_eq!(label(&m(&[], Some(3))), ["3."]);
        assert!(label(&m(&[""], None)).is_empty());
        assert_eq!(tab(BlockKind::Loop, ""), "loop");
        assert_eq!(tab(BlockKind::Alt, "ok"), "alt [ok]");
        assert_eq!(tab(BlockKind::Rect, "red"), "");
        assert_eq!(
            header_size(ParticipantKind::Participant, &["ab".to_owned()]),
            (7, 3)
        );
        assert_eq!(
            header_size(ParticipantKind::Actor, &["a".to_owned()]),
            (3, 4)
        );
    }
}
