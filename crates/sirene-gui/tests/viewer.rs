//! The window's drawing in gpui's headless test platform (GUISHE-*).

mod common;

use common::{SECONDARY, open, texts, write};
use gpui::{
    Modifiers, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};
use sirene_core::scene::{Primitive, Segment, Theme};
use sirene_gui::document::Document;
use sirene_gui::viewer::{MAX_ZOOM, MIN_ZOOM, POLL};
use sirene_gui::{ActualSize, CloseWindow, FitWindow, Reload};

#[gpui::test]
fn shows_a_file(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA[Alpha] --> B[Beta]\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    assert_eq!(cx.window_title().as_deref(), Some("a.mmd"));
    assert_eq!(texts(&viewer, cx), ["Alpha", "Beta"]);
    viewer.read_with(cx, |v, _| {
        assert_eq!(v.status(), "a.mmd");
        assert_eq!(v.theme(), Theme::LIGHT);
    });
}

#[gpui::test]
fn new_documents_start_with_a_flowchart(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::untitled());
    assert_eq!(cx.window_title().as_deref(), Some("Untitled"));
    assert_eq!(texts(&viewer, cx), ["A", "B"]);
}

#[gpui::test]
fn reloads_when_the_file_changes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA --> B\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    write(dir.path(), "a.mmd", "flowchart LR\nA --> B --> C\n");
    cx.executor().advance_clock(POLL);
    cx.run_until_parked();
    assert_eq!(texts(&viewer, cx), ["A", "B", "C"]);

    write(dir.path(), "a.mmd", "flowchart LR\nlinkStyle 4 color:red\n");
    cx.dispatch_action(Reload);
    cx.run_until_parked();
    let status = viewer.read_with(cx, |v, _| v.status());
    assert!(status.starts_with("2:11: error: "), "{status}");
}

#[gpui::test]
fn zoom_keys(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, "flowchart\nA\n".into()));
    let zoom = |cx: &mut VisualTestContext| viewer.read_with(cx, |v, _| v.zoom);
    cx.simulate_keystrokes(&format!("{SECONDARY}-="));
    assert_eq!(zoom(cx), 1.25);
    cx.simulate_keystrokes(&format!("{SECONDARY}--"));
    cx.simulate_keystrokes(&format!("{SECONDARY}--"));
    assert_eq!(zoom(cx), 0.8);
    cx.simulate_keystrokes(&format!("{SECONDARY}-0"));
    assert_eq!(zoom(cx), 1.0);
    viewer.update(cx, |v, cx| v.set_zoom(100.0, cx));
    assert_eq!(zoom(cx), MAX_ZOOM);
    viewer.update(cx, |v, cx| v.set_zoom(0.0, cx));
    assert_eq!(zoom(cx), MIN_ZOOM);
}

#[gpui::test]
fn scrolling_pans_and_zooms(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, "flowchart\nA\n".into()));
    // Over the drawing, which is right of the source pane.
    let scroll = |modifiers| ScrollEvent {
        position: point(px(900.0), px(300.0)),
        delta: ScrollDelta::Pixels(point(px(3.0), px(-40.0))),
        modifiers,
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    };
    cx.simulate_event(scroll(Modifiers::default()));
    viewer.read_with(cx, |v, _| {
        assert_eq!(v.pan, point(px(3.0), px(-40.0)));
        assert_eq!(v.zoom, 1.0);
    });
    let secondary = cx.update(|_, cx| Modifiers::secondary_key(cx));
    cx.simulate_event(scroll(secondary));
    viewer.read_with(cx, |v, _| {
        assert_eq!(v.pan, point(px(3.0), px(-40.0)));
        assert!((v.zoom - 0.2f32.exp()).abs() < 1e-4, "{}", v.zoom);
    });
    cx.dispatch_action(ActualSize);
    viewer.read_with(cx, |v, _| {
        assert_eq!((v.zoom, v.pan), (1.0, Default::default()))
    });
}

#[gpui::test]
fn fit_window(cx: &mut TestAppContext) {
    let chain: Vec<String> = (0..40)
        .map(|i| format!("N{i}[A long node label]"))
        .collect();
    let wide = format!("flowchart LR\n{}\n", chain.join(" --> "));
    let (viewer, cx) = open(cx, Document::from_source(None, wide));
    cx.dispatch_action(FitWindow);
    let (zoom, width) = cx.update(|window, cx| {
        let pane = window.viewport_size().width * 0.6;
        viewer.update(cx, |v, _| (v.zoom, v.scene(window).width / f32::from(pane)))
    });
    assert!(zoom < 1.0, "{zoom}");
    assert!((zoom * width - 1.0).abs() < 0.2, "{zoom} {width}");
}

#[gpui::test]
fn follows_the_appearance(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, "flowchart\nA\n".into()));
    viewer.update(cx, |v, cx| v.set_dark(true, cx));
    let background = cx.update(|window, cx| viewer.update(cx, |v, _| v.scene(window).background));
    assert_eq!(background, Theme::DARK.background);
    viewer.update(cx, |v, cx| v.set_dark(true, cx));
    viewer.read_with(cx, |v, _| assert_eq!(v.theme(), Theme::DARK));
    let kit_dark = cx.update(|_, cx| gpui::component::Theme::global(cx).is_dark());
    assert!(kit_dark);
}

/// Arcs, cubic jogs, and dashed strokes all reach the painter.
#[gpui::test]
fn paints_every_segment_kind(cx: &mut TestAppContext) {
    let src = "flowchart LR\nA((round)) -.-> B([pill]) --> C\nA --> D --> C\nA --> E --> F --> C\n";
    let (viewer, cx) = open(cx, Document::from_source(None, src.into()));
    let scene = cx.update(|window, cx| viewer.update(cx, |v, _| v.scene(window)));
    let segments = || {
        scene.elements.iter().flat_map(|e| match &e.primitive {
            Primitive::Path { segments, .. } => segments.as_slice(),
            Primitive::Text { .. } => &[],
        })
    };
    assert!(segments().any(|s| matches!(s, Segment::Arc { .. })));
    assert!(segments().any(|s| matches!(s, Segment::Cubic { .. })));
    assert!(scene.elements.iter().any(|e| matches!(
        &e.primitive,
        Primitive::Path { stroke: Some(s), .. } if s.dash.is_some()
    )));
    cx.dispatch_action(FitWindow);
    cx.run_until_parked();
}

#[gpui::test]
fn close_window(cx: &mut TestAppContext) {
    let (_viewer, vcx) = open(cx, Document::untitled());
    vcx.dispatch_action(CloseWindow);
    vcx.run_until_parked();
    assert!(cx.windows().is_empty());
}

#[gpui::test]
fn open_windows_for_paths(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.mmd", "flowchart\nA\n");
    cx.update(|cx| {
        sirene_gui::init(cx);
        sirene_gui::open_paths(&[a.clone(), dir.path().join("missing.mmd")], cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 2);
}
