//! The shape bar, double-clicking empty space, and AddShape (WYSIWY-15).

mod common;

use common::{SECONDARY, open, source};
use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};
use sirene_core::flowchart::Shape;
use sirene_gui::AddShape;
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::shape_bar::SHAPES;
use sirene_gui::viewer::Viewer;

const SRC: &str = "flowchart TD\n    A[Alpha] --> B[Beta]\n";

fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (viewer, cx)
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn selection(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Vec<Item> {
    viewer.read_with(cx, |v, _| v.selection.clone())
}

fn label_open(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> bool {
    viewer.read_with(cx, |v, _| v.label.is_some())
}

fn close_label(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) {
    viewer.update_in(cx, |v, window, cx| v.cancel_label(window, cx));
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

/// The middle of the bar button at `index`.
fn button(cx: &mut VisualTestContext, index: usize) -> Point<Pixels> {
    let selectors = [
        "shape-0", "shape-1", "shape-2", "shape-3", "shape-4", "shape-5", "shape-6", "shape-7",
    ];
    let b = cx
        .debug_bounds(selectors[index])
        .expect("the bar button is drawn");
    b.center()
}

#[gpui::test]
fn add_shape_links_from_the_selected_node(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    viewer.update(cx, |v, cx| v.select(vec![node("A")], cx));
    cx.dispatch_action(AddShape {
        shape: Shape::Diamond,
    });
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C{New node}\n    A --> C\n"
    );
    assert_eq!(selection(&viewer, cx), [node("C")]);
    assert!(label_open(&viewer, cx));
    close_label(&viewer, cx);

    // One change, one undo (with the drawing focused by a click on empty space).
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    cx.simulate_click(
        area.origin + point(px(10.0), area.size.height - px(10.0)),
        Modifiers::none(),
    );
    cx.simulate_keystrokes(&format!("{SECONDARY}-z"));
    assert_eq!(source(&viewer, cx), SRC);

    // With nothing selected, the node stands alone; named shapes go in metadata.
    viewer.update(cx, |v, cx| v.select(Vec::new(), cx));
    cx.dispatch_action(AddShape {
        shape: Shape::Cloud,
    });
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C[New node]@{ shape: cloud }\n"
    );
}

#[gpui::test]
fn the_bar_adds_its_shapes(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    assert_eq!(SHAPES[3], Shape::Diamond);
    viewer.update(cx, |v, cx| v.select(vec![node("B")], cx));
    let diamond = button(cx, 3);
    cx.simulate_click(diamond, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C{New node}\n    B --> C\n"
    );
    assert_eq!(selection(&viewer, cx), [node("C")]);
    assert!(label_open(&viewer, cx));
    close_label(&viewer, cx);

    // The bar's first button is a plain node, and a press on the bar keeps the selection.
    let rect = button(cx, 0);
    cx.simulate_click(rect, Modifiers::none());
    cx.run_until_parked();
    assert!(
        source(&viewer, cx).ends_with("    D[New node]\n    C --> D\n"),
        "{}",
        source(&viewer, cx)
    );
    // Presses on the bar do not reach the drawing: a double-click there is two clicks on the
    // button, not a third node from the drawing's double-click.
    close_label(&viewer, cx);
    let before = source(&viewer, cx).matches("New node").count();
    double_click(cx, rect);
    let after = source(&viewer, cx).matches("New node").count();
    assert!(after - before <= 2, "{}", source(&viewer, cx));
}

#[gpui::test]
fn double_clicking_empty_space_adds_a_node(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    viewer.update(cx, |v, cx| v.select(vec![node("A")], cx));
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    // Below the bar, away from the nodes.
    double_click(
        cx,
        area.origin + point(px(10.0), area.size.height - px(10.0)),
    );
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A[Alpha] --> B[Beta]\n    C[New node]\n"
    );
    assert_eq!(selection(&viewer, cx), [node("C")]);
    assert!(label_open(&viewer, cx));
}
