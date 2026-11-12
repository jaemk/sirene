//! Railroad diagrams as text (MRAIL-4): each rule's name, then its track from a start to an end
//! marker, with boxed elements, alternatives branching down, bypasses over the top, and loops
//! back underneath.

use super::canvas::{Canvas, Glyph, Line, glyph};
use super::{Options, ROUND, width};
use crate::railroad::{Expr, Railroad};

const MARKER: Glyph = glyph('●', '*');
const BACK: Glyph = glyph('◄', '<');

/// An expression's size in cells: its width, and its rows above and below its track.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dim {
    w: i32,
    up: i32,
    down: i32,
}

impl Dim {
    fn h(self) -> i32 {
        self.up + 1 + self.down
    }
}

/// The text in an element's box, on one line.
fn atom(e: &Expr) -> Option<(String, Line, bool)> {
    let one_line = |s: &str| s.replace(['\n', '\r', '\t'], " ");
    Some(match e {
        Expr::Terminal(t) => (one_line(t), Line::Light, true),
        Expr::NonTerminal(n) => (one_line(n), Line::Light, false),
        Expr::Special(s) => (format!("? {} ?", one_line(s)), Line::Dotted, false),
        _ => return None,
    })
}

fn measure(e: &Expr) -> Dim {
    if let Some((text, ..)) = atom(e) {
        return Dim {
            w: width(&text) + 4,
            up: 1,
            down: 1,
        };
    }
    match e {
        Expr::Sequence(items) => {
            let dims: Vec<Dim> = items.iter().map(measure).collect();
            Dim {
                w: dims.iter().map(|d| d.w).sum::<i32>() + 2 * (dims.len() as i32 - 1).max(0),
                up: dims.iter().map(|d| d.up).max().unwrap_or(0),
                down: dims.iter().map(|d| d.down).max().unwrap_or(0),
            }
        }
        Expr::Choice(alts) => {
            let dims: Vec<Dim> = alts.iter().map(measure).collect();
            let first = dims.first().copied().unwrap_or(Dim {
                w: 0,
                up: 0,
                down: 0,
            });
            let h: i32 = dims.iter().map(|d| d.h()).sum();
            Dim {
                w: dims.iter().map(|d| d.w).max().unwrap_or(0) + 4,
                up: first.up,
                down: h - first.up - 1,
            }
        }
        Expr::Optional(inner) => {
            let d = measure(inner);
            Dim {
                w: d.w + 4,
                up: d.up + 1,
                down: d.down,
            }
        }
        Expr::Repeat { expr, min, .. } => {
            let d = measure(expr);
            Dim {
                w: d.w + 4,
                up: d.up + i32::from(*min == 0),
                down: d.down + 1,
            }
        }
        _ => Dim {
            w: 0,
            up: 0,
            down: 0,
        },
    }
}

/// Draws `e` with its left edge at `x` and its track on row `y`.
fn draw(c: &mut Canvas, e: &Expr, x: i32, y: i32) {
    let d = measure(e);
    if let Some((text, line, rounded)) = atom(e) {
        let (l, r, t, b) = (x, x + d.w - 1, y - 1, y + 1);
        c.hline(l, r, t, line);
        c.hline(l, r, b, line);
        c.vline(l, t, b, line);
        c.vline(r, t, b, line);
        if rounded {
            c.glyph(l, t, ROUND[0]);
            c.glyph(r, t, ROUND[1]);
            c.glyph(l, b, ROUND[2]);
            c.glyph(r, b, ROUND[3]);
        }
        c.text(x + 2, y, &text);
        return;
    }
    let right = x + d.w - 1;
    match e {
        Expr::Sequence(items) => {
            let mut cx = x;
            for (i, item) in items.iter().enumerate() {
                let w = measure(item).w;
                draw(c, item, cx, y);
                if i + 1 < items.len() {
                    c.hline(cx + w - 1, cx + w + 2, y, Line::Light);
                }
                cx += w + 2;
            }
        }
        Expr::Choice(alts) => {
            let mut top = y - d.up;
            let mut last = y;
            for alt in alts {
                let ad = measure(alt);
                let ay = top + ad.up;
                draw(c, alt, x + 2, ay);
                c.hline(x, x + 2, ay, Line::Light);
                c.hline(x + 2 + ad.w - 1, right, ay, Line::Light);
                last = ay;
                top += ad.h();
            }
            c.vline(x, y, last, Line::Light);
            c.vline(right, y, last, Line::Light);
        }
        Expr::Optional(inner) => {
            through(c, inner, x, right, y);
            bypass(c, x, right, y - d.up, y);
        }
        Expr::Repeat { expr, min, .. } => {
            through(c, expr, x, right, y);
            let below = y + d.down;
            c.vline(x, y, below, Line::Light);
            c.vline(right, y, below, Line::Light);
            c.hline(x, right, below, Line::Light);
            c.arrow(x + d.w / 2, below, BACK);
            if *min == 0 {
                bypass(c, x, right, y - d.up, y);
            }
        }
        _ => {}
    }
}

/// `e` two cells in from `x`, on a track from `x` to `right`.
fn through(c: &mut Canvas, e: &Expr, x: i32, right: i32, y: i32) {
    let w = measure(e).w;
    draw(c, e, x + 2, y);
    c.hline(x, x + 2, y, Line::Light);
    c.hline(x + 2 + w - 1, right, y, Line::Light);
}

/// A path over the top on row `top`, from and back to the track on row `y`.
fn bypass(c: &mut Canvas, x: i32, right: i32, top: i32, y: i32) {
    c.hline(x, right, top, Line::Light);
    c.vline(x, top, y, Line::Light);
    c.vline(right, top, y, Line::Light);
}

/// `name = `, or nothing for an unnamed rule, such as a PlantUML regex.
fn rule_label(name: &str) -> String {
    if name.is_empty() {
        String::new()
    } else {
        format!("{name} = ")
    }
}

/// Renders `chart` as text (MRAIL-4).
pub fn render(chart: &Railroad, options: &Options) -> String {
    let rows: Vec<(String, Dim)> = chart
        .rules
        .iter()
        .map(|r| (rule_label(&r.name), measure(&r.definition)))
        .collect();
    let w = rows
        .iter()
        .map(|(label, d)| width(label) + d.w + 4)
        .max()
        .unwrap_or(0);
    let h = rows.iter().map(|(_, d)| d.h() + 1).sum::<i32>() - 1;
    let mut c = Canvas::new(w, h.max(0));
    let mut top = 0;
    for (rule, (label, d)) in chart.rules.iter().zip(&rows) {
        let y = top + d.up;
        let m = width(label);
        c.text(0, y, label);
        c.hline(m, m + 2, y, Line::Light);
        draw(&mut c, &rule.definition, m + 2, y);
        let end = m + 2 + d.w + 1;
        c.hline(end - 2, end, y, Line::Light);
        c.arrow(m, y, MARKER);
        c.arrow(end, y, MARKER);
        top += d.h() + 1;
    }
    c.render(options.charset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascii::Charset;

    fn text(src: &str) -> String {
        let (chart, d) = crate::railroad::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        render(&chart, &Options::default())
    }

    #[test]
    fn elements() {
        assert_eq!(
            text("railroad-ebnf-beta\na = \"x\" b ? c ? ;\n"),
            "      ╭───╮  ┌───┐  ┌┄┄┄┄┄┄┄┐\n\
             a = ●─┤ x ├──┤ b ├──┤ ? c ? ├─●\n\
             \x20     ╰───╯  └───┘  └┄┄┄┄┄┄┄┘\n"
        );
        let (chart, _) = crate::railroad::parse("railroad-beta\na = terminal(\"x\") ;\n").lower();
        let ascii = Options {
            charset: Charset::Ascii,
        };
        assert_eq!(
            render(&chart, &ascii),
            "      .---.\na = *-+ x +-*\n      '---'\n"
        );
    }

    #[test]
    fn choice() {
        assert_eq!(
            text("railroad-ebnf-beta\na = x | y ;\n"),
            "        ┌───┐\n\
             a = ●─┬─┤ x ├─┬─●\n\
             \x20     │ └───┘ │\n\
             \x20     │ ┌───┐ │\n\
             \x20     └─┤ y ├─┘\n\
             \x20       └───┘\n"
        );
    }

    #[test]
    fn optional_and_repeats() {
        assert_eq!(
            text("railroad-ebnf-beta\na = [x] ;\nb = x+ ;\nc = {x} ;\n"),
            "      ┌───────┐\n\
             \x20     │ ┌───┐ │\n\
             a = ●─┴─┤ x ├─┴─●\n\
             \x20       └───┘\n\
             \n\
             \x20       ┌───┐\n\
             b = ●─┬─┤ x ├─┬─●\n\
             \x20     │ └───┘ │\n\
             \x20     └───◄───┘\n\
             \n\
             \x20     ┌───────┐\n\
             \x20     │ ┌───┐ │\n\
             c = ●─┼─┤ x ├─┼─●\n\
             \x20     │ └───┘ │\n\
             \x20     └───◄───┘\n"
        );
        let (chart, _) = crate::railroad::parse("railroad-beta\n").lower();
        assert_eq!(render(&chart, &Options::default()), "");
    }
}
