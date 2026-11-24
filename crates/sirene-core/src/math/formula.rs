//! The public entry point of the typesetter: source to a `Formula` that draws itself (LEMB-4).
//!
//! `typeset` parses and lays out a formula and keeps the owned display list; `Formula::place`
//! draws it as scene primitives at a pixel size, position and color. Sizes are in em of the
//! root font, so one `Formula` serves any pixel size.

use std::borrow::Cow;

use super::{
    boxes::{Glyph, Item, Path},
    draw::draw,
    error::MathError,
    layout::layout,
    limits,
    parser::{Settings, parse_with},
};
use crate::scene::{Color, Element, Owner, Pt, Scene};

/// Whether a formula is set as a display (own line, large operators, limits above and below) or
/// inline in text (TeX's text style).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MathStyle {
    Display,
    #[default]
    Text,
}

/// A laid out formula: its box in em of the root font and the glyphs and rules in it.
///
/// The box runs from x = 0 to `width`, and from `ascent` above the baseline to `descent` below.
#[derive(Clone, Debug, PartialEq)]
pub struct Formula {
    /// Advance width, in em.
    pub width: f32,
    /// Height above the baseline, in em.
    pub ascent: f32,
    /// Depth below the baseline, in em.
    pub descent: f32,
    items: Vec<Owned>,
}

/// A display list item that borrows nothing: its own color, if any, is held beside it.
#[derive(Clone, Debug, PartialEq)]
struct Owned {
    item: Item<'static>,
    color: Option<String>,
}

impl Owned {
    fn new(item: &Item<'_>) -> Self {
        match item {
            Item::Glyph {
                x,
                y,
                scale,
                glyph,
                mirrored,
            } => Self {
                item: Item::Glyph {
                    x: *x,
                    y: *y,
                    scale: *scale,
                    glyph: Glyph {
                        text: Cow::Owned(glyph.text.to_string()),
                        face: glyph.face,
                        italic: glyph.italic,
                        skew: glyph.skew,
                        color: None,
                    },
                    mirrored: *mirrored,
                },
                color: glyph.color.map(str::to_owned),
            },
            Item::Rule {
                x,
                y,
                width,
                height,
                color,
            } => Self {
                item: Item::Rule {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                    color: None,
                },
                color: color.map(str::to_owned),
            },
            Item::Path {
                x,
                y,
                width,
                height,
                path,
                mirrored,
            } => Self {
                item: Item::Path {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                    path: Path {
                        data: path.data.clone(),
                        view_box: path.view_box,
                        fit: path.fit,
                        color: None,
                    },
                    mirrored: *mirrored,
                },
                color: path.color.map(str::to_owned),
            },
        }
    }

    /// The item with its own color put back.
    fn borrowed(&self) -> Item<'_> {
        let color = self.color.as_deref();
        match &self.item {
            Item::Glyph {
                x,
                y,
                scale,
                glyph,
                mirrored,
            } => Item::Glyph {
                x: *x,
                y: *y,
                scale: *scale,
                glyph: Glyph {
                    text: Cow::Borrowed(&glyph.text),
                    face: glyph.face,
                    italic: glyph.italic,
                    skew: glyph.skew,
                    color,
                },
                mirrored: *mirrored,
            },
            Item::Rule {
                x,
                y,
                width,
                height,
                ..
            } => Item::Rule {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color,
            },
            Item::Path {
                x,
                y,
                width,
                height,
                path,
                mirrored,
            } => Item::Path {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                path: Path {
                    data: path.data.clone(),
                    view_box: path.view_box,
                    fit: path.fit,
                    color,
                },
                mirrored: *mirrored,
            },
        }
    }
}

/// Typesets `source`, a formula in TeX math mode (no `$` delimiters), in `style`.
///
/// Fails with a message and the byte range of the offending command in `source`; never panics.
/// Source length, expansion and nesting limits apply (`limits`).
pub fn typeset(source: &str, style: MathStyle) -> Result<Formula, MathError> {
    let display = style == MathStyle::Display;
    let nodes = parse_with(
        source,
        Settings {
            display_mode: display,
        },
    )?;
    let bx = checked_layout(&nodes, display)?;
    let items = bx.items();
    Ok(Formula {
        width: extent(bx.width.0)?,
        ascent: extent(bx.height.0)?,
        descent: extent(bx.depth.0)?,
        items: items.iter().map(Owned::new).collect(),
    })
}

/// Lays out `nodes` and applies the limits on the result, so the scene and the text renderer
/// agree on what is an error: at most `limits::MAX_ITEMS` glyphs and rules, a box within
/// `limits::MAX_EXTENT_EM`, and every item drawn within that distance of the origin (a box can be
/// empty while its contents are shifted far away, as with `\mathrlap` and `\raisebox`).
pub(super) fn checked_layout<'t>(
    nodes: &'t [super::ast::Node<'_>],
    display: bool,
) -> Result<super::boxes::Bx<'t>, MathError> {
    let bx = layout(nodes, display)?;
    let items = bx.items();
    if items.len() > limits::MAX_ITEMS {
        return Err(MathError::new(
            format!(
                "formula draws more than {} glyphs and rules",
                limits::MAX_ITEMS
            ),
            0..0,
        ));
    }
    extent(bx.width.0)?;
    extent(bx.height.0)?;
    extent(bx.depth.0)?;
    let inside = |v: f64| v.is_finite() && v.abs() <= limits::MAX_EXTENT_EM;
    let in_bounds = items.iter().all(|item| match item {
        Item::Glyph { x, y, scale, .. } => inside(*x) && inside(*y) && scale.is_finite(),
        Item::Rule {
            x,
            y,
            width,
            height,
            ..
        }
        | Item::Path {
            x,
            y,
            width,
            height,
            ..
        } => inside(*x) && inside(*y) && inside(x + width) && inside(y + height),
    });
    if !in_bounds {
        return Err(MathError::new(
            format!(
                "formula draws further than {} em out",
                limits::MAX_EXTENT_EM
            ),
            0..0,
        ));
    }
    Ok(bx)
}

/// A box side in em for the public `Formula`: a negative side (a `\kern` back past the start)
/// is zero, and one that is not finite or is past `limits::MAX_EXTENT_EM` is an error.
fn extent(em: f64) -> Result<f32, MathError> {
    if em.is_finite() && em <= limits::MAX_EXTENT_EM {
        return Ok(em.max(0.0) as f32);
    }
    Err(MathError::new(
        format!("formula larger than {} em", limits::MAX_EXTENT_EM),
        0..0,
    ))
}

impl Formula {
    /// The formula as filled paths with its left edge on the baseline at `origin`, one em being
    /// `size` pixels. Anything without a color of its own is drawn in `color`. A non-finite or
    /// non-positive `size` draws nothing.
    pub fn place(&self, origin: Pt, size: f32, color: Color) -> Vec<crate::scene::Primitive> {
        let items: Vec<Item<'_>> = self.items.iter().map(Owned::borrowed).collect();
        draw(&items, origin, size, color)
    }

    /// Appends the formula's primitives to `scene` as elements of `owner` (see `place`).
    pub fn draw_into(&self, scene: &mut Scene, owner: Owner, origin: Pt, size: f32, color: Color) {
        scene.elements.extend(
            self.place(origin, size, color)
                .into_iter()
                .map(|primitive| Element { owner, primitive }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Primitive;

    const BLACK: Color = Color::rgb(0);

    fn bounds(primitives: &[Primitive]) -> Option<(f32, f32, f32, f32)> {
        let mut b: Option<(f32, f32, f32, f32)> = None;
        for p in primitives {
            let Primitive::Path { segments, .. } = p else {
                continue;
            };
            for s in segments {
                for q in s.points() {
                    b = Some(match b {
                        None => (q.x, q.y, q.x, q.y),
                        Some((x0, y0, x1, y1)) => {
                            (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y))
                        }
                    });
                }
            }
        }
        b
    }

    #[test]
    fn a_label_past_the_last_column_is_inside_the_box() {
        // The right label of the last column is out of the flow, so layout leaves it out of the
        // width; the formula's box takes it in.
        let labeled = typeset(r"\begin{CD}A\\@VVdddV\\B\end{CD}", MathStyle::Display).unwrap();
        let prims = labeled.place(Pt::new(0.0, 0.0), 100.0, BLACK);
        let (_, _, right, _) = bounds(&prims).unwrap();
        // Italic ink may pass its advance by its italic correction.
        assert!(
            right <= labeled.width * 100.0 + 2.0,
            "{right} {}",
            labeled.width
        );
    }

    #[test]
    fn a_letter_has_a_box_and_draws() {
        let f = typeset("x", MathStyle::Text).unwrap();
        assert!(f.width > 0.4 && f.width < 0.8, "{}", f.width);
        assert!(f.ascent > 0.3 && f.ascent < 0.6, "{}", f.ascent);
        assert!(f.descent.abs() < 0.01, "{}", f.descent);
        assert!(!f.place(Pt::new(0.0, 0.0), 10.0, BLACK).is_empty());
    }

    #[test]
    fn a_fraction_is_taller_than_a_letter_and_draws_inside_its_box() {
        let x = typeset("x", MathStyle::Text).unwrap();
        let f = typeset(r"\frac12", MathStyle::Text).unwrap();
        assert!(f.ascent + f.descent > x.ascent + x.descent);
        let (origin, size) = (Pt::new(20.0, 100.0), 40.0);
        let primitives = f.place(origin, size, BLACK);
        let (x0, y0, x1, y1) = bounds(&primitives).unwrap();
        let slack = 0.1 * size;
        assert!(x0 >= origin.x - slack && x1 <= origin.x + f.width * size + slack);
        assert!(y0 >= origin.y - f.ascent * size - slack);
        assert!(y1 <= origin.y + f.descent * size + slack);
    }

    #[test]
    fn display_style_sets_a_sum_larger_than_text_style() {
        let text = typeset(r"\sum_a^b", MathStyle::Text).unwrap();
        let display = typeset(r"\sum_a^b", MathStyle::Display).unwrap();
        assert!(
            display.ascent + display.descent > text.ascent + text.descent,
            "{display:?} {text:?}"
        );
    }

    #[test]
    fn errors_carry_the_span_of_the_offending_command() {
        let src = r"x + \nosuchcommand + y";
        let e = typeset(src, MathStyle::Text).unwrap_err();
        // The token's span includes the space after a control word.
        assert_eq!(src[e.span.clone()].trim_end(), r"\nosuchcommand", "{e}");
        let e = typeset(r"\frac{1}", MathStyle::Text).unwrap_err();
        assert!(e.span.end <= 9, "{e}");
    }

    #[test]
    fn a_color_survives_the_owned_copy() {
        let f = typeset(r"\textcolor{red}{x}", MathStyle::Text).unwrap();
        let red = f.place(Pt::new(0.0, 0.0), 10.0, BLACK);
        let plain = typeset("x", MathStyle::Text)
            .unwrap()
            .place(Pt::new(0.0, 0.0), 10.0, BLACK);
        assert_ne!(red, plain);
    }

    #[test]
    fn draw_into_appends_elements_of_the_owner() {
        let f = typeset("x", MathStyle::Text).unwrap();
        let mut scene = Scene {
            width: 10.0,
            height: 10.0,
            background: Color::rgb(0xffffff),
            elements: vec![],
        };
        f.draw_into(&mut scene, Owner::Node(3), Pt::new(1.0, 8.0), 10.0, BLACK);
        assert!(!scene.elements.is_empty());
        assert!(scene.elements.iter().all(|e| e.owner == Owner::Node(3)));
    }

    #[test]
    fn a_bad_size_draws_nothing() {
        let f = typeset("x", MathStyle::Text).unwrap();
        assert!(f.place(Pt::new(0.0, 0.0), f32::NAN, BLACK).is_empty());
        assert!(f.place(Pt::new(0.0, 0.0), 0.0, BLACK).is_empty());
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_hang() {
        let src = format!("{}x{}", "{".repeat(20_000), "}".repeat(20_000));
        assert!(typeset(&src, MathStyle::Display).is_err());
        let src = format!("{}x{}", r"\frac{1}{".repeat(5_000), "}".repeat(5_000));
        assert!(typeset(&src, MathStyle::Text).is_err());
    }

    #[test]
    fn a_runaway_macro_is_an_error() {
        assert!(typeset(r"\def\a{\a\a}\a", MathStyle::Text).is_err());
    }

    #[test]
    fn oversized_source_is_an_error() {
        let src = "x".repeat(super::super::limits::MAX_SOURCE + 1);
        assert!(typeset(&src, MathStyle::Text).is_err());
    }

    /// Writes the fuzz seed corpus from the inputs of the KaTeX oracle, one file per input. Run
    /// once with `cargo test -p sirene-core --lib write_fuzz_corpus -- --ignored`.
    #[test]
    #[ignore = "writes fuzz/corpus/math"]
    fn write_fuzz_corpus() {
        let data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/math/katex-oracle.json"))
                .unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/math");
        std::fs::create_dir_all(&dir).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for key in data.keys() {
            let (input, _display, _macros): (String, bool, serde_json::Value) =
                serde_json::from_str(key).unwrap();
            if input.len() > 1024 || !seen.insert(input.clone()) {
                continue;
            }
            let name = format!("{:04}", seen.len());
            std::fs::write(dir.join(name), input).unwrap();
        }
    }
}
