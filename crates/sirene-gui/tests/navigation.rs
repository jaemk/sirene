//! Moving around the drawing in gpui's headless test platform (GUISHE-8).

mod common;

use common::{SECONDARY, open};
use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::navigation::{PAN_FAR, PAN_STEP, SELECTION_ZOOM};
use sirene_gui::viewer::Viewer;
use sirene_gui::{ActualSize, Pan, ZoomToSelection};

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

/// A left-to-right chain of `n` nodes, wider than the window.
fn wide(n: usize) -> String {
    let chain: Vec<String> = (0..n).map(|i| format!("N{i}[A long node label]")).collect();
    format!("flowchart LR\n{}\n", chain.join(" --> "))
}

/// Opens `src` in an active window, with the drawing focused.
fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    // Bottom right of the drawing area is empty in every test diagram.
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    let corner = area.origin + point(area.size.width - px(5.0), area.size.height - px(5.0));
    cx.simulate_click(corner, Modifiers::none());
    cx.run_until_parked();
    (viewer, cx)
}

fn keys(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn zoom_pan(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> (f32, Point<Pixels>) {
    viewer.read_with(cx, |v, _| (v.zoom, v.pan))
}

fn selection(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> Vec<Item> {
    viewer.read_with(cx, |v, _| v.selection.clone())
}

/// Whether `item` is drawn wholly inside the drawing area.
fn in_view(viewer: &Entity<Viewer>, cx: &mut VisualTestContext, item: &Item) -> bool {
    cx.update(|window, cx| {
        viewer.update(cx, |v, _| {
            let (lo, hi) = v.item_bounds(item, window).expect("item is drawn");
            let a = v.drawing_area();
            lo.x >= a.origin.x
                && lo.y >= a.origin.y
                && hi.x <= a.origin.x + a.size.width
                && hi.y <= a.origin.y + a.size.height
        })
    })
}

#[gpui::test]
fn large_drawings_fit_on_open(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::from_source(None, wide(20)));
    let (zoom, pan) = zoom_pan(&viewer, vcx);
    assert!(zoom < 1.0, "{zoom}");
    assert_eq!(pan, Point::default());
    let width = vcx.update(|window, cx| viewer.update(cx, |v, _| v.scene(window).width));
    let area = viewer.read_with(vcx, |v, _| v.drawing_area());
    assert!(
        (zoom * width - f32::from(area.size.width)).abs() < 2.0,
        "{zoom} {width}"
    );

    // Small drawings are not magnified.
    let (small, vcx) =
        common::open_another(cx, Document::from_source(None, "flowchart\nA\n".into()));
    assert_eq!(zoom_pan(&small, vcx).0, 1.0);

    // Showing another document in a blank window fits it too.
    let (blank, vcx) = common::open_another(cx, Document::untitled());
    let dir = tempfile::tempdir().unwrap();
    let path = common::write(dir.path(), "wide.mmd", &wide(40));
    blank.update_in(vcx, |v, window, cx| v.show_paths(&[path], window, cx));
    vcx.run_until_parked();
    assert!(zoom_pan(&blank, vcx).0 < 1.0);
}

#[gpui::test]
fn zoom_to_selection(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, &wide(40));
    cx.dispatch_action(ActualSize);
    // Nothing selected: nothing happens.
    cx.dispatch_action(ZoomToSelection);
    assert_eq!(zoom_pan(&viewer, cx), (1.0, Point::default()));

    let far = node("N30");
    viewer.update(cx, |v, cx| v.select(vec![far.clone()], cx));
    keys(cx, &format!("{SECONDARY}-8"));
    let (zoom, _) = zoom_pan(&viewer, cx);
    assert_eq!(zoom, SELECTION_ZOOM);
    assert!(in_view(&viewer, cx, &far));
    let center = cx
        .update(|window, cx| viewer.update(cx, |v, _| v.item_center(&far, window)))
        .unwrap();
    let area = viewer.read_with(cx, |v, _| v.drawing_area());
    let mid = area.center();
    assert!((center.x - mid.x).abs() < px(1.0) && (center.y - mid.y).abs() < px(1.0));

    // A wide selection zooms out to fit it.
    let ten: Vec<Item> = (0..10).map(|i| node(&format!("N{i}"))).collect();
    viewer.update(cx, |v, cx| v.select(ten.clone(), cx));
    cx.dispatch_action(ZoomToSelection);
    assert!(zoom_pan(&viewer, cx).0 < 1.0);
    for item in [&ten[0], &ten[9]] {
        assert!(in_view(&viewer, cx, item), "{item:?}");
    }
}

#[gpui::test]
fn arrow_keys_pan(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, "flowchart\nA --> B\n");
    let pan = |cx: &mut VisualTestContext| zoom_pan(&viewer, cx).1;
    cx.dispatch_action(Pan::new(3.0, -4.0));
    assert_eq!(pan(cx), point(px(3.0), px(-4.0)));
    cx.dispatch_action(ActualSize);
    keys(cx, "right");
    assert_eq!(pan(cx), point(px(-PAN_STEP), px(0.0)));
    keys(cx, "shift-left");
    assert_eq!(pan(cx), point(px(PAN_FAR - PAN_STEP), px(0.0)));
    keys(cx, "down");
    keys(cx, "up");
    keys(cx, "up");
    assert_eq!(pan(cx), point(px(PAN_FAR - PAN_STEP), px(PAN_STEP)));

    // While a label is being edited, the arrows move its cursor instead.
    viewer.update_in(cx, |v, window, cx| v.edit_label(node("A"), window, cx));
    cx.run_until_parked();
    keys(cx, "up");
    keys(cx, "left");
    assert_eq!(pan(cx), point(px(PAN_FAR - PAN_STEP), px(PAN_STEP)));
}

#[gpui::test]
fn keyboard_selection(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, "flowchart TD\nA --> B --> C\n");
    keys(cx, &format!("{SECONDARY}-a"));
    assert_eq!(
        selection(&viewer, cx),
        [
            node("A"),
            node("B"),
            node("C"),
            Item::Edge(0),
            Item::Edge(1)
        ]
    );
    viewer.update(cx, |v, cx| v.select(Vec::new(), cx));
    keys(cx, "tab");
    assert_eq!(selection(&viewer, cx), [node("A")]);
    keys(cx, "tab");
    keys(cx, "tab");
    assert_eq!(selection(&viewer, cx), [node("C")]);
    keys(cx, "tab");
    assert_eq!(selection(&viewer, cx), [node("A")]);
    keys(cx, "shift-tab");
    assert_eq!(selection(&viewer, cx), [node("C")]);
    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(0)], cx));
    keys(cx, "shift-tab");
    assert_eq!(selection(&viewer, cx), [node("C")]);

    // Keys go to the label field while it is open.
    viewer.update_in(cx, |v, window, cx| v.edit_label(node("B"), window, cx));
    cx.run_until_parked();
    keys(cx, &format!("{SECONDARY}-a"));
    keys(cx, "tab");
    assert_eq!(selection(&viewer, cx), [node("C")]);
}

#[gpui::test]
fn tab_brings_the_node_into_view(cx: &mut TestAppContext) {
    let (viewer, cx) = open_src(cx, &wide(40));
    cx.dispatch_action(ActualSize);
    viewer.update(cx, |v, cx| v.select(vec![node("N30")], cx));
    assert!(!in_view(&viewer, cx, &node("N30")));
    keys(cx, "tab");
    assert_eq!(selection(&viewer, cx), [node("N31")]);
    assert!(in_view(&viewer, cx, &node("N31")));
    assert_eq!(zoom_pan(&viewer, cx).0, 1.0);
    // A node already in view does not move the drawing.
    let pan = zoom_pan(&viewer, cx).1;
    keys(cx, "shift-tab");
    assert_eq!(zoom_pan(&viewer, cx).1, pan);
    // Nor does an empty diagram fail.
    viewer.update_in(cx, |v, window, cx| {
        v.set_document(
            Document::from_source(None, "flowchart\n".into()),
            window,
            cx,
        )
    });
    keys(cx, "tab");
    assert!(selection(&viewer, cx).is_empty());
}
