//! The styles panel: classes for the selected nodes, and a class editor (WYSIWY-17).

mod common;

use common::{SECONDARY, open, source};
use gpui::{Entity, Modifiers, Pixels, Point, Rgba, TestAppContext, VisualTestContext, hsla, px};
use sirene_gui::ToggleStyles;
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::styles::{DASH, hex};
use sirene_gui::viewer::Viewer;

const SRC: &str =
    "flowchart TD\n    A --> B\n    classDef hot fill:#f00,stroke:#900\n    class A hot\n";

fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (viewer, cx)
}

fn select(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, ids: &[&str]) {
    let items = ids.iter().map(|id| Item::Node((*id).to_owned())).collect();
    viewer.update(cx, |v, cx| v.select(items, cx));
}

fn editing(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| viewer.read(cx).styles.read(cx).editing.clone())
}

/// The middle of the panel control with debug selector `name`.
fn control(cx: &mut VisualTestContext, name: &'static str) -> Point<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} is drawn"))
        .center()
}

fn click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

#[test]
fn hex_colors() {
    assert_eq!(
        hex(Rgba {
            r: 1.0,
            g: 0.0,
            b: 0.5,
            a: 1.0
        }
        .into()),
        "#ff0080"
    );
    assert_eq!(hex(hsla(0.0, 0.0, 0.0, 0.5)), "#00000080");
}

#[gpui::test]
fn the_panel_toggles_and_edits_the_selections_class(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    select(&viewer, cx, &["A"]);
    cx.dispatch_action(ToggleStyles);
    cx.run_until_parked();
    assert!(viewer.read_with(cx, |v, _| v.show_styles));
    assert_eq!(editing(&viewer, cx).as_deref(), Some("hot"));
    // The pickers show the class's colors.
    let fill = cx.update(|_, cx| {
        let p = viewer.read(cx).styles.read(cx).pickers()[0].clone();
        p.read(cx).value().map(hex)
    });
    assert_eq!(fill.as_deref(), Some("#ff0000"));

    // Width and dashes write to the class.
    let two = control(cx, "width-2");
    click(cx, two);
    let dashed = control(cx, "dashed");
    click(cx, dashed);
    assert!(
        source(&viewer, cx).contains(&format!(
            "classDef hot fill:#f00,stroke:#900,stroke-width:2px,stroke-dasharray:{DASH}\n"
        )),
        "{}",
        source(&viewer, cx)
    );
    let solid = control(cx, "solid");
    click(cx, solid);
    assert!(source(&viewer, cx).contains("classDef hot fill:#f00,stroke:#900,stroke-width:2px\n"));

    // A picked color is written as hex; one change, one undo.
    cx.update(|window, cx| {
        let p = viewer.read(cx).styles.read(cx).pickers()[1].clone();
        p.update(cx, |p, cx| {
            p.select_color(hsla(0.0, 0.0, 0.0, 1.0), window, cx)
        });
    });
    cx.run_until_parked();
    assert!(
        source(&viewer, cx).contains("classDef hot fill:#f00,stroke:#000000,stroke-width:2px\n")
    );
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    click(
        cx,
        area.origin + gpui::point(px(5.0), area.size.height - px(5.0)),
    );
    cx.simulate_keystrokes(&format!("{SECONDARY}-z"));
    cx.run_until_parked();
    assert!(source(&viewer, cx).contains("classDef hot fill:#f00,stroke:#900,stroke-width:2px\n"));

    // Hiding keeps the editing class.
    cx.dispatch_action(ToggleStyles);
    assert!(!viewer.read_with(cx, |v, _| v.show_styles));
}

#[gpui::test]
fn classes_apply_to_the_selected_nodes(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, SRC);
    cx.dispatch_action(ToggleStyles);
    cx.run_until_parked();
    assert_eq!(editing(&viewer, cx), None);

    // Checking the class gives it to B; unchecking takes it off both.
    select(&viewer, cx, &["B"]);
    let hot = control(cx, "class-0");
    click(cx, hot);
    assert!(source(&viewer, cx).ends_with("    class A hot\n    class B hot\n"));
    select(&viewer, cx, &["A", "B"]);
    let hot = control(cx, "class-0");
    click(cx, hot);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    classDef hot fill:#f00,stroke:#900\n"
    );

    // New Class defines one, gives it to the selection, and edits it.
    let new = control(cx, "new-class");
    click(cx, new);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B\n    classDef hot fill:#f00,stroke:#900\n    classDef style1 fill:#e8f1fb,stroke:#3b6ea5\n    class A,B style1\n"
    );
    assert_eq!(editing(&viewer, cx).as_deref(), Some("style1"));
    // Edit switches the class being edited.
    let edit_hot = control(cx, "edit-class-0");
    click(cx, edit_hot);
    assert_eq!(editing(&viewer, cx).as_deref(), Some("hot"));
}

fn edge_color(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, row: usize) -> Option<String> {
    cx.update(|_, cx| {
        let p = viewer.read(cx).styles.read(cx).edge_pickers()[row].clone();
        p.read(cx).value().map(hex)
    })
}

#[gpui::test]
fn edges_get_link_styles(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(
        cx,
        "flowchart TD\n    A --> B --> C\n    linkStyle 0 stroke:#00f\n",
    );
    cx.dispatch_action(ToggleStyles);
    cx.run_until_parked();
    // No edge selected: no edge controls.
    assert!(cx.debug_bounds("edge-width-2").is_none());

    // The pickers follow the selected edge's colors.
    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(0)], cx));
    cx.run_until_parked();
    assert_eq!(edge_color(&viewer, cx, 0).as_deref(), Some("#0000ff"));
    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(1)], cx));
    cx.run_until_parked();
    assert_eq!(edge_color(&viewer, cx, 0), None);

    // Width, dashes, and colors apply to every selected edge as one change.
    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(0), Item::Edge(1)], cx));
    cx.run_until_parked();
    let three = control(cx, "edge-width-3");
    click(cx, three);
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B --> C\n    linkStyle 0 stroke:#00f,stroke-width:3px\n    linkStyle 1 stroke-width:3px\n"
    );
    let dashed = control(cx, "edge-dashed");
    click(cx, dashed);
    assert!(source(&viewer, cx).ends_with(&format!(
        "    linkStyle 1 stroke-width:3px,stroke-dasharray:{DASH}\n"
    )));
    let solid = control(cx, "edge-solid");
    click(cx, solid);
    cx.update(|window, cx| {
        let p = viewer.read(cx).styles.read(cx).edge_pickers()[1].clone();
        p.update(cx, |p, cx| {
            p.select_color(hsla(0.0, 1.0, 0.5, 1.0), window, cx)
        });
    });
    cx.run_until_parked();
    assert_eq!(
        source(&viewer, cx),
        "flowchart TD\n    A --> B --> C\n    linkStyle 0 stroke:#00f,stroke-width:3px,color:#ff0000\n    linkStyle 1 stroke-width:3px,color:#ff0000\n"
    );
    assert_eq!(edge_color(&viewer, cx, 1).as_deref(), Some("#ff0000"));
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    click(
        cx,
        area.origin + gpui::point(px(5.0), area.size.height - px(5.0)),
    );
    cx.simulate_keystrokes(&format!("{SECONDARY}-z"));
    cx.run_until_parked();
    assert!(source(&viewer, cx).ends_with("    linkStyle 1 stroke-width:3px\n"));
}
