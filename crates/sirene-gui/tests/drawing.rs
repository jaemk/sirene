//! Editing on the drawing in gpui's headless test platform (WYSIWY-9..13).

mod common;

use common::{SECONDARY, open, source};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, VisualTestContext, point, px,
};
use sirene_core::flowchart::Arrow;
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::viewer::Viewer;
use sirene_gui::{
    AddMember, AddNode, Delete, Deselect, RemoveMember, SetCardinality, SetIdentifying,
    SetRelationshipType,
};

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

#[gpui::test]
fn sequence_diagrams(cx: &mut TestAppContext) {
    let src = "sequenceDiagram\n    participant A as Alice\n    A->>B: hi\n";
    let (viewer, cx) = open_src(cx, src);
    // Clicking selects participants and messages, and their source (WYSIWY-21).
    let a = center(&viewer, cx, &node("A"));
    click(cx, a, Modifiers::none());
    assert_eq!(selection(&viewer, cx), [node("A")]);
    assert_eq!(selected_source(&viewer, cx), "A");
    let m = center(&viewer, cx, &Item::Edge(0));
    click(cx, m, Modifiers::none());
    assert_eq!(selection(&viewer, cx), [Item::Edge(0)]);
    assert_eq!(selected_source(&viewer, cx), "A->>B: hi");

    // Renaming a message and a participant.
    double_click(cx, m);
    assert_eq!(field(&viewer, cx).as_deref(), Some("hi"));
    cx.simulate_input("hello; there");
    keys(cx, "enter");
    let b = center(&viewer, cx, &node("B"));
    double_click(cx, b);
    assert_eq!(field(&viewer, cx).as_deref(), Some("B"));
    cx.simulate_input("Bob");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: hello#59; there\n"
    );

    // Dragging between participants adds a message, with its text open.
    let (a, b) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("B")),
    );
    drag(cx, b, a);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    assert_eq!(field(&viewer, cx).as_deref(), Some(""));
    cx.simulate_input("back");
    keys(cx, "enter");
    assert!(source(&viewer, cx).ends_with("    A->>B: hello#59; there\n    B->>A: back\n"));

    // Adding a participant from the selected one adds a message to it.
    let a = center(&viewer, cx, &node("A"));
    click(cx, a, Modifiers::none());
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("P1")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New node"));
    cx.simulate_input("Carol");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    participant P1 as Carol\n    A->>P1:\n"),
        "{}",
        source(&viewer, cx)
    );

    // Deleting a participant takes its messages; undo brings them back.
    let before = source(&viewer, cx);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(Delete);
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "sequenceDiagram\n    participant A as Alice\n    participant P1 as Carol\n    A->>P1:\n"
    );
    assert!(selection(&viewer, cx).is_empty());
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), before);
}

#[gpui::test]
fn state_diagrams(cx: &mut TestAppContext) {
    let src = "stateDiagram-v2\n    [*] --> A\n    A --> B : go\n";
    let (viewer, cx) = open_src(cx, src);
    // Renaming a state and a transition (WYSIWY-22).
    let a = center(&viewer, cx, &node("A"));
    double_click(cx, a);
    assert_eq!(field(&viewer, cx).as_deref(), Some("A"));
    cx.simulate_input("Idle #1");
    keys(cx, "enter");
    let t = center(&viewer, cx, &Item::Edge(1));
    double_click(cx, t);
    assert_eq!(field(&viewer, cx).as_deref(), Some("go"));
    cx.simulate_input("start");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "stateDiagram-v2\n    [*] --> A\n    A --> B : start\n    A : Idle #35;1\n"
    );
    // Start states have no label.
    let start = center(&viewer, cx, &node(":[*]start"));
    double_click(cx, start);
    assert_eq!(field(&viewer, cx), None);

    // Dragging between states adds a transition, with its label open.
    let (a, b) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("B")),
    );
    drag(cx, b, a);
    assert_eq!(selection(&viewer, cx), [Item::Edge(2)]);
    assert_eq!(field(&viewer, cx).as_deref(), Some(""));
    cx.simulate_input("back");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    B --> A : back\n"),
        "{}",
        source(&viewer, cx)
    );
    // Into the start state is refused.
    let (b, start) = (
        center(&viewer, cx, &node("B")),
        center(&viewer, cx, &node(":[*]start")),
    );
    let before = source(&viewer, cx);
    drag(cx, b, start);
    assert_eq!(source(&viewer, cx), before);

    // Adding a state from the selected one adds a transition to it.
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("S1")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New state"));
    cx.simulate_input("Done");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    state \"Done\" as S1\n    B --> S1\n"),
        "{}",
        source(&viewer, cx)
    );

    // Deleting a state takes its transitions; undo brings them back.
    let before = source(&viewer, cx);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(Delete);
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "stateDiagram-v2\n    [*] --> A\n    A : Idle #35;1\n    state \"Done\" as S1\n"
    );
    assert!(selection(&viewer, cx).is_empty());
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), before);
}

#[gpui::test]
fn class_diagrams(cx: &mut TestAppContext) {
    let src = "classDiagram\n    <<interface>> A\n    A <|-- B : is\n";
    let (viewer, cx) = open_src(cx, src);
    // Renaming a class (its field leaves out the annotation) and a relation (WYSIWY-23).
    let a = center(&viewer, cx, &node("A"));
    double_click(cx, a);
    assert_eq!(field(&viewer, cx).as_deref(), Some("A"));
    cx.simulate_input("Shape #1");
    keys(cx, "enter");
    let r = center(&viewer, cx, &Item::Edge(0));
    double_click(cx, r);
    assert_eq!(field(&viewer, cx).as_deref(), Some("is"));
    cx.simulate_input("extends");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "classDiagram\n    <<interface>> A\n    A <|-- B : extends\n    class A[\"Shape #35;1\"]\n"
    );

    // Dragging between classes adds a relation, with its label open.
    let (a, b) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("B")),
    );
    drag(cx, b, a);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    assert_eq!(field(&viewer, cx).as_deref(), Some(""));
    cx.simulate_input("uses");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    B --> A : uses\n"),
        "{}",
        source(&viewer, cx)
    );

    // Adding a class from the selected one adds a relation to it.
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("C1")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New class"));
    cx.simulate_input("Circle");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    class C1[\"Circle\"]\n    B --> C1\n"),
        "{}",
        source(&viewer, cx)
    );

    // Deleting a class takes its relations; undo brings them back.
    let before = source(&viewer, cx);
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(Delete);
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "classDiagram\n    <<interface>> A\n    class A[\"Shape #35;1\"]\n    class C1[\"Circle\"]\n"
    );
    assert!(selection(&viewer, cx).is_empty());
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), before);
}

#[gpui::test]
fn er_diagrams(cx: &mut TestAppContext) {
    let src = "erDiagram\n    A ||--o{ B : has\n";
    let (viewer, cx) = open_src(cx, src);
    // Renaming an entity and a relationship (WYSIWY-24).
    let a = center(&viewer, cx, &node("A"));
    double_click(cx, a);
    assert_eq!(field(&viewer, cx).as_deref(), Some("A"));
    cx.simulate_input("Customer");
    keys(cx, "enter");
    let r = center(&viewer, cx, &Item::Edge(0));
    double_click(cx, r);
    assert_eq!(field(&viewer, cx).as_deref(), Some("has"));
    cx.simulate_input("places");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "erDiagram\n    A[\"Customer\"] ||--o{ B : \"places\"\n"
    );

    // Dragging between entities adds a relationship, with its label open.
    let (a, b) = (
        center(&viewer, cx, &node("A")),
        center(&viewer, cx, &node("B")),
    );
    drag(cx, b, a);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    cx.simulate_input("owns");
    keys(cx, "enter");
    assert!(
        source(&viewer, cx).ends_with("    B ||--o{ A : \"owns\"\n"),
        "{}",
        source(&viewer, cx)
    );

    // Cardinalities and the line, for the selected relationship.
    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(1)], cx));
    cx.dispatch_action(SetCardinality {
        start: false,
        marker: Arrow::OneMany,
    });
    cx.dispatch_action(SetIdentifying { identifying: false });
    assert!(
        source(&viewer, cx).ends_with("    B ||..|{ A : \"owns\"\n"),
        "{}",
        source(&viewer, cx)
    );

    // Adding an entity from the selected one adds a relationship to it; deleting it takes it.
    let b = center(&viewer, cx, &node("B"));
    click(cx, b, Modifiers::none());
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("E1")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New entity"));
    keys(cx, "escape");
    let before = source(&viewer, cx);
    assert!(
        before.ends_with("    E1[\"New entity\"]\n    B ||--o{ E1 : \"\"\n"),
        "{before}"
    );
    viewer.update(cx, |v, cx| v.select(vec![node("E1")], cx));
    cx.dispatch_action(Delete);
    cx.run_until_parked();
    assert!(!source(&viewer, cx).contains("E1"));
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), before);
}

#[gpui::test]
fn class_members(cx: &mut TestAppContext) {
    let src = "classDiagram\n    class Duck {\n        +swim()\n    }\n";
    let (viewer, cx) = open_src(cx, src);
    // Add Method opens the field over the class, and Enter adds the member (WYSIWY-23).
    viewer.update(cx, |v, cx| v.select(vec![node("Duck")], cx));
    cx.dispatch_action(AddMember { method: true });
    cx.run_until_parked();
    assert_eq!(field(&viewer, cx).as_deref(), Some("+method()"));
    cx.simulate_input("+quack()");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "classDiagram\n    class Duck {\n        +swim()\n        +quack()\n    }\n"
    );
    // Escape adds nothing.
    viewer.update(cx, |v, cx| v.select(vec![node("Duck")], cx));
    cx.dispatch_action(AddMember { method: false });
    cx.run_until_parked();
    assert_eq!(field(&viewer, cx).as_deref(), Some("+attribute"));
    keys(cx, "escape");
    viewer.update(cx, |v, cx| v.select(vec![node("Duck")], cx));
    cx.dispatch_action(RemoveMember { n: 0 });
    assert_eq!(
        source(&viewer, cx),
        "classDiagram\n    class Duck {\n        +quack()\n    }\n"
    );
    keys(cx, &format!("{SECONDARY}-z"));
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), src);
}

#[gpui::test]
fn requirement_diagrams(cx: &mut TestAppContext) {
    let src = "requirementDiagram\n    requirement r {\n        id: 1\n    }\n    element e {\n    }\n    e - satisfies -> r\n";
    let (viewer, cx) = open_src(cx, src);
    // Renaming a requirement renames it everywhere (WYSIWY-25).
    let r = center(&viewer, cx, &node("r"));
    double_click(cx, r);
    assert_eq!(field(&viewer, cx).as_deref(), Some("r"));
    cx.simulate_input("speed");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        src.replace("requirement r", "requirement speed")
            .replace("-> r", "-> speed")
    );
    // A relationship's label is its type: no field opens.
    let rel = center(&viewer, cx, &Item::Edge(0));
    double_click(cx, rel);
    assert_eq!(field(&viewer, cx), None);
    keys(cx, "escape");

    // Dragging between nodes adds a `traces` relationship, selected.
    let (speed, e) = (
        center(&viewer, cx, &node("speed")),
        center(&viewer, cx, &node("e")),
    );
    drag(cx, speed, e);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    cx.dispatch_action(SetRelationshipType {
        kind: "derives".into(),
    });
    assert!(
        source(&viewer, cx).ends_with("    speed - derives -> e\n"),
        "{}",
        source(&viewer, cx)
    );

    // Adding a requirement, then deleting it.
    viewer.update(cx, |v, cx| v.select(Vec::new(), cx));
    cx.dispatch_action(AddNode);
    cx.run_until_parked();
    assert_eq!(selection(&viewer, cx), [node("New requirement")]);
    assert_eq!(field(&viewer, cx).as_deref(), Some("New requirement"));
    keys(cx, "escape");
    let before = source(&viewer, cx);
    viewer.update(cx, |v, cx| v.select(vec![node("New requirement")], cx));
    cx.dispatch_action(Delete);
    cx.run_until_parked();
    assert!(!source(&viewer, cx).contains("New requirement"));
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), before);
}

#[gpui::test]
fn state_transitions_inside_composites(cx: &mut TestAppContext) {
    let src = "stateDiagram-v2\n    state W {\n        [*] --> a\n    }\n    a --> b\n";
    let (viewer, cx) = open_src(cx, src);
    // The new transition goes inside W, before `a --> b`, and is the one selected and labeled.
    let (start, a) = (
        center(&viewer, cx, &node("W:[*]start")),
        center(&viewer, cx, &node("a")),
    );
    drag(cx, start, a);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    cx.simulate_input("again");
    keys(cx, "enter");
    assert_eq!(
        source(&viewer, cx),
        "stateDiagram-v2\n    state W {\n        [*] --> a\n        [*] --> a : again\n    }\n    a --> b\n"
    );
}

#[gpui::test]
fn states_move_into_composites(cx: &mut TestAppContext) {
    let src = "stateDiagram-v2\n    a : desc\n    state W {\n        b\n    }\n    a --> b\n";
    let (viewer, cx) = open_src(cx, src);
    // Released on empty space in W, `a` moves into it with its statements (WYSIWY-22).
    let (lo, hi) = cx
        .update(|window, cx| viewer.update(cx, |v, _| v.subgraph_bounds("W", window)))
        .expect("W is drawn");
    let inside = gpui::point(lo.x + gpui::px(6.0), hi.y - gpui::px(6.0));
    let at = cx.update(|window, cx| viewer.update(cx, |v, _| v.subgraph_at(inside, window)));
    assert_eq!(at.as_deref(), Some("W"));
    let a = center(&viewer, cx, &node("a"));
    drag(cx, a, inside);
    assert_eq!(
        source(&viewer, cx),
        "stateDiagram-v2\n    state W {\n        b\n        a : desc\n        a --> b\n    }\n"
    );
    keys(cx, &format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), src);
}

/// ERRORH-1: scene owners from non-graph diagrams never index the flowchart model.
#[gpui::test]
fn read_only_scene_nodes_ignore_graph_selection(cx: &mut TestAppContext) {
    use sirene_core::scene::{Owner, Pt};
    for source in [
        "C4Context\nPerson(user, User)\n",
        "architecture-beta\nservice api(server)[API]\n",
    ] {
        let (viewer, vcx) = open_src(cx, source);
        let at = vcx.update(|window, cx| {
            viewer.update(cx, |v, _| {
                assert!(v.doc.other.is_some());
                assert!(v.doc.chart.nodes.is_empty());
                let scene = v.scene(window);
                let (lo, hi) = scene.bounds(Owner::Node(0)).expect("diagram draws a node");
                let center = Pt::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
                assert_eq!(scene.hit(center, 4.0), Some(Owner::Node(0)));
                let at = v.placement(window).map(center);
                assert_eq!(v.item_at(at, window), None);
                at
            })
        });
        click(vcx, at, Modifiers::none());
        vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        vcx.run_until_parked();
        assert!(selection(&viewer, vcx).is_empty());
        assert_eq!(viewer.read_with(vcx, |v, _| v.doc.source.clone()), source);
    }
}
