//! The sirene editor: one window per diagram file, with its source beside the drawing
//! (GUISHE-*, WYSIWY-*).

pub mod document;
pub mod drawing;
pub mod paint;
pub mod viewer;

use std::path::{Path, PathBuf};

use futures::StreamExt;
use gpui::{
    AnyWindowHandle, App, Bounds, Entity, KeyBinding, Menu, MenuItem, PathPromptOptions, QuitMode,
    Task, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, prelude::*, px, size,
};

use document::Document;
use viewer::Viewer;

actions!(
    sirene,
    [
        /// Zoom in one step.
        ZoomIn,
        /// Zoom out one step.
        ZoomOut,
        /// Show the drawing at 100%, unpanned.
        ActualSize,
        /// Zoom so the whole drawing fits the window.
        FitWindow,
        /// Re-read the file now, dropping unsaved edits.
        Reload,
        /// Open a window with a new document.
        New,
        /// Choose files to open.
        Open,
        /// Write the document to its file.
        Save,
        /// Write the document to a chosen file.
        SaveAs,
        /// Show or hide the source pane.
        ToggleSource,
        /// Close the focused window.
        CloseWindow,
        /// Quit the application.
        Quit,
        /// Add a node, connected from the selected node.
        AddNode,
        /// Edit the label of the selected item.
        Rename,
        /// Delete the selected nodes and edges.
        Delete,
        /// Clear the selection.
        Deselect,
        /// Undo the last change made on the drawing.
        Undo,
        /// Redo the last change undone on the drawing.
        Redo,
    ]
);

/// A keystroke with the secondary modifier: Cmd on macOS, Ctrl elsewhere.
macro_rules! secondary {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

/// Key bindings (GUISHE-5).
pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new(secondary!("="), ZoomIn, None),
        KeyBinding::new(secondary!("+"), ZoomIn, None),
        KeyBinding::new(secondary!("-"), ZoomOut, None),
        KeyBinding::new(secondary!("0"), ActualSize, None),
        KeyBinding::new(secondary!("9"), FitWindow, None),
        KeyBinding::new(secondary!("r"), Reload, None),
        KeyBinding::new(secondary!("n"), New, None),
        KeyBinding::new(secondary!("o"), Open, None),
        KeyBinding::new(secondary!("s"), Save, None),
        KeyBinding::new(secondary!("shift-s"), SaveAs, None),
        KeyBinding::new(secondary!("e"), ToggleSource, None),
        KeyBinding::new(secondary!("w"), CloseWindow, None),
        KeyBinding::new(secondary!("q"), Quit, None),
        KeyBinding::new(secondary!("shift-n"), AddNode, None),
        KeyBinding::new("enter", Rename, Some("Drawing")),
        KeyBinding::new("backspace", Delete, Some("Drawing")),
        KeyBinding::new("delete", Delete, Some("Drawing")),
        KeyBinding::new("escape", Deselect, Some("Drawing")),
        KeyBinding::new(secondary!("z"), Undo, Some("Drawing")),
        KeyBinding::new(secondary!("shift-z"), Redo, Some("Drawing")),
    ]
}

fn menu(name: &str, items: Vec<MenuItem>) -> Menu {
    Menu {
        name: name.to_owned().into(),
        items,
        disabled: false,
    }
}

pub fn menus() -> Vec<Menu> {
    vec![
        menu("sirene", vec![MenuItem::action("Quit sirene", Quit)]),
        menu(
            "File",
            vec![
                MenuItem::action("New", New),
                MenuItem::action("Open...", Open),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As...", SaveAs),
                MenuItem::action("Reload", Reload),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Add Node", AddNode),
                MenuItem::action("Rename", Rename),
                MenuItem::action("Delete", Delete),
                MenuItem::action("Deselect", Deselect),
            ],
        ),
        menu(
            "View",
            vec![
                MenuItem::action("Show or Hide Source", ToggleSource),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Actual Size", ActualSize),
                MenuItem::action("Fit Window", FitWindow),
            ],
        ),
    ]
}

/// Initializes GPUI Kit and registers key bindings, menus, and app-wide actions.
pub fn init(cx: &mut App) {
    gpui::init(cx);
    cx.bind_keys(key_bindings());
    cx.set_menus(menus());
    cx.on_action(|_: &Quit, cx| {
        let quit = confirm_quit(cx);
        cx.spawn(async move |cx| {
            if quit.await {
                cx.update(|cx| cx.quit());
            }
        })
        .detach();
    });
    cx.on_action(|_: &New, cx| {
        open_or_report(Document::untitled(), cx);
    });
    cx.on_action(|_: &Open, cx| prompt_open(cx));
}

/// Opens a window showing `doc`.
pub fn open_window(
    doc: Document,
    cx: &mut App,
) -> anyhow::Result<(AnyWindowHandle, Entity<Viewer>)> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1200.0), px(760.0)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(doc.title().into()),
            ..Default::default()
        }),
        app_id: Some("sirene".into()),
        ..Default::default()
    };
    gpui::open_window(options, cx, |window, cx| {
        cx.new(|cx| Viewer::new(doc, window, cx))
    })
}

fn open_or_report(doc: Document, cx: &mut App) {
    let name = doc.name();
    if let Err(e) = open_window(doc, cx) {
        eprintln!("error: cannot open a window for {name}: {e}");
    }
}

/// Asks for files and opens each in a new window.
pub fn prompt_open(cx: &mut App) {
    let chosen = cx.prompt_for_paths(open_options());
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = chosen.await {
            cx.update(|cx| open_paths(&paths, cx));
        }
    })
    .detach();
}

pub(crate) fn open_options() -> PathPromptOptions {
    PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: None,
    }
}

/// Opens each path in a new window, or brings forward the window already showing it.
pub fn open_paths(paths: &[PathBuf], cx: &mut App) {
    for p in paths {
        if !focus_existing(p, cx) {
            open_or_report(Document::open(p), cx);
        }
    }
}

/// Brings forward the window showing `path` (compared after resolving links and `..`), if any.
pub fn focus_existing(path: &Path, cx: &mut App) -> bool {
    let Ok(want) = std::fs::canonicalize(path) else {
        return false;
    };
    for handle in cx.windows() {
        let found = handle.update(cx, |_, window, cx| {
            let shows = viewer_in(window, cx).is_some_and(|v| {
                let shown = v.read(cx).doc.path.as_deref();
                shown.and_then(|p| std::fs::canonicalize(p).ok()).as_ref() == Some(&want)
            });
            if shows {
                window.activate_window();
            }
            shows
        });
        if found.unwrap_or(false) {
            return true;
        }
    }
    false
}

/// The editor in `window`, if it shows one.
pub fn viewer_in(window: &Window, cx: &App) -> Option<Entity<Viewer>> {
    gpui::base::Root::read(window, cx)
        .view()
        .clone()
        .downcast::<Viewer>()
        .ok()
}

/// Asks each window with unsaved edits whether to save them, stopping at the first Cancel
/// (WYSIWY-6). Resolves to whether the app may quit.
pub fn confirm_quit(cx: &mut App) -> Task<bool> {
    let windows = cx.windows();
    cx.spawn(async move |cx| {
        for handle in windows {
            let asked = handle.update(cx, |_, window, cx| {
                viewer_in(window, cx).map(|v| v.update(cx, |v, cx| v.confirm_close(window, cx)))
            });
            if let Ok(Some(asked)) = asked
                && !asked.await
            {
                return false;
            }
        }
        true
    })
}

/// The local path of a `file://` URL, percent-decoded. A URL naming another host has none.
pub fn url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let slash = rest.find('/')?;
    if !matches!(&rest[..slash], "" | "localhost") {
        return None;
    }
    let rest = &rest[slash..];
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = match bytes.get(i..i + 3) {
            Some([b'%', h, l]) if h.is_ascii_hexdigit() && l.is_ascii_hexdigit() => {
                let digit = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
                Some(digit(*h) * 16 + digit(*l))
            }
            _ => None,
        };
        match escaped {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    bytes_path(out)
}

/// A path from raw bytes: any bytes on Unix, UTF-8 elsewhere.
#[cfg(unix)]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

/// Runs the app, opening a window per path, or a new document when there are none
/// (GUISHE-1). Files the platform asks to open, e.g. from Finder, open in new windows.
pub fn run(paths: Vec<PathBuf>) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Vec<String>>();
    let app = gpui::application()
        .with_assets(gpui::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed);
    app.on_open_urls(move |urls| {
        let _ = tx.unbounded_send(urls);
    });
    app.run(move |cx| {
        init(cx);
        if paths.is_empty() {
            open_or_report(Document::untitled(), cx);
        } else {
            open_paths(&paths, cx);
        }
        cx.spawn(async move |cx| {
            while let Some(urls) = rx.next().await {
                let paths: Vec<PathBuf> = urls.iter().filter_map(|u| url_path(u)).collect();
                cx.update(|cx| open_paths(&paths, cx));
            }
        })
        .detach();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls() {
        assert_eq!(
            url_path("file:///Users/me/a%20b.mmd"),
            Some(PathBuf::from("/Users/me/a b.mmd"))
        );
        assert_eq!(
            url_path("file://localhost/tmp/%E6%97%A5.mmd"),
            Some(PathBuf::from("/tmp/日.mmd"))
        );
        assert_eq!(url_path("file:///a%2"), Some(PathBuf::from("/a%2")));
        assert_eq!(url_path("file:///a%zz"), Some(PathBuf::from("/a%zz")));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let p = url_path("file:///a%ff").unwrap();
            assert_eq!(p.as_os_str().as_bytes(), b"/a\xff");
        }
        assert_eq!(url_path("file://host"), None);
        assert_eq!(url_path("file://otherhost/etc/a.mmd"), None);
        assert_eq!(url_path("https://example.com/a.mmd"), None);
    }

    #[test]
    fn every_action_has_a_menu_item_and_key() {
        let menus = menus();
        let items = menus
            .iter()
            .flat_map(|m| &m.items)
            .filter(|i| !matches!(i, MenuItem::Separator))
            .count();
        assert_eq!(items, 18);
        assert_eq!(key_bindings().len(), 20);
    }
}
