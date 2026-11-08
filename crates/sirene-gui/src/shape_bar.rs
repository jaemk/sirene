//! The shape bar over the drawing: one button per common shape, each adding a node of that
//! shape (WYSIWY-15).

use std::rc::Rc;

use gpui::component::button::{Button, ButtonVariants};
use gpui::{Bounds, Context, Font, IntoElement, MouseButton, Pixels, Window, canvas, div, point};
use gpui::{ParentElement, Styled, prelude::*, px};
use sirene_core::flowchart::{self, Shape};
use sirene_core::scene::{self, Owner, Scene, TextMetrics, Theme};

use crate::AddShape;
use crate::paint::{self, Placement};
use crate::viewer::Viewer;

/// The shapes on the bar, in order.
pub const SHAPES: [Shape; 8] = [
    Shape::Rect,
    Shape::Rounded,
    Shape::Stadium,
    Shape::Diamond,
    Shape::Hexagon,
    Shape::Circle,
    Shape::Cylinder,
    Shape::LeanRight,
];

/// The size of a button's icon.
const ICON: (f32, f32) = (26.0, 16.0);

/// One single-node scene per bar shape, drawn by the diagram renderer in `theme`.
pub fn icons(theme: &Theme, metrics: &impl TextMetrics) -> Vec<Scene> {
    SHAPES
        .iter()
        .map(|shape| {
            let (open, close) = shape.delimiters().unwrap_or(("[", "]"));
            let src = format!("flowchart TD\n    A{open}\"  \"{close}\n");
            let chart = flowchart::parse(&src).lower().flowchart;
            scene::flowchart(&chart, metrics, theme)
        })
        .collect()
}

/// Paints the node of `icon` scaled to fit `bounds`, centered.
fn paint_icon(
    icon: &Scene,
    bounds: Bounds<Pixels>,
    font: &Font,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let Some((lo, hi)) = icon.bounds(Owner::Node(0)) else {
        return;
    };
    let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
    let zoom = (f32::from(bounds.size.width) / w).min(f32::from(bounds.size.height) / h);
    let at = Placement {
        origin: point(
            bounds.origin.x + (bounds.size.width - px(w * zoom)) / 2.0 - px(lo.x * zoom),
            bounds.origin.y + (bounds.size.height - px(h * zoom)) / 2.0 - px(lo.y * zoom),
        ),
        zoom,
    };
    paint::paint(icon, &at, font, window, cx);
}

/// The bar: a row of buttons that add a node of their shape, linked from the selected node.
pub(crate) fn bar(
    icons: Rc<Vec<Scene>>,
    font: Font,
    background: gpui::Hsla,
    border: gpui::Hsla,
    cx: &Context<Viewer>,
) -> impl IntoElement + use<> {
    let buttons = SHAPES.iter().enumerate().map(|(i, &shape)| {
        let (icons, font) = (icons.clone(), font.clone());
        Button::new(("shape", i))
            .ghost()
            .compact()
            .tooltip(format!("Add {}", shape.title().to_lowercase()))
            .on_click(cx.listener(move |v, _, window, cx| {
                window.focus(&v.drawing_focus, cx);
                v.add_shape(&AddShape { shape }, window, cx);
            }))
            .child(
                div()
                    .debug_selector(|| format!("shape-{i}"))
                    .w(px(ICON.0))
                    .h(px(ICON.1))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, cx| {
                                paint_icon(&icons[i], bounds, &font, window, cx)
                            },
                        )
                        .size_full(),
                    ),
            )
    });
    div()
        .id("shape-bar")
        .absolute()
        .top(px(8.0))
        .left(px(8.0))
        .flex()
        .flex_row()
        .gap_0p5()
        .p_0p5()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(background)
        // Presses on the bar are not presses on the drawing.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .children(buttons)
}
