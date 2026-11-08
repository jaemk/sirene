//! The drawing's right-click menu (WYSIWY-14).

use gpui::component::menu::PopupMenu;
use gpui::{Context, WeakEntity, Window, px};
use sirene_core::flowchart::edit::Line;
use sirene_core::flowchart::{Arrow, Direction, Flowchart, Node, Shape, Stroke};

use crate::drawing::Item;
use crate::refusal;
use crate::viewer::Viewer;
use crate::{
    AddNode, AddNodeBefore, ConnectFrom, ConnectTo, Delete, FitWindow, Group, KeepTogether,
    LongerEdge, MoveEarlier, MoveLater, MoveTo, Rename, ReverseEdge, SelectAll, SetArrows,
    SetDirection, SetShape, SetStroke, ShorterEdge, Ungroup,
};

/// The directions in Direction, with their names.
pub const DIRECTIONS: [(&str, Direction); 4] = [
    ("Top to Bottom", Direction::TopBottom),
    ("Left to Right", Direction::LeftRight),
    ("Bottom to Top", Direction::BottomTop),
    ("Right to Left", Direction::RightLeft),
];

/// The strokes in Line, with their names.
pub const STROKES: [(&str, Stroke); 4] = [
    ("Solid", Stroke::Normal),
    ("Thick", Stroke::Thick),
    ("Dotted", Stroke::Dotted),
    ("Invisible", Stroke::Invisible),
];

/// The arrowheads in Arrows: name, start, end.
pub const ARROWS: [(&str, Option<Arrow>, Option<Arrow>); 6] = [
    ("Arrow at End", None, Some(Arrow::Point)),
    ("Arrow at Start", Some(Arrow::Point), None),
    (
        "Arrows at Both Ends",
        Some(Arrow::Point),
        Some(Arrow::Point),
    ),
    ("No Arrows", None, None),
    ("Circle at End", None, Some(Arrow::Circle)),
    ("Cross at End", None, Some(Arrow::Cross)),
];

/// The tallest a list of nodes or shapes gets before it scrolls.
const LIST_HEIGHT: f32 = 420.0;

/// How a node is named in Connect To and Connect From: its label's first line, and its id
/// when that differs.
fn node_name(n: &Node) -> String {
    let label = n.label.lines().into_iter().next().unwrap_or_default();
    if label.is_empty() || label == n.id {
        n.id.clone()
    } else {
        format!("{label} ({})", n.id)
    }
}

/// The menu for what `viewer` has selected: the selection's own items, or the drawing's when
/// nothing is selected. Its actions go to the drawing.
pub(crate) fn build(
    viewer: &WeakEntity<Viewer>,
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let Some(viewer) = viewer.upgrade() else {
        return menu;
    };
    let (nodes, edges, chart, src, subgraph, focus) = {
        let v = viewer.read(cx);
        (
            v.selected_nodes(),
            v.selected_edges(),
            v.doc.chart.clone(),
            v.doc.source.clone(),
            v.menu_subgraph.clone(),
            v.drawing_focus.clone(),
        )
    };
    let mut menu = menu.action_context(focus);
    if nodes.is_empty() && edges.is_empty() {
        menu = menu.menu("Add Node", Box::new(AddNode)).separator();
        // On a subgraph: its direction, and Ungroup (WYSIWY-16).
        let current = match &subgraph {
            Some(id) => chart.subgraph(id).and_then(|s| s.direction),
            None => Some(chart.direction),
        };
        menu = direction_items(menu, subgraph.clone(), current, window, cx);
        if let Some(id) = subgraph {
            let why = refusal::ungroup(&src, &chart, &id);
            menu = menu.menu_with_disabled(
                refusal::label("Ungroup", why),
                Box::new(Ungroup { id }),
                why.is_some(),
            );
        }
        return menu
            .separator()
            .menu("Select All", Box::new(SelectAll))
            .menu("Fit Window", Box::new(FitWindow));
    }
    if nodes.len() + edges.len() == 1 {
        menu = menu.menu("Rename", Box::new(Rename));
    }
    if !nodes.is_empty() {
        menu = node_items(menu, &nodes, &chart, window, cx);
        menu = layout_items(menu, &nodes, &chart, &src, window, cx);
    }
    if !edges.is_empty() {
        menu = edge_items(menu, &edges, &chart, window, cx);
    }
    if let [edge] = edges.as_slice()
        && nodes.is_empty()
    {
        menu = move_items(menu.separator(), &src, &[Item::Edge(*edge)]);
    }
    menu.separator().menu("Delete", Box::new(Delete))
}

/// The four directions, with `current` checked, for the diagram or `subgraph`.
fn direction_items(
    menu: PopupMenu,
    subgraph: Option<String>,
    current: Option<Direction>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    menu.submenu("Direction", window, cx, move |m, _, _| {
        DIRECTIONS.iter().fold(m, |m, &(name, direction)| {
            m.menu_with_check(
                name,
                current == Some(direction),
                Box::new(SetDirection {
                    direction,
                    subgraph: subgraph.clone(),
                }),
            )
        })
    })
}

/// Group, Keep Together, Move To, and Move Earlier or Later for the selected nodes
/// (WYSIWY-16).
fn layout_items(
    menu: PopupMenu,
    nodes: &[String],
    chart: &Flowchart,
    src: &str,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let ids: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let why = refusal::group(src, &ids);
    let mut menu = menu.separator().menu_with_disabled(
        refusal::label("Group", why),
        Box::new(Group),
        why.is_some(),
    );
    if nodes.len() == 2 {
        menu = menu.menu("Keep Together", Box::new(KeepTogether));
    }
    // Where the one selected node is now, checked in Move To.
    let parent = match nodes {
        [id] => Some(
            chart
                .subgraphs
                .iter()
                .find(|s| s.members.iter().any(|m| m == id))
                .map(|s| s.id.clone()),
        ),
        _ => None,
    };
    let targets: Vec<(String, String)> = chart
        .subgraphs
        .iter()
        .map(|s| {
            let title = s.title.lines().into_iter().next().unwrap_or_default();
            let name = if title.is_empty() || title == s.id {
                s.id.clone()
            } else {
                format!("{title} ({})", s.id)
            };
            (s.id.clone(), name)
        })
        .collect();
    if !targets.is_empty() {
        menu = menu.submenu("Move To", window, cx, move |m, _, _| {
            let top = parent.as_ref().is_some_and(Option::is_none);
            let m = m
                .menu_with_check("Top Level", top, Box::new(MoveTo { subgraph: None }))
                .separator();
            targets.iter().fold(
                m.scrollable(true).max_h(px(LIST_HEIGHT)),
                |m, (id, name)| {
                    let here = parent.as_ref().is_some_and(|p| p.as_deref() == Some(id));
                    m.menu_with_check(
                        name.clone(),
                        here,
                        Box::new(MoveTo {
                            subgraph: Some(id.clone()),
                        }),
                    )
                },
            )
        });
    }
    if let [id] = nodes {
        menu = move_items(menu, src, &[Item::Node(id.clone())]);
    }
    menu
}

/// Move Earlier and Move Later for the one item in `selection`, with why either is disabled.
fn move_items(menu: PopupMenu, src: &str, selection: &[Item]) -> PopupMenu {
    let earlier = refusal::move_item(src, selection, false);
    let later = refusal::move_item(src, selection, true);
    menu.menu_with_disabled(
        refusal::label("Move Earlier", earlier),
        Box::new(MoveEarlier),
        earlier.is_some(),
    )
    .menu_with_disabled(
        refusal::label("Move Later", later),
        Box::new(MoveLater),
        later.is_some(),
    )
}

fn node_items(
    menu: PopupMenu,
    nodes: &[String],
    chart: &Flowchart,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let current = match nodes {
        [id] => chart.node(id).map(|n| n.shape),
        _ => None,
    };
    let mut menu = menu.submenu("Shape", window, cx, move |m, window, cx| {
        shape_items(m, current, window, cx)
    });
    if nodes.len() == 1 {
        menu = menu
            .menu("Add Node After", Box::new(AddNode))
            .menu("Add Node Before", Box::new(AddNodeBefore));
    }
    let others: Vec<(String, String)> = chart
        .nodes
        .iter()
        .filter(|n| !nodes.contains(&n.id))
        .map(|n| (n.id.clone(), node_name(n)))
        .collect();
    if others.is_empty() {
        return menu;
    }
    let to = others.clone();
    menu.submenu("Connect To", window, cx, move |m, _, _| {
        to.iter().fold(
            m.scrollable(true).max_h(px(LIST_HEIGHT)),
            |m, (id, name)| m.menu(name.clone(), Box::new(ConnectTo { id: id.clone() })),
        )
    })
    .submenu("Connect From", window, cx, move |m, _, _| {
        others.iter().fold(
            m.scrollable(true).max_h(px(LIST_HEIGHT)),
            |m, (id, name)| m.menu(name.clone(), Box::new(ConnectFrom { id: id.clone() })),
        )
    })
}

/// The classic shapes, then the rest under More.
fn shape_items(
    menu: PopupMenu,
    current: Option<Shape>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let item = move |m: PopupMenu, shape: Shape| {
        m.menu_with_check(
            shape.title(),
            current == Some(shape),
            Box::new(SetShape { shape }),
        )
    };
    let menu = Shape::ALL
        .iter()
        .filter(|s| s.delimiters().is_some())
        .fold(menu, |m, &s| item(m, s));
    menu.separator()
        .submenu("More", window, cx, move |m, _, _| {
            Shape::ALL
                .iter()
                .filter(|s| s.delimiters().is_none())
                .fold(m.scrollable(true).max_h(px(LIST_HEIGHT)), |m, &s| {
                    item(m, s)
                })
        })
}

fn edge_items(
    menu: PopupMenu,
    edges: &[usize],
    chart: &Flowchart,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let line = match edges {
        [i] => chart.edges.get(*i).map(Line::of),
        _ => None,
    };
    let lines: Vec<Line> = edges
        .iter()
        .filter_map(|&i| chart.edges.get(i).map(Line::of))
        .collect();
    let (longer, shorter) = (
        refusal::length(&lines, true),
        refusal::length(&lines, false),
    );
    menu.submenu("Line", window, cx, move |m, _, _| {
        STROKES.iter().fold(m, |m, &(name, stroke)| {
            m.menu_with_check(
                name,
                line.is_some_and(|l| l.stroke == stroke),
                Box::new(SetStroke { stroke }),
            )
        })
    })
    .submenu("Arrows", window, cx, move |m, _, _| {
        ARROWS.iter().fold(m, |m, &(name, start, end)| {
            m.menu_with_check(
                name,
                line.is_some_and(|l| l.start == start && l.end == end),
                Box::new(SetArrows { start, end }),
            )
        })
    })
    .menu("Reverse", Box::new(ReverseEdge))
    .menu_with_disabled(
        refusal::label("Longer", longer),
        Box::new(LongerEdge),
        longer.is_some(),
    )
    .menu_with_disabled(
        refusal::label("Shorter", shorter),
        Box::new(ShorterEdge),
        shorter.is_some(),
    )
}
