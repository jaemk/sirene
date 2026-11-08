//! The source pane and document editing in gpui's headless test platform (WYSIWY-*).

mod common;

use common::{SECONDARY, open, open_another, source, texts, write};
use futures::FutureExt;
use gpui::component::input::Position;
use gpui::{Entity, TestAppContext, VisualTestContext};
use sirene_gui::document::{Document, STARTER};
use sirene_gui::viewer::{POLL, Viewer};
use sirene_gui::{CloseWindow, New, Open, Reload, Save, SaveAs};

/// Types `text` at `line` and `column` (0-based) in the source pane.
fn type_at(
    viewer: &Entity<Viewer>,
    cx: &mut VisualTestContext,
    line: u32,
    column: u32,
    text: &str,
) {
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.focus(window, cx);
            s.set_cursor_position(Position::new(line, column), window, cx);
        });
    });
    cx.simulate_input(text);
    cx.run_until_parked();
}

fn title(cx: &mut VisualTestContext) -> String {
    cx.window_title().unwrap_or_default()
}

#[gpui::test]
fn the_pane_shows_the_source(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::untitled());
    assert_eq!(source(&viewer, cx), STARTER);
    viewer.read_with(cx, |v, _| assert!(v.show_source));
}

#[gpui::test]
fn typing_redraws_and_marks_the_document(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::untitled());
    type_at(&viewer, cx, 1, 11, " --> C");
    assert_eq!(source(&viewer, cx), "flowchart TD\n    A --> B --> C\n");
    assert_eq!(texts(&viewer, cx), ["A", "B", "C"]);
    assert_eq!(title(cx), "Untitled (edited)");
    assert_eq!(viewer.read_with(cx, |v, _| v.status()), "Untitled (edited)");

    cx.simulate_keystrokes(&format!("{SECONDARY}-z"));
    cx.run_until_parked();
    assert_eq!(source(&viewer, cx), STARTER);
    assert_eq!(title(cx), "Untitled");
}

#[gpui::test]
fn diagnostics_show_in_the_editor(cx: &mut TestAppContext) {
    let src = "flowchart\nclick Z cb\nlinkStyle 9 color:red\n";
    let (viewer, cx) = open(cx, Document::from_source(None, src.into()));
    let count = |viewer: &Entity<Viewer>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let editor = viewer.read(cx).source.read(cx);
            editor.diagnostics().map_or(0, |d| d.len())
        })
    };
    assert_eq!(count(&viewer, cx), 2);
    // Deleting the bad `linkStyle` line clears its error.
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.replace_all("flowchart\nclick Z cb\n", window, cx)
        });
    });
    cx.run_until_parked();
    assert_eq!(count(&viewer, cx), 1);
    let status = viewer.read_with(cx, |v, _| v.status());
    assert!(status.starts_with("2:7: warning:"), "{status}");
}

#[gpui::test]
fn hiding_the_pane_keeps_its_text(cx: &mut TestAppContext) {
    let (viewer, cx) = open(cx, Document::untitled());
    type_at(&viewer, cx, 1, 11, " --> C");
    cx.simulate_keystrokes(&format!("{SECONDARY}-e"));
    viewer.read_with(cx, |v, _| assert!(!v.show_source));
    cx.simulate_keystrokes(&format!("{SECONDARY}-e"));
    viewer.read_with(cx, |v, _| assert!(v.show_source));
    assert_eq!(source(&viewer, cx), "flowchart TD\n    A --> B --> C\n");
}

#[gpui::test]
fn saves_to_the_file(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA --> B\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    type_at(&viewer, cx, 1, 7, " --> C");
    assert_eq!(title(cx), "a.mmd (edited)");
    cx.simulate_keystrokes(&format!("{SECONDARY}-s"));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "flowchart LR\nA --> B --> C\n"
    );
    assert_eq!(title(cx), "a.mmd");
}

#[gpui::test]
fn saving_untitled_asks_for_a_path(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("b.mmd");
    let (viewer, vcx) = open(cx, Document::untitled());
    type_at(&viewer, vcx, 1, 11, " --> C");
    vcx.dispatch_action(Save);
    vcx.run_until_parked();
    assert!(cx.did_prompt_for_new_path());
    let chosen = target.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "flowchart TD\n    A --> B --> C\n"
    );
    assert_eq!(title(vcx), "b.mmd");

    // Save As writes a copy elsewhere and shows it; a cancelled prompt changes nothing.
    let copy = dir.path().join("c.mmd");
    vcx.dispatch_action(SaveAs);
    vcx.run_until_parked();
    let chosen = copy.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    assert!(copy.exists());
    assert_eq!(title(vcx), "c.mmd");
    vcx.dispatch_action(SaveAs);
    vcx.run_until_parked();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(title(vcx), "c.mmd");
}

#[gpui::test]
fn failed_saves_show_the_error(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    type_at(&viewer, cx, 1, 1, " --> B");
    std::fs::remove_dir_all(dir.path()).unwrap();
    cx.dispatch_action(Save);
    cx.run_until_parked();
    let status = viewer.read_with(cx, |v, _| v.status());
    assert!(status.starts_with("cannot write "), "{status}");
    assert_eq!(title(cx), "a.mmd (edited)");
}

#[gpui::test]
fn disk_changes_reach_the_editor(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA --> B\nB --> C\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    type_at(&viewer, cx, 2, 0, "");
    write(dir.path(), "a.mmd", "flowchart LR\nA --> B\nB --> D\n");
    cx.executor().advance_clock(POLL);
    cx.run_until_parked();
    assert_eq!(source(&viewer, cx), "flowchart LR\nA --> B\nB --> D\n");
    let line = cx.update(|_, cx| viewer.read(cx).source.read(cx).cursor_position().line);
    assert_eq!(line, 2);

    // Under unsaved edits the file's text waits until Reload.
    type_at(&viewer, cx, 1, 7, " --> X");
    write(dir.path(), "a.mmd", "flowchart LR\nA --> Theirs\n");
    cx.executor().advance_clock(POLL);
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "flowchart LR\nA --> B --> X\nB --> D\n"
    );
    let status = viewer.read_with(cx, |v, _| v.status());
    assert_eq!(status, "a.mmd changed on disk; Reload discards your edits");
    cx.dispatch_action(Reload);
    cx.run_until_parked();
    assert_eq!(source(&viewer, cx), "flowchart LR\nA --> Theirs\n");
    assert_eq!(title(cx), "a.mmd");
}

#[gpui::test]
fn closing_asks_about_unsaved_edits(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a.mmd", "flowchart LR\nA\n");
    let (viewer, vcx) = open(cx, Document::open(&path));
    type_at(&viewer, vcx, 1, 1, " --> B");

    vcx.dispatch_action(CloseWindow);
    vcx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 1);

    vcx.dispatch_action(CloseWindow);
    vcx.run_until_parked();
    cx.simulate_prompt_answer("Save");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "flowchart LR\nA --> B\n"
    );
}

#[gpui::test]
fn closing_can_drop_the_edits(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::untitled());
    type_at(&viewer, vcx, 1, 11, " --> C");
    let closes = vcx.update(|window, cx| viewer.update(cx, |v, cx| v.should_close(window, cx)));
    assert!(!closes);
    vcx.run_until_parked();
    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}

#[gpui::test]
fn quitting_stops_at_cancel(cx: &mut TestAppContext) {
    let (_, vcx) = open(cx, Document::untitled());
    let unedited = vcx.update(|window, cx| {
        let viewer = sirene_gui::viewer_in(window, cx).unwrap();
        viewer.update(cx, |v, cx| v.should_close(window, cx))
    });
    assert!(unedited);
    let (edited, ecx) = open_another(cx, Document::untitled());
    type_at(&edited, ecx, 1, 11, " --> C");

    let quit = cx.update(sirene_gui::confirm_quit);
    cx.run_until_parked();
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(quit.now_or_never(), Some(false));

    let quit = cx.update(sirene_gui::confirm_quit);
    cx.run_until_parked();
    cx.simulate_prompt_answer("Don't Save");
    cx.run_until_parked();
    assert_eq!(quit.now_or_never(), Some(true));
}

#[gpui::test]
fn new_and_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.mmd", "flowchart\nA\n");
    let b = write(dir.path(), "b.mmd", "flowchart\nB\n");
    let (viewer, vcx) = open(cx, Document::untitled());
    vcx.dispatch_action(New);
    vcx.run_until_parked();
    assert_eq!(cx.windows().len(), 2);

    // A blank window shows the first file and opens the rest.
    vcx.dispatch_action(Open);
    vcx.run_until_parked();
    let (a2, b2) = (a.clone(), b.clone());
    cx.simulate_path_prompt_response(move |_| Some(vec![a2, b2]));
    cx.run_until_parked();
    assert_eq!(title(vcx), "a.mmd");
    assert_eq!(source(&viewer, vcx), "flowchart\nA\n");
    assert_eq!(cx.windows().len(), 3);

    // A window showing a file leaves Open to the app, which opens new windows for files not
    // already open and brings forward the windows of those that are.
    let c = write(dir.path(), "c.mmd", "flowchart\nC\n");
    vcx.dispatch_action(Open);
    vcx.run_until_parked();
    cx.simulate_path_prompt_response(move |_| Some(vec![b, c]));
    cx.run_until_parked();
    assert_eq!(cx.windows().len(), 4);
    assert_eq!(title(vcx), "a.mmd");
}
