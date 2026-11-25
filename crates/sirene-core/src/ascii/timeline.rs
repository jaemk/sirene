//! Timelines as text (TIMELI-4).

use super::canvas::{Canvas, Line, glyph};
use super::mathtext::{Rows, tag_rows};
use super::{Options, draw_box, draw_lines, width};
use crate::layout::Rect;
use crate::timeline::{Direction, Text, Timeline};

/// Widest text in a box before it wraps.
const WRAP: i32 = 24;

/// `text` broken at spaces to fit `WRAP` columns; a longer word keeps a line of its own.
fn wrap(text: &Text) -> Vec<String> {
    let mut out = Vec::new();
    for line in text {
        let mut cur = String::new();
        for word in line.split_whitespace() {
            if !cur.is_empty() && width(&cur) + 1 + width(word) > WRAP {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        out.push(cur);
    }
    out
}

/// `text` as the rows of its formulas, when `tags` says `<latex>` and `<math>` tags are formulas
/// and it holds one (not wrapped: a formula is rows, not words); else wrapped as `wrap` does.
fn wrap_tags(text: &Text, tags: bool, rows: &Rows) -> Vec<String> {
    match tag_rows(rows, &text.join("\n"), tags) {
        Some((rows, _)) => rows,
        None => wrap(text),
    }
}

/// The width of a box around `lines`: the text, a space each side, and the borders.
fn box_w(lines: &[String]) -> i32 {
    lines.iter().map(|l| width(l)).max().unwrap_or(0) + 4
}

fn box_h(lines: &[String]) -> i32 {
    lines.len() as i32 + 2
}

/// Draws `lines` in a box at `r`, centered.
fn boxed(canvas: &mut Canvas, r: Rect, lines: &[String]) {
    draw_box(canvas, r, Line::Light);
    let top = r.y + (r.h - lines.len() as i32) / 2;
    draw_lines(canvas, lines, r.x + 1, r.w - 2, top);
}

/// Renders `timeline` as text (TIMELI-4).
pub fn render(timeline: &Timeline, options: &Options) -> String {
    let formulas = Rows::new(options.charset);
    let tags = timeline.tags;
    let wrap = |text: &Text| wrap_tags(text, tags, &formulas);
    let periods: Vec<(usize, Vec<String>, Vec<Vec<String>>)> = timeline
        .periods()
        .map(|(s, p)| (s, wrap(&p.label), p.events.iter().map(wrap).collect()))
        .collect();
    // Only a section with periods has a box.
    let sections: Vec<Option<Vec<String>>> = timeline
        .sections
        .iter()
        .map(|s| s.name.as_ref().filter(|_| !s.periods.is_empty()).map(wrap))
        .collect();
    let mut next = 0;
    let ranges: Vec<_> = timeline
        .sections
        .iter()
        .map(|s| {
            let start = next;
            next += s.periods.len();
            start..next
        })
        .collect();
    // Boxes to draw, and the lines between them.
    let mut boxes: Vec<(Rect, &[String])> = Vec::new();
    let mut vlines: Vec<(i32, i32, i32)> = Vec::new();
    let mut hlines: Vec<(i32, i32, i32)> = Vec::new();
    let arrow = if timeline.direction == Direction::LeftRight {
        let mut col_w: Vec<i32> = periods
            .iter()
            .map(|(_, l, ev)| ev.iter().map(|e| box_w(e)).fold(box_w(l), i32::max))
            .collect();
        for (s, name) in sections.iter().enumerate() {
            let (Some(name), cols) = (name, ranges[s].clone()) else {
                continue;
            };
            let span = cols.clone().map(|i| col_w[i] + 1).sum::<i32>() - 1;
            if let Some(last) = cols.last()
                && box_w(name) > span
            {
                col_w[last] += box_w(name) - span;
            }
        }
        let mut col_x = Vec::new();
        let mut x = 0;
        for w in &col_w {
            col_x.push(x);
            x += w + 1;
        }
        let section_h = sections
            .iter()
            .flatten()
            .map(|n| box_h(n))
            .max()
            .unwrap_or(0);
        let period_h = periods.iter().map(|(_, l, _)| box_h(l)).max().unwrap_or(0);
        for (s, name) in sections.iter().enumerate() {
            let (Some(name), cols) = (name, ranges[s].clone()) else {
                continue;
            };
            if let Some(b) = cols.clone().last() {
                let a = cols.start;
                let r = Rect::new(col_x[a], 0, col_x[b] + col_w[b] - col_x[a], section_h);
                boxes.push((r, name));
            }
        }
        let axis = section_h + period_h;
        for (i, (_, label, events)) in periods.iter().enumerate() {
            let center = col_x[i] + col_w[i] / 2;
            boxes.push((Rect::new(col_x[i], section_h, col_w[i], period_h), label));
            // Joined in the gaps only: the period to the first event across the arrow, then
            // each event to the next.
            let mut y = axis + 1;
            let mut above = axis - 1;
            for e in events {
                vlines.push((center, above, y));
                boxes.push((Rect::new(col_x[i], y, col_w[i], box_h(e)), e));
                y += box_h(e);
                above = y - 1;
            }
        }
        let end = x.max(1);
        hlines.push((0, end - 1, axis));
        (end, axis, glyph('►', '>'))
    } else {
        let section_w = sections
            .iter()
            .flatten()
            .map(|n| box_w(n))
            .max()
            .unwrap_or(0);
        let period_w = periods.iter().map(|(_, l, _)| box_w(l)).max().unwrap_or(0);
        let period_x = if section_w > 0 { section_w + 1 } else { 0 };
        let axis = period_x + period_w + 1;
        let events_x = axis + 2;
        let mut row_h: Vec<i32> = periods
            .iter()
            .map(|(_, l, events)| events.iter().map(|e| box_h(e)).fold(box_h(l), i32::max))
            .collect();
        for (s, name) in sections.iter().enumerate() {
            if let Some(name) = name {
                let rows = ranges[s].clone();
                let span = rows.clone().map(|i| row_h[i] + 1).sum::<i32>() - 1;
                if let Some(last) = rows.last() {
                    row_h[last] += (box_h(name) - span).max(0);
                }
            }
        }
        let mut row_y = Vec::new();
        let mut y = 0;
        for &h in &row_h {
            row_y.push(y);
            y += h + 1;
        }
        for (s, name) in sections.iter().enumerate() {
            let (Some(name), rows) = (name, ranges[s].clone()) else {
                continue;
            };
            if let Some(b) = rows.clone().last() {
                let a = rows.start;
                let h = row_y[b] + row_h[b] - row_y[a];
                boxes.push((Rect::new(0, row_y[a], section_w, h), name));
            }
        }
        for (i, (_, label, events)) in periods.iter().enumerate() {
            boxes.push((Rect::new(period_x, row_y[i], period_w, row_h[i]), label));
            let mid = row_y[i] + row_h[i] / 2;
            let mut x = events_x;
            let mut left = period_x + period_w - 1;
            for e in events {
                hlines.push((left, x, mid));
                boxes.push((Rect::new(x, mid - box_h(e) / 2, box_w(e), box_h(e)), e));
                x += box_w(e) + 1;
                left = x - 2;
            }
        }
        let end = y.max(1);
        vlines.push((axis, 0, end - 1));
        (axis, end, glyph('▼', 'v'))
    };
    let right = boxes
        .iter()
        .map(|(r, _)| r.right())
        .chain(hlines.iter().map(|h| h.1))
        .chain([arrow.0])
        .max()
        .unwrap_or(0);
    let bottom = boxes
        .iter()
        .map(|(r, _)| r.bottom())
        .chain([arrow.1])
        .max()
        .unwrap_or(0);
    let mut canvas = Canvas::new(right + 1, bottom + 1);
    for &(x0, x1, y) in &hlines {
        canvas.hline(x0, x1, y, Line::Light);
    }
    for &(x, y0, y1) in &vlines {
        canvas.vline(x, y0, y1, Line::Light);
    }
    for &(r, lines) in &boxes {
        boxed(&mut canvas, r, lines);
    }
    canvas.arrow(arrow.0, arrow.1, arrow.2);
    let body = canvas.render(options.charset);
    match timeline
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            let rows = tag_rows(&formulas, t, tags).map_or_else(|| vec![t.to_owned()], |r| r.0);
            let mut out = String::new();
            for row in rows {
                let pad = (right + 1 - width(&row)).max(0) / 2;
                out.push_str(&" ".repeat(pad as usize));
                out.push_str(row.trim_end());
                out.push('\n');
            }
            format!("{out}\n{body}")
        }
        None => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping() {
        let long = vec!["one two three four five six seven eight".to_owned()];
        assert_eq!(wrap(&long), ["one two three four five", "six seven eight"]);
        assert_eq!(wrap(&vec!["a".to_owned(), "b".to_owned()]), ["a", "b"]);
    }

    #[test]
    fn tall_vertical_sections_do_not_overlap_following_sections() {
        let src = "timeline TD\nsection one<br>two<br>three<br>four<br>five<br>six<br>seven<br>eight\nA : event\nsection next\nB : event\n";
        let (timeline, diagnostics) = crate::timeline::parse(src).lower();
        assert!(diagnostics.is_empty());
        for charset in [super::super::Charset::Unicode, super::super::Charset::Ascii] {
            let rendered = render(&timeline, &Options { charset });
            let positions: Vec<_> = [
                "one", "two", "three", "four", "five", "six", "seven", "eight", "next",
            ]
            .iter()
            .map(|word| rendered.find(word).expect(word))
            .collect();
            assert!(positions.windows(2).all(|p| p[0] < p[1]), "{rendered}");
        }
    }
}
