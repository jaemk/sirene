//! Why an edit from the menus or keys would change nothing, shown beside a disabled menu item
//! and in the status bar when a key asks for it (WYSIWY-19).

use sirene_core::flowchart::Flowchart;
use sirene_core::flowchart::edit::{self, Line, MAX_LENGTH};

use crate::drawing::Item;

/// Why Move To makes no edit for a node not already in the target.
pub const MOVE_TO: &str = "it would move other nodes too";

/// Why Group makes no edit for nodes `ids`.
pub fn group(src: &str, ids: &[&str]) -> Option<&'static str> {
    if ids.is_empty() {
        return Some("no nodes are selected");
    }
    edit::group(src, ids, "")
        .is_none()
        .then_some("the nodes are in different subgraphs")
}

/// Why Ungroup makes no edit for subgraph `id`.
pub fn ungroup(src: &str, chart: &Flowchart, id: &str) -> Option<&'static str> {
    if edit::ungroup(src, id).is_some() {
        return None;
    }
    let sg = chart.subgraph(id)?;
    let linked: Vec<_> = chart
        .edges
        .iter()
        .filter(|e| e.from == id || e.to == id)
        .collect();
    Some(if linked.iter().any(|e| e.id.is_some()) {
        "an edge to it has an id"
    } else if !linked.is_empty() && sg.members.is_empty() {
        "it is empty and has edges"
    } else {
        "its first or last line has another statement"
    })
}

/// Why Move Earlier (Move Later with `later`) makes no edit for `selection`.
pub fn move_item(src: &str, selection: &[Item], later: bool) -> Option<&'static str> {
    let moved = match selection {
        [Item::Node(id)] => edit::move_node(src, id, later),
        [Item::Edge(i)] => edit::move_edge(src, *i, later),
        _ => return Some("select one node or edge"),
    };
    match (moved, later) {
        (Some(_), _) => None,
        (None, false) => Some("nothing before it to swap with"),
        (None, true) => Some("nothing after it to swap with"),
    }
}

/// Why Longer (Shorter without `longer`) makes no edit for the edges with `lines`.
pub fn length(lines: &[Line], longer: bool) -> Option<&'static str> {
    if lines.is_empty() {
        return Some("no edges are selected");
    }
    if longer && lines.iter().all(|l| l.length >= MAX_LENGTH) {
        Some("already the longest")
    } else if !longer && lines.iter().all(|l| l.length <= 1) {
        Some("already the shortest")
    } else {
        None
    }
}

/// A menu label with the reason it is disabled, if any.
pub fn label(name: &str, reason: Option<&str>) -> String {
    match reason {
        Some(r) => format!("{name} ({r})"),
        None => name.to_owned(),
    }
}

/// The status bar text for an action refused for `reason`.
pub fn notice(action: &str, reason: &str) -> String {
    format!("Can't {action}: {reason}")
}
