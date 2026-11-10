//! Sequence diagram to scene (MSEQ-3, MSEQ-4).

use super::shapes::{self, BoxF, PAD_X, PAD_Y};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
    parse_color,
};
use crate::flowchart::Shape;
use crate::sequence::layout::{self, Placed, Sizes, Spacing};
use crate::sequence::{BlockKind, Event, Head, LineStyle, ParticipantKind, Sequence};

const SPACING: Spacing = Spacing {
    margin: 16.0,
    col_gap: 40.0,
    label_pad: 12.0,
    label_gap: 4.0,
    arrow_h: 0.0,
    row_gap: 14.0,
    self_w: 36.0,
    self_h: 20.0,
    note_gap: 10.0,
    frame_pad: 12.0,
    frame_top: 10.0,
    divider_h: 10.0,
    frame_end: 12.0,
    bar_w: 10.0,
    group_pad: 10.0,
    group_title_h: 24.0,
};

/// Narrowest header.
const MIN_W: f32 = 90.0;
/// Height of an actor's stick figure.
const FIGURE: f32 = 44.0;
/// Height of a boundary, control, or entity symbol.
const ICON: f32 = 32.0;
/// Radius of a symbol's circle.
const ICON_R: f32 = 12.0;
/// Space between a figure or symbol and its label.
const BELOW: f32 = 4.0;
/// Offset of the back box of `collections`.
const STACK: f32 = 6.0;
/// Radius of a queue's end ellipses.
const QUEUE_RX: f32 = 8.0;
/// Padding inside a note.
const NOTE_PAD: f32 = 10.0;
/// Padding inside a frame's tab.
const TAB_PAD: f32 = 8.0;
/// Arrowhead length and half width.
const HEAD: f32 = 10.0;
const HEAD_HALF: f32 = 5.0;
/// Radius of an `autonumber` badge.
const BADGE: f32 = 9.0;

struct Painter<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
    elements: Vec<Element>,
}

fn solid(width: f32, color: Color) -> Stroke {
    Stroke {
        width,
        color,
        dash: None,
    }
}

fn dashed(width: f32, color: Color) -> Stroke {
    Stroke {
        width,
        color,
        dash: Some((5.0, 4.0)),
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    shapes::polygon(&[
        (b.x, b.y),
        (b.right(), b.y),
        (b.right(), b.bottom()),
        (b.x, b.bottom()),
    ])
}

fn line(a: Pt, b: Pt) -> Vec<Segment> {
    vec![Segment::Move(a), Segment::Line(b)]
}

impl<M: TextMetrics> Painter<'_, M> {
    fn width(&self, text: &str) -> f32 {
        self.metrics.width(text, self.theme.font_size)
    }

    fn bold_width(&self, text: &str) -> f32 {
        let bold = TextStyle {
            bold: true,
            italic: false,
        };
        self.metrics.styled_width(text, self.theme.font_size, bold)
    }

    /// The width and height of `lines`.
    fn measure(&self, lines: &[String]) -> (f32, f32) {
        let w = lines.iter().map(|l| self.width(l)).fold(0.0, f32::max);
        (w, lines.len() as f32 * self.theme.line_height)
    }

    fn path(&mut self, segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                segments,
                fill,
                stroke,
            },
        });
    }

    fn text_at(&mut self, text: &str, center: Pt, size: f32, color: Color, style: TextStyle) {
        if text.is_empty() {
            return;
        }
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: text.to_owned(),
                center,
                size,
                color,
                style,
            },
        });
    }

    /// `lines` centered on `cx`, the first line's top at `top`.
    fn lines(&mut self, lines: &[String], cx: f32, top: f32) {
        let lh = self.theme.line_height;
        let color = self.theme.text;
        for (i, l) in lines.iter().enumerate() {
            let center = Pt::new(cx, top + (i as f32 + 0.5) * lh);
            self.text_at(l, center, self.theme.font_size, color, TextStyle::default());
        }
    }

    /// The size of a header for `kind` around `lines`.
    fn header_size(&self, kind: ParticipantKind, lines: &[String]) -> (f32, f32) {
        let (tw, th) = self.measure(lines);
        let w = (tw + 2.0 * PAD_X).max(MIN_W);
        match kind {
            ParticipantKind::Participant => (w, th + 2.0 * PAD_Y),
            ParticipantKind::Collections => (w + STACK, th + 2.0 * PAD_Y + STACK),
            ParticipantKind::Database => {
                let (_, h) = shapes::size(Shape::Cylinder, w - 2.0 * PAD_X, th);
                (w, h)
            }
            ParticipantKind::Queue => (w + 2.0 * QUEUE_RX, th + 2.0 * PAD_Y),
            ParticipantKind::Actor => (tw.max(40.0), FIGURE + BELOW + th),
            ParticipantKind::Boundary | ParticipantKind::Control | ParticipantKind::Entity => {
                (tw.max(2.0 * ICON_R + 24.0), ICON + BELOW + th)
            }
        }
    }

    /// Draws a header filling `b`.
    fn header(&mut self, kind: ParticipantKind, lines: &[String], b: BoxF) {
        let fill = Some(self.theme.node_fill);
        let stroke = Some(solid(1.5, self.theme.node_stroke));
        let cx = b.x + b.w / 2.0;
        let (_, th) = self.measure(lines);
        let centered =
            |p: &mut Self, t: BoxF| p.lines(lines, t.x + t.w / 2.0, t.y + (t.h - th) / 2.0);
        match kind {
            ParticipantKind::Participant => {
                self.path(shapes::rounded(b, 4.0), fill, stroke);
                centered(self, b);
            }
            ParticipantKind::Collections => {
                let back = BoxF {
                    x: b.x + STACK,
                    y: b.y,
                    w: b.w - STACK,
                    h: b.h - STACK,
                };
                let front = BoxF {
                    x: b.x,
                    y: b.y + STACK,
                    ..back
                };
                self.path(rect(back), fill, stroke);
                self.path(rect(front), fill, stroke);
                centered(self, front);
            }
            ParticipantKind::Database => {
                let o = shapes::outline(Shape::Cylinder, b);
                self.path(o.body, fill, stroke);
                for l in o.lines {
                    self.path(l, None, stroke);
                }
                centered(self, shapes::text_box(Shape::Cylinder, b));
            }
            ParticipantKind::Queue => {
                let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
                let ry = b.h / 2.0;
                let arc = |sweep, to: Pt| Segment::Arc {
                    radii: Pt::new(QUEUE_RX, ry),
                    large: false,
                    sweep,
                    to,
                };
                let body = vec![
                    Segment::Move(Pt::new(x + QUEUE_RX, y)),
                    Segment::Line(Pt::new(r - QUEUE_RX, y)),
                    arc(true, Pt::new(r - QUEUE_RX, bottom)),
                    Segment::Line(Pt::new(x + QUEUE_RX, bottom)),
                    arc(true, Pt::new(x + QUEUE_RX, y)),
                    Segment::Close,
                ];
                self.path(body, fill, stroke);
                let inner = vec![
                    Segment::Move(Pt::new(r - QUEUE_RX, y)),
                    arc(false, Pt::new(r - QUEUE_RX, bottom)),
                ];
                self.path(inner, None, stroke);
                centered(
                    self,
                    BoxF {
                        w: b.w - 2.0 * QUEUE_RX,
                        ..b
                    },
                );
            }
            ParticipantKind::Actor => {
                let y = b.y;
                let s = stroke;
                self.path(shapes::ellipse(Pt::new(cx, y + 8.0), 7.0, 7.0), fill, s);
                self.path(line(Pt::new(cx, y + 15.0), Pt::new(cx, y + 30.0)), None, s);
                self.path(
                    line(Pt::new(cx - 12.0, y + 20.0), Pt::new(cx + 12.0, y + 20.0)),
                    None,
                    s,
                );
                self.path(
                    line(Pt::new(cx, y + 30.0), Pt::new(cx - 10.0, y + 42.0)),
                    None,
                    s,
                );
                self.path(
                    line(Pt::new(cx, y + 30.0), Pt::new(cx + 10.0, y + 42.0)),
                    None,
                    s,
                );
                self.lines(lines, cx, y + FIGURE + BELOW);
            }
            ParticipantKind::Boundary | ParticipantKind::Control | ParticipantKind::Entity => {
                let cy = b.y + ICON / 2.0;
                let c = match kind {
                    ParticipantKind::Boundary => Pt::new(cx + 6.0, cy),
                    _ => Pt::new(cx, cy),
                };
                self.path(shapes::ellipse(c, ICON_R, ICON_R), fill, stroke);
                match kind {
                    ParticipantKind::Boundary => {
                        let bar = c.x - ICON_R - 8.0;
                        self.path(
                            line(Pt::new(bar, cy - ICON_R), Pt::new(bar, cy + ICON_R)),
                            None,
                            stroke,
                        );
                        self.path(
                            line(Pt::new(bar, cy), Pt::new(c.x - ICON_R, cy)),
                            None,
                            stroke,
                        );
                    }
                    ParticipantKind::Control => {
                        let top = cy - ICON_R;
                        let chevron = vec![
                            Segment::Move(Pt::new(cx + 5.0, top - 4.0)),
                            Segment::Line(Pt::new(cx - 1.0, top)),
                            Segment::Line(Pt::new(cx + 5.0, top + 4.0)),
                        ];
                        self.path(chevron, None, stroke);
                    }
                    _ => {
                        let under = cy + ICON_R + 2.0;
                        self.path(
                            line(Pt::new(cx - ICON_R, under), Pt::new(cx + ICON_R, under)),
                            None,
                            stroke,
                        );
                    }
                }
                self.lines(lines, cx, b.y + ICON + BELOW);
            }
        }
    }

    /// An arrowhead of `head` with its tip at `tip`, pointing along `dir` (+1 right, -1 left).
    /// Returns where the line should stop.
    fn head(&mut self, head: Head, tip: Pt, dir: f32, color: Color) -> Pt {
        let base = Pt::new(tip.x - dir * HEAD, tip.y);
        match head {
            Head::None => tip,
            Head::Filled => {
                let tri = shapes::polygon(&[
                    (base.x, base.y - HEAD_HALF),
                    (tip.x, tip.y),
                    (base.x, base.y + HEAD_HALF),
                ]);
                self.path(tri, Some(color), None);
                base
            }
            Head::Open => {
                let chevron = vec![
                    Segment::Move(Pt::new(base.x, base.y - HEAD_HALF)),
                    Segment::Line(tip),
                    Segment::Line(Pt::new(base.x, base.y + HEAD_HALF)),
                ];
                self.path(chevron, None, Some(solid(1.5, color)));
                tip
            }
            Head::Cross => {
                let c = Pt::new(tip.x - dir * HEAD_HALF, tip.y);
                let d = HEAD_HALF;
                self.path(
                    line(Pt::new(c.x - d, c.y - d), Pt::new(c.x + d, c.y + d)),
                    None,
                    Some(solid(1.5, color)),
                );
                self.path(
                    line(Pt::new(c.x - d, c.y + d), Pt::new(c.x + d, c.y - d)),
                    None,
                    Some(solid(1.5, color)),
                );
                tip
            }
        }
    }

    /// An `autonumber` badge centered on `at`.
    fn badge(&mut self, n: u32, at: Pt) {
        self.path(
            shapes::ellipse(at, BADGE, BADGE),
            Some(self.theme.edge),
            None,
        );
        let size = self.theme.font_size * 0.75;
        let bg = self.theme.background;
        self.text_at(&n.to_string(), at, size, bg, TextStyle::default());
    }
}

/// A block's tab text.
fn tab_text(kind: BlockKind) -> &'static str {
    kind.keyword()
}

fn bracketed(label: &str) -> String {
    if label.is_empty() {
        String::new()
    } else {
        format!("[{label}]")
    }
}

/// Draws `seq` (MSEQ-3, MSEQ-4).
pub fn sequence(seq: &Sequence, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
    };
    let lh = theme.line_height;
    let title = seq
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let headers: Vec<(f32, f32)> = seq
        .participants
        .iter()
        .map(|part| p.header_size(part.kind, &part.label))
        .collect();
    let tab_w = |p: &Painter<_>, kind| p.bold_width(tab_text(kind)) + 2.0 * TAB_PAD;
    let sizes = Sizes {
        headers: headers.clone(),
        events: seq
            .events
            .iter()
            .map(|e| match e {
                Event::Message(m) => p.measure(&m.text),
                Event::Note(n) => {
                    let (w, h) = p.measure(&n.text);
                    ((w + 2.0 * NOTE_PAD).max(60.0), h + 2.0 * NOTE_PAD)
                }
                Event::Start(BlockKind::Rect, _) => (0.0, 0.0),
                Event::Start(kind, label) => {
                    let lw = p.width(&bracketed(label));
                    (tab_w(&p, *kind) + 2.0 * TAB_PAD + lw, lh + 4.0)
                }
                Event::Divider(label) if label.is_empty() => (0.0, 0.0),
                Event::Divider(label) => (p.width(&bracketed(label)) + 4.0 * TAB_PAD, lh),
                _ => (0.0, 0.0),
            })
            .collect(),
        title_h: if title.is_some() { lh + 12.0 } else { 0.0 },
        groups: seq
            .groups
            .iter()
            .map(|g| {
                g.title
                    .as_deref()
                    .map_or(0.0, |t| p.bold_width(t) + 2.0 * TAB_PAD)
            })
            .collect(),
    };
    let l = layout::layout(seq, &sizes, &SPACING);
    let header_h = l.header_h;
    let lifeline = theme.edge.with_alpha(0x60).over(theme.background);

    // Groups.
    for g in &l.groups {
        let group = &seq.groups[g.group];
        let fill = group
            .color
            .as_deref()
            .and_then(parse_color)
            .filter(|c| c.a > 0)
            .map(|c| c.with_alpha(0x50).over(theme.background));
        let b = BoxF {
            x: g.x0,
            y: g.y0,
            w: g.x1 - g.x0,
            h: g.y1 - g.y0,
        };
        p.path(rect(b), fill, Some(solid(1.0, theme.cluster_stroke)));
        if let Some(t) = &group.title {
            let center = Pt::new(
                b.x + b.w / 2.0,
                g.y0 + SPACING.group_pad / 2.0 + SPACING.group_title_h / 2.0,
            );
            p.text_at(
                t,
                center,
                theme.font_size,
                theme.text,
                TextStyle {
                    bold: true,
                    italic: false,
                },
            );
        }
    }

    // `rect` backgrounds behind the lifelines.
    for f in &l.frames {
        if let Event::Start(BlockKind::Rect, color) = &seq.events[f.event] {
            let fill = parse_color(color).unwrap_or(theme.cluster_fill);
            let b = BoxF {
                x: f.x0,
                y: f.y0,
                w: f.x1 - f.x0,
                h: f.y1 - f.y0,
            };
            p.path(rect(b), Some(fill), None);
        }
    }

    // Lifelines and activation bars.
    for (i, &x) in l.x.iter().enumerate() {
        let y0 = l.head_y[i] + header_h;
        p.path(
            line(Pt::new(x, y0), Pt::new(x, l.life_end[i])),
            None,
            Some(solid(1.0, lifeline)),
        );
    }
    let mut bars = l.bars.clone();
    bars.sort_by(|a, b| a.x.total_cmp(&b.x));
    for bar in &bars {
        let b = BoxF {
            x: bar.x - SPACING.bar_w / 2.0,
            y: bar.y0,
            w: SPACING.bar_w,
            h: (bar.y1 - bar.y0).max(4.0),
        };
        p.path(
            rect(b),
            Some(theme.edge_label_fill),
            Some(solid(1.0, theme.edge)),
        );
    }

    // Frames.
    for f in &l.frames {
        let Event::Start(kind, label) = &seq.events[f.event] else {
            continue;
        };
        if *kind == BlockKind::Rect {
            continue;
        }
        let b = BoxF {
            x: f.x0,
            y: f.y0,
            w: f.x1 - f.x0,
            h: f.y1 - f.y0,
        };
        p.path(rect(b), None, Some(solid(1.0, theme.edge)));
        let tw = tab_w(&p, *kind);
        let th = lh + 4.0;
        let tab = shapes::polygon(&[
            (b.x, b.y),
            (b.x + tw, b.y),
            (b.x + tw, b.y + th - 6.0),
            (b.x + tw - 6.0, b.y + th),
            (b.x, b.y + th),
        ]);
        p.path(
            tab,
            Some(theme.edge_label_fill),
            Some(solid(1.0, theme.edge)),
        );
        let bold = TextStyle {
            bold: true,
            italic: false,
        };
        p.text_at(
            tab_text(*kind),
            Pt::new(b.x + tw / 2.0, b.y + th / 2.0),
            theme.font_size,
            theme.text,
            bold,
        );
        let text = bracketed(label);
        let lw = p.width(&text);
        p.text_at(
            &text,
            Pt::new(b.x + tw + TAB_PAD + lw / 2.0, b.y + th / 2.0),
            theme.font_size,
            theme.text,
            TextStyle::default(),
        );
        for &(k, y) in &f.dividers {
            p.path(
                line(Pt::new(b.x, y), Pt::new(b.right(), y)),
                None,
                Some(dashed(1.0, theme.edge)),
            );
            if let Event::Divider(label) = &seq.events[k] {
                let center = Pt::new(b.x + b.w / 2.0, y + lh / 2.0 + 2.0);
                p.text_at(
                    &bracketed(label),
                    center,
                    theme.font_size,
                    theme.text,
                    TextStyle::default(),
                );
            }
        }
    }

    // Messages and notes.
    for (k, place) in l.events.iter().enumerate() {
        match (*place, &seq.events[k]) {
            (Placed::Message { x0, x1, y, label_y }, Event::Message(m)) => {
                let stroke = match m.arrow.line {
                    LineStyle::Solid => solid(1.5, theme.edge),
                    LineStyle::Dotted => dashed(1.5, theme.edge),
                };
                if m.from == m.to {
                    let right = x0 + SPACING.self_w;
                    let bottom = y + SPACING.self_h;
                    let end = p.head(m.arrow.head, Pt::new(x0, bottom), -1.0, theme.edge);
                    let path = vec![
                        Segment::Move(Pt::new(x0, y)),
                        Segment::Line(Pt::new(right, y)),
                        Segment::Line(Pt::new(right, bottom)),
                        Segment::Line(end),
                    ];
                    p.path(path, None, Some(stroke));
                    let (tw, _) = p.measure(&m.text);
                    p.lines(&m.text, x0 + SPACING.label_pad + tw / 2.0, label_y);
                } else {
                    let dir = if x1 > x0 { 1.0 } else { -1.0 };
                    let end = p.head(m.arrow.head, Pt::new(x1, y), dir, theme.edge);
                    let start = if m.arrow.both {
                        p.head(m.arrow.head, Pt::new(x0, y), -dir, theme.edge)
                    } else {
                        Pt::new(x0, y)
                    };
                    p.path(line(start, end), None, Some(stroke));
                    p.lines(&m.text, (x0 + x1) / 2.0, label_y);
                }
                if let Some(n) = m.number {
                    p.badge(n, Pt::new(x0, y));
                }
            }
            (Placed::Note { x, y, w, h }, Event::Note(n)) => {
                let b = BoxF { x, y, w, h };
                p.path(
                    rect(b),
                    Some(theme.cluster_fill),
                    Some(solid(1.0, theme.cluster_stroke)),
                );
                p.lines(&n.text, x + w / 2.0, y + NOTE_PAD);
            }
            _ => {}
        }
    }

    // Headers, and crosses where lifelines end early.
    for (i, part) in seq.participants.iter().enumerate() {
        let (w, h) = headers[i];
        let x = l.x[i] - w / 2.0;
        let top = BoxF {
            x,
            y: l.head_y[i] + header_h - h,
            w,
            h,
        };
        p.header(part.kind, &part.label, top);
        if l.destroyed[i] {
            let c = Pt::new(l.x[i], l.life_end[i]);
            let d = 8.0;
            let s = Some(solid(2.0, theme.edge));
            p.path(
                line(Pt::new(c.x - d, c.y - d), Pt::new(c.x + d, c.y + d)),
                None,
                s,
            );
            p.path(
                line(Pt::new(c.x - d, c.y + d), Pt::new(c.x + d, c.y - d)),
                None,
                s,
            );
        } else {
            let bottom = BoxF {
                y: l.footer_y,
                ..top
            };
            p.header(part.kind, &part.label, bottom);
        }
    }

    let width = l
        .width
        .max(title.map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2)) + 2.0 * SPACING.margin);
    let mut elements = p.elements;
    if let Some(t) = title {
        elements.insert(
            0,
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: t.to_owned(),
                    center: Pt::new(width / 2.0, SPACING.margin + lh / 2.0),
                    size: theme.font_size * 1.2,
                    color: theme.text,
                    style: TextStyle::default(),
                },
            },
        );
    }
    Scene {
        width,
        height: l.height,
        background: theme.background,
        elements,
    }
}
