//! Packet diagrams as text (MPKT-4): the RFC style, with a bit ruler on top and each row's
//! first bit at its left.

use unicode_width::UnicodeWidthChar;

use super::canvas::{Canvas, Line};
use super::{Options, width};
use crate::packet::{Block, Packet};

/// `s` in lines at most `max` wide: broken at spaces, and a longer word broken anywhere.
fn wrap(s: &str, max: i32) -> Vec<String> {
    let max = max.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut line_width = 0;
    for word in s.split_whitespace() {
        let word_width = width(word);
        if !line.is_empty() && line_width + 1 + word_width <= max {
            line.push(' ');
            line.push_str(word);
            line_width += 1 + word_width;
            continue;
        }
        if !line.is_empty() {
            out.push(std::mem::take(&mut line));
            line_width = 0;
        }
        for ch in word.chars() {
            let char_width = ch.width().unwrap_or(0) as i32;
            if !line.is_empty() && line_width + char_width > max {
                out.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push(ch);
            line_width += char_width;
        }
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

/// A block's leftmost and rightmost columns within its row, as bit positions.
fn span(packet: &Packet, b: &Block) -> (i32, i32) {
    let (a, z) = (packet.column(b.start), packet.column(b.end));
    (a.min(z) as i32, a.max(z) as i32)
}

/// Renders `packet` as text (MPKT-4).
pub fn render(packet: &Packet, options: &Options) -> String {
    let per = packet.bits_per_row.max(1) as i32;
    let rows = packet.rows();
    // A one-bit box normally has one interior column. Widen the grid if it needs two.
    let bit_width = if rows
        .iter()
        .flatten()
        .any(|b| b.bits() == 1 && b.label.chars().any(|ch| ch.width() == Some(2)))
    {
        3
    } else {
        2
    };
    let grid_width = bit_width * per + 1;
    // Each block's label lines, and each row's height inside its borders.
    let labels: Vec<Vec<Vec<String>>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|b| wrap(&b.label, bit_width * b.bits() as i32 - 1))
                .collect()
        })
        .collect();
    let heights: Vec<i32> = labels
        .iter()
        .map(|ls| ls.iter().map(|l| l.len() as i32).max().unwrap_or(1).max(1))
        .collect();
    let gutter = rows
        .len()
        .checked_sub(1)
        .map_or(0, |last| (last as i32 * per).to_string().len() as i32 + 1);
    let ruler = if packet.show_bits { 2 } else { 0 };
    let total_h = ruler + 1 + heights.iter().map(|h| h + 1).sum::<i32>();
    let mut canvas = Canvas::new(
        gutter + grid_width,
        if rows.is_empty() { 0 } else { total_h },
    );
    if packet.show_bits && !rows.is_empty() {
        for c in 0..per {
            let bit = if packet.descending { per - 1 - c } else { c };
            let x = gutter + bit_width * c + bit_width / 2;
            if bit % 10 == 0 {
                canvas.text(x, 0, &(bit / 10 % 10).to_string());
            }
            canvas.text(x, 1, &(bit % 10).to_string());
        }
    }
    let mut y = ruler;
    for (r, (row, ls)) in rows.iter().zip(&labels).enumerate() {
        let h = heights[r];
        let bottom = y + h + 1;
        if let Some(first) = row.first() {
            canvas.text(0, y + 1 + (h - 1) / 2, &first.start.to_string());
        }
        for (b, lines) in row.iter().zip(ls) {
            let (lo, hi) = span(packet, b);
            let (x0, x1) = (gutter + bit_width * lo, gutter + bit_width * (hi + 1));
            canvas.hline(x0, x1, y, Line::Light);
            canvas.hline(x0, x1, bottom, Line::Light);
            canvas.vline(x0, y, bottom, Line::Light);
            canvas.vline(x1, y, bottom, Line::Light);
            let inner = x1 - x0 - 1;
            let top = y + 1 + (h - lines.len() as i32) / 2;
            for (k, l) in lines.iter().enumerate() {
                canvas.text(x0 + 1 + (inner - width(l)) / 2, top + k as i32, l);
            }
        }
        y = bottom;
    }
    let body = canvas.render(options.charset);
    match packet
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            let pad = (gutter + (grid_width - width(t)).max(0) / 2) as usize;
            format!("{}{t}\n\n{body}", " ".repeat(pad))
        }
        None => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascii::Charset;
    use crate::packet::Field;

    #[test]
    fn wrapping() {
        assert_eq!(wrap("Source Port", 31), ["Source Port"]);
        assert_eq!(wrap("Data Offset", 7), ["Data", "Offset"]);
        assert_eq!(wrap("URG", 1), ["U", "R", "G"]);
        assert_eq!(wrap("", 3), [""]);
    }

    #[test]
    fn wrapping_long_unicode_words_preserves_characters_and_word_boundaries() {
        let word = "é界".repeat(20_000);
        let lines = wrap(&format!("{word} end"), 3);
        assert!(lines.iter().all(|line| width(line) <= 3));
        assert_eq!(lines[..lines.len() - 1].concat(), word);
        assert_eq!(lines.last().unwrap(), "end");
    }

    #[test]
    fn wide_label_fits_inside_a_single_bit() {
        let src = "---\nconfig:\n  packet:\n    bitsPerRow: 1\n---\npacket\n0: \"界A\"\n";
        let (packet, diagnostics) = crate::packet::parse(src).lower();
        assert!(diagnostics.is_empty());
        assert_eq!(
            render(&packet, &Options::ASCII),
            "   0\n   0\n  +--+\n0 |界|\n  |A |\n  +--+\n"
        );
    }

    #[test]
    fn rows_and_ruler() {
        let p = Packet {
            bits_per_row: 4,
            fields: vec![
                Field {
                    start: 0,
                    end: 2,
                    label: "abc".into(),
                },
                Field {
                    start: 3,
                    end: 5,
                    label: "d".into(),
                },
            ],
            ..Packet::default()
        };
        let out = render(
            &p,
            &Options {
                charset: Charset::Ascii,
            },
        );
        assert_eq!(
            out,
            "   0\n   0 1 2 3\n  +-----+-+\n0 | abc |d|\n  +---+-+-+\n4 | d |\n  +---+\n"
        );
    }
}
