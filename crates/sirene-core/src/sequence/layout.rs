//! Where a sequence diagram's parts go, shared by the scene and the text renderer (MSEQ-3).
//! Units are the renderer's: pixels for the scene, cells for text.

use super::{Event, NoteSide, Sequence};

/// Spacing, in the renderer's units.
pub(crate) struct Spacing {
    pub margin: f32,
    /// Between neighboring headers.
    pub col_gap: f32,
    /// Between a message label and the lifelines beside it.
    pub label_pad: f32,
    /// Between a message label and its arrow.
    pub label_gap: f32,
    /// The height an arrow takes: none in pixels, a row in text.
    pub arrow_h: f32,
    /// After each message and note.
    pub row_gap: f32,
    /// A self-message's loop.
    pub self_w: f32,
    pub self_h: f32,
    /// Between a note and the lifeline beside it, and how far a note over two lifelines
    /// reaches past them.
    pub note_gap: f32,
    /// Between a frame and what it holds, on each side.
    pub frame_pad: f32,
    /// Below a frame's tab, below a divider's label, and after a frame.
    pub frame_top: f32,
    pub divider_h: f32,
    pub frame_end: f32,
    /// An activation bar's width; a nested bar is offset by half of it.
    pub bar_w: f32,
    /// Between a group's edge and its headers, and its title's row.
    pub group_pad: f32,
    pub group_title_h: f32,
}

/// What the renderer measured.
pub(crate) struct Sizes {
    /// Each participant's header.
    pub headers: Vec<(f32, f32)>,
    /// Per event: a message's label, a note's box, a block's tab, or a divider's label.
    pub events: Vec<(f32, f32)>,
    /// The title's height, or 0.
    pub title_h: f32,
    /// The narrowest each group can be, to fit its title.
    pub groups: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Placed {
    None,
    /// The arrow runs from `x0` to `x1` at `y`, under a label whose top is `label_y`; a
    /// self-message's loop leaves and returns at `x0`, from `y` down to `y + self_h`.
    Message {
        x0: f32,
        x1: f32,
        y: f32,
        label_y: f32,
    },
    Note {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

/// An activation bar, centered on `x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bar {
    pub participant: usize,
    pub x: f32,
    pub y0: f32,
    pub y1: f32,
}

/// A block's frame; `event` is its `Start`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Frame {
    pub event: usize,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    /// Each divider's event and y.
    pub dividers: Vec<(usize, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GroupBox {
    pub group: usize,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub header_h: f32,
    /// Each lifeline's x.
    pub x: Vec<f32>,
    /// Each header's top: the header row, or a created participant's creation row.
    pub head_y: Vec<f32>,
    /// Where each lifeline ends: the footers' top, or a destroyed participant's cross.
    pub life_end: Vec<f32>,
    pub destroyed: Vec<bool>,
    /// The footers' top.
    pub footer_y: f32,
    pub events: Vec<Placed>,
    pub bars: Vec<Bar>,
    /// Inner frames before the frames around them.
    pub frames: Vec<Frame>,
    pub groups: Vec<GroupBox>,
}

/// A frame still waiting for its `end`, with the horizontal extent of what it holds.
struct OpenFrame {
    event: usize,
    y0: f32,
    x0: f32,
    x1: f32,
    dividers: Vec<(usize, f32)>,
}

struct Rows<'a> {
    s: &'a Spacing,
    x: Vec<f32>,
    /// The open activation bars on each participant, by their top.
    stacks: Vec<Vec<f32>>,
    bars: Vec<Bar>,
    open: Vec<OpenFrame>,
    frames: Vec<Frame>,
}

impl Rows<'_> {
    /// The center of a participant's bar at `depth` (1 for the first).
    fn bar_x(&self, p: usize, depth: usize) -> f32 {
        self.x[p] + depth.saturating_sub(1) as f32 * self.s.bar_w / 2.0
    }

    /// Where a message meets participant `p` with `depth` open bars, from the right or the
    /// left side.
    fn edge(&self, p: usize, depth: usize, right: bool) -> f32 {
        if depth == 0 {
            return self.x[p];
        }
        let half = self.s.bar_w / 2.0;
        self.bar_x(p, depth) + if right { half } else { -half }
    }

    fn activate(&mut self, p: usize, y: f32) {
        self.stacks[p].push(y);
    }

    fn deactivate(&mut self, p: usize, y: f32) {
        let depth = self.stacks[p].len();
        if let Some(y0) = self.stacks[p].pop() {
            self.bars.push(Bar {
                participant: p,
                x: self.bar_x(p, depth),
                y0,
                y1: y,
            });
        }
    }

    /// Widens the innermost open frame to hold `x0..x1`.
    fn extend(&mut self, x0: f32, x1: f32) {
        if let Some(f) = self.open.last_mut() {
            f.x0 = f.x0.min(x0);
            f.x1 = f.x1.max(x1);
        }
    }

    /// Closes the innermost frame at `y`, at least `min_w` wide.
    fn close(&mut self, y: f32, min_w: f32) {
        let Some(f) = self.open.pop() else { return };
        let pad = self.s.frame_pad;
        let (mut x0, mut x1) = if f.x0 <= f.x1 {
            (f.x0 - pad, f.x1 + pad)
        } else {
            let x = self.x.first().copied().unwrap_or(0.0);
            (x - pad, x + pad)
        };
        x1 = x1.max(x0 + min_w);
        x0 = x0.min(x1 - min_w);
        self.extend(x0, x1);
        self.frames.push(Frame {
            event: f.event,
            x0,
            x1,
            y0: f.y0,
            y1: y,
            dividers: f.dividers,
        });
    }
}

/// Places `seq`'s parts (MSEQ-3).
pub(crate) fn layout(seq: &Sequence, sizes: &Sizes, s: &Spacing) -> Layout {
    let n = seq.participants.len();
    let hw = |i: usize| sizes.headers[i].0;
    let header_h = sizes.headers.iter().map(|h| h.1).fold(0.0, f32::max);
    let grouped = |i: usize| seq.participants[i].group;
    let group_pad = |i: usize| {
        if grouped(i).is_some() {
            s.group_pad
        } else {
            0.0
        }
    };

    // Columns: `x[b] - x[a] >= d` for each pair, and room at each edge.
    let mut pairs: Vec<(usize, usize, f32)> = Vec::new();
    let mut left: Vec<f32> = (0..n).map(|i| hw(i) / 2.0 + group_pad(i)).collect();
    let mut right = left.clone();
    for i in 1..n {
        let mut d = hw(i - 1) / 2.0 + hw(i) / 2.0 + s.col_gap;
        if grouped(i - 1) != grouped(i) {
            d += group_pad(i - 1) + group_pad(i);
        }
        pairs.push((i - 1, i, d));
    }
    let mut spans: Vec<(usize, usize, f32)> = Vec::new();
    // Room beside participant `p`: to its right when `after`, else to its left.
    let mut beside = |p: usize, after: bool, d: f32| {
        if after && p + 1 < n {
            pairs.push((p, p + 1, d));
        } else if !after && p > 0 {
            pairs.push((p - 1, p, d));
        } else if after {
            right[p] = right[p].max(d);
        } else {
            left[p] = left[p].max(d);
        }
    };
    for (k, e) in seq.events.iter().enumerate() {
        let (w, _) = sizes.events[k];
        match e {
            Event::Message(m) if m.from == m.to => {
                let d = s.self_w.max(s.label_pad + w) + s.label_pad + s.bar_w;
                beside(m.from, true, d);
            }
            Event::Message(m) => {
                let (lo, hi) = (m.from.min(m.to), m.from.max(m.to));
                spans.push((lo, hi, w + 2.0 * s.label_pad + s.bar_w));
            }
            Event::Note(note) => {
                let Some(&a) = note.participants.first() else {
                    continue;
                };
                match note.side {
                    NoteSide::Left => beside(a, false, w + 2.0 * s.note_gap),
                    NoteSide::Right => beside(a, true, w + 2.0 * s.note_gap),
                    NoteSide::Over if note.participants.len() == 1 => {
                        beside(a, false, w / 2.0 + s.note_gap);
                        beside(a, true, w / 2.0 + s.note_gap);
                    }
                    NoteSide::Over => {}
                }
            }
            _ => {}
        }
    }
    pairs.extend(spans);
    let mut x = vec![0.0; n];
    for b in 0..n {
        let mut v = s.margin + left[b];
        for &(a, bb, d) in &pairs {
            if bb == b {
                v = v.max(x[a] + d);
            }
        }
        x[b] = v;
    }
    let mut max_x = (0..n).map(|i| x[i] + right[i]).fold(s.margin, f32::max);

    // Rows.
    let any_title = seq.groups.iter().any(|g| g.title.is_some());
    let groups_top = s.margin + sizes.title_h;
    let top = if seq.groups.is_empty() {
        groups_top
    } else {
        groups_top + s.group_pad + if any_title { s.group_title_h } else { 0.0 }
    };
    let mut head_y = vec![top; n];
    let mut life_end = vec![0.0; n];
    let mut destroyed = vec![false; n];
    let mut doomed = vec![false; n];
    let mut placed = Vec::with_capacity(seq.events.len());
    let mut rows = Rows {
        s,
        x: x.clone(),
        stacks: vec![Vec::new(); n],
        bars: Vec::new(),
        open: Vec::new(),
        frames: Vec::new(),
    };
    let mut y = top + header_h + s.row_gap;
    // The arrow of the event just before, where an `activate` after it begins.
    let mut last_arrow: Option<f32> = None;
    for (k, e) in seq.events.iter().enumerate() {
        let (w, h) = sizes.events[k];
        let mut arrow = None;
        let mut place = Placed::None;
        match e {
            Event::Message(m) => {
                let label_y = y;
                let ay = y + h + if h > 0.0 { s.label_gap } else { 0.0 };
                let from_depth = rows.stacks[m.from].len();
                if m.from == m.to {
                    let x0 = rows.edge(m.from, from_depth, true);
                    rows.extend(x[m.from], x0 + s.self_w.max(s.label_pad + w));
                    place = Placed::Message {
                        x0,
                        x1: x0,
                        y: ay,
                        label_y,
                    };
                    arrow = Some(ay + s.self_h);
                    y = ay + s.self_h + s.arrow_h + s.row_gap;
                } else {
                    let rightward = m.to > m.from;
                    let to_depth = rows.stacks[m.to].len() + usize::from(m.activate);
                    let x0 = rows.edge(m.from, from_depth, rightward);
                    let x1 = rows.edge(m.to, to_depth, !rightward);
                    rows.extend(x0.min(x1), x0.max(x1));
                    place = Placed::Message {
                        x0,
                        x1,
                        y: ay,
                        label_y,
                    };
                    arrow = Some(ay);
                    y = ay + s.arrow_h + s.row_gap;
                }
                let at = arrow.unwrap_or(ay);
                for p in [m.from, m.to] {
                    if doomed[p] {
                        doomed[p] = false;
                        destroyed[p] = true;
                        life_end[p] = at;
                    }
                }
                if m.deactivate {
                    rows.deactivate(m.from, at);
                }
                if m.activate {
                    rows.activate(m.to, at);
                }
            }
            Event::Note(note) => {
                let (Some(&a), Some(&b)) = (note.participants.first(), note.participants.last())
                else {
                    placed.push(place);
                    continue;
                };
                let (nx, nw) = match note.side {
                    NoteSide::Left => (x[a] - s.note_gap - w, w),
                    NoteSide::Right => (x[a] + s.note_gap, w),
                    NoteSide::Over if a == b => (x[a] - w / 2.0, w),
                    NoteSide::Over => {
                        let (lo, hi) = (x[a].min(x[b]), x[a].max(x[b]));
                        let nw = w.max(hi - lo + 2.0 * s.note_gap);
                        ((lo + hi) / 2.0 - nw / 2.0, nw)
                    }
                };
                rows.extend(nx, nx + nw);
                place = Placed::Note { x: nx, y, w: nw, h };
                y += h + s.row_gap;
            }
            Event::Activate(p) => {
                rows.activate(*p, last_arrow.unwrap_or(y));
                placed.push(place);
                continue;
            }
            Event::Deactivate(p) => {
                rows.deactivate(*p, last_arrow.unwrap_or(y));
                placed.push(place);
                continue;
            }
            Event::Start(..) => {
                rows.open.push(OpenFrame {
                    event: k,
                    y0: y,
                    x0: f32::INFINITY,
                    x1: f32::NEG_INFINITY,
                    dividers: Vec::new(),
                });
                y += h + s.frame_top;
            }
            Event::Divider(_) => {
                if let Some(f) = rows.open.last_mut() {
                    f.dividers.push((k, y));
                }
                y += h + s.divider_h;
            }
            Event::End => {
                let min_w = min_frame_w(sizes, rows.open.last());
                rows.close(y, min_w);
                y += s.frame_end;
            }
            Event::Create(p) => {
                head_y[*p] = y;
                y += header_h + s.row_gap;
            }
            Event::Destroy(p) => doomed[*p] = true,
        }
        last_arrow = arrow;
        placed.push(place);
    }
    while !rows.open.is_empty() {
        let min_w = min_frame_w(sizes, rows.open.last());
        rows.close(y, min_w);
        y += s.frame_end;
    }
    for p in 0..n {
        while !rows.stacks[p].is_empty() {
            rows.deactivate(p, y);
        }
        if doomed[p] {
            destroyed[p] = true;
            life_end[p] = y;
        }
    }
    if doomed.iter().any(|&d| d) {
        y += s.row_gap;
    }
    let footer_y = y;
    for p in 0..n {
        if !destroyed[p] {
            life_end[p] = footer_y;
        }
    }
    let mut bottom = footer_y + header_h;

    let mut groups = Vec::new();
    for g in 0..seq.groups.len() {
        let members: Vec<usize> = (0..n).filter(|&i| grouped(i) == Some(g)).collect();
        if members.is_empty() {
            continue;
        }
        let x0 = members
            .iter()
            .map(|&i| x[i] - hw(i) / 2.0)
            .fold(f32::INFINITY, f32::min);
        let x1 = members
            .iter()
            .map(|&i| x[i] + hw(i) / 2.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_w = sizes.groups.get(g).copied().unwrap_or(0.0);
        groups.push(GroupBox {
            group: g,
            x0: x0 - s.group_pad,
            x1: (x1 + s.group_pad).max(x0 - s.group_pad + min_w),
            y0: groups_top,
            y1: footer_y + header_h + s.group_pad,
        });
    }
    if !groups.is_empty() {
        bottom += s.group_pad;
    }

    // Shift right when a frame or note reaches past the left margin.
    let mut min_x = (0..n).map(|i| x[i] - left[i]).fold(s.margin, f32::min);
    for f in &rows.frames {
        min_x = min_x.min(f.x0);
        max_x = max_x.max(f.x1);
    }
    for p in &placed {
        if let Placed::Note { x, w, .. } = p {
            min_x = min_x.min(*x);
            max_x = max_x.max(x + w);
        }
    }
    for g in &groups {
        min_x = min_x.min(g.x0);
        max_x = max_x.max(g.x1);
    }
    let dx = (s.margin - min_x).max(0.0);
    let mut layout = Layout {
        width: max_x + dx + s.margin,
        height: bottom + s.margin,
        header_h,
        x: x.iter().map(|v| v + dx).collect(),
        head_y,
        life_end,
        destroyed,
        footer_y,
        events: placed,
        bars: rows.bars,
        frames: rows.frames,
        groups,
    };
    if dx > 0.0 {
        for p in &mut layout.events {
            match p {
                Placed::Message { x0, x1, .. } => {
                    *x0 += dx;
                    *x1 += dx;
                }
                Placed::Note { x, .. } => *x += dx,
                Placed::None => {}
            }
        }
        for b in &mut layout.bars {
            b.x += dx;
        }
        for f in &mut layout.frames {
            f.x0 += dx;
            f.x1 += dx;
        }
        for g in &mut layout.groups {
            g.x0 += dx;
            g.x1 += dx;
        }
    }
    layout
}

/// The narrowest a frame can be: its tab and its dividers' labels.
fn min_frame_w(sizes: &Sizes, f: Option<&OpenFrame>) -> f32 {
    let Some(f) = f else { return 0.0 };
    f.dividers
        .iter()
        .map(|&(k, _)| sizes.events[k].0)
        .fold(sizes.events[f.event].0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::parse;

    const SPACING: Spacing = Spacing {
        margin: 0.0,
        col_gap: 2.0,
        label_pad: 1.0,
        label_gap: 0.0,
        arrow_h: 1.0,
        row_gap: 1.0,
        self_w: 3.0,
        self_h: 1.0,
        note_gap: 1.0,
        frame_pad: 2.0,
        frame_top: 0.0,
        divider_h: 0.0,
        frame_end: 1.0,
        bar_w: 2.0,
        group_pad: 1.0,
        group_title_h: 1.0,
    };

    fn place(src: &str) -> (Sequence, Layout) {
        let (seq, _) = parse(src).lower();
        let sizes = Sizes {
            headers: vec![(5.0, 3.0); seq.participants.len()],
            events: seq
                .events
                .iter()
                .map(|e| match e {
                    Event::Message(m) => (m.text.join(" ").len() as f32, 1.0),
                    Event::Note(n) => (n.text.join(" ").len() as f32 + 4.0, 3.0),
                    _ => (4.0, 1.0),
                })
                .collect(),
            title_h: 0.0,
            groups: Vec::new(),
        };
        let l = layout(&seq, &sizes, &SPACING);
        (seq, l)
    }

    #[test]
    fn columns_fit_labels() {
        let (_, l) = place("sequenceDiagram\n  A->>B: a long message label\n  B->>C: x\n");
        // Label 22 wide, plus a pad each side.
        assert_eq!(l.x[1] - l.x[0], 24.0);
        // Headers 5 wide, 2 apart.
        assert_eq!(l.x[2] - l.x[1], 7.0);
        assert_eq!(l.x[0], 2.5);
    }

    #[test]
    fn rows_activations_and_frames() {
        let (_, l) =
            place("sequenceDiagram\n  loop L\n  A->>+B: x\n  B-->>-A: y\n  end\n  A->>A: self\n");
        let Placed::Message { y: y1, x1, .. } = l.events[1] else {
            panic!()
        };
        let Placed::Message { y: y2, .. } = l.events[2] else {
            panic!()
        };
        assert!(y2 > y1);
        // The arrow stops at the bar's left edge.
        assert_eq!(x1, l.x[1] - 1.0);
        assert_eq!(l.bars.len(), 1);
        assert_eq!((l.bars[0].y0, l.bars[0].y1), (y1, y2));
        assert_eq!(l.frames.len(), 1);
        let f = &l.frames[0];
        assert!(f.y0 < y1 && f.y1 > y2);
        assert!(f.x0 < l.x[0] && f.x1 > l.x[1]);
    }

    #[test]
    fn created_and_destroyed() {
        let (_, l) = place(
            "sequenceDiagram\n  A->>B: x\n  create participant C\n  A->>C: hi\n  destroy C\n  C-xA: bye\n",
        );
        assert!(l.head_y[2] > l.head_y[0]);
        assert!(l.destroyed[2] && !l.destroyed[0]);
        assert!(l.life_end[2] < l.footer_y);
        assert_eq!(l.life_end[0], l.footer_y);
    }

    #[test]
    fn notes_widen_and_shift() {
        let (_, l) = place("sequenceDiagram\n  note left of A: a wide note on the left\n");
        let Placed::Note { x, .. } = l.events[0] else {
            panic!()
        };
        assert!(x >= 0.0);
        assert!(l.x[0] > 20.0);
    }
}
