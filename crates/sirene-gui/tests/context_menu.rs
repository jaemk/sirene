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
}
