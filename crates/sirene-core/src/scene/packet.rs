//! Packet diagram to scene (MPKT-3): a row of bit cells per `bitsPerRow` bits, each field a box
//! across its bits.

use super::Theme;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Label, Span};
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

/// Wrap at spaces first, then split words that are still too wide. Measuring each character
/// separately avoids repeatedly measuring growing prefixes of a long word.
fn label_lines(px: &Px<'_, impl TextMetrics>, label: &str, max: f32) -> Vec<Vec<Span>> {
    let lines = px.wrap(&Label::plain(label).spans(), max);
    let mut result = Vec::new();
    let push = |result: &mut Vec<Vec<Span>>, part: String| {
        // Shaping can make a run wider than the sum of individual glyphs. In that case
        // use one glyph per line: each was measured when sizing the cells. Each chunk is
        // measured only once, even for very long words.
        if px
            .metrics
            .styled_width(&part, px.theme.font_size, TextStyle::default())
            > max
        {
            result.extend(part.chars().map(|ch| vec![Span::plain(ch.to_string())]));
        } else {
            result.push(vec![Span::plain(part)]);
        }
    };
    for line in lines {
        if px.text_size(std::slice::from_ref(&line)).0 <= max {
            result.push(line);
            continue;
        }
        let mut part = String::new();
        let mut width = 0.0;
        for ch in line.iter().flat_map(|span| span.text.chars()) {
            let mut buffer = [0; 4];
            let s = ch.encode_utf8(&mut buffer);
            let advance = px
                .metrics
                .styled_width(s, px.theme.font_size, TextStyle::default());
            if !part.is_empty() && width + advance > max {
                push(&mut result, std::mem::take(&mut part));
                width = 0.0;
            }
            part.push(ch);
            width += advance;
        }
        if !part.is_empty() {
            push(&mut result, part);
        }
    }
    result
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
    // If a glyph is wider than a bit, enlarge the cells so even a single-character line fits.
    let mut glyphs = std::collections::HashSet::new();
    let bit_w = packet
        .fields
        .iter()
        .flat_map(|f| f.label.chars())
        .filter(|&ch| glyphs.insert(ch))
        .map(|ch| metrics.styled_width(ch.encode_utf8(&mut [0; 4]), fs, TextStyle::default()) + 4.0)
        .fold(BIT_W, f32::max);
    let row_w = packet.bits_per_row.max(1) as f32 * bit_w;
    let numbers_h = if packet.show_bits { NUMBERS_H } else { 0.0 };
    let wrapped: Vec<Vec<Vec<Vec<Span>>>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|b| {
                    let bits = b.end - b.start + 1;
                    label_lines(&px, &b.label, bits as f32 * bit_w - 4.0)
                })
                .collect()
        })
        .collect();
    let heights: Vec<f32> = wrapped
        .iter()
        .map(|row| {
            row.iter()
                .map(|lines| lines.len() as f32 * lh + 4.0)
                .fold(ROW_H, f32::max)
        })
        .collect();
    let top = MARGIN + if title.is_some() { lh + 16.0 } else { 0.0 };
    let width = (row_w + 2.0 * MARGIN).max(
        title.map_or(0.0, |t| {
            metrics.styled_width(t, title_size, TextStyle::default())
        }) + 2.0 * MARGIN,
    );
    let left = (width - row_w) / 2.0;
    let height = top + heights.iter().map(|h| numbers_h + h + ROW_GAP).sum::<f32>() + MARGIN;

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
    let mut row_top = top;
    for ((row, lines), &row_height) in rows.iter().zip(&wrapped).zip(&heights) {
        let y = row_top + numbers_h;
        for (b, lines) in row.iter().zip(lines) {
            let (a, z) = (packet.column(b.start), packet.column(b.end));
            let (lo, hi) = (a.min(z), a.max(z));
            let bx = BoxF {
                x: left + lo as f32 * bit_w,
                y,
                w: (hi - lo + 1) as f32 * bit_w,
                h: row_height,
            };
            p.path(owner, rect(bx), Some(theme.node_fill), stroke);
            p.lines(owner, lines, bx.center(), theme.text);
            if packet.show_bits {
                // The first bit's number over its cell, and the last's over its own.
                let ny = y - NUMBERS_H / 2.0;
                let over = |bit: u32| left + (packet.column(bit) as f32 + 0.5) * bit_w;
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
        row_top += numbers_h + row_height + ROW_GAP;
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::Field;
    use crate::scene::Approx;

    #[test]
    fn long_labels_fit_and_rows_grow() {
        let packet = Packet {
            bits_per_row: 2,
            fields: vec![
                Field {
                    start: 0,
                    end: 0,
                    label: "averylongunbrokenword".into(),
                },
                Field {
                    start: 1,
                    end: 1,
                    label: "R".into(),
                },
                Field {
                    start: 2,
                    end: 2,
                    label: "N".into(),
                },
            ],
            ..Packet::default()
        };
        let scene = super::packet(&packet, &Approx, &Theme::LIGHT);
        let texts: Vec<_> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text {
                    text, center, size, ..
                } => Some((text.as_str(), *center, *size)),
                _ => None,
            })
            .collect();
        let right = texts.iter().find(|(t, _, _)| *t == "R").unwrap().1;
        let next = texts.iter().find(|(t, _, _)| *t == "N").unwrap().1;
        assert!(next.y > right.y + ROW_H);
        let words: Vec<_> = texts
            .iter()
            .filter(|(t, _, _)| "averylongunbrokenword".contains(t) && t.len() > 1)
            .collect();
        assert!(words.len() > 1);
        for (text, center, size) in words {
            let width = Approx.width(text, *size);
            assert!(center.x - width / 2.0 >= MARGIN);
            assert!(center.x + width / 2.0 <= MARGIN + BIT_W);
            assert!(center.y >= MARGIN && center.y <= scene.height - MARGIN);
        }
    }

    #[test]
    fn wide_glyph_enlarges_bit_cell() {
        struct Wide;
        impl TextMetrics for Wide {
            fn width(&self, text: &str, _: f32) -> f32 {
                text.chars().count() as f32 * 50.0
            }
        }
        let packet = Packet {
            bits_per_row: 1,
            fields: vec![Field {
                start: 0,
                end: 0,
                label: "X".into(),
            }],
            ..Packet::default()
        };
        let scene = super::packet(&packet, &Wide, &Theme::LIGHT);
        assert!(scene.width >= 50.0 + 4.0 + 2.0 * MARGIN);
    }
    #[test]
    fn nonadditive_text_metrics_do_not_overflow_a_cell() {
        struct Shaped;
        impl TextMetrics for Shaped {
            fn width(&self, text: &str, _: f32) -> f32 {
                match text.chars().count() {
                    0 => 0.0,
                    1 => 10.0,
                    n => 40.0 * (n - 1) as f32,
                }
            }
        }
        let px = Px {
            metrics: &Shaped,
            theme: &Theme::LIGHT,
        };
        let lines = label_lines(&px, "abcdefgh", 28.0);
        assert!(lines.len() > 1);
        for line in lines {
            assert!(px.text_size(&[line]).0 <= 28.0);
        }
    }
    #[test]
    fn regular_styled_width_is_used_to_size_cells() {
        struct Styled;
        impl TextMetrics for Styled {
            fn width(&self, text: &str, _: f32) -> f32 {
                text.chars().count() as f32
            }
            fn styled_width(&self, text: &str, _: f32, _: TextStyle) -> f32 {
                text.chars().count() as f32 * 50.0
            }
        }
        let packet = Packet {
            bits_per_row: 1,
            fields: vec![Field {
                start: 0,
                end: 0,
                label: "X".into(),
            }],
            ..Packet::default()
        };
        let scene = super::packet(&packet, &Styled, &Theme::LIGHT);
        assert!(scene.width >= 50.0 + 4.0 + 2.0 * MARGIN);
        let px = Px {
            metrics: &Styled,
            theme: &Theme::LIGHT,
        };
        assert_eq!(label_lines(&px, "abc", 54.0).len(), 3);
    }
}
