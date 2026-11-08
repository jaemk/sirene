//! Editing a Markdown file's mermaid blocks (WYSIWY-20).

mod common;

use common::{SECONDARY, open, source, texts, write};
use gpui::component::input::Position;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use sirene_gui::document::Document;
use sirene_gui::export::Format;
use sirene_gui::viewer::Viewer;
use sirene_gui::{AddNode, Deselect, NextDiagram, PreviousDiagram, Reload, Save};

const MD: &str = "# Notes\n\nIntro.\n\n```mermaid\nflowchart LR\n    A --> B\n```\n\n\
                  Between.\n\n- item\n\n  ```mermaid\n  graph TD\n    X --> Y\n  ```\n\nEnd.\n";

fn status(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> String {
    viewer.read_with(cx, |v, _| v.status())
}

fn on_disk(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> String {
    let path = viewer.read_with(cx, |v, _| v.doc.path.clone().unwrap());
    std::fs::read_to_string(path).unwrap()
}

#[gpui::test]
fn blocks_are_edited_in_place(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "notes.md", MD);
    let (viewer, cx) = open(cx, Document::open(&path));
    // The first block shows; the switcher says where it is.
    assert_eq!(source(&viewer, cx), "flowchart LR\n    A --> B\n");
    assert_eq!(texts(&viewer, cx), ["A", "B"]);
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((1, 2, 6))
    );
    assert!(cx.debug_bounds("next-diagram").is_some());

    // Typing goes into the block; the rest of the file is kept.
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.focus(window, cx);
            s.set_cursor_position(Position::new(1, 11), window, cx);
        });
    });
    cx.simulate_input(" --> C");
    cx.run_until_parked();
    assert_eq!(texts(&viewer, cx), ["A", "B", "C"]);
    assert_eq!(cx.window_title().unwrap_or_default(), "notes.md (edited)");
    cx.simulate_keystrokes(&format!("{SECONDARY}-s"));
    cx.run_until_parked();
    assert_eq!(
        on_disk(&viewer, cx),
        MD.replace("A --> B\n", "A --> B --> C\n")
    );

    // The next block, indented in a list: drawing edits keep its indent.
    cx.dispatch_action(NextDiagram);
    assert_eq!(source(&viewer, cx), "graph TD\n  X --> Y\n");
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((2, 2, 15))
    );
    assert_eq!(
        viewer.read_with(cx, |v, _| v.export_name(Format::Png)),
        "notes-2.png"
    );
    cx.dispatch_action(Deselect);
    cx.dispatch_action(AddNode);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.dispatch_action(Save);
    cx.run_until_parked();
    let saved = on_disk(&viewer, cx);
    assert!(
        saved.ends_with("  ```mermaid\n  graph TD\n    X --> Y\n    A[New node]\n  ```\n\nEnd.\n"),
        "{saved}"
    );
    assert!(
        saved
            .starts_with("# Notes\n\nIntro.\n\n```mermaid\nflowchart LR\n    A --> B --> C\n```\n")
    );

    // Wraps around both ways.
    cx.dispatch_action(NextDiagram);
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((1, 2, 6))
    );
    cx.dispatch_action(PreviousDiagram);
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((2, 2, 15))
    );

    // The previous button, and a reload keeps the block shown.
    let previous = cx.debug_bounds("previous-diagram").unwrap().center();
    cx.simulate_click(previous, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((1, 2, 6))
    );
    std::fs::write(&path, MD).unwrap();
    cx.dispatch_action(Reload);
    cx.run_until_parked();
    assert_eq!(source(&viewer, cx), "flowchart LR\n    A --> B\n");
    assert_eq!(status(&viewer, cx), "notes.md");
}

#[gpui::test]
fn a_file_without_blocks_gets_one(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "empty.md", "# Empty\n");
    let (viewer, cx) = open(cx, Document::open(&path));
    assert_eq!(source(&viewer, cx), "");
    assert_eq!(
        status(&viewer, cx),
        "empty.md has no mermaid blocks; editing adds one at the end"
    );
    assert!(cx.debug_bounds("next-diagram").is_none());
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.replace_all("flowchart TD\n    A --> B\n", window, cx)
        });
    });
    cx.run_until_parked();
    cx.dispatch_action(Save);
    cx.run_until_parked();
    assert_eq!(
        on_disk(&viewer, cx),
        "# Empty\n\n```mermaid\nflowchart TD\n    A --> B\n```\n"
    );
    assert_eq!(
        viewer.read_with(cx, |v, _| v.doc.block_info()),
        Some((1, 1, 4))
    );
}

#[test]
fn save_as_converts() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(dir.path(), "notes.md", MD);
    // A block saved as a diagram file is the block alone.
    let mut d = Document::open(&md);
    d.save_as(dir.path().join("first.mmd")).unwrap();
    assert!(d.markdown.is_none());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("first.mmd")).unwrap(),
        "flowchart LR\n    A --> B\n"
    );
    // A diagram saved as Markdown is put in a block.
    d.save_as(dir.path().join("wrapped.md")).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("wrapped.md")).unwrap(),
        "```mermaid\nflowchart LR\n    A --> B\n```\n"
    );
    assert_eq!(d.block_info(), Some((1, 1, 2)));
    assert!(!d.is_edited());
    // Showing a block that is shown already, or not there, changes nothing.
    assert!(!d.show_block(0));
    assert!(!d.show_block(5));
    assert!(!Document::untitled().show_block(0));
}
