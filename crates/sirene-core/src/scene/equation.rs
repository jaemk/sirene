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

#[cfg(feature = "math")]
fn formula(equation: &Equation, theme: &Theme) -> Option<Scene> {
    let f = equation.typeset().ok()?;
    let size = theme.font_size;
    let mut scene = Scene {
        width: f.width * size + 2.0 * MARGIN,
        height: (f.ascent + f.descent) * size + 2.0 * MARGIN,
        background: theme.background,
        elements: Vec::new(),
    };
    let origin = Pt::new(MARGIN, MARGIN + f.ascent * size);
    f.draw_into(&mut scene, Owner::Diagram, origin, size, theme.text);
    Some(scene)
}

#[cfg(not(feature = "math"))]
fn formula(_: &Equation, _: &Theme) -> Option<Scene> {
    None
}
