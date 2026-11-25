//! A diagram with its `header`, `footer`, `caption`, and `legend` (PCOM-1): the diagram's own
//! scene, moved down and widened to make room, with the decorations drawn around it.
//!
//! From the top: the header, a top legend, the diagram, a bottom legend, the caption, the
//! footer. Each is set apart from its neighbors by [`PAD`]. Text is set at the theme's font size
//! on lines one line height tall, or as tall as the formulas in them (LEMB-4).

use super::mathtext::{self, Formulas};
use super::{
    Color, Element, Owner, Primitive, Pt, Rect, Scene, Segment, Stroke, TextMetrics, Theme,
    parse_color,
};
use crate::decor::{self, Decorated, HAlign, VAlign};
use crate::flowchart::Span;

/// Space above and below each decoration, in pixels.
const PAD: f32 = 8.0;
/// The least space between a decoration and the side of the drawing.
const SIDE: f32 = 16.0;
/// Space between a legend's text and its border.
const INSET: f32 = 8.0;

/// A decoration set out from its top left corner.
struct Part {
    w: f32,
    h: f32,
    primitives: Vec<Primitive>,
    align: HAlign,
}

/// The scene of `d`: its diagram with the decorations around it.
pub fn decorated(d: &Decorated, metrics: &impl TextMetrics, theme: &Theme) -> crate::Result<Scene> {
    let mut scene = d.diagram.scene(metrics, theme)?;
    let formulas = Formulas::default();
    let banner = |b: &Option<decor::Banner>| {
        b.as_ref()
            .map(|b| text_block(metrics, &formulas, theme, &b.lines, b.align, b.align))
    };
    let header = banner(&d.decor.header);
    let footer = banner(&d.decor.footer);
    let caption = banner(&d.decor.caption);
    let legend = d.decor.legend.as_ref().map(|l| {
        let inner = text_block(
            metrics,
            &formulas,
            theme,
            &l.lines,
            HAlign::Left,
            l.horizontal,
        );
        boxed(inner, theme)
    });
    let top_legend = (d.decor.legend.as_ref()).is_some_and(|l| l.vertical == VAlign::Top);
    let (above, below) = if top_legend {
        (vec![header, legend], vec![caption, footer])
    } else {
        (vec![header], vec![legend, caption, footer])
    };
    let above: Vec<Part> = above.into_iter().flatten().collect();
    let below: Vec<Part> = below.into_iter().flatten().collect();

    let stack = |parts: &[Part]| parts.iter().map(|p| p.h + 2.0 * PAD).sum::<f32>();
    let widest = above
        .iter()
        .chain(&below)
        .map(|p| p.w + 2.0 * SIDE)
        .fold(0.0, f32::max);
    let width = scene.width.max(widest);
    let top = stack(&above);
    let height = top + scene.height + stack(&below);

    let dx = (width - scene.width) / 2.0;
    for e in &mut scene.elements {
        translate(&mut e.primitive, Pt::new(dx, top));
    }
    let mut elements = Vec::new();
    let mut y = 0.0;
    for p in &above {
        place(p, y, width, &mut elements);
        y += p.h + 2.0 * PAD;
    }
    y += scene.height;
    let inner = std::mem::take(&mut scene.elements);
    for p in &below {
        place(p, y, width, &mut elements);
        y += p.h + 2.0 * PAD;
    }
    // The decorations above are painted first, then the diagram, then the ones below.
    let split = above.iter().map(|p| p.primitives.len()).sum::<usize>();
    let mut out: Vec<Element> = elements.drain(..split).collect();
    out.extend(inner);
    out.extend(elements);
    Ok(Scene {
        width,
        height,
        background: scene.background,
        elements: out,
    })
}

/// Adds `part` to `out`, its box starting `PAD` below `y`, set against the side `part.align` says
/// of a drawing `width` wide.
fn place(part: &Part, y: f32, width: f32, out: &mut Vec<Element>) {
    let x = match part.align {
        HAlign::Left => SIDE,
        HAlign::Center => (width - part.w) / 2.0,
        HAlign::Right => width - SIDE - part.w,
    };
    for p in &part.primitives {
        let mut p = p.clone();
        translate(&mut p, Pt::new(x, y + PAD));
        out.push(Element {
            owner: Owner::Diagram,
            primitive: p,
        });
    }
}

/// `part` in a bordered box.
fn boxed(part: Part, theme: &Theme) -> Part {
    let (w, h) = (part.w + 2.0 * INSET, part.h + 2.0 * INSET);
    let mut primitives = vec![Primitive::Path {
        segments: vec![
            Segment::Move(Pt::new(0.0, 0.0)),
            Segment::Line(Pt::new(w, 0.0)),
            Segment::Line(Pt::new(w, h)),
            Segment::Line(Pt::new(0.0, h)),
            Segment::Close,
        ],
        fill: Some(theme.cluster_fill),
        stroke: Some(Stroke {
            width: 1.0,
            color: theme.cluster_stroke,
            dash: None,
        }),
        clip: None,
    }];
    for mut p in part.primitives {
        translate(&mut p, Pt::new(INSET, INSET));
        primitives.push(p);
    }
    Part {
        w,
        h,
        primitives,
        align: part.align,
    }
}

/// The color a `<color:...>` names, or the theme's text color.
fn color_of(name: Option<&str>, theme: &Theme) -> Color {
    name.and_then(|n| {
        parse_color(n).or_else(|| parse_color(&format!("#{}", n.trim_start_matches('#'))))
    })
    .unwrap_or(theme.text)
}

/// `lines` as a block of text: each line is a row of runs on one baseline, set against the side
/// `within` says of the block's own width. The block sits at the side `align` says.
fn text_block(
    metrics: &(impl TextMetrics + ?Sized),
    formulas: &Formulas,
    theme: &Theme,
    lines: &[String],
    within: HAlign,
    align: HAlign,
) -> Part {
    let (lh, size) = (theme.line_height, theme.font_size);
    let rows: Vec<(Vec<Span>, Vec<Option<String>>)> = decor::lines(lines)
        .into_iter()
        .map(|line| {
            let mut spans = Vec::new();
            let mut colors = Vec::new();
            let mut prefix = line.marker.prefix("\u{2022}");
            for (i, mut run) in line.runs.into_iter().enumerate() {
                // The marker is the start of the line's first run, when that is text.
                if i == 0 && !run.span.math {
                    run.span.text.insert_str(0, &std::mem::take(&mut prefix));
                }
                spans.push(run.span);
                colors.push(run.color);
            }
            if !prefix.is_empty() {
                spans.insert(0, Span::plain(prefix));
                colors.insert(0, None);
            }
            (spans, colors)
        })
        .collect();
    let width_of = |spans: &[Span]| -> f32 {
        spans
            .iter()
            .map(|s| mathtext::span_width(metrics, formulas, size, s))
            .sum()
    };
    let boxes: Vec<Option<(f32, f32)>> = rows
        .iter()
        .map(|(spans, _)| mathtext::line_box(formulas, theme, spans))
        .collect();
    let height = |b: &Option<(f32, f32)>| b.map_or(lh, |(above, below)| above + below);
    let w = rows
        .iter()
        .map(|(spans, _)| width_of(spans))
        .fold(0.0, f32::max);
    let h = boxes.iter().map(height).sum::<f32>();
    let mut primitives = Vec::new();
    let mut top = 0.0;
    for ((spans, colors), b) in rows.iter().zip(&boxes) {
        let (baseline, mid) = match *b {
            Some((above, _)) => (top + above, top + above - mathtext::BASELINE * size),
            None => (top + lh / 2.0, top + lh / 2.0),
        };
        let mut x = match within {
            HAlign::Left => 0.0,
            HAlign::Center => (w - width_of(spans)) / 2.0,
            HAlign::Right => w - width_of(spans),
        };
        for (span, color) in spans.iter().zip(colors) {
            let color = color_of(color.as_deref(), theme);
            if let Some(f) = formulas.inline(span) {
                primitives.extend(f.place(Pt::new(x, baseline), size, color));
                x += f.width * size;
                continue;
            }
            let style = mathtext::style_of(span);
            let width = |t: &str| metrics.styled_width(t, size, style);
            let trimmed = span.text.trim();
            if !trimmed.is_empty() {
                let lead = &span.text[..span.text.len() - span.text.trim_start().len()];
                let left = x + width(lead);
                primitives.push(Primitive::Text {
                    text: trimmed.to_owned(),
                    center: Pt::new(left + width(trimmed) / 2.0, mid),
                    size,
                    color,
                    style,
                });
            }
            x += width(&span.text);
        }
        top += height(b);
    }
    Part {
        w,
        h,
        primitives,
        align,
    }
}

/// Moves a primitive by `d`, and the rectangle it is clipped to.
fn translate(primitive: &mut Primitive, d: Pt) {
    let shift = |p: &mut Pt| {
        p.x += d.x;
        p.y += d.y;
    };
    match primitive {
        Primitive::Path { segments, clip, .. } => {
            for s in segments {
                match s {
                    Segment::Move(p) | Segment::Line(p) => shift(p),
                    Segment::Arc { to, .. } => shift(to),
                    Segment::Cubic { c1, c2, to } => {
                        shift(c1);
                        shift(c2);
                        shift(to);
                    }
                    Segment::Close => {}
                }
            }
            if let Some(Rect { x, y, .. }) = clip {
                *x += d.x;
                *y += d.y;
            }
        }
        Primitive::Text { center, .. } => shift(center),
    }
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::super::{Approx, Theme};

    /// A formula in a decoration is typeset once to check it when lowering and once to draw it,
    /// in the scene and as text, however often a line is measured and placed.
    #[test]
    fn decorations_typeset_each_formula_once_per_render() {
        let src = "@startuml\nheader <latex>a_1</latex> and <math>x^2</math>\nfooter <latex>b_2</latex>\ncaption <latex>c_3</latex> <latex>c_3</latex>\nlegend\n* <latex>d_4</latex>\nend legend\nA -> B\n@enduml\n";
        crate::equation::count::reset();
        let lowered = crate::diagram::lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        assert_eq!(crate::equation::count::get(), 6, "lowering checks each");
        crate::equation::count::reset();
        let scene = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert_eq!(crate::equation::count::get(), 5, "drawing sets each once");
        assert!(!scene.elements.is_empty());
        crate::equation::count::reset();
        let options = crate::ascii::Options::default();
        let text = lowered.diagram.ascii(&options).unwrap();
        assert_eq!(crate::equation::count::get(), 5, "text sets each once");
        assert!(!text.contains("latex>"), "{text}");
    }
}
