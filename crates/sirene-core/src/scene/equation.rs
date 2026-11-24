//! A `@startlatex` or `@startmath` document to scene (PCREO-3): the formula as paths, or its
//! source as text when it does not typeset or math support is off.

use super::{Element, Owner, Primitive, Pt, Scene, TextMetrics, TextStyle, Theme};
use crate::equation::Equation;

const MARGIN: f32 = 16.0;

/// Draws `equation` in display style, one em being the theme's font size.
pub fn equation(equation: &Equation, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    if let Some(scene) = formula(equation, theme) {
        return scene;
    }
    let size = theme.font_size;
    let width = metrics.width(&equation.source, size) + 2.0 * MARGIN;
    let height = theme.line_height + 2.0 * MARGIN;
    Scene {
        width,
        height,
        background: theme.background,
        elements: vec![Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: equation.source.clone(),
                center: Pt::new(width / 2.0, height / 2.0),
                size,
                color: theme.text,
                style: TextStyle::default(),
            },
        }],
    }
}

/// The left, top, right, and bottom of the paths of `primitives`, control points included.
#[cfg(feature = "math")]
fn ink(primitives: &[Primitive]) -> Option<(f32, f32, f32, f32)> {
    use super::Segment;
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for p in primitives {
        let Primitive::Path { segments, .. } = p else {
            continue;
        };
        for s in segments {
            let pts = match s {
                Segment::Move(a) | Segment::Line(a) => vec![*a],
                Segment::Arc { to, .. } => vec![*to],
                Segment::Cubic { c1, c2, to } => vec![*c1, *c2, *to],
                Segment::Close => vec![],
            };
            for p in pts.iter().filter(|p| p.x.is_finite() && p.y.is_finite()) {
                b = Some(match b {
                    None => (p.x, p.y, p.x, p.y),
                    Some((l, t, r, d)) => (l.min(p.x), t.min(p.y), r.max(p.x), d.max(p.y)),
                });
            }
        }
    }
    b
}

/// The formula as a scene sized to hold both its box (width, ascent, and descent) and what it
/// draws: a box that is empty, negative, or smaller than the ink (`\kern-5em x`) does not put
/// ink outside the scene.
#[cfg(feature = "math")]
fn formula(equation: &Equation, theme: &Theme) -> Option<Scene> {
    let f = equation.typeset().ok()?;
    let size = theme.font_size;
    let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
    // The box, with its top left corner at the origin, and the ink drawn in it.
    let (box_w, ascent) = (finite(f.width * size), finite(f.ascent * size));
    let box_h = ascent + finite(f.descent * size);
    let probe = f.place(Pt::new(0.0, ascent), size, theme.text);
    let (x0, y0, x1, y1) = match ink(&probe) {
        Some((l, t, r, d)) => (l.min(0.0), t.min(0.0), r.max(box_w), d.max(box_h)),
        None => (0.0, 0.0, box_w, box_h),
    };
    let mut scene = Scene {
        width: x1 - x0 + 2.0 * MARGIN,
        height: y1 - y0 + 2.0 * MARGIN,
        background: theme.background,
        elements: Vec::new(),
    };
    let origin = Pt::new(MARGIN - x0, MARGIN - y0 + ascent);
    f.draw_into(&mut scene, Owner::Diagram, origin, size, theme.text);
    Some(scene)
}

#[cfg(not(feature = "math"))]
fn formula(_: &Equation, _: &Theme) -> Option<Scene> {
    None
}

#[cfg(test)]
#[cfg(feature = "math")]
mod tests {
    use super::*;
    use crate::equation::Flavor;
    use crate::scene::{Approx, Segment};

    fn scene(source: &str, flavor: Flavor) -> Scene {
        let e = Equation {
            source: source.to_owned(),
            flavor,
        };
        equation(&e, &Approx, &Theme::LIGHT)
    }

    /// Every point of every path is inside the scene.
    fn assert_ink_inside(s: &Scene) {
        let (mut seen, slack) = (0, 0.01);
        for e in &s.elements {
            let Primitive::Path { segments, .. } = &e.primitive else {
                continue;
            };
            for seg in segments {
                let pts: Vec<Pt> = match seg {
                    Segment::Move(a) | Segment::Line(a) => vec![*a],
                    Segment::Arc { to, .. } => vec![*to],
                    Segment::Cubic { c1, c2, to } => vec![*c1, *c2, *to],
                    Segment::Close => vec![],
                };
                for p in pts {
                    seen += 1;
                    assert!(
                        p.x >= -slack && p.y >= -slack,
                        "{p:?} is above or left of the scene"
                    );
                    assert!(
                        p.x <= s.width + slack && p.y <= s.height + slack,
                        "{p:?} is outside {}x{}",
                        s.width,
                        s.height
                    );
                }
            }
        }
        assert!(seen > 0, "nothing was drawn");
    }

    #[test]
    fn a_formula_is_drawn_inside_its_scene() {
        for src in [r"x^2 + \frac{a}{b}", r"\sum_{i=0}^n i", r"\int_0^1 f"] {
            let s = scene(src, Flavor::Tex);
            assert!(s.width > 2.0 * MARGIN && s.height > 2.0 * MARGIN, "{src}");
            assert_ink_inside(&s);
        }
        assert_ink_inside(&scene("sum_(i=1)^n i", Flavor::AsciiMath));
    }

    #[test]
    fn a_negative_kern_does_not_draw_outside_the_scene() {
        for src in [
            r"\kern-5em x",
            r"\kern-50em x",
            r"\mkern-18mu x",
            r"x \kern-9em",
        ] {
            let s = scene(src, Flavor::Tex);
            assert!(
                s.width.is_finite() && s.width >= 2.0 * MARGIN,
                "{src}: {}",
                s.width
            );
            assert!(s.height.is_finite() && s.height >= 2.0 * MARGIN, "{src}");
            assert_ink_inside(&s);
        }
    }

    #[test]
    fn a_kern_to_the_right_makes_the_scene_wider() {
        let plain = scene("x", Flavor::Tex);
        let kern = scene(r"x \kern5em", Flavor::Tex);
        assert!(kern.width > plain.width);
    }
}
