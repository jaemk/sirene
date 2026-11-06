//! The sirene viewer: one window per diagram file (GUISHE-*).

pub mod document;
pub mod paint;
pub mod viewer;

use std::path::PathBuf;

use futures::StreamExt;
use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, PathPromptOptions, TitlebarOptions,
    WindowBounds, WindowHandle, WindowOptions, actions, prelude::*, px, size,
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
        /// Re-read the file now.
        Reload,
        /// Choose files to open.
        Open,
        /// Close the focused window.
        CloseWindow,
        /// Quit the application.
        Quit,
    ]
);

/// Key bindings; `secondary` is Cmd on macOS and Ctrl elsewhere (GUISHE-5).
pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary-+", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ActualSize, None),
        KeyBinding::new("secondary-9", FitWindow, None),
        KeyBinding::new("secondary-r", Reload, None),
        KeyBinding::new("secondary-o", Open, None),
        KeyBinding::new("secondary-w", CloseWindow, None),
        KeyBinding::new("secondary-q", Quit, None),
    ]
}

pub fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "sirene".into(),
            items: vec![MenuItem::action("Quit sirene", Quit)],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open...", Open),
                MenuItem::action("Reload", Reload),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Actual Size", ActualSize),
                MenuItem::action("Fit Window", FitWindow),
            ],
        },
    ]
}

/// Registers key bindings, menus, and app-wide actions.
pub fn init(cx: &mut App) {
    cx.bind_keys(key_bindings());
    cx.set_menus(menus());
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Open, cx| prompt_open(cx));
}

/// Opens a window showing `doc`.
pub fn open_window(doc: Document, cx: &mut App) -> anyhow::Result<WindowHandle<Viewer>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(960.0), px(720.0)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(doc.name().into()),
            ..Default::default()
        }),
        app_id: Some("sirene".into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Viewer::new(doc, window, cx))
    })
}

/// Asks for files and opens each in a new window.
pub fn prompt_open(cx: &mut App) {
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: None,
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = chosen.await {
            let _ = cx.update(|cx| open_paths(&paths, cx));
        }
    })
    .detach();
}

/// Opens each path in a new window.
pub fn open_paths(paths: &[PathBuf], cx: &mut App) {
    for p in paths {
        if let Err(e) = open_window(Document::open(p), cx) {
            eprintln!("error: cannot open a window for {}: {e}", p.display());
        }
    }
}

/// The local path of a `file://` URL, percent-decoded.
pub fn url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // Skip an authority such as `localhost`.
    let rest = &rest[rest.find('/')?..];
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
    String::from_utf8(out).ok().map(PathBuf::from)
}

/// Runs the app, opening a window per path, or one empty window when there are none
/// (GUISHE-1). Files the platform asks to open, e.g. from Finder, open in new windows.
pub fn run(paths: Vec<PathBuf>) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Vec<String>>();
    let app = Application::new();
    app.on_open_urls(move |urls| {
        let _ = tx.unbounded_send(urls);
    });
    app.run(move |cx| {
        init(cx);
        if paths.is_empty() {
            if let Err(e) = open_window(Document::empty(), cx) {
                eprintln!("error: cannot open a window: {e}");
                cx.quit();
            }
        } else {
            open_paths(&paths, cx);
        }
        cx.spawn(async move |cx| {
            while let Some(urls) = rx.next().await {
                let paths: Vec<PathBuf> = urls.iter().filter_map(|u| url_path(u)).collect();
                let _ = cx.update(|cx| open_paths(&paths, cx));
            }
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
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
        assert_eq!(url_path("file:///%ff"), None);
        assert_eq!(url_path("file://host"), None);
        assert_eq!(url_path("https://example.com/a.mmd"), None);
    }

    #[test]
    fn every_action_has_a_menu_item_and_key() {
        let menus = menus();
        let items: usize = menus.iter().map(|m| m.items.len()).sum();
        assert_eq!(items, 9);
        assert_eq!(key_bindings().len(), 9);
    }
}
