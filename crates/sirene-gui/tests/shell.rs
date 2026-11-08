//! Menus drawn in the window, Edit menu actions on text, the drawing's focus ring, and where
//! new windows open, in gpui's headless test platform (GUISHE-11, GUISHE-12).

mod common;

use common::{open, open_another, source};
use gpui::component::GlobalState;
use gpui::component::input::Position;
use gpui::{Modifiers, OwnedMenuItem, TestAppContext, point, px};
use sirene_gui::document::{Document, STARTER};
use sirene_gui::{CASCADE, IN_WINDOW_MENUS, Redo, SelectAll, Undo};

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

#[gpui::test]
fn windows_draw_the_menus_without_a_platform_menu_bar(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::untitled());
    let has_bar = viewer.read_with(vcx, |v, _| v.menu_bar.is_some());
    assert_eq!(has_bar, IN_WINDOW_MENUS);
    // The drawn menus are the platform's.
    let (drawn, platform) = cx.update(|cx| {
        let drawn: Vec<String> = GlobalState::global(cx)
            .app_menus()
            .iter()
            .flat_map(|m| names(&m.items))
            .collect();
        let platform: Vec<String> = cx
            .get_menus()
            .expect("menus are set")
            .iter()
            .flat_map(|m| names(&m.items))
            .collect();
        (drawn, platform)
    });
    assert_eq!(drawn, platform);
    assert!(drawn.contains(&"Dark".to_owned()));
    assert!(drawn.contains(&"Export as SVG...".to_owned()));
    // The bar sits above the panes.
    let top = viewer.read_with(vcx, |v, _| v.drawing_area().origin.y);
    if IN_WINDOW_MENUS {
        assert!(top >= px(30.0), "{top:?}");
    } else {
        assert_eq!(top, px(0.0));
    }

    // Open Recent follows the recent files.
    let dir = tempfile::tempdir().unwrap();
    let file = common::write(dir.path(), "a.mmd", "flowchart\nA\n");
    cx.update(|cx| sirene_gui::note_recent(&file, cx));
    cx.run_until_parked();
    let drawn: Vec<String> = cx.update(|cx| {
        GlobalState::global(cx)
            .app_menus()
            .iter()
            .flat_map(|m| names(&m.items))
            .collect()
    });
    let shown = std::fs::canonicalize(&file).unwrap().display().to_string();
    assert!(drawn.contains(&shown), "{drawn:?}");
}

#[gpui::test]
fn edit_menu_acts_on_the_source_text(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::untitled());
    vcx.update(|window, _| window.activate_window());
    vcx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.focus(window, cx);
            s.set_cursor_position(Position::new(1, 11), window, cx);
        });
    });
    vcx.simulate_input(" --> C");
    vcx.run_until_parked();
    assert_ne!(source(&viewer, vcx), STARTER);

    vcx.dispatch_action(Undo);
    vcx.run_until_parked();
    assert_eq!(source(&viewer, vcx), STARTER);
    vcx.dispatch_action(Redo);
    vcx.run_until_parked();
    assert_eq!(source(&viewer, vcx), "flowchart TD\n    A --> B --> C\n");

    vcx.dispatch_action(SelectAll);
    vcx.run_until_parked();
    let selected = vcx.update(|_, cx| viewer.read(cx).source.read(cx).selected_value().to_string());
    assert_eq!(selected, source(&viewer, vcx));
    // The drawing's selection is untouched.
    assert!(viewer.read_with(vcx, |v, _| v.selection.is_empty()));
}

#[gpui::test]
fn the_drawing_shows_when_it_has_the_keyboard(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::untitled());
    vcx.update(|window, _| window.activate_window());
    vcx.run_until_parked();
    let focused = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|window, cx| viewer.read(cx).drawing_focused(window))
    };
    // The source pane has the keyboard at first.
    assert!(!focused(vcx));
    let area = viewer.read_with(vcx, |v, _| v.drawing_area());
    vcx.simulate_click(area.origin + point(px(5.0), px(5.0)), Modifiers::none());
    vcx.run_until_parked();
    assert!(focused(vcx));
}

#[gpui::test]
fn new_windows_cascade(cx: &mut TestAppContext) {
    let (_, first) = open(cx, Document::untitled());
    first.update(|window, _| window.activate_window());
    let a = first.update(|window, _| window.bounds());
    let (_, second) = open_another(cx, Document::untitled());
    let b = second.update(|window, _| window.bounds());
    assert_eq!(b.size, a.size);
    assert_eq!(b.origin, a.origin + point(px(CASCADE), px(CASCADE)));
}
