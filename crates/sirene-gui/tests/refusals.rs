//! Why an edit changes nothing, beside disabled menu items and in the status bar (WYSIWY-19).

mod common;

use common::{open, source};
use gpui::{Entity, TestAppContext, VisualTestContext};
use sirene_core::flowchart::edit::{Line, MAX_LENGTH};
use sirene_core::flowchart::{self, Stroke};
use sirene_gui::document::Document;
use sirene_gui::drawing::Item;
use sirene_gui::refusal;
use sirene_gui::viewer::Viewer;
use sirene_gui::{Group, LongerEdge, MoveEarlier, MoveLater, MoveTo, ShorterEdge, Ungroup};

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn line(length: u8) -> Line {
    Line {
        stroke: Stroke::Normal,
        start: None,
        end: None,
        length,
    }
}

#[test]
fn reasons() {
    let src = "flowchart TD\n    A --> B\n    subgraph s\n        C\n    end\n";
    assert_eq!(refusal::group(src, &[]), Some("no nodes are selected"));
    assert_eq!(
        refusal::group(src, &["A", "C"]),
        Some("the nodes are in different subgraphs")
    );
    assert_eq!(refusal::group(src, &["A", "B"]), None);

    let chart = |s: &str| flowchart::parse(s).lower().flowchart;
    let id = "flowchart TD\n    subgraph s\n        C\n    end\n    s e1@--> D\n";
    assert_eq!(
        refusal::ungroup(id, &chart(id), "s"),
        Some("an edge to it has an id")
    );
    let empty = "flowchart TD\n    subgraph s\n    end\n    s --> D\n";
    assert_eq!(
        refusal::ungroup(empty, &chart(empty), "s"),
        Some("it is empty and has edges")
    );
    let tight = "flowchart TD\n    subgraph s; C; end\n";
    assert_eq!(
        refusal::ungroup(tight, &chart(tight), "s"),
        Some("its first or last line has another statement")
    );
    assert_eq!(refusal::ungroup(src, &chart(src), "s"), None);
    assert_eq!(refusal::ungroup(src, &chart(src), "nope"), None);

    assert_eq!(
        refusal::move_item(src, &[node("A")], false),
        Some("nothing before it to swap with")
    );
    assert_eq!(refusal::move_item(src, &[node("A")], true), None);
    assert_eq!(
        refusal::move_item(src, &[Item::Edge(0)], false),
        Some("nothing before it to swap with")
    );
    assert_eq!(
        refusal::move_item(src, &[node("C")], true),
        Some("nothing after it to swap with")
    );
    assert_eq!(
        refusal::move_item(src, &[], true),
        Some("select one node or edge")
    );

    assert_eq!(refusal::length(&[], true), Some("no edges are selected"));
    assert_eq!(
        refusal::length(&[line(MAX_LENGTH)], true),
        Some("already the longest")
    );
    assert_eq!(
        refusal::length(&[line(1)], false),
        Some("already the shortest")
    );
    assert_eq!(refusal::length(&[line(1), line(MAX_LENGTH)], true), None);
    assert_eq!(refusal::length(&[line(2)], false), None);

    assert_eq!(refusal::label("Ungroup", None), "Ungroup");
    assert_eq!(refusal::label("Ungroup", Some("why")), "Ungroup (why)");
}

fn open_src(
    cx: &mut TestAppContext,
    src: &str,
) -> (Entity<Viewer>, &'static mut VisualTestContext) {
    let (viewer, cx) = open(cx, Document::from_source(None, src.to_owned()));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (viewer, cx)
}

fn status(viewer: &Entity<Viewer>, cx: &mut VisualTestContext) -> String {
    viewer.read_with(cx, |v, _| v.status())
}

#[gpui::test]
fn refused_actions_say_why(cx: &mut TestAppContext) {
    let src = "flowchart TD\n    A --> B\n    subgraph s\n        C\n    end\n    s e1@--> D\n    \
               subgraph t\n    end\n";
    let (viewer, cx) = open_src(cx, src);

    viewer.update(cx, |v, cx| v.select(vec![node("A"), node("C")], cx));
    cx.dispatch_action(Group);
    assert_eq!(
        status(&viewer, cx),
        "Can't group: the nodes are in different subgraphs"
    );

    cx.dispatch_action(Ungroup { id: "s".into() });
    assert_eq!(
        status(&viewer, cx),
        "Can't ungroup: an edge to it has an id"
    );

    viewer.update(cx, |v, cx| v.select(vec![node("A")], cx));
    cx.dispatch_action(MoveEarlier);
    assert_eq!(
        status(&viewer, cx),
        "Can't move earlier: nothing before it to swap with"
    );
    viewer.update(cx, |v, cx| v.select(vec![node("C")], cx));
    cx.dispatch_action(MoveLater);
    assert_eq!(
        status(&viewer, cx),
        "Can't move later: nothing after it to swap with"
    );

    viewer.update(cx, |v, cx| v.select(vec![Item::Edge(0)], cx));
    cx.dispatch_action(ShorterEdge);
    assert_eq!(status(&viewer, cx), "Can't shorten: already the shortest");
    assert_eq!(source(&viewer, cx), src);

    // A change clears the notice.
    cx.dispatch_action(LongerEdge);
    assert!(source(&viewer, cx).contains("A ---> B"));
    assert!(!status(&viewer, cx).starts_with("Can't"));
}

#[gpui::test]
fn refused_moves_say_why(cx: &mut TestAppContext) {
    // Moving a node into a subgraph declared before its own would take another node too.
    let before =
        "flowchart TD\n    subgraph t\n    end\n    subgraph s\n        A --> B\n    end\n";
    let (viewer, cx) = open_src(cx, before);
    viewer.update(cx, |v, cx| v.select(vec![node("A")], cx));
    // Already there is no refusal.
    cx.dispatch_action(MoveTo {
        subgraph: Some("s".into()),
    });
    assert!(!status(&viewer, cx).starts_with("Can't"));
    cx.dispatch_action(MoveTo {
        subgraph: Some("t".into()),
    });
    assert_eq!(
        status(&viewer, cx),
        format!("Can't move: {}", refusal::MOVE_TO)
    );
    assert_eq!(source(&viewer, cx), before);
}
