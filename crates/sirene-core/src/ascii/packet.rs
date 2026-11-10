//! Packet diagrams as text (MPKT-4): the RFC style, two columns per bit, with a bit ruler on top
//! and each row's first bit at its left.

use super::canvas::{Canvas, Line};
use super::{Options, width};
use crate::packet::{Block, Packet};

/// `s` in lines at most `max` wide: broken at spaces, and a longer word broken anywhere.
fn wrap(s: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        if !line.is_empty() && line.chars().count() + 1 + word.len() <= max {
            line.push(' ');
            line.extend(word.drain(..));
            continue;
        }
        if !line.is_empty() {
            out.push(std::mem::take(&mut line));
        }
        while word.len() > max {
            out.push(word.drain(..max).collect());
        }
        line.extend(word);
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
    // Each block's label lines, and each row's height inside its borders.
    let labels: Vec<Vec<Vec<String>>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|b| wrap(&b.label, (2 * b.bits() - 1) as usize))
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
        gutter + 2 * per + 1,
        if rows.is_empty() { 0 } else { total_h },
    );
    if packet.show_bits && !rows.is_empty() {
        for c in 0..per {
            let bit = if packet.descending { per - 1 - c } else { c };
            let x = gutter + 2 * c + 1;
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
            let (x0, x1) = (gutter + 2 * lo, gutter + 2 * (hi + 1));
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
            let pad = (gutter + (2 * per + 1 - width(t)).max(0) / 2) as usize;
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
