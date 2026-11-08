//! PNG export of scenes with an embedded font (DISPLA-11).

use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, fontdb};

use super::{Approx, Scene, TextMetrics};

/// Noto Sans Regular, the only font PNG output uses.
pub const FONT: &[u8] = include_bytes!("../../fonts/NotoSans-Regular.ttf");
const FAMILY: &str = "Noto Sans";
/// The largest image side, in pixels.
pub const MAX_SIDE: u32 = 16384;

fn face() -> &'static ttf_parser::Face<'static> {
    static FACE: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| ttf_parser::Face::parse(FONT, 0).expect("the embedded font parses"))
}

fn fonts() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_font_data(FONT.to_vec());
        db.set_sans_serif_family(FAMILY);
        Arc::new(db)
    })
    .clone()
}

/// Text widths in the embedded font.
#[derive(Debug, Clone, Copy, Default)]
pub struct FontMetrics;

impl TextMetrics for FontMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        let face = face();
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

/// Draws `scene` as a PNG with `scale` pixels per scene unit.
pub fn write(scene: &Scene, scale: f32) -> Result<Vec<u8>, String> {
    if !(scale.is_finite() && scale > 0.0) {
        return Err(format!("scale must be a positive number, not {scale}"));
    }
    let side = |v: f32| (v * scale).ceil();
    let (w, h) = (side(scene.width), side(scene.height));
    if w > MAX_SIDE as f32 || h > MAX_SIDE as f32 {
        return Err(format!(
            "a {w}x{h} image is too large (at most {MAX_SIDE} pixels a side)"
        ));
    }
    let options = usvg::Options {
        font_family: FAMILY.to_owned(),
        fontdb: fonts(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&super::svg::write(scene), &options)
        .map_err(|e| format!("cannot draw the diagram: {e}"))?;
    // Nonzero, as usvg rejects an empty SVG size above.
    let mut pixmap = Pixmap::new(w as u32, h as u32).expect("the size is checked above");
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}
