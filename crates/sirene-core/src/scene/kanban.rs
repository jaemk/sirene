//! Kanban board to scene (MKAN-3): columns side by side, each with its cards stacked under its
//! title.

use super::Theme;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Delim, Span, formula};
use crate::kanban::{Kanban, Priority};

const MARGIN: f32 = 16.0;
/// Width of a column, the space between columns, and the padding inside one.
const COL_W: f32 = 220.0;
const COL_GAP: f32 = 12.0;
const COL_PAD: f32 = 10.0;
/// Padding inside a card, the space between cards, and the width of its priority stripe.
const CARD_PAD: f32 = 10.0;
const CARD_GAP: f32 = 8.0;
const STRIPE: f32 = 5.0;

/// A priority's stripe color.
pub(super) fn priority_color(p: Priority) -> Color {
    Color::rgb(match p {
        Priority::VeryHigh => 0xe03131,
        Priority::High => 0xf08c00,
        Priority::Low => 0x1c7ed6,
        Priority::VeryLow => 0x74c0fc,
    })
}

/// `s` cut short with `...` to fit `max` at `size` (just `...` when nothing more fits).
fn fit(metrics: &impl TextMetrics, s: &str, size: f32, max: f32) -> String {
    if metrics.width(s, size) <= max {
        return s.to_owned();
    }
    let ends: Vec<usize> = s.char_indices().map(|(i, _)| i).skip(1).collect();
    // The longest prefix that fits with `...`, by binary search over its end.
    let (mut lo, mut hi) = (0, ends.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if metrics.width(&format!("{}...", &s[..ends[mid - 1]]), size) <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    match lo {
        0 => "...".to_owned(),
        n => format!("{}...", &s[..ends[n - 1]]),
    }
}

/// A card's ticket or assignee: text cut to fit in small type, or, with a formula (LEMB-1), one
/// line of runs at the normal size, which is not cut.
enum Meta {
    Text(String),
    Math(Vec<Span>),
}

/// A card as laid out: its label's lines, its ticket and assignee (cut to share the card's
/// width), the height of their line, and its height.
struct CardBox {
    lines: Vec<Vec<Span>>,
    ticket: Option<Meta>,
    assigned: Option<Meta>,
    meta_h: f32,
    priority: Option<Priority>,
    h: f32,
}

/// Draws `board` (MKAN-3).
pub fn kanban(board: &Kanban, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px::new(metrics, theme);
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = fs - 2.0;
    let small_lh = lh - 4.0;
    let text_w = COL_W - 2.0 * COL_PAD - 2.0 * CARD_PAD;
    let title = board
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let top = MARGIN + if title.is_some() { lh + 16.0 } else { 0.0 };

    let headers: Vec<Vec<Vec<Span>>> = board
        .columns
        .iter()
        .map(|c| {
            let bold: Vec<Vec<Span>> = c
                .label
                .runs()
                .into_iter()
                .map(|l| l.into_iter().map(|s| Span { bold: !s.math, ..s }).collect())
                .collect();
            px.wrap(&bold, COL_W - 2.0 * COL_PAD)
        })
        .collect();
    let header_h = headers.iter().map(|l| px.text_size(l).1).fold(lh, f32::max) + 2.0 * COL_PAD;
    let cards: Vec<Vec<CardBox>> = board
        .columns
        .iter()
        .map(|c| {
            c.cards
                .iter()
                .map(|card| {
                    let lines = px.wrap(&card.label.runs(), text_w);
                    let meta = card.ticket.is_some() || card.assigned.is_some();
                    // The ticket takes up to half the width when there is an assignee, which
                    // takes the rest after a gap.
                    let half = if card.assigned.is_some() {
                        (text_w - 8.0) / 2.0
                    } else {
                        text_w
                    };
                    let one = |m: &Meta| match m {
                        Meta::Text(t) => metrics.width(t, small),
                        Meta::Math(l) => px.text_size(std::slice::from_ref(l)).0,
                    };
                    let meta_of = |t: &str, max: f32| {
                        if formula::has(t, Delim::Dollar) {
                            Meta::Math(formula::runs(t, Delim::Dollar))
                        } else {
                            Meta::Text(fit(metrics, t, small, max))
                        }
                    };
                    let ticket = card.ticket.as_deref().map(|t| meta_of(t, half));
                    let used = ticket.as_ref().map_or(0.0, |m| one(m) + 8.0);
                    let assigned = card.assigned.as_deref().map(|a| meta_of(a, text_w - used));
                    // A formula makes its line taller than the small type's.
                    let meta_h = [&ticket, &assigned]
                        .into_iter()
                        .flatten()
                        .filter_map(|m| match m {
                            Meta::Math(l) => Some(px.text_size(std::slice::from_ref(l)).1),
                            Meta::Text(_) => None,
                        })
                        .fold(small_lh, f32::max);
                    let h = 2.0 * CARD_PAD
                        + px.text_size(&lines).1
                        + if meta { 4.0 + meta_h } else { 0.0 };
                    CardBox {
                        lines,
                        ticket,
                        assigned,
                        meta_h,
                        priority: card.priority,
                        h,
                    }
                })
                .collect()
        })
        .collect();
    let body_h = cards
        .iter()
        .map(|cs| cs.iter().map(|c| c.h + CARD_GAP).sum::<f32>())
        .fold(0.0f32, f32::max);
    let col_h = header_h + body_h + COL_PAD;
    let n = board.columns.len() as f32;
    let content_w = (n * COL_W + (n - 1.0).max(0.0) * COL_GAP).max(0.0);
    let width = (content_w + 2.0 * MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN);
    let left = (width - content_w) / 2.0;
    let height = if board.columns.is_empty() {
        top + MARGIN
    } else {
        top + col_h + MARGIN
    };

    let mut p = Painter::new(theme, metrics);
    // The formulas typeset to size the board are the ones drawn.
    p.formulas = px.formulas.clone();
    let text = |p: &mut Painter, owner: Owner, s: &str, center: Pt, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            Owner::Diagram,
            t,
            Pt::new(width / 2.0, MARGIN + lh / 2.0),
            title_size,
        );
    }
    let card_stroke = Some(Stroke {
        width: 1.0,
        color: theme.node_stroke,
        dash: None,
    });
    // Every column and card, in order, is owned by `Owner::Node` of its place (WYSIWY-28).
    let mut n = 0;
    for (i, (lines, column)) in headers.iter().zip(&cards).enumerate() {
        let owner = Owner::Node(n);
        n += 1;
        let x = left + i as f32 * (COL_W + COL_GAP);
        let col = BoxF {
            x,
            y: top,
            w: COL_W,
            h: col_h,
        };
        p.path(
            owner,
            shapes::rounded(col, 6.0),
            Some(theme.edge_label_fill),
            None,
        );
        p.lines(
            owner,
            lines,
            Pt::new(x + COL_W / 2.0, top + header_h / 2.0),
            theme.text,
        );
        let mut y = top + header_h;
        for c in column {
            let owner = Owner::Node(n);
            n += 1;
            let b = BoxF {
                x: x + COL_PAD,
                y,
                w: COL_W - 2.0 * COL_PAD,
                h: c.h,
            };
            p.path(
                owner,
                shapes::rounded(b, 4.0),
                Some(theme.background),
                card_stroke,
            );
            if let Some(pr) = c.priority {
                let stripe = BoxF {
                    x: b.x + 1.0,
                    y: b.y + 1.0,
                    w: STRIPE,
                    h: b.h - 2.0,
                };
                p.path(
                    owner,
                    shapes::rounded(stripe, 2.0),
                    Some(priority_color(pr)),
                    None,
                );
            }
            // Each line on its own, so the text is aligned left.
            let th = px.text_size(&c.lines).1;
            let mut line_top = b.y + CARD_PAD;
            for line in &c.lines {
                let one = std::slice::from_ref(line);
                let (w, h) = px.text_size(one);
                let y = line_top + h / 2.0;
                p.lines(owner, one, Pt::new(b.x + CARD_PAD + w / 2.0, y), theme.text);
                line_top += h;
            }
            let meta_y = b.y + CARD_PAD + th + 4.0 + c.meta_h / 2.0;
            for (m, right) in [(&c.ticket, false), (&c.assigned, true)] {
                let Some(m) = m else { continue };
                let w = match m {
                    Meta::Text(t) => metrics.width(t, small),
                    Meta::Math(l) => px.text_size(std::slice::from_ref(l)).0,
                };
                let x = if right {
                    b.right() - CARD_PAD - w / 2.0
                } else {
                    b.x + CARD_PAD + w / 2.0
                };
                match m {
                    Meta::Text(t) => text(&mut p, owner, t, Pt::new(x, meta_y), small),
                    Meta::Math(l) => {
                        p.lines(
                            owner,
                            std::slice::from_ref(l),
                            Pt::new(x, meta_y),
                            theme.text,
                        );
                    }
                }
            }
            y += c.h + CARD_GAP;
        }
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}
