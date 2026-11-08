//! The styles panel: the document's classes, applied to the selected nodes, and an editor for
//! one class's fill, stroke, text color, line width, and dashes (WYSIWY-17).

use gpui::component::button::{Button, ButtonVariants};
use gpui::component::checkbox::Checkbox;
use gpui::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui::{
    AppContext, Context, Entity, Hsla, IntoElement, Render, Rgba, Subscription, WeakEntity, Window,
    div, prelude::*, px,
};
use sirene_core::flowchart::Flowchart;
use sirene_core::flowchart::edit::{self, TextEdit};
use sirene_core::scene::parse_color;

use crate::paint;
use crate::viewer::Viewer;

/// The color rows: label and the property they set.
pub const COLORS: [(&str, &str); 3] = [("Fill", "fill"), ("Stroke", "stroke"), ("Text", "color")];
/// The line widths offered, in pixels.
pub const WIDTHS: [u8; 4] = [1, 2, 3, 4];
/// The dash pattern written for Dashed.
pub const DASH: &str = "5 5";
/// The declarations of a class made by New Class.
pub const NEW_CLASS: [(&str, &str); 2] = [("fill", "#e8f1fb"), ("stroke", "#3b6ea5")];

pub struct StylePanel {
    viewer: WeakEntity<Viewer>,
    /// The class being edited.
    pub editing: Option<String>,
    /// One picker per [`COLORS`] row.
    pickers: Vec<Entity<ColorPickerState>>,
    /// Set while the pickers are being filled from the source, so they write nothing back.
    syncing: bool,
    _subscriptions: Vec<Subscription>,
}

/// `c` as `#rrggbb`, or `#rrggbbaa` when it is translucent.
pub fn hex(c: Hsla) -> String {
    let c = Rgba::from(c);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte(c.r), byte(c.g), byte(c.b), byte(c.a));
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// The last value class `name` gives `property`.
fn class_value(chart: &Flowchart, name: &str, property: &str) -> Option<String> {
    chart
        .classes
        .iter()
        .filter(|c| c.name == name)
        .flat_map(|c| &c.styles)
        .rfind(|s| s.property == property)
        .map(|s| s.value.clone())
}

impl StylePanel {
    pub fn new(viewer: WeakEntity<Viewer>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut pickers = Vec::new();
        let mut subscriptions = Vec::new();
        for (_, property) in COLORS {
            let picker = cx.new(|cx| ColorPickerState::new(window, cx));
            subscriptions.push(cx.subscribe_in(
                &picker,
                window,
                move |this, _, e: &ColorPickerEvent, window, cx| {
                    let ColorPickerEvent::Change(color) = e;
                    this.picked(property, *color, window, cx);
                },
            ));
            pickers.push(picker);
        }
        Self {
            viewer,
            editing: None,
            pickers,
            syncing: false,
            _subscriptions: subscriptions,
        }
    }

    /// The color picker of each [`COLORS`] row.
    pub fn pickers(&self) -> &[Entity<ColorPickerState>] {
        &self.pickers
    }

    fn picked(
        &mut self,
        property: &str,
        color: Option<Hsla>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let value = color.map(hex);
        self.set(property, value.as_deref(), window, cx);
    }

    /// Sets `property` of the class being edited, as one drawing change.
    pub fn set(
        &mut self,
        property: &str,
        value: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.editing.clone() else {
            return;
        };
        let property = property.to_owned();
        let value = value.map(str::to_owned);
        self.viewer
            .update(cx, |v, cx| {
                let edit = edit::set_class_style(&v.doc.source, &name, &property, value.as_deref());
                if let Some(edit) = edit {
                    v.apply(edit, window, cx);
                }
            })
            .ok();
        cx.notify();
    }

    /// Edits class `name`, filling the pickers from its declarations.
    pub fn edit(&mut self, name: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewer) = self.viewer.upgrade() else {
            return;
        };
        let chart = viewer.read(cx).doc.chart.clone();
        self.edit_in(name, &chart, window, cx);
    }

    /// Edits class `name` of `chart`; for the viewer, which cannot be read while it updates.
    pub(crate) fn edit_in(
        &mut self,
        name: Option<String>,
        chart: &Flowchart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing = name;
        self.syncing = true;
        for (picker, (_, property)) in self.pickers.clone().iter().zip(COLORS) {
            let color = self
                .editing
                .as_deref()
                .and_then(|n| class_value(chart, n, property))
                .and_then(|v| parse_color(&v))
                .map(paint::color);
            picker.update(cx, |p, cx| match color {
                Some(c) => p.set_value(c, window, cx),
                None => p.clear_value(window, cx),
            });
        }
        self.syncing = false;
        cx.notify();
    }

    /// Gives the selected nodes class `name`, or takes it off them.
    pub fn toggle(&mut self, name: &str, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_owned();
        self.viewer
            .update(cx, |v, cx| {
                let nodes = v.selected_nodes();
                let ids: Vec<&str> = nodes.iter().map(String::as_str).collect();
                let src = &v.doc.source;
                let edit = if on {
                    edit::apply_class(src, &ids, &name)
                } else {
                    edit::remove_class(src, &ids, &name)
                };
                if let Some(edit) = edit {
                    v.apply(edit, window, cx);
                }
            })
            .ok();
        cx.notify();
    }

    /// Defines a new class, gives it to the selected nodes, and edits it, as one change.
    pub fn new_class(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self
            .viewer
            .update(cx, |v, cx| {
                let src = v.doc.source.clone();
                let name = edit::fresh_class_name(&src);
                let mut out = src.clone();
                for (property, value) in NEW_CLASS {
                    if let Some(e) = edit::set_class_style(&out, &name, property, Some(value)) {
                        out = e.apply(&out);
                    }
                }
                let nodes = v.selected_nodes();
                let ids: Vec<&str> = nodes.iter().map(String::as_str).collect();
                if let Some(e) = edit::apply_class(&out, &ids, &name) {
                    out = e.apply(&out);
                }
                if let Some(edit) = TextEdit::between(&src, &out) {
                    v.apply(edit, window, cx);
                }
                name
            })
            .ok()
        else {
            return;
        };
        self.edit(Some(name), window, cx);
    }
}

impl Render for StylePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(viewer) = self.viewer.upgrade() else {
            return div();
        };
        let (chart, nodes, dark) = {
            let v = viewer.read(cx);
            (v.doc.chart.clone(), v.selected_nodes(), v.dark)
        };
        let rule = paint::color(if dark {
            sirene_core::scene::Theme::DARK.cluster_stroke
        } else {
            sirene_core::scene::Theme::LIGHT.cluster_stroke
        });
        let mut names: Vec<String> = Vec::new();
        for c in &chart.classes {
            if !names.contains(&c.name) {
                names.push(c.name.clone());
            }
        }
        let has = |id: &String, name: &str| {
            chart
                .node(id)
                .is_some_and(|n| n.classes.iter().any(|c| c == name))
        };
        let rows = names.iter().enumerate().map(|(i, name)| {
            let on = !nodes.is_empty() && nodes.iter().all(|id| has(id, name));
            let swatch = |property| {
                class_value(&chart, name, property)
                    .and_then(|v| parse_color(&v))
                    .map(paint::color)
            };
            let (fill, stroke) = (swatch("fill"), swatch("stroke"));
            let editing = self.editing.as_deref() == Some(name.as_str());
            let (toggle_name, edit_name) = (name.clone(), name.clone());
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div().debug_selector(|| format!("class-{i}")).child(
                        Checkbox::new(("class", i))
                            .checked(on)
                            .label(name.clone())
                            .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                                this.toggle(&toggle_name, *checked, window, cx);
                            })),
                    ),
                )
                .child(
                    div()
                        .size(px(14.0))
                        .rounded_sm()
                        .border_2()
                        .border_color(stroke.unwrap_or(rule))
                        .when_some(fill, |d, f| d.bg(f)),
                )
                .child(div().flex_1())
                .child(
                    div().debug_selector(|| format!("edit-class-{i}")).child(
                        Button::new(("edit-class", i))
                            .ghost()
                            .compact()
                            .label("Edit")
                            .toggled(editing)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(Some(edit_name.clone()), window, cx);
                            })),
                    ),
                )
        });
        let mut panel = div()
            .id("styles")
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .size_full()
            .overflow_y_scroll()
            .text_sm()
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Classes"),
            )
            .children(rows)
            .child(
                div().debug_selector(|| "new-class".into()).child(
                    Button::new("new-class")
                        .ghost()
                        .compact()
                        .label("New Class")
                        .on_click(cx.listener(|this, _, window, cx| this.new_class(window, cx))),
                ),
            );
        if let Some(name) = self.editing.clone() {
            let width = class_value(&chart, &name, "stroke-width");
            let dashed = class_value(&chart, &name, "stroke-dasharray").is_some();
            let colors = COLORS
                .iter()
                .zip(&self.pickers)
                .map(|(&(label, _), picker)| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(label)
                        .child(ColorPicker::new(picker))
                });
            let widths = WIDTHS.iter().map(|&w| {
                let value = format!("{w}px");
                let on = width.as_deref() == Some(value.as_str());
                div().debug_selector(move || format!("width-{w}")).child(
                    Button::new(("width", usize::from(w)))
                        .compact()
                        .outline()
                        .label(w.to_string())
                        .toggled(on)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set("stroke-width", Some(&value), window, cx);
                        })),
                )
            });
            let dashes = [("Solid", None), ("Dashed", Some(DASH))].map(|(label, value)| {
                div().debug_selector(move || label.to_lowercase()).child(
                    Button::new(label)
                        .compact()
                        .outline()
                        .label(label)
                        .toggled(dashed == value.is_some())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set("stroke-dasharray", value, window, cx);
                        })),
                )
            });
            panel = panel
                .child(div().h(px(1.0)).bg(rule))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("Class {name}")),
                )
                .children(colors)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child("Width")
                        .child(div().flex().flex_row().gap_1().children(widths)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child("Line")
                        .child(div().flex().flex_row().gap_1().children(dashes)),
                );
        }
        div().size_full().child(panel)
    }
}
