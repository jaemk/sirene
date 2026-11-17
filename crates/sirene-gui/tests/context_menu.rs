//! Shape, line, and link edits from the drawing's right-click menu (WYSIWY-14).

mod common;

use common::{SECONDARY, open, source};
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};
use sirene_core::flowchart::{Arrow, Shape, Stroke};
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::viewer::Viewer;
use sirene_gui::{
    AddNodeBefore, ConnectFrom, ConnectTo, LongerEdge, ReverseEdge, SetArrows, SetShape, SetStroke,
    ShorterEdge,
};

const SRC: &str = "flowchart TD\n    A[Alpha] --> B[Beta]\n    B --> C\n";

fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (viewer, cx)
}

fn select(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, items: Vec<Item>) {
    viewer.update(cx, |v, cx| v.select(items, cx));
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn center(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, item: &Item) -> Point<Pixels> {
    cx.update(|window, cx| viewer.update(cx, |v, _| v.item_center(item, window)))
        .expect("item is drawn")
}

/// Right-clicks at `at` and picks the menu's first item.
fn first_item(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
}

#[gpui::test]
fn sequence_diagram_menu(cx: &mut TestAppContext) {
    let src = "sequenceDiagram\n    A->>B: hi\n";
    let (viewer, cx) = open_src(cx, src);
    // Flowchart edits stay refused (WYSIWY-21).
    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(SetShape {
        shape: Shape::Diamond,
    });
    assert_eq!(source(&viewer, cx), src);
    // The first item adds a participant with a message from the one right-clicked.
    let b = center(&viewer, cx, &node("B"));
    first_item(cx, b);
    assert_eq!(
        source(&viewer, cx),
        "sequenceDiagram\n    A->>B: hi\n    participant P1 as New node\n    B->>P1:\n"
    );
}

#[gpui::test]
fn state_diagram_menu(cx: &mut TestAppContext) {
    let src = "stateDiagram-v2\n    A --> B\n";
    let (viewer, cx) = open_src(cx, src);
    // Flowchart edits stay refused (WYSIWY-22).
    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(SetShape {
        shape: Shape::Diamond,
    });
    assert_eq!(source(&viewer, cx), src);
    // The first item adds a state with a transition from the one right-clicked.
    let b = center(&viewer, cx, &node("B"));
    first_item(cx, b);
    assert_eq!(
        source(&viewer, cx),
        "stateDiagram-v2\n    A --> B\n    state \"New state\" as S1\n    B --> S1\n"
    );
}

#[gpui::test]
fn class_diagram_menu(cx: &mut TestAppContext) {
    let src = "classDiagram\n    A <|-- B\n";
    let (viewer, cx) = open_src(cx, src);
    // Flowchart edits stay refused (WYSIWY-23).
    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(SetShape {
        shape: Shape::Diamond,
    });
    assert_eq!(source(&viewer, cx), src);
    // The first item adds a class with a relation from the one right-clicked.
    let b = center(&viewer, cx, &node("B"));
    first_item(cx, b);
    assert_eq!(
        source(&viewer, cx),
        "classDiagram\n    A <|-- B\n    class C1[\"New class\"]\n    B --> C1\n"
    );
}

#[gpui::test]
fn er_diagram_menu(cx: &mut TestAppContext) {
    let src = "erDiagram\n    A ||--o{ B : has\n";
    let (viewer, cx) = open_src(cx, src);
    // On a relationship: Add Entity, Rename, Start > Exactly One, Zero or One.
    let r = center(&viewer, cx, &Item::Edge(0));
    pick(cx, r, "down down down enter down enter");
    assert_eq!(source(&viewer, cx), "erDiagram\n    A |o--o{ B : has\n");
    // Identifying, after End.
    let r = center(&viewer, cx, &Item::Edge(0));
    pick(cx, r, "down down down down down enter");
    assert_eq!(source(&viewer, cx), "erDiagram\n    A |o..o{ B : has\n");
    // The first item on an entity adds one with a relationship from it.
    let b = center(&viewer, cx, &node("B"));
    first_item(cx, b);
    assert!(
        source(&viewer, cx).ends_with("    E1[\"New entity\"]\n    B ||--o{ E1 : \"\"\n"),
        "{}",
        source(&viewer, cx)
    );
}

#[gpui::test]
fn requirement_diagram_menu(cx: &mut TestAppContext) {
    let src = "requirementDiagram\n    requirement r {\n    }\n    element e {\n    }\n    e - satisfies -> r\n";
    let (viewer, cx) = open_src(cx, src);
    // On a relationship: Add Requirement, Type > Contains, ..., Verifies (WYSIWY-25).
    let rel = center(&viewer, cx, &Item::Edge(0));
    pick(cx, rel, "down down enter down down down down enter");
    assert_eq!(source(&viewer, cx), src.replace("satisfies", "verifies"));
    // The first item on a node adds a requirement with a relationship from it.
    let e = center(&viewer, cx, &node("e"));
    first_item(cx, e);
    assert!(
        source(&viewer, cx).ends_with(
            "    requirement \"New requirement\" {\n    }\n    e - traces -> \"New requirement\"\n"
        ),
        "{}",
        source(&viewer, cx)
    );
}

#[gpui::test]
fn shapes_and_links_for_the_selected_nodes(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    select(&viewer, cx, vec![node("A"), node("C")]);
    cx.dispatch_action(SetShape {
        shape: Shape::Diamond,
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A{Alpha} --> B[Beta]\n    B --> C{C}\n"
    );
    // One change, one undo.
    cx.simulate_keystrokes(&format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), SRC);

    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(SetShape {
        shape: Shape::Cloud,
    });
    assert!(source(&viewer, cx).contains("A[Alpha]@{ shape: cloud }"));

    select(&viewer, cx, vec![node("A"), node("B")]);
    cx.dispatch_action(ConnectTo { id: "C".into() });
    assert!(source(&viewer, cx).ends_with("    A --> C\n    B --> C\n"));
    cx.dispatch_action(ConnectFrom { id: "C".into() });
    assert!(source(&viewer, cx).ends_with("    C --> A\n    C --> B\n"));

    // A node before the selected one, linked to it, with its label open.
    select(&viewer, cx, vec![node("B")]);
    cx.dispatch_action(AddNodeBefore);
    cx.run_until_parked();
    let text = source(&viewer, cx);
    assert!(text.ends_with("    D[New node]\n    D --> B\n"), "{text}");
    assert_eq!(
        viewer.read_with(cx, |v, _| v.selection.clone()),
        [node("D")]
    );
    assert!(viewer.read_with(cx, |v, _| v.label.is_some()));
}

#[gpui::test]
fn lines_for_the_selected_edges(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    select(&viewer, cx, vec![Item::Edge(0), Item::Edge(1)]);
    cx.dispatch_action(SetStroke {
        stroke: Stroke::Thick,
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] ==> B[Beta]\n    B ==> C\n"
    );
    cx.dispatch_action(SetArrows {
        start: Some(Arrow::Circle),
        end: None,
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] o=== B[Beta]\n    B o=== C\n"
    );
    cx.dispatch_action(LongerEdge);
    cx.dispatch_action(LongerEdge);
    cx.dispatch_action(ShorterEdge);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] o==== B[Beta]\n    B o==== C\n"
    );

    select(&viewer, cx, vec![Item::Edge(1)]);
    cx.dispatch_action(ReverseEdge);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] o==== B[Beta]\n    C o==== B\n"
    );
}

#[gpui::test]
fn right_click_selects_and_opens_the_menu(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    // On a node: it is selected, and the first item (Rename) opens its label.
    let b = center(&viewer, cx, &node("B"));
    first_item(cx, b);
    assert_eq!(
        viewer.read_with(cx, |v, _| v.selection.clone()),
        [node("B")]
    );
    assert!(viewer.read_with(cx, |v, _| v.label.is_some()));
    viewer.update_in(cx, |v, window, cx| v.cancel_label(window, cx));

    // On empty space: the selection clears, and the first item is Add Node.
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    first_item(cx, area.origin + point(px(5.0), px(5.0)));
    let text = source(&viewer, cx);
    assert!(text.ends_with("    D[New node]\n"), "{text}");

    // On an edge: its first item is Rename too.
    let edge = center(&viewer, cx, &Item::Edge(1));
    first_item(cx, edge);
    assert_eq!(
        viewer.read_with(cx, |v, _| v.selection.clone()),
        [Item::Edge(1)]
    );
    assert!(viewer.read_with(cx, |v, _| v.label.is_some()));
}

/// Right-clicks at `at` and types `keys` into the menu.
fn pick(cx: &mut VisualTestContext, at: Point<Pixels>, keys: &str) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

#[gpui::test]
fn submenus_for_a_mixed_selection(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    select(&viewer, cx, vec![node("A"), Item::Edge(0)]);
    let a = center(&viewer, cx, &node("A"));
    // Up from Delete, past a disabled Shorter: Longer, Reverse, Arrows, Line > Solid, Thick.
    pick(cx, a, "up up up up up enter down enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] ==> B[Beta]\n    B --> C\n"
    );
    // Shape > Rect, Rounded.
    pick(cx, a, "down enter down enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A(Alpha) ==> B[Beta]\n    B --> C\n"
    );
}

#[gpui::test]
fn a_lone_node_has_nothing_to_connect(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, "flowchart TD\n    A\n");
    let a = center(&viewer, cx, &node("A"));
    // Rename, Shape, Add Node After: no Connect To in between.
    pick(cx, a, "down down down enter");
    cx.run_until_parked();
    let text = source(&viewer, cx);
    assert!(text.ends_with("    B[New node]\n    A --> B\n"), "{text}");
}
