//! A window's view of one diagram (GUISHE-2).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Bounds, Context, FocusHandle, PathPromptOptions, Pixels, Point, Render, ScrollWheelEvent,
    Subscription, Task, Window, WindowAppearance, canvas, div, prelude::*, px,
};
use sirene_core::scene::{self, Scene, Theme};

use crate::document::Document;
use crate::paint::{self, Metrics, Placement};
use crate::{ActualSize, CloseWindow, FitWindow, Open, Reload, ZoomIn, ZoomOut};

/// How often the file is checked for changes.
pub const POLL: Duration = Duration::from_millis(500);
pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 8.0;
const ZOOM_STEP: f32 = 1.25;

pub struct Viewer {
    pub doc: Document,
    pub zoom: f32,
    /// Offset of the drawing from its resting place.
    pub pan: Point<Pixels>,
    pub dark: bool,
    scene: Option<Rc<Scene>>,
    /// The drawing area from the last paint.
    viewport: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    _watch: Task<()>,
    _appearance: Subscription,
}

/// Where a drawing lands in `bounds`: centered when it fits, else at the top left, then moved
/// by `pan`.
fn place(scene: &Scene, zoom: f32, pan: Point<Pixels>, bounds: Bounds<Pixels>) -> Placement {
    let free_x = (bounds.size.width - px(scene.width * zoom)).max(px(0.0));
    let free_y = (bounds.size.height - px(scene.height * zoom)).max(px(0.0));
    Placement {
        origin: bounds.origin + Point::new(free_x / 2.0, free_y / 2.0) + pan,
        zoom,
    }
}

fn is_dark(a: WindowAppearance) -> bool {
    matches!(a, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

impl Viewer {
    pub fn new(doc: Document, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        window.set_window_title(&doc.name());
        let watch = cx.spawn(async move |this, cx| {
            while this.update(cx, |v, cx| v.poll(cx)).is_ok() {
                cx.background_executor().timer(POLL).await;
            }
        });
        let appearance = cx.observe_window_appearance(window, |v, window, cx| {
            v.set_dark(is_dark(window.appearance()), cx);
        });
        Self {
            doc,
            zoom: 1.0,
            pan: Point::default(),
            dark: is_dark(window.appearance()),
            scene: None,
            viewport: Rc::default(),
            focus,
            _watch: watch,
            _appearance: appearance,
        }
    }

    /// Reloads the document if its file changed.
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        if self.doc.reload() {
            self.scene = None;
            cx.notify();
        }
    }

    /// Shows `doc` in place of the current document.
    pub fn set_document(&mut self, doc: Document, window: &mut Window, cx: &mut Context<Self>) {
        window.set_window_title(&doc.name());
        self.doc = doc;
        self.scene = None;
        self.zoom = 1.0;
        self.pan = Point::default();
        cx.notify();
    }

    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if dark != self.dark {
            self.dark = dark;
            self.scene = None;
            cx.notify();
        }
    }

    pub fn theme(&self) -> Theme {
        if self.dark { Theme::DARK } else { Theme::LIGHT }
    }

    /// The drawing, built on first use after a change.
    pub fn scene(&mut self, window: &Window) -> Rc<Scene> {
        let theme = self.theme();
        let doc = &self.doc;
        self.scene
            .get_or_insert_with(|| {
                let metrics = Metrics {
                    window,
                    font: window.text_style().font(),
                };
                Rc::new(scene::flowchart(&doc.chart, &metrics, &theme))
            })
            .clone()
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        cx.notify();
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom * ZOOM_STEP, cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom / ZOOM_STEP, cx);
    }

    fn actual_size(&mut self, _: &ActualSize, _: &mut Window, cx: &mut Context<Self>) {
        self.pan = Point::default();
        self.set_zoom(1.0, cx);
    }

    /// Zooms so the whole drawing fits the last painted drawing area.
    fn fit_window(&mut self, _: &FitWindow, window: &mut Window, cx: &mut Context<Self>) {
        let scene = self.scene(window);
        let view = self.viewport.get().size;
        if scene.width > 0.0 && scene.height > 0.0 && view.width > px(0.0) {
            let fit =
                (f32::from(view.width) / scene.width).min(f32::from(view.height) / scene.height);
            self.pan = Point::default();
            self.set_zoom(fit, cx);
        }
    }

    fn reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        self.poll(cx);
    }

    fn close(&mut self, _: &CloseWindow, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    /// An empty window shows the first chosen file itself; otherwise the app opens new
    /// windows.
    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        if self.doc.path.is_some() {
            cx.propagate();
            return;
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                let _ = this.update_in(cx, |v, window, cx| v.show_paths(&paths, window, cx));
            }
        })
        .detach();
    }

    /// Shows the first path here when this window is empty, and opens the rest in new windows.
    pub fn show_paths(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        let rest = match paths.split_first() {
            Some((first, rest)) if self.doc.path.is_none() => {
                self.set_document(Document::open(first), window, cx);
                rest
            }
            _ => paths,
        };
        crate::open_paths(rest, cx);
    }

    fn scroll(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = e.delta.pixel_delta(px(20.0));
        if e.modifiers.secondary() {
            let factor = (-f32::from(delta.y) / 200.0).exp();
            self.set_zoom(self.zoom * factor, cx);
        } else {
            self.pan += delta;
            cx.notify();
        }
    }

    pub fn status(&self) -> String {
        self.doc.status().unwrap_or_else(|| self.doc.name())
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme();
        let scene = self.scene(window);
        let (zoom, pan) = (self.zoom, self.pan);
        let viewport = self.viewport.clone();
        let font = window.text_style().font();
        let empty = self.doc.path.is_none();
        let area = div()
            .id("drawing")
            .flex_1()
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(Self::scroll))
            .child(
                canvas(
                    move |bounds, _, _| viewport.set(bounds),
                    move |bounds, (), window, cx| {
                        let at = place(&scene, zoom, pan, bounds);
                        paint::paint(&scene, &at, &font, window, cx);
                    },
                )
                .size_full(),
            );
        let area = if empty {
            area.flex()
                .items_center()
                .justify_center()
                .child("Open a Mermaid file to view it.")
        } else {
            area
        };
        div()
            .track_focus(&self.focus)
            .key_context("Viewer")
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_window))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::open))
            .size_full()
            .flex()
            .flex_col()
            .bg(paint::color(theme.background))
            .text_color(paint::color(theme.text))
            .child(area)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .border_t_1()
                    .border_color(paint::color(theme.cluster_stroke))
                    .child(self.status())
                    .child(format!("{:.0}%", self.zoom * 100.0)),
            )
    }
}
