//! SVG export of scenes (DISPLA-9).

use std::fmt::Write;

use super::{Color, Primitive, Scene, Segment};

const FONT: &str = "trebuchet ms, verdana, arial, sans-serif";

/// Writes `scene` as a standalone SVG document.
pub fn write(scene: &Scene) -> String {
    let (w, h) = (num(scene.width), num(scene.height));
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"{FONT}\">\n"
    );
    out.push_str(&format!(
        "<rect width=\"100%\" height=\"100%\"{}/>\n",
        paint("fill", scene.background)
    ));
    for e in &scene.elements {
        match &e.primitive {
            Primitive::Path {
                segments,
                fill,
                stroke,
            } => {
                out.push_str("<path d=\"");
                out.push_str(&path_data(segments));
                out.push('"');
                match fill {
                    Some(c) => out.push_str(&paint("fill", *c)),
                    None => out.push_str(" fill=\"none\""),
                }
                if let Some(s) = stroke {
                    out.push_str(&paint("stroke", s.color));
                    let _ = write!(out, " stroke-width=\"{}\"", num(s.width));
                    if let Some((d, g)) = s.dash {
                        let _ = write!(out, " stroke-dasharray=\"{} {}\"", num(d), num(g));
                    }
                }
                out.push_str("/>\n");
            }
            Primitive::Text {
                text,
                center,
                size,
                color,
            } => {
                let _ = writeln!(
                    out,
                    "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"middle\" \
                     dominant-baseline=\"central\"{}>{}</text>",
                    num(center.x),
                    num(center.y),
                    num(*size),
                    paint("fill", *color),
                    escape(text)
                );
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

/// A color attribute, with an opacity attribute when it is translucent.
fn paint(attr: &str, c: Color) -> String {
    let mut s = format!(" {attr}=\"#{:02x}{:02x}{:02x}\"", c.r, c.g, c.b);
    if c.a != 255 {
        let _ = write!(s, " {attr}-opacity=\"{}\"", num(f32::from(c.a) / 255.0));
    }
    s
}

fn path_data(segments: &[Segment]) -> String {
    let mut d = Vec::new();
    for s in segments {
        d.push(match *s {
            Segment::Move(p) => format!("M{} {}", num(p.x), num(p.y)),
            Segment::Line(p) => format!("L{} {}", num(p.x), num(p.y)),
            Segment::Arc {
                radii,
                large,
                sweep,
                to,
            } => format!(
                "A{} {} 0 {} {} {} {}",
                num(radii.x),
                num(radii.y),
                u8::from(large),
                u8::from(sweep),
                num(to.x),
                num(to.y)
            ),
            Segment::Cubic { c1, c2, to } => format!(
                "C{} {} {} {} {} {}",
                num(c1.x),
                num(c1.y),
                num(c2.x),
                num(c2.y),
                num(to.x),
                num(to.y)
            ),
            Segment::Close => "Z".to_owned(),
        });
    }
    d.join(" ")
}

/// A number with at most two decimals and no trailing zeros.
fn num(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}
