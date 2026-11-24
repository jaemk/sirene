//! Painting scenes with gpui (GUISHE-4).

use gpui::{
    App, BorderStyle, Bounds, ContentMask, Font, FontStyle, FontWeight, Hsla, PathBuilder, Pixels,
    Point, Rgba, SharedString, TextAlign, TextRun, Window, point, px,
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
    if owners.is_empty() {
        return;
    }
    let owners: std::collections::HashSet<Owner> = owners.iter().copied().collect();
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

/// A box in scene units around everything `primitive` draws: it holds control points, arcs'
/// whole ellipses, half the stroke width, and text up to two ems a character, so it may be
/// larger than the drawing but not smaller. `None` when it draws nothing.
fn extent(primitive: &Primitive) -> Option<(Pt, Pt)> {
    let mut lo = Pt::new(f32::INFINITY, f32::INFINITY);
    let mut hi = Pt::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut add = |p: Pt, rx: f32, ry: f32| {
        lo = Pt::new(lo.x.min(p.x - rx), lo.y.min(p.y - ry));
        hi = Pt::new(hi.x.max(p.x + rx), hi.y.max(p.y + ry));
    };
    match primitive {
        Primitive::Path {
            segments, stroke, ..
        } => {
            let pad = stroke.as_ref().map_or(0.0, |s| s.width.abs() / 2.0);
            let mut at = Pt::default();
            for s in segments {
                match *s {
                    Segment::Move(p) | Segment::Line(p) => {
                        add(p, pad, pad);
                        at = p;
                    }
                    Segment::Arc { radii, to, .. } => {
                        // An arc stays on an ellipse through both ends: within its diameter of
                        // `to`, or, when its radii are too small to reach and so grow, within
                        // the ends' distance.
                        let d = 2.0 * radii.x.abs().max(radii.y.abs());
                        let r = d.max((to.x - at.x).hypot(to.y - at.y)) + pad;
                        add(to, r, r);
                        at = to;
                    }
                    Segment::Cubic { c1, c2, to } => {
                        add(c1, pad, pad);
                        add(c2, pad, pad);
                        add(to, pad, pad);
                        at = to;
                    }
                    Segment::Close => {}
                }
            }
        }
        Primitive::Text {
            text, center, size, ..
        } => {
            let size = size.abs();
            add(*center, text.chars().count() as f32 * size, size);
        }
    }
    (lo.x <= hi.x && lo.y <= hi.y).then_some((lo, hi))
}

/// Whether `primitive`, placed by `at`, may show in `clip`.
fn shows(primitive: &Primitive, at: &Placement, clip: &Bounds<Pixels>) -> bool {
    let Some((lo, hi)) = extent(primitive) else {
        return false;
    };
    let (lo, hi) = (at.map(lo), at.map(hi));
    let (c0, c1) = (
        clip.origin,
        clip.origin + point(clip.size.width, clip.size.height),
    );
    // Written so a NaN compares false and keeps the primitive.
    !(hi.x < c0.x || lo.x > c1.x || hi.y < c0.y || lo.y > c1.y)
}

/// Paints `scene` at `at`, with text in `font`; with `clip`, only what may show inside it.
pub fn paint(
    scene: &Scene,
    at: &Placement,
    clip: Option<&Bounds<Pixels>>,
    font: &Font,
    window: &mut Window,
    cx: &mut App,
) {
    for e in &scene.elements {
        if clip.is_some_and(|c| !shows(&e.primitive, at, c)) {
            continue;
        }
        match &e.primitive {
            Primitive::Path {
                segments,
                fill,
                stroke,
                clip,
            } => {
                let mask = clip.map(|r| {
                    let (a, b) = (
                        at.map(Pt::new(r.x, r.y)),
                        at.map(Pt::new(r.x + r.w, r.y + r.h)),
                    );
                    ContentMask {
                        bounds: Bounds::from_corners(a, b),
                    }
                });
                window.with_content_mask(mask, |window| {
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
                });
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

#[cfg(test)]
mod tests {
    use gpui::{Bounds, point, px, size};
    use sirene_core::scene::{Color, Primitive, Pt, Segment, Stroke, TextStyle};

    use super::{Placement, extent, shows};

    fn path(segments: Vec<Segment>, width: f32) -> Primitive {
        Primitive::Path {
            segments,
            fill: None,
            stroke: Some(Stroke {
                width,
                color: Color::rgb(0),
                dash: None,
            }),
            clip: None,
        }
    }

    fn text(s: &str, center: Pt, size: f32) -> Primitive {
        Primitive::Text {
            text: s.to_owned(),
            center,
            size,
            color: Color::rgb(0),
            style: TextStyle::default(),
        }
    }

    #[test]
    fn extents_hold_strokes_controls_arcs_and_text() {
        let line = path(
            vec![
                Segment::Move(Pt::new(0.0, 0.0)),
                Segment::Line(Pt::new(10.0, 4.0)),
            ],
            2.0,
        );
        assert_eq!(
            extent(&line),
            Some((Pt::new(-1.0, -1.0), Pt::new(11.0, 5.0)))
        );
        let curve = path(
            vec![
                Segment::Move(Pt::new(0.0, 0.0)),
                Segment::Cubic {
                    c1: Pt::new(0.0, -20.0),
                    c2: Pt::new(10.0, 30.0),
                    to: Pt::new(10.0, 0.0),
                },
            ],
            0.0,
        );
        assert_eq!(
            extent(&curve),
            Some((Pt::new(0.0, -20.0), Pt::new(10.0, 30.0)))
        );
        // A half circle of radius 5 from (0, 0) to (10, 0) bulges 5 past them.
        let arc = path(
            vec![
                Segment::Move(Pt::new(0.0, 0.0)),
                Segment::Arc {
                    radii: Pt::new(5.0, 5.0),
                    large: false,
                    sweep: false,
                    to: Pt::new(10.0, 0.0),
                },
            ],
            0.0,
        );
        let (lo, hi) = extent(&arc).unwrap();
        assert!(lo.y <= -5.0 && hi.y >= 5.0 && lo.x <= 0.0 && hi.x >= 10.0);
        // Radii too small to reach grow to half the ends' distance.
        let tiny = path(
            vec![
                Segment::Move(Pt::new(0.0, 0.0)),
                Segment::Arc {
                    radii: Pt::new(0.1, 0.1),
                    large: false,
                    sweep: true,
                    to: Pt::new(0.0, 40.0),
                },
            ],
            0.0,
        );
        let (lo, hi) = extent(&tiny).unwrap();
        assert!(lo.x <= -20.0 && hi.x >= 20.0);
        assert_eq!(
            extent(&text("abc", Pt::new(50.0, 50.0), 10.0)),
            Some((Pt::new(20.0, 40.0), Pt::new(80.0, 60.0)))
        );
        assert_eq!(extent(&path(Vec::new(), 1.0)), None);
    }

    #[test]
    fn only_what_may_show_in_the_clip_is_painted() {
        let clip = Bounds::new(point(px(100.0), px(100.0)), size(px(200.0), px(100.0)));
        // Scene point p lands at (100, 100) + 2p.
        let at = Placement {
            origin: point(px(100.0), px(100.0)),
            zoom: 2.0,
        };
        let dot = |x: f32, y: f32| {
            path(
                vec![
                    Segment::Move(Pt::new(x, y)),
                    Segment::Line(Pt::new(x + 1.0, y)),
                ],
                0.0,
            )
        };
        assert!(shows(&dot(10.0, 10.0), &at, &clip));
        assert!(shows(&dot(99.0, 49.0), &at, &clip));
        assert!(!shows(&dot(120.0, 10.0), &at, &clip));
        assert!(!shows(&dot(10.0, 60.0), &at, &clip));
        assert!(!shows(&dot(-10.0, 10.0), &at, &clip));
        // Text centered outside shows when it may reach in.
        assert!(shows(
            &text("wide label", Pt::new(-20.0, 10.0), 10.0),
            &at,
            &clip
        ));
        assert!(!shows(&text("a", Pt::new(-20.0, 10.0), 10.0), &at, &clip));
        // A line across the clip shows though both ends are outside.
        let across = path(
            vec![
                Segment::Move(Pt::new(-50.0, 20.0)),
                Segment::Line(Pt::new(500.0, 20.0)),
            ],
            0.0,
        );
        assert!(shows(&across, &at, &clip));
    }
}
