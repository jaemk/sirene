//! Settings kept between launches, in gpui's headless test platform (GUISHE-10, GUISHE-11).

mod common;

use std::path::{Path, PathBuf};

use common::{open, open_another, write};
use gpui::{Modifiers, MouseButton, OwnedMenuItem, TestAppContext, point, px, size};
use sirene_core::scene::Theme;
use sirene_gui::document::Document;
use sirene_gui::settings::{self, Appearance, Settings};
use sirene_gui::{
    AppearanceDark, AppearanceLight, AppearanceSystem, ClearRecent, OpenRecent, SaveAs,
    ToggleSource,
};

/// Keeps the settings in `dir/settings`.
fn use_settings(cx: &mut TestAppContext, dir: &Path) -> PathBuf {
    let file = dir.join("settings");
    cx.update(|cx| cx.set_global(Settings::load(file.clone())));
    file
}

fn current(cx: &mut TestAppContext) -> Settings {
    cx.update(|cx| settings::get(cx).clone())
}

#[gpui::test]
fn appearance_overrides_the_system(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let file = use_settings(cx, dir.path());
    let (viewer, vcx) = open(cx, Document::untitled());
    let theme = |vcx: &mut gpui::VisualTestContext| viewer.read_with(vcx, |v, _| v.theme());
    assert_eq!(theme(vcx), Theme::LIGHT);
    vcx.dispatch_action(AppearanceDark);
    vcx.run_until_parked();
    assert_eq!(theme(vcx), Theme::DARK);
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("appearance = dark")
    );
    let kit_dark = vcx.update(|_, cx| gpui::component::Theme::global(cx).is_dark());
    assert!(kit_dark);

    // Every window follows, new ones too.
    let (other, ocx) = open_another(cx, Document::untitled());
    assert_eq!(other.read_with(ocx, |v, _| v.theme()), Theme::DARK);
    ocx.dispatch_action(AppearanceLight);
    ocx.run_until_parked();
    assert_eq!(theme(vcx), Theme::LIGHT);
    // The system's appearance (light here) no longer wins over Light or Dark...
    vcx.dispatch_action(AppearanceDark);
    vcx.run_until_parked();
    viewer.update_in(vcx, |v, window, cx| {
        let dark = settings::get(cx).appearance.is_dark(false);
        v.set_dark(dark, cx);
        let _ = window;
    });
    assert_eq!(theme(vcx), Theme::DARK);
    // ...until the setting follows it again.
    vcx.dispatch_action(AppearanceSystem);
    vcx.run_until_parked();
    assert_eq!(theme(vcx), Theme::LIGHT);
    assert_eq!(Settings::load(file).appearance, Appearance::System);
}

#[gpui::test]
fn recent_files(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let file = use_settings(cx, dir.path());
    let a = write(dir.path(), "a.mmd", "flowchart\nA\n");
    let b = write(dir.path(), "b.mmd", "flowchart\nB\n");
    cx.update(|cx| {
        sirene_gui::init(cx);
        sirene_gui::open_paths(&[a.clone(), b.clone()], cx);
    });
    cx.run_until_parked();
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap();
    assert_eq!(current(cx).recent, [canon(&b), canon(&a)]);
    assert_eq!(Settings::load(file.clone()).recent, [canon(&b), canon(&a)]);
    fn names(items: &[OwnedMenuItem]) -> Vec<String> {
        items
            .iter()
            .flat_map(|i| match i {
                OwnedMenuItem::Action { name, .. } => vec![name.clone()],
                OwnedMenuItem::Submenu(m) => names(&m.items),
                _ => Vec::new(),
            })
            .collect()
    }
    let menus = cx.update(|cx| cx.get_menus()).expect("menus are set");
    let all: Vec<String> = menus.iter().flat_map(|m| names(&m.items)).collect();
    assert!(
        all.contains(&b.canonicalize().unwrap().display().to_string()),
        "{all:?}"
    );

    // Open Recent opens (or brings forward) the file, and moves it first.
    let c = write(dir.path(), "c.mmd", "flowchart\nC\n");
    cx.update(|cx| cx.dispatch_action(&OpenRecent { path: c.clone() }));
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 3);
    assert_eq!(current(cx).recent[0], canon(&c));

    // Save As and showing a file in a blank window count too.
    let (viewer, vcx) = open_another(cx, Document::untitled());
    let d = dir.path().join("d.mmd");
    vcx.dispatch_action(SaveAs);
    vcx.run_until_parked();
    let chosen = d.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    assert_eq!(current(cx).recent[0], canon(&d));
    let (blank, vcx) = open_another(cx, Document::untitled());
    blank.update_in(vcx, |v, window, cx| {
        v.show_paths(std::slice::from_ref(&a), window, cx)
    });
    // a.mmd is already open, so its window comes forward; the blank one stays blank.
    assert!(blank.read_with(vcx, |v, _| v.is_blank()));
    let e = write(dir.path(), "e.mmd", "flowchart\nE\n");
    blank.update_in(vcx, |v, window, cx| {
        v.show_paths(std::slice::from_ref(&e), window, cx)
    });
    assert_eq!(current(cx).recent[0], canon(&e));
    let _ = viewer;

    cx.update(|cx| cx.dispatch_action(&ClearRecent));
    cx.run_until_parked();
    assert!(current(cx).recent.is_empty());
    assert!(Settings::load(file).recent.is_empty());
}

#[gpui::test]
fn windows_remember_their_layout(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let file = use_settings(cx, dir.path());
    cx.update(|cx| {
        settings::update(cx, |s| {
            s.window = Some((900.0, 600.0));
            s.source_width = 300.0;
        })
    });
    let (viewer, vcx) = open(cx, Document::untitled());
    let viewport = vcx.update(|window, _| window.viewport_size());
    assert_eq!(viewport, size(px(900.0), px(600.0)));
    let area = viewer.read_with(vcx, |v, _| v.drawing_area());
    assert!((area.origin.x - px(300.0)).abs() < px(2.0), "{area:?}");

    // Resizing the window is remembered, and saved when the window closes.
    vcx.simulate_resize(size(px(1000.0), px(700.0)));
    vcx.run_until_parked();
    assert_eq!(current(cx).window, Some((1000.0, 700.0)));

    // Dragging the divider sets the width for new windows.
    // The pane keeps its width; the drawing takes the rest.
    let area = viewer.read_with(vcx, |v, _| v.drawing_area());
    assert!((area.origin.x - px(300.0)).abs() < px(2.0), "{area:?}");
    assert!((area.size.width - px(700.0)).abs() < px(2.0), "{area:?}");
    // The handle straddles the boundary; press just left of it, as gpui-base's tests do.
    let divider = area.origin.x - px(2.0);
    let y = area.origin.y + px(100.0);
    vcx.simulate_mouse_down(point(divider, y), MouseButton::Left, Modifiers::none());
    vcx.simulate_mouse_move(
        point(divider + px(50.0), y),
        MouseButton::Left,
        Modifiers::none(),
    );
    vcx.simulate_mouse_move(
        point(divider + px(100.0), y),
        MouseButton::Left,
        Modifiers::none(),
    );
    vcx.simulate_mouse_up(
        point(divider + px(100.0), y),
        MouseButton::Left,
        Modifiers::none(),
    );
    vcx.run_until_parked();
    let width = current(cx).source_width;
    assert!((width - 400.0).abs() < 4.0, "{width}");

    // Hiding the pane is saved at once and new windows start without it.
    vcx.dispatch_action(ToggleSource);
    vcx.run_until_parked();
    assert!(!Settings::load(file.clone()).show_source);
    let (other, ocx) = open_another(cx, Document::untitled());
    assert!(!other.read_with(ocx, |v, _| v.show_source));
    let area = other.read_with(ocx, |v, _| v.drawing_area());
    assert_eq!(area.origin.x, px(0.0));

    vcx.dispatch_action(sirene_gui::CloseWindow);
    vcx.run_until_parked();
    let saved = Settings::load(file);
    assert_eq!(saved.window, Some((1000.0, 700.0)));
    assert!((saved.source_width - 400.0).abs() < 4.0);
}

/// A size or pane remembered from a larger screen is fitted to this one; the setting is kept.
#[gpui::test]
fn remembered_sizes_fit_the_screen(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    use_settings(cx, dir.path());
    cx.update(|cx| {
        settings::update(cx, |s| {
            s.window = Some((3000.0, 2000.0));
            s.source_width = 2900.0;
        })
    });
    // The test display is 1920x1080.
    let (viewer, vcx) = open(cx, Document::untitled());
    let viewport = vcx.update(|window, _| window.viewport_size());
    assert_eq!(viewport, size(px(1920.0), px(1080.0)));
    let area = viewer.read_with(vcx, |v, _| v.drawing_area());
    let left = px(1920.0 - settings::DRAWING_MIN);
    assert!((area.origin.x - left).abs() < px(2.0), "{area:?}");
    assert_eq!(current(cx).source_width, 2900.0);
}

/// Without settings loaded (as in `init` alone), nothing is written anywhere.
#[gpui::test]
fn settings_stay_in_memory_without_a_file(cx: &mut TestAppContext) {
    let (_viewer, vcx) = open(cx, Document::untitled());
    vcx.dispatch_action(AppearanceDark);
    vcx.run_until_parked();
    let s = current(cx);
    assert_eq!(s.appearance, Appearance::Dark);
    assert!(s.file.is_none());
}
