//! Painting scenes with gpui (GUISHE-4).

use gpui::{
    App, BorderStyle, Bounds, Font, FontStyle, FontWeight, Hsla, PathBuilder, Pixels, Point, Rgba,
    SharedString, TextAlign, TextRun, Window, point, px,
};
use sirene_core::scene::{Color, Owner, Primitive, Pt, Scene, Segment, TextMetrics, TextStyle};

/// Text measured by the window's text system.
pub struct Metrics<'a> {
    pub window: &'a Window,
    pub font: Font,
}

/// `font` in the weight and slant of `style`.
pub fn styled(font: &Font, style: TextStyle) -> Font {
    let mut f = font.clone();
    if style.bold {
        f.weight = FontWeight::BOLD;
    }
    if style.italic {
        f.style = FontStyle::Italic;
    }
    f
}

fn run(text: &str, font: &Font, color: Hsla) -> TextRun {
    TextRun {
        len: text.len(),
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

impl TextMetrics for Metrics<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.styled_width(text, size, TextStyle::default())
    }

    fn styled_width(&self, text: &str, size: f32, style: TextStyle) -> f32 {
        let shaped = self.window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            px(size),
            &[run(text, &styled(&self.font, style), Hsla::default())],
            None,
        );
        f32::from(shaped.width)
    }
}

pub fn color(c: Color) -> Hsla {
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
    .into()
}

/// Where a scene is drawn: scene point `p` lands at `origin + p * zoom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub origin: Point<Pixels>,
    pub zoom: f32,
}

impl Placement {
    pub fn map(&self, p: Pt) -> Point<Pixels> {
        point(
            self.origin.x + px(p.x * self.zoom),
            self.origin.y + px(p.y * self.zoom),
        )
    }
}

fn build(mut b: PathBuilder, segments: &[Segment], at: &Placement) -> PathBuilder {
    for s in segments {
        match *s {
            Segment::Move(p) => b.move_to(at.map(p)),
            Segment::Line(p) => b.line_to(at.map(p)),
            Segment::Arc {
                radii,
                large,
                sweep,
                to,
            } => b.arc_to(
                point(px(radii.x * at.zoom), px(radii.y * at.zoom)),
                px(0.0),
                large,
                sweep,
                at.map(to),
            ),
            Segment::Cubic { c1, c2, to } => {
                b.cubic_bezier_to(at.map(to), at.map(c1), at.map(c2));
            }
            Segment::Close => b.close(),
        }
    }
    b
}

/// Outlines the paths drawn for `owners` in `color`, 2.5px wide at any zoom (WYSIWY-9).
pub fn outline(scene: &Scene, owners: &[Owner], at: &Placement, color: Hsla, window: &mut Window) {
    for e in scene.elements.iter().filter(|e| owners.contains(&e.owner)) {
        if let Primitive::Path { segments, .. } = &e.primitive
            && let Ok(path) = build(PathBuilder::stroke(px(2.5)), segments, at).build()
        {
            window.paint_path(path, color);
        }
    }
}

/// A 2px line from `from` to `to` (WYSIWY-11).
pub fn line(from: Point<Pixels>, to: Point<Pixels>, color: Hsla, window: &mut Window) {
    let mut b = PathBuilder::stroke(px(2.0));
    b.move_to(from);
    b.line_to(to);
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

/// A 2px frame just inside `bounds`, shown while the drawing has the keyboard (GUISHE-12).
pub fn focus_ring(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    window.paint_quad(gpui::quad(
        bounds,
        0.0,
        gpui::transparent_black(),
        2.0,
        color,
        BorderStyle::Solid,
    ));
}

/// Paints `scene` at `at`, with text in `font`.
pub fn paint(scene: &Scene, at: &Placement, font: &Font, window: &mut Window, cx: &mut App) {
    for e in &scene.elements {
        match &e.primitive {
            Primitive::Path {
                segments,
                fill,
                stroke,
            } => {
                if let Some(c) = fill
                    && let Ok(path) = build(PathBuilder::fill(), segments, at).build()
                {
                    window.paint_path(path, color(*c));
                }
                if let Some(s) = stroke {
                    let mut b = PathBuilder::stroke(px(s.width * at.zoom));
                    if let Some((d, g)) = s.dash {
                        b = b.dash_array(&[px(d * at.zoom), px(g * at.zoom)]);
                    }
                    if let Ok(path) = build(b, segments, at).build() {
                        window.paint_path(path, color(s.color));
                    }
                }
            }
            Primitive::Text {
                text,
                center,
                size,
                color: c,
                style,
            } => {
                let size = px(size * at.zoom);
                let shaped = window.text_system().shape_line(
                    SharedString::from(text.clone()),
                    size,
                    &[run(text, &styled(font, *style), color(*c))],
                    None,
                );
                let line_height = size * 1.3;
                let mid = at.map(*center);
                let origin = point(mid.x - shaped.width / 2.0, mid.y - line_height / 2.0);
                let _ = shaped.paint(origin, line_height, TextAlign::Left, None, window, cx);
            }
        }
    }
}
