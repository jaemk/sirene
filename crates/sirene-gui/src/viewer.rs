//! A window's document: the source pane beside the drawing (GUISHE-2, WYSIWY-1..6).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::base::input::{Diagnostic, DiagnosticSeverity};
use gpui::component::input::{
    self as text, Editor, EditorState, Escape, Input, InputEvent, Position, RopeExt,
};
use gpui::component::menu::{AppMenuBar, ContextMenuExt};
use gpui::component::resizable::{ResizableState, h_resizable, resizable_panel};
use gpui::component::{Theme as KitTheme, ThemeMode};
use gpui::{
    App, Bounds, Context, Entity, FocusHandle, MouseButton, Pixels, Point, PromptLevel, Render,
    ScrollEvent, Subscription, Task, Window, WindowAppearance, canvas, div, prelude::*, px,
};
use sirene_core::Severity;
use sirene_core::scene::{self, Owner, Scene, Theme};

use crate::document::{Disk, Document};
use crate::drawing::{Drag, Item, LabelEdit};
use crate::paint::{self, Metrics, Placement};
use crate::settings::{self, Settings};
use crate::{
    ActualSize, CloseWindow, FitWindow, Open, Redo, Reload, Save, SaveAs, SelectAll, ToggleSource,
    Undo, ZoomIn, ZoomOut,
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
    /// The source pane's width, in pixels, as last dragged (GUISHE-11).
    pub source_width: f32,
    panes: Entity<ResizableState>,
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
    /// Fit the drawing once the drawing area is known (GUISHE-8).
    pub(crate) fit_pending: bool,
    /// The outcome of the last export or copy, shown in the status bar until the document
    /// changes (GUISHE-9).
    pub notice: Option<String>,
    /// The menus drawn in the window, where the platform has no menu bar (GUISHE-12).
    pub menu_bar: Option<Entity<AppMenuBar>>,
    /// The recent files the menu bar was last built with.
    menu_recent: Vec<PathBuf>,
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

/// Whether `window` draws dark: the Appearance setting, or the system's appearance.
fn effective_dark(window: &Window, cx: &App) -> bool {
    settings::get(cx)
        .appearance
        .is_dark(is_dark(window.appearance()))
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
            v.set_dark(effective_dark(window, cx), cx);
        });
        // The Appearance setting, and the pane, follow the app's settings (GUISHE-10).
        let chosen = cx.observe_global_in::<Settings>(window, |v, window, cx| {
            v.set_dark(effective_dark(window, cx), cx);
            // Open Recent changed: rebuild the window's menus.
            let recent = &settings::get(cx).recent;
            if let Some(bar) = &v.menu_bar
                && v.menu_recent != *recent
            {
                v.menu_recent = recent.clone();
                bar.update(cx, |bar, cx| bar.reload(cx));
            }
        });
        let resized = cx.observe_window_bounds(window, |_, window, cx| {
            let size = window.window_bounds().get_bounds().size;
            let size = (f32::from(size.width), f32::from(size.height));
            settings::remember(cx, |s| s.window = Some(size));
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
        let dark = effective_dark(window, cx);
        KitTheme::change(theme_mode(dark), Some(window), cx);
        let remembered = settings::get(cx);
        let show_source = remembered.show_source;
        // A pane remembered from a wider window still leaves room for the drawing.
        let source_width = settings::source_width_in(
            remembered.source_width,
            f32::from(window.viewport_size().width),
        );
        let panes = cx.new(|_| ResizableState::default());
        let menu_bar = crate::IN_WINDOW_MENUS.then(|| AppMenuBar::new(cx));
        let menu_recent = settings::get(cx).recent.clone();
        let mut viewer = Self {
            doc,
            source,
            show_source,
            source_width,
            panes,
            zoom: 1.0,
            pan: Point::default(),
            dark,
            selection: Vec::new(),
            label: None,
            drag: None,
            history: Vec::new(),
            future: Vec::new(),
            fit_pending: true,
            notice: None,
            menu_bar,
            menu_recent,
            scene: None,
            viewport: Rc::default(),
            focus: cx.focus_handle(),
            drawing_focus: cx.focus_handle(),
            _watch: watch,
            _subscriptions: vec![edits, appearance, chosen, resized],
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
        self.notice = None;
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
        self.fit_pending = true;
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

    /// Re-reads the file, asking first when that would drop unsaved edits.
    fn reload(&mut self, _: &Reload, window: &mut Window, cx: &mut Context<Self>) {
        if !self.doc.is_edited() {
            self.reload_now(window, cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Discard your changes to {}?", self.doc.name()),
            Some("Reload replaces them with the file's text."),
            &["Reload", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(0) = answer.await {
                log::debug!("reload: discarding unsaved edits");
                let _ = this.update_in(cx, |v, window, cx| v.reload_now(window, cx));
            } else {
                log::debug!("reload cancelled");
            }
        })
        .detach();
    }

    fn reload_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.reload();
        self.load_source(window, cx);
    }

    fn toggle_source(&mut self, _: &ToggleSource, window: &mut Window, cx: &mut Context<Self>) {
        self.show_source = !self.show_source;
        let shown = self.show_source;
        settings::update(cx, |s| s.show_source = shown);
        if self.show_source {
            self.source.update(cx, |s, cx| s.focus(window, cx));
        } else {
            window.focus(&self.drawing_focus, cx);
        }
        cx.notify();
    }

    /// Saves to the file, asking for one first when there is none, and asking before replacing
    /// changes made on disk (WYSIWY-4). Resolves to whether the document was written.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        if self.doc.path.is_none() {
            return self.save_as(window, cx);
        }
        if !self.doc.disk_changed {
            return Task::ready(self.write_file(window, cx));
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("{} changed on disk. Overwrite it?", self.doc.name()),
            Some("Saving replaces the changes made outside sirene."),
            &["Overwrite", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => {
                log::debug!("save: overwriting the changes made on disk");
                this.update_in(cx, |v, window, cx| v.write_file(window, cx))
                    .unwrap_or(false)
            }
            _ => {
                log::debug!("save cancelled");
                false
            }
        })
    }

    /// Writes the document to its file. Returns whether it was written.
    fn write_file(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let saved = self.doc.save().is_ok();
        self.refresh(window, cx);
        saved
    }

    /// Asks for a path and saves there.
    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let dir = crate::dialog_dir(self.doc.path.as_deref());
        let name = match &self.doc.path {
            Some(_) => self.doc.name(),
            None => "diagram.mmd".to_owned(),
        };
        let chosen = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                log::debug!("save as cancelled");
                return false;
            };
            log::debug!("save as {}", path.display());
            this.update_in(cx, |v, window, cx| {
                let saved = v.doc.save_as(path.clone()).is_ok();
                if saved {
                    crate::note_recent(&path, cx);
                }
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
            Ok(0) => {
                log::debug!("close: saving first");
                match this.update_in(cx, |v, window, cx| v.save(window, cx)) {
                    Ok(saved) => saved.await,
                    Err(_) => false,
                }
            }
            Ok(1) => {
                log::debug!("close: discarding unsaved edits");
                true
            }
            _ => {
                log::debug!("close cancelled");
                false
            }
        })
    }

    /// Closes the window once unsaved edits are dealt with.
    fn close_when_confirmed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let confirmed = self.confirm_close(window, cx);
        cx.spawn_in(window, async move |_, cx| {
            if confirmed.await {
                log::debug!("closing the window");
                let _ = cx.update(|window, cx| {
                    settings::get(cx).save();
                    window.remove_window()
                });
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
            settings::get(cx).save();
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

    /// Shows the first path here when this window is blank and no other window shows it, and
    /// opens the rest as `open_paths` does.
    pub fn show_paths(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        let rest = match paths.split_first() {
            Some((first, rest)) if self.is_blank() && !crate::focus_existing(first, cx) => {
                crate::note_recent(first, cx);
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

    /// The drawing area, in window coordinates, from the last paint.
    pub fn drawing_area(&self) -> Bounds<Pixels> {
        self.viewport.get()
    }

    pub fn status(&self) -> String {
        self.notice.clone().unwrap_or_else(|| self.doc.status())
    }

    /// Whether the drawing has the keyboard; it then shows a focus ring (GUISHE-12).
    pub fn drawing_focused(&self, window: &Window) -> bool {
        self.drawing_focus.is_focused(window)
    }

    // Edit > Undo, Redo, and Select All reach here when the drawing did not take them: the
    // source pane or the label field has the keyboard, so they act on its text (GUISHE-12).

    fn text_undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(text::Undo), cx);
    }

    fn text_redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(text::Redo), cx);
    }

    fn text_select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(text::SelectAll), cx);
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
        let (zoom, pan, fit) = (self.zoom, self.pan, self.fit_pending);
        let viewport = self.viewport.clone();
        let this = cx.entity().downgrade();
        let menu_viewer = this.clone();
        let font = window.text_style().font();
        let rule = paint::color(theme.cluster_stroke);
        let accent = paint::color(theme.accent);
        let selected: Vec<Owner> = self
            .selection
            .iter()
            .filter_map(|i| self.owner(i))
            .collect();
        let band = self.rubber_band(window);
        let ring = self.drawing_focused(window);
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
            .on_action(cx.listener(Self::pan_action))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .on_scroll(cx.listener(Self::scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::right_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .child(
                canvas(
                    {
                        let scene = scene.clone();
                        move |bounds, _, cx| {
                            viewport.set(bounds);
                            // The first paint after a document is shown fits it (GUISHE-8).
                            fit.then(|| {
                                this.update(cx, |v, cx| v.fit_on_open(&scene, bounds.size, cx))
                                    .ok()
                            })
                            .flatten()
                            .unwrap_or((zoom, pan))
                        }
                    },
                    move |bounds, (zoom, pan), window, cx| {
                        let at = place(&scene, zoom, pan, bounds);
                        paint::paint(&scene, &at, &font, window, cx);
                        paint::outline(&scene, &selected, &at, accent, window);
                        if let Some((from, to)) = band {
                            paint::line(from, to, accent, window);
                        }
                        if ring {
                            paint::focus_ring(bounds, accent, window);
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
            }))
            .context_menu(move |menu, window, cx| {
                crate::menu::build(&menu_viewer, menu, window, cx)
            });
        // The source pane is as wide as last dragged, in any window (GUISHE-11).
        let panes = h_resizable("panes")
            .with_state(&self.panes)
            .on_resize(|state, _, cx| {
                if let Some(width) = state.read(cx).sizes().first().copied() {
                    let width = f32::from(width);
                    log::debug!("source pane resized to {width:.0}px");
                    settings::remember(cx, |s| s.source_width = width);
                }
            })
            .child(
                resizable_panel()
                    .size(px(self.source_width))
                    .size_range(px(settings::SOURCE_MIN)..px(4000.0))
                    // Window resizes go to the drawing.
                    .fixed()
                    .visible(self.show_source)
                    .child(
                        div()
                            .id("source")
                            .size_full()
                            .border_r_1()
                            .border_color(rule)
                            .child(Editor::new(&self.source).bordered(false).size_full()),
                    ),
            )
            .child(resizable_panel().child(drawing));
        let body = div().flex().flex_row().flex_1().min_h_0().w_full();
        div()
            .track_focus(&self.focus)
            .key_context("Viewer")
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_window))
            .on_action(cx.listener(Self::zoom_to_selection))
            .on_action(cx.listener(Self::export_png))
            .on_action(cx.listener(Self::export_svg))
            .on_action(cx.listener(Self::copy_image))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::toggle_source))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::save_as_action))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::add_node))
            .on_action(cx.listener(Self::add_node_before))
            .on_action(cx.listener(Self::set_shape))
            .on_action(cx.listener(Self::set_stroke))
            .on_action(cx.listener(Self::set_arrows))
            .on_action(cx.listener(Self::longer_edge))
            .on_action(cx.listener(Self::shorter_edge))
            .on_action(cx.listener(Self::reverse_edge))
            .on_action(cx.listener(Self::connect_to))
            .on_action(cx.listener(Self::connect_from))
            .on_action(cx.listener(Self::text_undo))
            .on_action(cx.listener(Self::text_redo))
            .on_action(cx.listener(Self::text_select_all))
            .size_full()
            .flex()
            .flex_col()
            .bg(paint::color(theme.background))
            .text_color(paint::color(theme.text))
            .children(self.menu_bar.clone().map(|bar| {
                div()
                    .id("menu-bar")
                    .h(px(30.0))
                    .px_1()
                    .flex_none()
                    .border_b_1()
                    .border_color(rule)
                    .child(bar)
            }))
            .child(body.child(panes))
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
