//! Cynefin diagrams as text (MCYN-4): the four quadrants in a box split by dotted boundaries,
//! the cliff as a heavy line under the middle, confusion in a box over the middle, and the
//! transitions listed below.

use super::canvas::{Canvas, Line};
use super::{Charset, Options, width};
use crate::cynefin::{Cynefin, Domain};

/// A domain's lines: its name, its descriptions, and its items after a blank line.
fn lines(chart: &Cynefin, d: Domain, arrow: &str) -> Vec<String> {
    let mut out = vec![d.title().to_owned()];
    if chart.descriptions {
        let model = d.model().join(&format!(" {arrow} "));
        if !model.is_empty() {
            out.push(model);
        }
        out.push(d.practice().to_owned());
    }
    let (items, more) = chart.shown(d);
    if !items.is_empty() && d != Domain::Confusion {
        out.push(String::new());
    }
    out.extend(items.iter().cloned());
    if more > 0 {
        out.push(format!("+{more} more"));
    }
    out
}

fn widest(lines: &[String]) -> i32 {
    lines.iter().map(|l| width(l)).max().unwrap_or(0)
}

/// Renders `chart` as text (MCYN-4).
pub fn render(chart: &Cynefin, options: &Options) -> String {
    let arrow = match options.charset {
        Charset::Unicode => "\u{2192}",
        Charset::Ascii => "->",
    };
    let [complex, complicated, chaotic, clear, confusion] =
        Domain::ALL.map(|d| lines(chart, d, arrow));
    // The confusion box: its inner size, and its rows above and below the middle line.
    let cw = widest(&confusion) + 2;
    let box_w = cw + 2;
    let box_h = confusion.len() as i32 + 2;
    let up = box_h / 2;
    let down = box_h - 1 - up;
    let half = box_w / 2;
    // Each half's inner width, with room for the confusion box's half.
    let left = (widest(&complex).max(widest(&chaotic)) + 4).max(half + 3);
    let right = (widest(&complicated).max(widest(&clear)) + 4).max(box_w - half + 3);
    let top = complex.len().max(complicated.len()) as i32;
    let bottom = chaotic.len().max(clear.len()) as i32;
    let (x0, xm) = (0, left + 1);
    let xr = xm + right + 1;
    let y0 = 0;
    let ym = y0 + 1 + top + 1 + up;
    let yb = ym + down + 1 + bottom + 1;
    let mut canvas = Canvas::new(xr + 1, yb + 1);
    canvas.hline(x0, xr, y0, Line::Light);
    canvas.hline(x0, xr, yb, Line::Light);
    canvas.vline(x0, y0, yb, Line::Light);
    canvas.vline(xr, y0, yb, Line::Light);
    let (bx0, by0) = (xm - half, ym - up);
    let (bx1, by1) = (bx0 + box_w - 1, ym + down);
    canvas.hline(bx0, bx1, by0, Line::Light);
    canvas.hline(bx0, bx1, by1, Line::Light);
    canvas.vline(bx0, by0, by1, Line::Light);
    canvas.vline(bx1, by0, by1, Line::Light);
    // The boundaries, up to the box.
    canvas.vline(xm, y0, by0, Line::Dotted);
    canvas.vline(xm, by1, yb, Line::Heavy);
    canvas.hline(x0, bx0, ym, Line::Dotted);
    canvas.hline(bx1, xr, ym, Line::Dotted);

    let centered = |canvas: &mut Canvas, lines: &[String], center: i32, first: i32| {
        for (i, l) in lines.iter().enumerate() {
            canvas.text(center - width(l) / 2, first + i as i32, l);
        }
    };
    let (cl, cr) = (x0 + 1 + left / 2, xm + 1 + right / 2);
    centered(&mut canvas, &complex, cl, y0 + 1);
    centered(&mut canvas, &complicated, cr, y0 + 1);
    centered(&mut canvas, &chaotic, cl, yb - chaotic.len() as i32);
    centered(&mut canvas, &clear, cr, yb - clear.len() as i32);
    centered(&mut canvas, &confusion, bx0 + box_w / 2, by0 + 1);

    let mut out = String::new();
    if let Some(t) = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pad = ((xr + 1 - width(t)) / 2).max(0) as usize;
        out.push_str(&" ".repeat(pad));
        out.push_str(t);
        out.push_str("\n\n");
    }
    out.push_str(&canvas.render(options.charset));
    if !chart.transitions.is_empty() {
        out.push('\n');
        for t in &chart.transitions {
            out.push_str(&format!("{} {arrow} {}", t.from.title(), t.to.title()));
            if let Some(l) = &t.label {
                out.push_str(": ");
                out.push_str(l);
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(src: &str, charset: Charset) -> String {
        let (chart, d) = crate::cynefin::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        render(&chart, &Options { charset })
    }

    #[test]
    fn quadrants_and_confusion() {
        let out = draw(
            "cynefin-beta\ncomplex \"Probe it\"\nclear \"Do it\"\nconfusion \"?\"\n\
             chaotic --> complex : \"Stabilized\"\n",
            Charset::Unicode,
        );
        let rows: Vec<&str> = out.lines().collect();
        // Complex at the top left, Clear's item on the last row inside the box.
        assert!(
            rows[1].contains("Complex") && rows[1].contains("Complicated"),
            "{out}"
        );
        assert!(
            rows[1].find("Complex") < rows[1].find("Complicated"),
            "{out}"
        );
        assert!(out.contains("Probe → Sense → Respond"), "{out}");
        assert!(out.contains("┃"), "the cliff: {out}");
        assert!(out.ends_with("\nChaotic → Complex: Stabilized\n"), "{out}");
        let clear_row = rows.iter().position(|r| r.contains("Do it")).unwrap();
        assert!(rows[clear_row + 1].starts_with('└'), "{out}");
    }

    #[test]
    fn ascii_and_no_descriptions() {
        let out = draw(
            "---\nconfig:\n  cynefin:\n    showDomainDescriptions: false\n---\ncynefin-beta\n\
             confusion \"a\" \"b\" \"c\" \"d\" \"e\"\nclear --> chaotic\n",
            Charset::Ascii,
        );
        assert!(out.is_ascii(), "{out}");
        assert!(!out.contains("Practices"), "{out}");
        assert!(out.contains("+2 more") && !out.contains(" d "), "{out}");
        assert!(out.ends_with("\nClear -> Chaotic\n"), "{out}");
    }
}
