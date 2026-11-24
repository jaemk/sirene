//! PNG export of scenes with an embedded font (DISPLA-11).

use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, fontdb};

use super::{Approx, Scene, TextMetrics, TextStyle};

/// Noto Sans Regular, the font of unstyled PNG text.
pub const FONT: &[u8] = include_bytes!("../../fonts/NotoSans-Regular.ttf");
/// Noto Sans Bold, Italic, and Bold Italic, for markdown emphasis.
pub const BOLD: &[u8] = include_bytes!("../../fonts/NotoSans-Bold.ttf");
pub const ITALIC: &[u8] = include_bytes!("../../fonts/NotoSans-Italic.ttf");
pub const BOLD_ITALIC: &[u8] = include_bytes!("../../fonts/NotoSans-BoldItalic.ttf");
const FAMILY: &str = "Noto Sans";
/// The largest image side, in pixels.
pub const MAX_SIDE: u32 = 16384;

/// The embedded font file for `style`.
fn font(style: TextStyle) -> &'static [u8] {
    match (style.bold, style.italic) {
        (false, false) => FONT,
        (true, false) => BOLD,
        (false, true) => ITALIC,
        (true, true) => BOLD_ITALIC,
    }
}

fn face(style: TextStyle) -> Option<&'static ttf_parser::Face<'static>> {
    static FACES: OnceLock<[Option<ttf_parser::Face<'static>>; 4]> = OnceLock::new();
    let faces = FACES.get_or_init(|| {
        [FONT, BOLD, ITALIC, BOLD_ITALIC].map(|f| match ttf_parser::Face::parse(f, 0) {
            Ok(face) => Some(face),
            Err(e) => {
                log::error!("cannot parse embedded font; using approximate metrics: {e}");
                None
            }
        })
    });
    let i = usize::from(style.bold) + 2 * usize::from(style.italic);
    faces[i].as_ref()
}

fn fonts() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        for bold in [false, true] {
            for italic in [false, true] {
                db.load_font_data(font(TextStyle { bold, italic }).to_vec());
            }
        }
        db.set_sans_serif_family(FAMILY);
        Arc::new(db)
    })
    .clone()
}

/// Text widths in the embedded fonts.
#[derive(Debug, Clone, Copy, Default)]
pub struct FontMetrics;

impl TextMetrics for FontMetrics {
    fn monotonic_prefixes(&self, _: &str, _: &str, size: f32, _: TextStyle) -> bool {
        // This backend sums independent, nonnegative glyph advances.
        size >= 0.0
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        self.styled_width(text, size, TextStyle::default())
    }

    fn styled_width(&self, text: &str, size: f32, style: TextStyle) -> f32 {
        let Some(face) = face(style) else {
            return Approx.width(text, size);
        };
        let em = f32::from(face.units_per_em());
        let mut buf = [0u8; 4];
        text.chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|g| face.glyph_hor_advance(g))
                    .map_or_else(
                        || Approx.width(c.encode_utf8(&mut buf), size),
                        |a| f32::from(a) * size / em,
                    )
            })
            .sum()
    }
}

/// How finely an image is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    /// 1 pixel per unit, without anti-aliasing.
    Draft,
    /// 2 pixels per unit, anti-aliased.
    Normal,
    /// 4 pixels per unit, anti-aliased, with text drawn from its exact outlines.
    #[default]
    Fine,
}

impl Quality {
    /// The pixels per scene unit this quality draws at when no scale is given.
    pub fn scale(self) -> f32 {
        match self {
            Quality::Draft => 1.0,
            Quality::Normal => 2.0,
            Quality::Fine => 4.0,
        }
    }

    fn rendering(self) -> (usvg::ShapeRendering, usvg::TextRendering) {
        match self {
            Quality::Draft => (
                usvg::ShapeRendering::CrispEdges,
                usvg::TextRendering::OptimizeSpeed,
            ),
            Quality::Normal => (
                usvg::ShapeRendering::GeometricPrecision,
                usvg::TextRendering::OptimizeLegibility,
            ),
            Quality::Fine => (
                usvg::ShapeRendering::GeometricPrecision,
                usvg::TextRendering::GeometricPrecision,
            ),
        }
    }
}

/// How `write` draws: pixels per scene unit, and anti-aliasing from the quality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub scale: f32,
    pub quality: Quality,
}

impl Options {
    /// `quality` at its own scale (`Quality::scale`).
    pub fn new(quality: Quality) -> Options {
        Options {
            scale: quality.scale(),
            quality,
        }
    }
}

impl Default for Options {
    fn default() -> Options {
        Options::new(Quality::default())
    }
}

/// `scale`, or the largest scale under it at which `scene` fits in `MAX_SIDE` pixels a side.
pub fn fit(scene: &Scene, scale: f32) -> f32 {
    let side = scene.width.max(scene.height);
    if (side * scale).ceil() <= MAX_SIDE as f32 {
        return scale;
    }
    // Shrink a hair below the exact ratio so rounding up stays inside.
    let fitted = MAX_SIDE as f32 / side * (1.0 - 1e-5);
    log::debug!("scale {scale} would exceed {MAX_SIDE} pixels a side; using {fitted}");
    fitted
}

/// Draws `scene` as a PNG with `options.scale` pixels per scene unit.
pub fn write(scene: &Scene, options: Options) -> crate::Result<Vec<u8>> {
    let _phase = crate::profile::Phase::start("png");
    let Options { scale, quality } = options;
    if !(scale.is_finite() && scale > 0.0) {
        return Err(crate::Error::new(format!(
            "scale must be a positive number, not {scale}"
        )));
    }
    let side = |v: f32| (v * scale).ceil();
    let (w, h) = (side(scene.width), side(scene.height));
    if w > MAX_SIDE as f32 || h > MAX_SIDE as f32 {
        return Err(crate::Error::new(format!(
            "a {w}x{h} image is too large (at most {MAX_SIDE} pixels a side)"
        )));
    }
    let (shape_rendering, text_rendering) = quality.rendering();
    let options = usvg::Options {
        font_family: FAMILY.to_owned(),
        fontdb: fonts(),
        shape_rendering,
        text_rendering,
        ..usvg::Options::default()
    };
    let phase = crate::profile::Phase::start("png.svg");
    let svg = super::svg::write(scene);
    drop(phase);
    let phase = crate::profile::Phase::start("png.parse");
    let tree = usvg::Tree::from_str(&svg, &options)
        .map_err(|e| crate::Error::with_source("cannot draw the diagram", e))?;
    drop(phase);
    // Nonzero, as usvg rejects an empty SVG size above.
    let mut pixmap = Pixmap::new(w as u32, h as u32)
        .ok_or_else(|| crate::Error::new(format!("cannot allocate a {w}x{h} image")))?;
    let phase = crate::profile::Phase::start("png.render");
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    drop(phase);
    let _phase = crate::profile::Phase::start("png.encode");
    let bytes = pixmap
        .encode_png()
        .map_err(|e| crate::Error::with_source("cannot encode PNG", e))?;
    log::debug!(
        "encoded a {w}x{h} PNG at scale {scale}, {quality:?} quality, {} bytes",
        bytes.len()
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Color, Element, Owner, Primitive, Pt, Rect, Segment};

    /// A 20x20 scene with a black square over (0,0)-(20,20), clipped to `clip`.
    fn square(clip: Option<Rect>) -> Scene {
        let (a, b) = (Pt::new(0.0, 0.0), Pt::new(20.0, 20.0));
        Scene {
            width: 20.0,
            height: 20.0,
            background: Color::rgb(0xffffff),
            elements: vec![Element {
                owner: Owner::Diagram,
                primitive: Primitive::Path {
                    segments: vec![
                        Segment::Move(a),
                        Segment::Line(Pt::new(b.x, a.y)),
                        Segment::Line(b),
                        Segment::Line(Pt::new(a.x, b.y)),
                        Segment::Close,
                    ],
                    fill: Some(Color::rgb(0)),
                    stroke: None,
                    clip,
                },
            }],
        }
    }

    fn dark(pixmap: &Pixmap, x: u32, y: u32) -> bool {
        pixmap.pixel(x, y).is_some_and(|p| p.red() < 128)
    }

    fn render(scene: &Scene) -> Pixmap {
        let bytes = write(scene, Options::default()).unwrap();
        Pixmap::decode_png(&bytes).unwrap()
    }

    #[test]
    fn clipped_path_draws_only_inside_the_clip() {
        let pixmap = render(&square(Some(Rect::new(5.0, 5.0, 10.0, 10.0))));
        let scale = Options::default().scale;
        let at = |v: f32| (v * scale) as u32;
        assert!(dark(&pixmap, at(10.0), at(10.0)));
        assert!(dark(&pixmap, at(6.0), at(14.0)));
        assert!(!dark(&pixmap, at(2.0), at(2.0)));
        assert!(!dark(&pixmap, at(10.0), at(2.0)));
        assert!(!dark(&pixmap, at(18.0), at(10.0)));
        assert!(!dark(&pixmap, at(18.0), at(18.0)));
    }

    #[test]
    fn unclipped_path_fills_everywhere() {
        let pixmap = render(&square(None));
        let scale = Options::default().scale;
        let at = |v: f32| (v * scale) as u32;
        assert!(dark(&pixmap, at(2.0), at(2.0)));
        assert!(dark(&pixmap, at(18.0), at(18.0)));
    }
}
