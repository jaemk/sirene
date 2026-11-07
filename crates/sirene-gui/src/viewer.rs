//! A window's document: the source pane beside the drawing (GUISHE-2, WYSIWY-1..6).

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::base::input::{Diagnostic, DiagnosticSeverity};
use gpui::component::input::{Editor, EditorState, Escape, Input, InputEvent, Position, RopeExt};
use gpui::component::{Theme as KitTheme, ThemeMode};
use gpui::{
    Bounds, Context, Entity, FocusHandle, MouseButton, Pixels, Point, PromptLevel, Render,
    ScrollEvent, Subscription, Task, Window, WindowAppearance, canvas, div, prelude::*, px,
    relative,
};
use sirene_core::Severity;
use sirene_core::scene::{self, Owner, Scene, Theme};

use crate::document::{Disk, Document};
use crate::drawing::{Drag, Item, LabelEdit};
use crate::paint::{self, Metrics, Placement};
use crate::{
    ActualSize, CloseWindow, FitWindow, Open, Reload, Save, SaveAs, ToggleSource, ZoomIn, ZoomOut,
};

/// How often the file is checked for changes.
pub const POLL: Duration = Duration::from_millis(500);
pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 8.0;
const ZOOM_STEP: f32 = 1.25;

pub struct Viewer {
    pub doc: Document,
    /// The source pane's editor.
    pub source: Entity<EditorState>,
    pub show_source: bool,
    pub zoom: f32,
    /// Offset of the drawing from its resting place.
    pub pan: Point<Pixels>,
    pub dark: bool,
    /// Selected nodes and edges, in selection order (WYSIWY-9).
    pub selection: Vec<Item>,
    /// The open label field (WYSIWY-10).
    pub label: Option<LabelEdit>,
    pub(crate) drag: Option<Drag>,
    /// Drawing changes as (before, after) texts, for Undo and Redo (WYSIWY-13).
    pub(crate) history: Vec<(String, String)>,
    pub(crate) future: Vec<(String, String)>,
    scene: Option<Rc<Scene>>,
    /// The drawing area from the last paint.
    pub(crate) viewport: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    pub(crate) drawing_focus: FocusHandle,
    _watch: Task<()>,
    _subscriptions: Vec<Subscription>,
}

/// Where a drawing lands in `bounds`: centered when it fits, else at the top left, then moved
/// by `pan`.
pub(crate) fn place(
    scene: &Scene,
    zoom: f32,
    pan: Point<Pixels>,
    bounds: Bounds<Pixels>,
) -> Placement {
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
        let text = doc.source.clone();
        let source = cx.new(|cx| EditorState::new(window, cx).default_value(text));
        let edits = cx.subscribe_in(&source, window, |v, _, e: &InputEvent, window, cx| {
            if let InputEvent::Change = e {
                v.edited(window, cx);
            }
        });
        let appearance = cx.observe_window_appearance(window, |v, window, cx| {
            v.set_dark(is_dark(window.appearance()), cx);
        });
        let watch = cx.spawn_in(window, async move |this, cx| {
            while this
                .update_in(cx, |v, window, cx| v.poll(window, cx))
                .is_ok()
            {
                cx.background_executor().timer(POLL).await;
            }
        });
        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            this.update(cx, |v, cx| v.should_close(window, cx))
                .unwrap_or(true)
        });
        source.update(cx, |s, cx| s.focus(window, cx));
        let dark = is_dark(window.appearance());
        KitTheme::change(theme_mode(dark), Some(window), cx);
        let mut viewer = Self {
            doc,
            source,
            show_source: true,
            zoom: 1.0,
            pan: Point::default(),
            dark,
            selection: Vec::new(),
            label: None,
            drag: None,
            history: Vec::new(),
            future: Vec::new(),
            scene: None,
            viewport: Rc::default(),
            focus: cx.focus_handle(),
            drawing_focus: cx.focus_handle(),
            _watch: watch,
            _subscriptions: vec![edits, appearance],
        };
        viewer.refresh(window, cx);
        viewer
    }

    /// Takes the editor's text after an edit (WYSIWY-2).
    pub(crate) fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.source.read(cx).value().to_string();
        if self.doc.set_source(text) {
            self.refresh(window, cx);
        }
    }

    /// Redraws after the document changed: drawing, diagnostics, and title.
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scene = None;
        self.prune();
        let problems = &self.doc.problems;
        self.source.update(cx, |s, cx| {
            let text = s.text().clone();
            if let Some(set) = s.diagnostics_mut() {
                set.reset(&text);
                for p in problems {
                    let start = text.offset_to_position(p.range.start);
                    let mut end = text.offset_to_position(p.range.end);
                    if end == start {
                        end.character += 1;
                    }
                    let severity = match p.severity {
                        Severity::Error => DiagnosticSeverity::Error,
                        Severity::Warning => DiagnosticSeverity::Warning,
                    };
                    set.push(
                        Diagnostic::new(start..end, p.message.clone()).with_severity(severity),
                    );
                }
            }
            cx.notify();
        });
        window.set_window_title(&self.doc.title());
        cx.notify();
    }

    /// Puts the document's text in the editor, keeping the cursor line where it still exists.
    fn load_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.doc.source.clone();
        self.source.update(cx, |s, cx| {
            if s.value().as_ref() == text {
                return;
            }
            let line = s.cursor_position().line;
            s.set_value(text, window, cx);
            let last = s.text().lines_len().saturating_sub(1) as u32;
            s.set_cursor_position(Position::new(line.min(last), 0), window, cx);
        });
        self.refresh(window, cx);
    }

    /// Checks the file for changes (WYSIWY-5).
    pub fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.doc.poll() {
            Disk::Unchanged => {}
            Disk::Reloaded => self.load_source(window, cx),
            Disk::Conflict => cx.notify(),
        }
    }

    /// Shows `doc` in place of the current document.
    pub fn set_document(&mut self, doc: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.doc = doc;
        self.zoom = 1.0;
        self.pan = Point::default();
        self.selection.clear();
        self.label = None;
        self.history.clear();
        self.future.clear();
        self.load_source(window, cx);
    }

    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if dark != self.dark {
            self.dark = dark;
            self.scene = None;
            KitTheme::change(theme_mode(dark), None, cx);
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

    /// Re-reads the file, dropping unsaved edits.
    fn reload(&mut self, _: &Reload, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.reload();
        self.load_source(window, cx);
    }

    fn toggle_source(&mut self, _: &ToggleSource, window: &mut Window, cx: &mut Context<Self>) {
        self.show_source = !self.show_source;
        if self.show_source {
            self.source.update(cx, |s, cx| s.focus(window, cx));
        } else {
            window.focus(&self.drawing_focus, cx);
        }
        cx.notify();
    }

    /// Saves to the file, asking for one first when there is none (WYSIWY-4). Resolves to
    /// whether the document was written.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        if self.doc.path.is_none() {
            return self.save_as(window, cx);
        }
        let saved = self.doc.save().is_ok();
        self.refresh(window, cx);
        Task::ready(saved)
    }

    /// Asks for a path and saves there.
    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let dir = self.doc.path.as_deref().and_then(Path::parent).map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            Path::to_owned,
        );
        let name = match &self.doc.path {
            Some(_) => self.doc.name(),
            None => "diagram.mmd".to_owned(),
        };
        let chosen = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return false;
            };
            this.update_in(cx, |v, window, cx| {
                let saved = v.doc.save_as(path).is_ok();
                v.refresh(window, cx);
                saved
            })
            .unwrap_or(false)
        })
    }

    fn save_action(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx).detach();
    }

    fn save_as_action(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save_as(window, cx).detach();
    }

    /// Asks whether to save unsaved edits (WYSIWY-6). Resolves to whether the window may
    /// close.
    pub fn confirm_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        if !self.doc.is_edited() {
            return Task::ready(true);
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Save changes to {}?", self.doc.name()),
            Some("Your changes will be lost if you don't save them."),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => match this.update_in(cx, |v, window, cx| v.save(window, cx)) {
                Ok(saved) => saved.await,
                Err(_) => false,
            },
            Ok(1) => true,
            _ => false,
        })
    }

    /// Closes the window once unsaved edits are dealt with.
    fn close_when_confirmed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let confirmed = self.confirm_close(window, cx);
        cx.spawn_in(window, async move |_, cx| {
            if confirmed.await {
                let _ = cx.update(|window, _| window.remove_window());
            }
        })
        .detach();
    }

    fn close(&mut self, _: &CloseWindow, window: &mut Window, cx: &mut Context<Self>) {
        self.close_when_confirmed(window, cx);
    }

    /// The platform asks before closing the window, e.g. from its title bar.
    pub fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.doc.is_edited() {
            return true;
        }
        self.close_when_confirmed(window, cx);
        false
    }

    /// An unedited Untitled window shows the first chosen file itself; otherwise the app opens
    /// new windows.
    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_blank() {
            cx.propagate();
            return;
        }
        let chosen = cx.prompt_for_paths(crate::open_options());
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                let _ = this.update_in(cx, |v, window, cx| v.show_paths(&paths, window, cx));
            }
        })
        .detach();
    }

    /// Whether this window shows an unedited Untitled document.
    pub fn is_blank(&self) -> bool {
        self.doc.path.is_none() && !self.doc.is_edited()
    }

    /// Shows the first path here when this window is blank, and opens the rest in new windows.
    pub fn show_paths(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        let rest = match paths.split_first() {
            Some((first, rest)) if self.is_blank() => {
                self.set_document(Document::open(first), window, cx);
                rest
            }
            _ => paths,
        };
        crate::open_paths(rest, cx);
    }

    fn scroll(&mut self, e: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = e.delta.pixel_delta(px(20.0));
        if e.modifiers.secondary(cx) {
            let factor = (-f32::from(delta.y) / 200.0).exp();
            self.set_zoom(self.zoom * factor, cx);
        } else {
            self.pan += delta;
            cx.notify();
        }
    }

    pub fn status(&self) -> String {
        self.doc.status()
    }
}

fn theme_mode(dark: bool) -> ThemeMode {
    if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme();
        let scene = self.scene(window);
        let (zoom, pan) = (self.zoom, self.pan);
        let viewport = self.viewport.clone();
        let font = window.text_style().font();
        let rule = paint::color(theme.cluster_stroke);
        let accent = paint::color(theme.accent);
        let selected: Vec<Owner> = self
            .selection
            .iter()
            .filter_map(|i| self.owner(i))
            .collect();
        let band = self.rubber_band(window);
        let field = self.label_field(window, px(160.0));
        let drawing = div()
            .id("drawing")
            .track_focus(&self.drawing_focus)
            .key_context("Drawing")
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::rename))
            .on_action(cx.listener(Self::deselect))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .on_scroll(cx.listener(Self::scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .child(
                canvas(
                    move |bounds, _, _| viewport.set(bounds),
                    move |bounds, (), window, cx| {
                        let at = place(&scene, zoom, pan, bounds);
                        paint::paint(&scene, &at, &font, window, cx);
                        paint::outline(&scene, &selected, &at, accent, window);
                        if let Some((from, to)) = band {
                            paint::line(from, to, accent, window);
                        }
                    },
                )
                .size_full(),
            )
            .children(field.map(|(input, at, width)| {
                div()
                    .absolute()
                    .left(at.x)
                    .top(at.y)
                    .w(width)
                    .on_action(cx.listener(|v, _: &Escape, window, cx| v.cancel_label(window, cx)))
                    .child(Input::new(&input))
            }));
        let body = div().flex().flex_row().flex_1().min_h_0().w_full();
        let body = if self.show_source {
            body.child(
                div()
                    .id("source")
                    .w(relative(0.4))
                    .h_full()
                    .border_r_1()
                    .border_color(rule)
                    .child(Editor::new(&self.source).bordered(false).size_full()),
            )
        } else {
            body
        };
        div()
            .track_focus(&self.focus)
            .key_context("Viewer")
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_window))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::toggle_source))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::save_as_action))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::add_node))
            .size_full()
            .flex()
            .flex_col()
            .bg(paint::color(theme.background))
            .text_color(paint::color(theme.text))
            .child(body.child(drawing))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .border_t_1()
                    .border_color(rule)
                    .child(self.status())
                    .child(format!("{:.0}%", self.zoom * 100.0)),
            )
    }
}
