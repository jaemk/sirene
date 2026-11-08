//! Direction, grouping, keeping together, and moving statements from the drawing (WYSIWY-16).

mod common;

use common::{open, source};
use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};
use sirene_core::flowchart::Direction;
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::viewer::Viewer;
use sirene_gui::{Group, KeepTogether, MoveEarlier, MoveLater, MoveTo, SetDirection, Ungroup};

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

fn select(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, items: Vec<Item>) {
    viewer.update(cx, |v, cx| v.select(items, cx));
}

fn selection(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Vec<Item> {
    viewer.read_with(cx, |v, _| v.selection.clone())
}

fn center(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, item: &Item) -> Point<Pixels> {
    cx.update(|window, cx| viewer.update(cx, |v, _| v.item_center(item, window)))
        .expect("item is drawn")
}

/// A point inside subgraph `id`, just in from its top left corner.
fn corner(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, id: &str) -> Point<Pixels> {
    let (lo, _) = cx
        .update(|window, cx| viewer.update(cx, |v, _| v.subgraph_bounds(id, window)))
        .expect("subgraph is drawn");
    lo + point(px(4.0), px(4.0))
}

fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
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

/// Right-clicks at `at` and types `keys` into the menu.
fn pick(cx: &mut VisualTestContext, at: Point<Pixels>, keys: &str) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn close_label(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) {
    viewer.update_in(cx, |v, window, cx| v.cancel_label(window, cx));
}

const SUB: &str = "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n    end\n";

#[gpui::test]
fn directions(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SUB);
    cx.dispatch_action(SetDirection {
        direction: Direction::LeftRight,
        subgraph: None,
    });
    cx.dispatch_action(SetDirection {
        direction: Direction::RightLeft,
        subgraph: Some("s".into()),
    });
    // Already so: nothing changes.
    cx.dispatch_action(SetDirection {
        direction: Direction::RightLeft,
        subgraph: Some("s".into()),
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart LR\n    A --> B\n    subgraph s [S]\n        direction RL\n        C\n    end\n"
    );

    // From the menus: empty space, then the subgraph.
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    let empty = area.origin + point(px(5.0), area.size.height - px(5.0));
    // Add Node, Direction > Top to Bottom.
    pick(cx, empty, "down down enter enter");
    assert!(source(&viewer, cx).starts_with("flowchart TD\n"));
    let s = corner(&viewer, cx, "s");
    // Add Node, Direction > Top to Bottom, Left to Right.
    pick(cx, s, "down down enter down enter");
    assert!(source(&viewer, cx).contains("        direction LR\n"));
    assert_eq!(
        viewer.read_with(cx, |v, _| v.menu_subgraph.clone()),
        Some("s".into())
    );
}

#[gpui::test]
fn grouping_and_ungrouping(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, "flowchart TD\n    A --> B\n    B --> C\n");
    select(&viewer, cx, vec![node("A"), node("B")]);
    cx.dispatch_action(Group);
    let grouped = "flowchart TD\n    A --> B\n    B --> C\n    subgraph group1 [Group]\n        A\n        B\n    end\n";
    assert_eq!(source(&viewer, cx), grouped);
    // An edge cannot be grouped, and selecting none groups nothing.
    select(&viewer, cx, vec![Item::Edge(0)]);
    cx.dispatch_action(Group);
    assert_eq!(source(&viewer, cx), grouped);

    cx.dispatch_action(Ungroup {
        id: "group1".into(),
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    B --> C\n    A\n    B\n"
    );

    // From the subgraph's menu: Add Node, Direction, Ungroup.
    let (viewer, cx) = open_src(cx, grouped);
    let g = corner(&viewer, cx, "group1");
    pick(cx, g, "down down down enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    B --> C\n    A\n    B\n"
    );
}

#[gpui::test]
fn keeping_together_and_moving_in(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SUB);
    select(&viewer, cx, vec![node("A"), node("C")]);
    cx.dispatch_action(KeepTogether);
    assert!(source(&viewer, cx).ends_with("    end\n    A ~~~ C\n"));
    // Two nodes only.
    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(KeepTogether);
    assert!(source(&viewer, cx).ends_with("    end\n    A ~~~ C\n"));

    let (viewer, cx) = open_src(cx, SUB);
    // A node's menu lists the subgraphs under Move To; dismissing it changes nothing.
    let c = center(&viewer, cx, &node("C"));
    pick(cx, c, "escape");
    assert_eq!(source(&viewer, cx), SUB);
    select(&viewer, cx, vec![node("A"), node("B")]);
    cx.dispatch_action(MoveTo {
        subgraph: Some("s".into()),
    });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n        A\n        B\n    end\n"
    );
    select(&viewer, cx, vec![node("A")]);
    cx.dispatch_action(MoveTo { subgraph: None });
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n        B\n    end\n    A\n"
    );
}

#[gpui::test]
fn dragging_onto_subgraphs(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SUB);
    // Dropping A on the subgraph moves it in.
    let a = center(&viewer, cx, &node("A"));
    let s = corner(&viewer, cx, "s");
    drag(cx, a, s);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n        A\n    end\n"
    );
    // Dropping C on its own subgraph adds a linked node inside it.
    let c = center(&viewer, cx, &node("C"));
    let s = corner(&viewer, cx, "s");
    drag(cx, c, s);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    subgraph s [S]\n        C\n        A\n        D\n    end\n    D[New node]\n    C --> D\n"
    );
    assert_eq!(selection(&viewer, cx), [node("D")]);
    close_label(&viewer, cx);
    // Double-clicking in the subgraph adds a node there.
    let s = corner(&viewer, cx, "s");
    double_click(cx, s);
    assert!(source(&viewer, cx).contains("        D\n        E\n    end\n"));
    let parent = viewer.read_with(cx, |v, _| v.parent_of("E"));
    assert_eq!(parent.as_deref(), Some("s"));
}

#[gpui::test]
fn moving_statements(cx: &mut TestAppContext) {
    let src = "flowchart TD\n    A --> B\n    C --> D\n";
    let (viewer, cx) = open_src(cx, src);
    select(&viewer, cx, vec![node("C")]);
    cx.dispatch_action(MoveEarlier);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    C --> D\n    A --> B\n"
    );
    // An edge stays selected as its index changes.
    select(&viewer, cx, vec![Item::Edge(0)]);
    cx.dispatch_action(MoveLater);
    assert_eq!(source(&viewer, cx), src);
    assert_eq!(selection(&viewer, cx), [Item::Edge(1)]);
    // At the end, nothing moves; nor with two items.
    cx.dispatch_action(MoveLater);
    select(&viewer, cx, vec![node("A"), node("C")]);
    cx.dispatch_action(MoveEarlier);
    assert_eq!(source(&viewer, cx), src);

    // From the menus, up from Delete: one node's menu ends with Move Earlier and Move Later
    // (disabled for the last statement).
    select(&viewer, cx, vec![node("C")]);
    let c = center(&viewer, cx, &node("C"));
    pick(cx, c, "up up enter");
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    C --> D\n    A --> B\n"
    );
    // An edge's menu ends with Move Earlier, Move Later, Delete.
    let e = center(&viewer, cx, &Item::Edge(0));
    pick(cx, e, "up up enter");
    assert_eq!(source(&viewer, cx), src);
}
