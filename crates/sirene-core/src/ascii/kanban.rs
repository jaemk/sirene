//! Kanban boards as text (MKAN-4): a box per column with its title in the top border, holding
//! a box per card.

use super::canvas::{Canvas, Line};
use super::mathtext::{self, Rows};
use super::{Charset, Options, ROUND, draw_box, width};
use crate::flowchart::Label;
use crate::kanban::{Card, Kanban, Priority};
use crate::layout::Rect;

/// The narrowest and widest a card's text runs before wrapping.
const MIN_TEXT: i32 = 12;
const MAX_TEXT: i32 = 28;

/// `s` broken at spaces into lines at most `max` wide; a longer word is broken into pieces
/// that fit.
fn wrap(s: &str, max: i32) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && width(&line) + 1 + width(word) > max {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            for c in word.chars() {
                let mut buf = [0; 4];
                let c = c.encode_utf8(&mut buf);
                if !line.is_empty() && width(&line) + width(c) > max {
                    out.push(std::mem::take(&mut line));
                }
                line.push_str(c);
            }
        }
        out.push(line);
    }
    out
}

/// A priority's marker before a card's details.
fn marker(p: Priority, charset: Charset) -> &'static str {
    match (p, charset == Charset::Ascii) {
        (Priority::VeryHigh, false) => "▲▲",
        (Priority::High, false) => "▲",
        (Priority::Low, false) => "▼",
        (Priority::VeryLow, false) => "▼▼",
        (Priority::VeryHigh, true) => "^^",
        (Priority::High, true) => "^",
        (Priority::Low, true) => "v",
        (Priority::VeryLow, true) => "vv",
    }
}

/// A card's details line: its priority, ticket, and assignee.
fn details(card: &Card, charset: Charset) -> String {
    let mut parts = Vec::new();
    if let Some(p) = card.priority {
        parts.push(format!("{} {}", marker(p, charset), p.name()));
    }
    if let Some(t) = &card.ticket {
        parts.push(t.clone());
    }
    if let Some(a) = &card.assigned {
        parts.push(format!("@{a}"));
    }
    parts.join("  ")
}

/// One source line of a label as rows: a formula's rows (LEMB-4), which are not wrapped, or its
/// text.
struct Source {
    rows: Vec<String>,
    math: bool,
}

/// The lines of `label`, each as the rows it is drawn in, formulas set in `formulas` (once each).
fn sources(label: &Label, formulas: &Rows) -> Vec<Source> {
    label
        .text
        .split('\n')
        .map(|line| {
            let one = Label {
                text: line.to_owned(),
                ..label.clone()
            };
            match mathtext::rows(formulas, &one) {
                Some(rows) => Source { rows, math: true },
                None => Source {
                    rows: one.lines(),
                    math: false,
                },
            }
        })
        .collect()
}

/// `sources` drawn at most `max` wide: text wrapped, a formula's rows as they are.
fn fitted(sources: &[Source], max: i32) -> Vec<String> {
    sources
        .iter()
        .flat_map(|s| {
            if s.math {
                s.rows.clone()
            } else {
                s.rows.iter().flat_map(|l| wrap(l, max)).collect()
            }
        })
        .collect()
}

/// The widest text and the widest formula row of `sources`.
fn widths(sources: &[Source]) -> (i32, i32) {
    let (mut text, mut math) = (0, 0);
    for s in sources {
        let w = s.rows.iter().map(|r| width(r)).max().unwrap_or(0);
        if s.math {
            math = math.max(w);
        } else {
            text = text.max(w);
        }
    }
    (text, math)
}

/// A card's label and details as rows.
type CardSource = (Vec<Source>, Vec<Source>);

/// Renders `board` as text (MKAN-4).
pub fn render(board: &Kanban, options: &Options) -> String {
    let charset = options.charset;
    let formulas = Rows::new(charset);
    // Each card's label and details as rows, and each column's title: a title with a formula is
    // rows inside the column, under its top border, which holds only a title of text.
    let card_src: Vec<Vec<CardSource>> = board
        .columns
        .iter()
        .map(|c| {
            c.cards
                .iter()
                .map(|card| {
                    let d = details(card, charset);
                    let details = if d.is_empty() {
                        Vec::new()
                    } else {
                        sources(&Label::plain(d), &formulas)
                    };
                    (sources(&card.label, &formulas), details)
                })
                .collect()
        })
        .collect();
    let heads: Vec<Option<Vec<String>>> = board
        .columns
        .iter()
        .map(|c| {
            c.label
                .has_formula()
                .then(|| fitted(&sources(&c.label, &formulas), i32::MAX))
        })
        .collect();
    // The text width that fits each column's title and each card's lines, and the widest formula.
    let mut natural = 0;
    let mut formula_w = 0;
    for (c, head) in board.columns.iter().zip(&heads) {
        match head {
            // A title row starts two cells in, and the column is four wider than the text.
            Some(rows) => {
                let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
                formula_w = formula_w.max(w - 4);
            }
            None => natural = natural.max(width(&c.label.lines().join(" ")) - 2),
        }
    }
    for (label, details) in card_src.iter().flatten() {
        for (text, math) in [widths(label), widths(details)] {
            natural = natural.max(text);
            formula_w = formula_w.max(math);
        }
    }
    let text_w = natural.clamp(MIN_TEXT, MAX_TEXT).max(formula_w);
    // A card is `│ text │`; a column holds cards with a space either side.
    let card_w = text_w + 4;
    let col_w = card_w + 4;
    let cards: Vec<Vec<Vec<String>>> = card_src
        .iter()
        .map(|cs| {
            cs.iter()
                .map(|(label, details)| {
                    let mut lines = fitted(label, text_w);
                    lines.extend(fitted(details, text_w));
                    lines
                })
                .collect()
        })
        .collect();
    let body = cards
        .iter()
        .zip(&heads)
        .map(|(cs, head)| {
            head.as_ref().map_or(0, |h| h.len() as i32)
                + cs.iter().map(|l| l.len() as i32 + 2).sum::<i32>()
        })
        .max()
        .unwrap_or(0)
        .max(1);
    let col_h = body + 2;
    let n = board.columns.len() as i32;
    let total_w = (n * col_w + (n - 1).max(0)).max(0);
    let mut canvas = Canvas::new(total_w, if n > 0 { col_h } else { 0 });
    for (i, ((column, cs), head)) in board.columns.iter().zip(&cards).zip(&heads).enumerate() {
        let x = i as i32 * (col_w + 1);
        draw_box(&mut canvas, Rect::new(x, 0, col_w, col_h), Line::Light);
        let mut y = 1;
        match head {
            Some(rows) => {
                for r in rows {
                    canvas.text(x + 2, y, r);
                    y += 1;
                }
            }
            None => {
                let mut title = column.label.lines().join(" ");
                while width(&title) > col_w - 4 {
                    title.pop();
                }
                canvas.text(x + 2, 0, &format!(" {title} "));
            }
        }
        for lines in cs {
            let r = Rect::new(x + 2, y, card_w, lines.len() as i32 + 2);
            draw_box(&mut canvas, r, Line::Light);
            let [tl, tr, bl, br] = ROUND;
            canvas.glyph(r.x, r.y, tl);
            canvas.glyph(r.right(), r.y, tr);
            canvas.glyph(r.x, r.bottom(), bl);
            canvas.glyph(r.right(), r.bottom(), br);
            for (k, l) in lines.iter().enumerate() {
                canvas.text(r.x + 2, r.y + 1 + k as i32, l);
            }
            y += r.h;
        }
    }
    let body = canvas.render(charset);
    match board
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => {
            let pad = ((total_w - width(t)).max(0) / 2) as usize;
            format!("{}{t}\n\n{body}", " ".repeat(pad))
        }
        None => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping() {
        assert_eq!(wrap("a bb ccc dddd", 6), ["a bb", "ccc", "dddd"]);
        assert_eq!(wrap("toolongword x", 4), ["tool", "ongw", "ord", "x"]);
        assert_eq!(wrap("ab cdefgh", 4), ["ab", "cdef", "gh"]);
        assert_eq!(wrap("", 4), Vec::<String>::new());
    }

    #[test]
    fn card_details() {
        let card = Card {
            ticket: Some("T-1".into()),
            assigned: Some("ann".into()),
            priority: Some(Priority::VeryHigh),
            ..Card::default()
        };
        assert_eq!(details(&card, Charset::Ascii), "^^ Very High  T-1  @ann");
        assert_eq!(details(&Card::default(), Charset::Unicode), "");
        assert_eq!(marker(Priority::VeryLow, Charset::Unicode), "▼▼");
    }
}
