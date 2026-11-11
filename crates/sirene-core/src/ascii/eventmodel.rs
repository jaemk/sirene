//! Event modeling diagrams as text (MEVM-4): the swimlanes as rows of a box split by dotted
//! lines, each frame a box tagged with its number and type, placed along the timeline;
//! relations between lanes are drawn as vertical arrows where a straight one fits, and the rest
//! are listed below.

use super::canvas::{Canvas, Line, glyph};
use super::{Charset, Options, draw_box, width};
use crate::eventmodel::EventModel;
use crate::layout::{Point, Rect};

/// The widest a name or data line runs before wrapping.
const NAME_MAX: i32 = 28;
const DATA_MAX: i32 = 36;

/// `s` broken at spaces into lines at most `max` wide; a longer word keeps a line of its own.
/// Leading spaces are kept.
fn wrap(s: &str, max: i32) -> Vec<String> {
    let lead = &s[..s.len() - s.trim_start().len()];
    let mut out: Vec<String> = Vec::new();
    let mut line = lead.to_owned();
    for word in s.split_whitespace() {
        if width(line.trim()) > 0 && width(&line) + 1 + width(word) > max {
            out.push(std::mem::take(&mut line));
        }
        if width(line.trim()) > 0 {
            line.push(' ');
        }
        line.push_str(word);
    }
    out.push(line);
    out
}

struct Boxed {
    tag: String,
    name: Vec<String>,
    data: Vec<String>,
    r: Rect,
}

/// Renders `chart` as text (MEVM-4).
pub fn render(chart: &EventModel, options: &Options) -> String {
    let arrow = match options.charset {
        Charset::Unicode => "\u{2192}",
        Charset::Ascii => "->",
    };
    let labels: Vec<String> = chart.lanes.iter().map(|l| l.label()).collect();
    let start = 2 + labels.iter().map(|l| width(l)).max().unwrap_or(0) + 3;

    // Boxes along the timeline: after the lane's last box, or else starting halfway along the
    // previous frame's box.
    let mut boxes: Vec<Boxed> = Vec::with_capacity(chart.frames.len());
    let mut lane_r: Vec<Option<i32>> = vec![None; chart.lanes.len()];
    let mut lane_h = vec![0; chart.lanes.len()];
    for (i, f) in chart.frames.iter().enumerate() {
        let tag = format!(" {} {} ", f.id, f.kind.short());
        let name = wrap(&f.name, NAME_MAX);
        let data: Vec<String> = f.data.iter().flat_map(|l| wrap(l, DATA_MAX)).collect();
        let iw = name
            .iter()
            .chain(&data)
            .map(|l| width(l))
            .max()
            .unwrap_or(0)
            .max(width(&tag) - 2);
        let h = name.len() as i32 + data.len() as i32 + 2 + i32::from(!data.is_empty());
        let w = iw + 4;
        let x = match i.checked_sub(1) {
            None => start,
            Some(p) if chart.frames[p].lane == f.lane => lane_r[f.lane].unwrap_or(start) + 3,
            Some(p) => {
                let prev = &boxes[p].r;
                let x = prev.x + prev.w / 2;
                lane_r[f.lane].map_or(x, |r| x.max(r + 3))
            }
        };
        lane_r[f.lane] = Some(x + w - 1);
        lane_h[f.lane] = lane_h[f.lane].max(h);
        boxes.push(Boxed {
            tag,
            name,
            data,
            r: Rect::new(x, 0, w, h),
        });
    }

    // Lanes: a separator, a blank row, the boxes, and a blank row.
    let mut tops = Vec::with_capacity(chart.lanes.len());
    let mut y = 0;
    for h in &lane_h {
        tops.push(y);
        y += h + 3;
    }
    let bottom = y;
    for (b, f) in boxes.iter_mut().zip(&chart.frames) {
        b.r.y = tops[f.lane] + 2;
    }
    let right = boxes.iter().map(|b| b.r.right()).max().unwrap_or(start) + 3;
    let mut canvas = Canvas::new(right + 1, bottom + 1);
    draw_box(
        &mut canvas,
        Rect::new(0, 0, right + 1, bottom + 1),
        Line::Light,
    );
    for &t in tops.iter().skip(1) {
        canvas.hline(0, right, t, Line::Dotted);
    }
    for (label, &t) in labels.iter().zip(&tops) {
        canvas.text(2, t + 2, label);
    }
    for b in &boxes {
        let r = b.r;
        draw_box(&mut canvas, r, Line::Light);
        canvas.text(r.x + 1, r.y, &b.tag);
        let mut row = r.y + 1;
        for n in &b.name {
            canvas.text(r.x + 2 + (r.w - 4 - width(n)) / 2, row, n);
            row += 1;
        }
        if !b.data.is_empty() {
            canvas.hline(r.x, r.right(), row, Line::Light);
            row += 1;
            for d in &b.data {
                canvas.text(r.x + 2, row, d);
                row += 1;
            }
        }
    }

    // Relations between lanes, as vertical arrows where one fits.
    let occupied = |x: i32, y: i32| boxes.iter().any(|b| b.r.contains(Point::new(x, y)));
    let mut listed = Vec::new();
    for rel in &chart.relations {
        let (s, t) = (&boxes[rel.from], &boxes[rel.to]);
        let down = s.r.y < t.r.y;
        let drawn = s.r.y != t.r.y && {
            // The rows the arrow crosses between the two boxes.
            let (y0, y1) = if down {
                (s.r.bottom() + 1, t.r.y - 1)
            } else {
                (t.r.bottom() + 1, s.r.y - 1)
            };
            // Columns inside both boxes, clear of the source's tag when leaving its top.
            let lo = s.r.x.max(t.r.x) + 1;
            let lo = if down {
                lo
            } else {
                lo.max(s.r.x + width(&s.tag) + 1)
            };
            let hi = s.r.right().min(t.r.right()) - 1;
            let mid = (lo + hi) / 2;
            let column = (lo..=hi)
                .filter(|&c| (y0..=y1).all(|y| !occupied(c, y)))
                .min_by_key(|&c| (c - mid).abs());
            match column {
                Some(c) if down => {
                    canvas.vline(c, s.r.bottom(), y1, Line::Light);
                    canvas.arrow(c, y1, glyph('▼', 'v'));
                    true
                }
                Some(c) => {
                    canvas.vline(c, y0, s.r.y, Line::Light);
                    canvas.arrow(c, y0, glyph('▲', '^'));
                    true
                }
                None => false,
            }
        };
        if !drawn {
            listed.push(rel);
        }
    }

    let mut out = String::new();
    if let Some(t) = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pad = ((right + 1 - width(t)) / 2).max(0) as usize;
        out.push_str(&" ".repeat(pad));
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str(&canvas.render(options.charset));
    if !listed.is_empty() {
        out.push('\n');
        for rel in listed {
            let (a, b) = (&chart.frames[rel.from], &chart.frames[rel.to]);
            out.push_str(&format!(
                "{} {} {arrow} {} {}\n",
                a.id, a.name, b.id, b.name
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(src: &str, charset: Charset) -> String {
        let (chart, d) = crate::eventmodel::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        render(&chart, &Options { charset })
    }

    #[test]
    fn wrapping() {
        assert_eq!(wrap("a b c", 3), ["a b", "c"]);
        assert_eq!(wrap("  a: b", 10), ["  a: b"]);
        assert_eq!(wrap("", 3), [""]);
    }

    #[test]
    fn lanes_boxes_and_arrows() {
        let out = draw(
            "eventmodeling\ntf 01 ui CartUI\ntf 02 cmd AddItem { description: string }\n\
             tf 03 evt ItemAdded\n",
            Charset::Unicode,
        );
        let rows: Vec<&str> = out.lines().collect();
        // A lane's label is level with its boxes' tops.
        assert!(
            rows[2].contains("UI/Automation") && rows[2].contains("┌ 01 ui ─┐"),
            "{out}"
        );
        assert!(rows[3].contains("│ CartUI │"), "{out}");
        assert!(out.contains("┌ 02 cmd ─"), "{out}");
        assert!(out.contains("description: string"), "{out}");
        // Both relations are drawn, so none is listed.
        assert_eq!(out.matches('▼').count(), 2, "{out}");
        assert!(out.ends_with("┘\n"), "{out}");
    }

    #[test]
    fn upward_and_listed() {
        let out = draw(
            "eventmodeling\ntitle Cart\nrf 01 evt ItemAdded\nrf 02 rmo CartItems ->> 01\n\
             tf 03 ui CartUI\nrf 04 cmd Clear ->> 03\nrf 05 evt Cleared ->> 04\n\
             rf 06 rmo Again ->> 01\n",
            Charset::Ascii,
        );
        assert!(out.is_ascii(), "{out}");
        assert!(
            out.starts_with("  ") && out.lines().next().unwrap().trim() == "Cart",
            "{out}"
        );
        assert!(out.contains('^'), "{out}");
        // ItemAdded is far behind Again, so that relation is listed.
        assert!(out.ends_with("\n01 ItemAdded -> 06 Again\n"), "{out}");
    }
}
