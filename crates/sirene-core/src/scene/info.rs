//! Info diagram to scene (MINFO-3).

use super::{Element, Owner, Primitive, Pt, Scene, TextMetrics, TextStyle, Theme};
use crate::info::Info;

const MARGIN: f32 = 16.0;
/// The version's font size, as Mermaid draws it.
const SIZE: f32 = 32.0;
/// The smallest width, as Mermaid draws it.
const MIN_WIDTH: f32 = 400.0;

/// Draws `info`: the title, if any, over sirene's version (MINFO-3).
pub fn info(info: &Info, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let title = info
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = theme.font_size * 1.2;
    let text = info.text();
    let width = [
        MIN_WIDTH,
        metrics.width(&text, SIZE) + 2.0 * MARGIN,
        title.map_or(0.0, |t| metrics.width(t, title_size) + 2.0 * MARGIN),
    ]
    .into_iter()
    .fold(0.0, f32::max);
    let mut elements = Vec::new();
    let mut y = MARGIN;
    let mut push = |text: String, size: f32, h: f32, y: &mut f32| {
        elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text,
                center: Pt::new(width / 2.0, *y + h / 2.0),
                size,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
        *y += h;
    };
    if let Some(t) = title {
        push(t.to_owned(), title_size, theme.line_height + 16.0, &mut y);
    }
    push(text, SIZE, SIZE * 2.0, &mut y);
    Scene {
        width,
        height: y + MARGIN,
        background: theme.background,
        elements,
    }
}
