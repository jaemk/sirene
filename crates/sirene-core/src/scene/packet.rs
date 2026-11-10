//! Packet diagram to scene (MPKT-3): a row of bit cells per `bitsPerRow` bits, each field a box
//! across its bits.

use super::Theme;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::Label;
use crate::packet::Packet;

const MARGIN: f32 = 16.0;
/// Width of one bit, and height of a row, as in Mermaid.
const BIT_W: f32 = 32.0;
const ROW_H: f32 = 32.0;
/// Space between rows, and the room above each for bit numbers.
const ROW_GAP: f32 = 5.0;
const NUMBERS_H: f32 = 14.0;

fn rect(b: BoxF) -> Vec<Segment> {
    let (r, bottom) = (b.right(), b.bottom());
    shapes::polygon(&[(b.x, b.y), (r, b.y), (r, bottom), (b.x, bottom)])
}

/// Draws `packet` (MPKT-3).
pub fn packet(packet: &Packet, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = (fs - 4.0).max(8.0);
    let title = packet
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let rows = packet.rows();
    let row_w = packet.bits_per_row.max(1) as f32 * BIT_W;
    let numbers_h = if packet.show_bits { NUMBERS_H } else { 0.0 };
    let step = numbers_h + ROW_H + ROW_GAP;
    let top = MARGIN + if title.is_some() { lh + 16.0 } else { 0.0 };
    let width = (row_w + 2.0 * MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN);
    let left = (width - row_w) / 2.0;
    let height = top + rows.len() as f32 * step + MARGIN;

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, size: f32, color: Color| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        let at = Pt::new(width / 2.0, MARGIN + lh / 2.0);
        text(&mut p, t, at, title_size, theme.text);
    }
    let stroke = Some(Stroke {
        width: 1.0,
        color: theme.node_stroke,
        dash: None,
    });
    for (r, row) in rows.iter().enumerate() {
        let y = top + r as f32 * step + numbers_h;
        for b in row {
            let (a, z) = (packet.column(b.start), packet.column(b.end));
            let (lo, hi) = (a.min(z), a.max(z));
            let bx = BoxF {
                x: left + lo as f32 * BIT_W,
                y,
                w: (hi - lo + 1) as f32 * BIT_W,
                h: ROW_H,
            };
            p.path(owner, rect(bx), Some(theme.node_fill), stroke);
            let lines = px.wrap(&Label::plain(b.label.as_str()).spans(), bx.w - 4.0);
            p.lines(owner, &lines, bx.center(), theme.text);
            if packet.show_bits {
                // The first bit's number over its cell, and the last's over its own.
                let ny = y - NUMBERS_H / 2.0;
                let over = |bit: u32| left + (packet.column(bit) as f32 + 0.5) * BIT_W;
                text(
                    &mut p,
                    &b.start.to_string(),
                    Pt::new(over(b.start), ny),
                    small,
                    theme.text,
                );
                if b.end != b.start {
                    text(
                        &mut p,
                        &b.end.to_string(),
                        Pt::new(over(b.end), ny),
                        small,
                        theme.text,
                    );
                }
            }
        }
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}
