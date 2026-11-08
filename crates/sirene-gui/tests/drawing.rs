//! Editing on the drawing in gpui's headless test platform (WYSIWY-9..13).

mod common;

use common::{SECONDARY, open, source};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, VisualTestContext, point, px,
};
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::viewer::Viewer;
use sirene_gui::{AddNode, Delete, Deselect};

const SRC: &str = "flowchart TD\n    A[Alpha] --> B[Beta]\n";

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

/// Opens `src` in an active window, so focus changes are reported.
fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (viewer, cx)
}

fn center(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, item: &Item) -> Point<Pixels> {
    cx.update(|window, cx| viewer.update(cx, |v, _| v.item_center(item, window)))
        .expect("item is drawn")
}

fn click(cx: &mut VisualTestContext, at: Point<Pixels>, modifiers: Modifiers) {
    cx.simulate_click(at, modifiers);
    cx.run_until_parked();
}

fn double_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    for count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: count,
        });
    }
    cx.run_until_parked();
}

fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    let mid = point((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    cx.simulate_mouse_move(mid, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn selection(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Vec<Item> {
    viewer.read_with(cx, |v, _| v.selection.clone())
}

/// The text selected in the source pane.
fn selected_source(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| viewer.read(cx).source.read(cx).selected_value().to_string())
}

/// The label field's text, when it is open.
fn field(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| {
        let v = viewer.read(cx);
        v.label
            .as_ref()
            .map(|l| l.input.read(cx).value().to_string())
    })
}

fn keys(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

#[gpui::test]
fn clicking_selects(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let (a, b) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("B")),
    );
    click(cx, a, Modifiers::none());
    assert_eq!(selection(&viewer, cx), [node("A")]);
    assert_eq!(selected_source(&viewer, cx), "A[Alpha]");

    click(cx, b, Modifiers::shift());
    assert_eq!(selection(&viewer, cx), [node("A"), node("B")]);
    click(cx, a, Modifiers::shift());
    assert_eq!(selection(&viewer, cx), [node("B")]);
    // Clicking a selected item keeps the selection.
    click(cx, b, Modifiers::none());
    assert_eq!(selection(&viewer, cx), [node("B")]);

    let edge = center(&viewer, cx, &Item::Edge(0));
    click(cx, edge, Modifiers::none());
    assert_eq!(selection(&viewer, cx), [Item::Edge(0)]);
    assert_eq!(selected_source(&viewer, cx), "-->");

    // Empty space clears, unless shift is held; so does escape.
    let empty = point(a.x + px(300.0), a.y);
    click(cx, empty, Modifiers::shift());
    assert_eq!(selection(&viewer, cx), [Item::Edge(0)]);
    click(cx, empty, Modifiers::none());
    assert!(selection(&viewer, cx).is_empty());
    click(cx, a, Modifiers::none());
    keys(cx, "escape");
    assert!(selection(&viewer, cx).is_empty());
}

#[gpui::test]
fn selection_follows_the_document(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    let edit = |cx: &mut VisualTestContext, text: &str| {
        let text = text.to_owned();
        cx.update(|window, cx| {
            let editor = viewer.read(cx).source.clone();
            editor.update(cx, |s, cx| s.replace_all(text, window, cx));
        });
        cx.run_until_parked();
    };
    edit(cx, "flowchart TD\n    X --> A[Alpha] --> B[Beta]\n");
    assert_eq!(selection(&viewer, cx), [node("B")]);
    edit(cx, "flowchart TD\n    A[Alpha]\n");
    assert!(selection(&viewer, cx).is_empty());
}

#[gpui::test]
fn editing_labels(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let a = center(&viewer, cx, &node("A"));
    double_click(cx, a);
    assert_eq!(field(&viewer, cx).as_deref(), Some("Alpha"));
    cx.simulate_input("Gamma ray");
    keys(cx, "enter");
    assert_eq!(field(&viewer, cx), None);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Gamma ray] --> B[Beta]\n"
    );
    assert_eq!(cx.window_title().as_deref(), Some("Untitled (edited)"));

    // Enter opens the field for the selection; escape cancels it.
    keys(cx, "enter");
    assert_eq!(field(&viewer, cx).as_deref(), Some("Gamma ray"));
    cx.simulate_input("nope");
    keys(cx, "escape");
    assert_eq!(field(&viewer, cx), None);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Gamma ray] --> B[Beta]\n"
    );

    // Line breaks show as `<br>`.
    keys(cx, "enter");
    cx.simulate_input("one<br>two");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[\"one<br>two\"] --> B[Beta]\n"
    );
    keys(cx, "enter");
    assert_eq!(field(&viewer, cx).as_deref(), Some("one<br>two"));
    keys(cx, "escape");

    // Edges, whose empty label removes it.
    let edge = center(&viewer, cx, &Item::Edge(0));
    double_click(cx, edge);
    assert_eq!(field(&viewer, cx).as_deref(), Some(""));
    cx.simulate_input("yes");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[\"one<br>two\"] -->|yes| B[Beta]\n"
    );
    let edge = center(&viewer, cx, &Item::Edge(0));
    double_click(cx, edge);
    keys(cx, "backspace");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[\"one<br>two\"] --> B[Beta]\n"
    );
}

#[gpui::test]
fn leaving_the_field_cancels(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let a = center(&viewer, cx, &node("A"));
    double_click(cx, a);
    let input = cx.update(|_, cx| viewer.read(cx).label.as_ref().map(|l| l.input.clone()));
    let input = input.expect("the field is open");
    assert!(cx.update(|window, cx| input.read(cx).focus_handle(cx).is_focused(window)));
    cx.simulate_input("changed");
    // Clicking the source pane closes the field and leaves focus in the pane.
    click(cx, point(px(100.0), px(100.0)), Modifiers::none());
    assert_eq!(field(&viewer, cx), None);
    assert_eq!(source(&viewer, cx), SRC);
    let editor = viewer.read_with(cx, |v, _| v.source.clone());
    assert!(cx.update(|window, cx| editor.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn adding_nodes(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let a = center(&viewer, cx, &node("A"));
    click(cx, a, Modifiers::none());
    keys(cx, &format!("{SECONDARY}-shift-n"));
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C[New node]\n    A --> C\n"
    );
    assert_eq!(selection(&viewer, cx), [node("C")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New node"));
    cx.simulate_input("Gamma");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C[Gamma]\n    A --> C\n"
    );

    // Without a selected node the new node stands alone.
    cx.dispatch_action(Deselect);
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert!(source(&viewer, cx).ends_with("    A --> C\n    D[New node]\n"));
}

#[gpui::test]
fn dragging_connects_and_pans(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, "flowchart LR\n    A --> B\n    C\n");
    let (a, c) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("C")),
    );
    drag(cx, c, a);
    assert_eq!(
        source(&viewer, cx),
        "flowchart LR\n    A --> B\n    C\n    C --> A\n"
    );
    // A press without a drag, or a drag ending outside the drawing, adds nothing.
    let a = center(&viewer, cx, &node("A"));
    drag(cx, a, a);
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    drag(cx, a, area.origin - point(px(5.0), px(5.0)));
    assert_eq!(
        source(&viewer, cx),
        "flowchart LR\n    A --> B\n    C\n    C --> A\n"
    );
    // A drag ending on empty space adds a node there, linked, with its label open
    // (WYSIWY-15).
    drag(cx, a, point(a.x, a.y - px(200.0)));
    assert_eq!(
        source(&viewer, cx),
        "flowchart LR\n    A --> B\n    C\n    C --> A\n    D[New node]\n    A --> D\n"
    );
    assert_eq!(selection(&viewer, cx), [node("D")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New node"));
    viewer.update_in(cx, |v, window, cx| v.cancel_label(window, cx));
    let a = center(&viewer, cx, &node("A"));

    // Dragging from empty space pans.
    let empty = point(a.x, a.y - px(200.0));
    let before = viewer.read_with(cx, |v, _| v.pan);
    drag(cx, empty, point(empty.x + px(30.0), empty.y + px(10.0)));
    let after = viewer.read_with(cx, |v, _| v.pan);
    assert_eq!(after - before, point(px(30.0), px(10.0)));
    // Moving with no button pressed does nothing.
    cx.simulate_mouse_move(a, None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(viewer.read_with(cx, |v, _| v.pan), after);
}

#[gpui::test]
fn deleting_and_undo(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    keys(cx, "backspace");
    assert_eq!(source(&viewer, cx), "flowchart TD\n    A[Alpha]\n");
    assert!(selection(&viewer, cx).is_empty());

    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), SRC);
    keys(cx, &format!("{SECONDARY}-shift-z"));
    assert_eq!(source(&viewer, cx), "flowchart TD\n    A[Alpha]\n");
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), SRC);

    // Edges, with the delete key; nothing selected deletes nothing.
    let edge = center(&viewer, cx, &Item::Edge(0));
    click(cx, edge, Modifiers::none());
    keys(cx, "delete");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha]\n    B[Beta]\n"
    );
    keys(cx, "delete");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha]\n    B[Beta]\n"
    );

    // Typing in the pane ends the drawing's history.
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.replace_all("flowchart TD\n    Q\n", window, cx)
        });
    });
    cx.run_until_parked();
    let q = center(&viewer, cx, &node("Q"));
    click(cx, q, Modifiers::none());
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), "flowchart TD\n    Q\n");
    keys(cx, &format!("{SECONDARY}-z"));
    keys(cx, &format!("{SECONDARY}-shift-z"));
    assert_eq!(source(&viewer, cx), "flowchart TD\n    Q\n");
}

#[gpui::test]
fn redo_stops_after_typing(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    keys(cx, "backspace");
    keys(cx, &format!("{SECONDARY}-z"));
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.replace_all("flowchart TD\n    Q\n", window, cx)
        });
    });
    cx.run_until_parked();
    let q = center(&viewer, cx, &node("Q"));
    click(cx, q, Modifiers::none());
    keys(cx, &format!("{SECONDARY}-shift-z"));
    assert_eq!(source(&viewer, cx), "flowchart TD\n    Q\n");
}

#[gpui::test]
fn subgraphs_and_missing_items(cx: &mut TestAppContext) {
    let src = "flowchart TD\n    subgraph G\n        A[Alpha]\n    end\n    B\n";
    let (viewer, cx) = open_src(cx, src);
    // A subgraph is not selectable: clicking it clears the selection.
    let a = center(&viewer, cx, &node("A"));
    click(cx, a, Modifiers::none());
    let top = cx
        .update(|window, cx| viewer.update(cx, |v, _| v.item_bounds(&node("A"), window)))
        .unwrap()
        .0;
    click(cx, point(a.x, top.y - px(10.0)), Modifiers::none());
    assert!(selection(&viewer, cx).is_empty());
    // Moving with the button held after a click does nothing.
    cx.simulate_mouse_move(a, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(source(&viewer, cx), src);

    // Items that do not exist: no source to select, no label to edit or commit.
    cx.update(|window, cx| {
        viewer.update(cx, |v, cx| {
            v.select(vec![node("Z")], cx);
            v.edit_label(node("Z"), window, cx);
            v.commit_label(window, cx);
        })
    });
    assert!(field(&viewer, cx).is_none());

    // With the field open, Delete and Deselect leave the selection alone.
    let b = center(&viewer, cx, &node("B"));
    double_click(cx, b);
    cx.dispatch_action(Delete);
    cx.dispatch_action(Deselect);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("B")]);
    assert_eq!(source(&viewer, cx), src);
    // The field closes when its node goes away.
    cx.update(|window, cx| {
        let editor = viewer.read(cx).source.clone();
        editor.update(cx, |s, cx| {
            s.replace_all("flowchart TD\n    A\n", window, cx)
        });
    });
    cx.run_until_parked();
    assert!(field(&viewer, cx).is_none());
    assert!(selection(&viewer, cx).is_empty());
}
