//! Helpers shared by the gpui tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gpui::{Entity, TestAppContext, VisualTestContext};
use sirene_core::scene::Primitive;
use sirene_gui::document::Document;
use sirene_gui::viewer::Viewer;

/// The platform's secondary modifier, as written in keystrokes.
pub const SECONDARY: &str = if cfg!(target_os = "macos") {
    "cmd"
} else {
    "ctrl"
};

pub fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, src).expect("write fixture");
    path
}

/// Opens `doc` in a real window (with GPUI Kit's `Root`).
pub fn open(
    cx: &mut TestAppContext,
    doc: Document,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    cx.update(sirene_gui::init);
    open_another(cx, doc)
}

/// Opens one more window, after `open`.
pub fn open_another(
    cx: &mut TestAppContext,
    doc: Document,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (handle, viewer) = cx
        .update(|cx| sirene_gui::open_window(doc, cx))
        .expect("open a window");
    let vcx = VisualTestContext::from_window(handle, cx).into_mut();
    vcx.run_until_parked();
    (viewer, vcx)
}

/// The labels in the drawing, in paint order.
pub fn texts(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Vec<String> {
    let scene = cx.update(|window, cx| viewer.update(cx, |v, _| v.scene(window)));
    scene
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.clone()),
            Primitive::Path { .. } => None,
        })
        .collect()
}

/// The source pane's text.
pub fn source(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| viewer.read(cx).source.read(cx).value().to_string())
}
