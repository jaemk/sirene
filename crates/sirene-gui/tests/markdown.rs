//! Editing a Markdown file's diagram blocks (WYSIWY-20).

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
        "empty.md has no diagram blocks; editing adds one at the end"
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

#[test]
fn edits_stay_in_their_block() {
    // A file ending in an open code block: the added block closes it first, and the next edit
    // goes in the same block.
    let mut d = Document::from_source(Some("open.md".into()), "# Doc\n\n```python\nx = 1\n".into());
    assert!(d.set_source("flowchart TD\n  A\n".into()));
    assert!(d.set_source("flowchart TD\n  A --> B\n".into()));
    assert_eq!(
        d.text(),
        "# Doc\n\n```python\nx = 1\n```\n\n```mermaid\nflowchart TD\n  A --> B\n```\n"
    );
    assert_eq!(d.block_info(), Some((1, 1, 8)));

    // A fence typed in the code makes the block's fences longer, so the code stays whole.
    let mut d = Document::from_source(Some("notes.md".into()), MD.into());
    let code = "flowchart LR\n```\n    A --> B\n";
    assert!(d.set_source(code.into()));
    assert!(d.set_source(format!("{code}    B --> C\n")));
    let md = d.markdown.as_ref().unwrap();
    assert_eq!(md.blocks.len(), 2);
    assert_eq!(md.blocks[0].code, format!("{code}    B --> C\n"));
    assert!(d.text().contains("\n````mermaid\n"), "{}", d.text());
    assert!(d.text().ends_with("  ```\n\nEnd.\n"));
}

#[test]
fn failed_save_as_keeps_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(dir.path(), "notes.md", MD);
    let mut d = Document::open(&md);
    d.set_source("flowchart LR\n    A --> C\n".into());
    assert!(d.save_as_drops_edits(&dir.path().join("a.mmd")));
    assert!(!d.save_as_drops_edits(&dir.path().join("a.md")));
    // Saving into a missing directory fails: still the Markdown file, still edited.
    assert!(d.save_as(dir.path().join("missing").join("a.mmd")).is_err());
    assert_eq!(d.path.as_deref(), Some(md.as_path()));
    assert!(d.markdown.is_some());
    assert!(d.is_edited());
    assert!(d.text().contains("A --> C"));
    assert_eq!(d.poll(), sirene_gui::document::Disk::Unchanged);
    d.save().unwrap();
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        MD.replace("A --> B", "A --> C")
    );
}

#[test]
fn save_as_reads_the_new_language() {
    let dir = tempfile::tempdir().unwrap();
    // DOT text in a `.mmd` file is read as Mermaid; saved as `.dot`, it is read as DOT.
    let mut d = Document::open(&write(dir.path(), "a.mmd", "digraph { a -> b }\n"));
    assert!(!d.problems.is_empty());
    d.save_as(dir.path().join("a.dot")).unwrap();
    assert!(d.problems.is_empty(), "{:?}", d.problems);
    assert_eq!(d.chart.nodes.len(), 2);
}

#[test]
fn blocks_are_named_for_their_language() {
    let dir = tempfile::tempdir().unwrap();
    // A diagram saved as Markdown goes in a block its language names.
    for (name, src, info) in [
        ("a.puml", "@startuml\nA -> B\n@enduml\n", "plantuml"),
        ("b.dot", "digraph { a -> b }\n", "dot"),
        ("c.gv", "flowchart LR\n  A --> B\n", "dot"),
    ] {
        let mut d = Document::open(&write(dir.path(), name, src));
        let md = dir.path().join(format!("{name}.md"));
        d.save_as(md.clone()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            format!("```{info}\n{src}```\n"),
            "{name}"
        );
        assert_eq!(d.lang(), Document::open(&dir.path().join(name)).lang());
    }
    // The block a file without one gets is named for the source typed in.
    let empty = write(dir.path(), "empty.md", "");
    let mut d = Document::open(&empty);
    assert_eq!(
        d.status(),
        "empty.md has no diagram blocks; editing adds one at the end"
    );
    assert!(d.set_source("digraph { a -> b }\n".into()));
    d.save().unwrap();
    assert_eq!(
        std::fs::read_to_string(&empty).unwrap(),
        "```dot\ndigraph { a -> b }\n```\n"
    );
}
