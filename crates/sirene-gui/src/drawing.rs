//! Editing on the drawing: selection, labels, adding, connecting, deleting, and undo
//! (WYSIWY-9..13).

use gpui::component::input::{InputEvent, InputState};
use gpui::{
    AppContext, Context, Entity, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Subscription, Window, point, px,
};
use sirene_core::diagram::Lang;
use sirene_core::flowchart::edit::{self, Line, TextEdit};
use sirene_core::flowchart::{self, Flowchart, Shape};
use sirene_core::scene::{Owner, Pt};

use crate::paint::Placement;
use crate::refusal;
use crate::viewer::{Viewer, place};
use crate::{
    AddNode, AddNodeBefore, AddShape, ConnectFrom, ConnectTo, Delete, Deselect, Group,
    KeepTogether, LongerEdge, MoveEarlier, MoveLater, MoveTo, Redo, Rename, ReverseEdge, SetArrows,
    SetDirection, SetShape, SetStroke, ShorterEdge, Undo, Ungroup,
};

/// A selectable diagram item: a node by id, or an edge by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Node(String),
    Edge(usize),
}

/// A pointer drag on the drawing.
pub(crate) enum Drag {
    /// From node `from` toward the pointer at `at`, adding an edge where it ends.
    Connect {
        from: String,
        start: Point<Pixels>,
        at: Point<Pixels>,
    },
    /// Moving the drawing with the pointer.
    Pan { last: Point<Pixels> },
}

/// How a new node is linked to an existing one.
pub(crate) enum Link {
    None,
    From(String),
    To(String),
}

/// The text field over an item whose label is being edited.
pub struct LabelEdit {
    pub item: Item,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

/// The flowchart `src` describes, read as `lang` when given.
fn chart(src: &str, lang: Option<Lang>) -> Flowchart {
    sirene_core::diagram::lower_as(src, lang)
        .diagram
        .into_graph()
}

/// How far, in screen pixels, a click may miss a stroke.
const SLOP: f32 = 4.0;
/// How far the pointer moves before a press on a node becomes a drag.
const DRAG: f32 = 4.0;
/// Label of nodes made by AddNode.
pub const NEW_NODE: &str = "New node";
/// Title of subgraphs made by Group.
pub const GROUP: &str = "Group";

impl Viewer {
    /// Where the drawing lands in the drawing area from the last paint.
    pub fn placement(&mut self, window: &Window) -> Placement {
        let scene = self.scene(window);
        place(&scene, self.zoom, self.pan, self.viewport.get())
    }

    /// The item drawn at window position `p`.
    pub fn item_at(&mut self, p: Point<Pixels>, window: &Window) -> Option<Item> {
        let sequence = self.doc.sequence().is_some();
        if self.doc.other.is_some() && !sequence {
            return None;
        }
        let at = self.placement(window);
        let q = Pt::new(
            f32::from(p.x - at.origin.x) / at.zoom,
            f32::from(p.y - at.origin.y) / at.zoom,
        );
        let owner = self.scene(window).hit(q, SLOP / at.zoom)?;
        if sequence {
            return self.sequence_item(owner);
        }
        match owner {
            Owner::Node(i) => self
                .doc
                .chart
                .nodes
                .get(i)
                .map(|n| Item::Node(n.id.clone())),
            Owner::Edge(i) => (i < self.doc.chart.edges.len()).then_some(Item::Edge(i)),
            Owner::Cluster(_) | Owner::Diagram => None,
        }
    }

    /// The scene owner of `item`, while it exists.
    pub fn owner(&self, item: &Item) -> Option<Owner> {
        if self.doc.sequence().is_some() {
            return self.sequence_owner(item);
        }
        let chart = &self.doc.chart;
        match item {
            Item::Node(id) => chart
                .nodes
                .iter()
                .position(|n| n.id == *id)
                .map(Owner::Node),
            Item::Edge(i) => (*i < chart.edges.len()).then_some(Owner::Edge(*i)),
        }
    }

    /// The scene owners of those `items` that exist, in order, looking nodes up by id once
    /// rather than per item.
    pub fn owners(&self, items: &[Item]) -> Vec<Owner> {
        if self.doc.sequence().is_some() {
            return items
                .iter()
                .filter_map(|i| self.sequence_owner(i))
                .collect();
        }
        let chart = &self.doc.chart;
        let mut ids: Option<std::collections::HashMap<&str, usize>> = None;
        items
            .iter()
            .filter_map(|item| match item {
                Item::Node(id) => {
                    let ids = ids.get_or_insert_with(|| {
                        let mut ids = std::collections::HashMap::new();
                        // The first node with an id, as `owner` finds.
                        for (i, n) in chart.nodes.iter().enumerate() {
                            ids.entry(n.id.as_str()).or_insert(i);
                        }
                        ids
                    });
                    ids.get(id.as_str()).copied().map(Owner::Node)
                }
                Item::Edge(_) => self.owner(item),
            })
            .collect()
    }

    /// The window bounds of what is drawn for `item` (top left, bottom right).
    pub fn item_bounds(
        &mut self,
        item: &Item,
        window: &Window,
    ) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let owner = self.owner(item)?;
        let (lo, hi) = self.scene(window).bounds(owner)?;
        let at = self.placement(window);
        Some((at.map(lo), at.map(hi)))
    }

    /// The window position of the middle of `item`.
    pub fn item_center(&mut self, item: &Item, window: &Window) -> Option<Point<Pixels>> {
        let (lo, hi) = self.item_bounds(item, window)?;
        Some(point((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
    }

    /// Selects `items`; one item also selects its source in the pane (WYSIWY-9).
    pub fn select(&mut self, items: Vec<Item>, cx: &mut Context<Self>) {
        self.selection = items;
        if let [item] = self.selection.as_slice() {
            let chart = &self.doc.chart;
            let range = if self.doc.sequence().is_some() {
                self.sequence_range(item)
            } else {
                match item {
                    Item::Node(id) => chart.node(id).and_then(|n| n.mentions.first().copied()),
                    Item::Edge(i) => chart.edges.get(*i).map(|e| e.range),
                }
                .map(|r| usize::from(r.start())..usize::from(r.end()))
            };
            if let Some(range) = range {
                self.source
                    .update(cx, |s, cx| s.set_selected_range(range, cx));
            }
        }
        cx.notify();
    }

    /// Drops selected items that no longer exist, and a label field whose item is gone.
    pub(crate) fn prune(&mut self) {
        if self.doc.sequence().is_some() {
            let mut selection = std::mem::take(&mut self.selection);
            selection.retain(|i| self.sequence_owner(i).is_some());
            self.selection = selection;
        } else {
            let ids: std::collections::HashSet<&str> =
                self.doc.chart.nodes.iter().map(|n| n.id.as_str()).collect();
            let edges = self.doc.chart.edges.len();
            self.selection.retain(|i| match i {
                Item::Node(id) => ids.contains(id.as_str()),
                Item::Edge(e) => *e < edges,
            });
        }
        if self
            .label
            .as_ref()
            .is_some_and(|l| self.owner(&l.item).is_none())
        {
            self.label = None;
        }
    }

    /// Applies a drawing change to the source pane's text (WYSIWY-13).
    pub fn apply(&mut self, edit: TextEdit, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(why) = self.doc.read_only() {
            self.refused("edit the drawing", Some(why), cx);
            return;
        }
        self.record(edit, window, cx);
    }

    /// Applies `edit` as one undoable drawing change, whatever the diagram's type.
    pub(crate) fn record(&mut self, edit: TextEdit, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.doc.source.clone();
        let after = edit.apply(&before);
        self.history.push((before, after));
        self.future.clear();
        self.write(edit, window, cx);
    }

    /// Replaces text in the editor as one undoable edit, and takes the result at once.
    fn write(&mut self, edit: TextEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.source.update(cx, |s, cx| {
            s.set_selected_range(edit.range, cx);
            s.replace(edit.text, window, cx);
        });
        self.edited(window, cx);
    }

    pub(crate) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        // The label field's own text comes first (GUISHE-12).
        if self.label.is_some() {
            cx.propagate();
            return;
        }
        let Some((before, after)) = self.history.pop() else {
            return;
        };
        if self.doc.source != after {
            self.history.clear();
            self.future.clear();
            return;
        }
        if let Some(e) = TextEdit::between(&after, &before) {
            self.write(e, window, cx);
        }
        self.future.push((before, after));
    }

    pub(crate) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_some() {
            cx.propagate();
            return;
        }
        let Some((before, after)) = self.future.pop() else {
            return;
        };
        if self.doc.source != before {
            self.history.clear();
            self.future.clear();
            return;
        }
        if let Some(e) = TextEdit::between(&before, &after) {
            self.write(e, window, cx);
        }
        self.history.push((before, after));
    }

    pub(crate) fn mouse_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.drawing_focus, cx);
        let hit = self.item_at(e.position, window);
        if e.click_count >= 2 {
            self.drag = None;
            match hit {
                Some(item) => {
                    self.select(vec![item.clone()], cx);
                    self.edit_label(item, window, cx);
                }
                // Double-clicking empty space adds a node there, in the subgraph under the
                // pointer if any (WYSIWY-15).
                None if self.doc.sequence().is_some() => self.add_participant(None, window, cx),
                None if self.doc.is_state() => self.add_state(Link::None, window, cx),
                None if self.doc.is_class() => self.add_class(Link::None, window, cx),
                None => {
                    let scope = self.subgraph_at(e.position, window);
                    self.add_linked(Shape::Rect, Link::None, scope, window, cx);
                }
            }
            return;
        }
        match hit {
            Some(item) => {
                if e.modifiers.shift {
                    let mut items = self.selection.clone();
                    match items.iter().position(|i| *i == item) {
                        Some(k) => {
                            items.remove(k);
                        }
                        None => items.push(item.clone()),
                    }
                    self.select(items, cx);
                } else if !self.selection.contains(&item) {
                    self.select(vec![item.clone()], cx);
                }
                self.drag = match item {
                    // A read-only drawing pans from anywhere.
                    _ if self.doc.read_only().is_some() && !self.doc.own_edits() => {
                        Some(Drag::Pan { last: e.position })
                    }
                    Item::Node(from) => Some(Drag::Connect {
                        from,
                        start: e.position,
                        at: e.position,
                    }),
                    Item::Edge(_) => None,
                };
            }
            None => {
                if !e.modifiers.shift {
                    self.select(Vec::new(), cx);
                }
                self.drag = Some(Drag::Pan { last: e.position });
            }
        }
    }

    pub(crate) fn mouse_move(
        &mut self,
        e: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !e.dragging() {
            self.drag = None;
            return;
        }
        match &mut self.drag {
            Some(Drag::Connect { at, .. }) => *at = e.position,
            Some(Drag::Pan { last }) => {
                self.pan += e.position - *last;
                *last = e.position;
            }
            None => return,
        }
        cx.notify();
    }

    pub(crate) fn mouse_up(
        &mut self,
        e: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Drag::Connect { from, start, .. }) = self.drag.take() else {
            cx.notify();
            return;
        };
        let moved = e.position - start;
        let far = f32::from(moved.x).hypot(f32::from(moved.y)) > DRAG;
        // On a sequence diagram: a message to the participant released on, or to a new one.
        if far && self.doc.sequence().is_some() {
            match self.item_at(e.position, window) {
                Some(Item::Node(to)) => self.add_message(&from, &to, window, cx),
                None if self.viewport.get().contains(&e.position) => {
                    self.add_participant(Some(from), window, cx);
                }
                _ => {}
            }
        } else if far && self.doc.is_state() {
            // A transition to the state released on, or to a new state.
            match self.item_at(e.position, window) {
                Some(Item::Node(to)) => self.add_transition(&from, &to, window, cx),
                None if self.viewport.get().contains(&e.position) => {
                    self.add_state(Link::From(from), window, cx);
                }
                _ => {}
            }
        } else if far && self.doc.is_class() {
            // A relation to the class released on, or to a new class.
            match self.item_at(e.position, window) {
                Some(Item::Node(to)) => self.add_relation(&from, &to, window, cx),
                None if self.viewport.get().contains(&e.position) => {
                    self.add_class(Link::From(from), window, cx);
                }
                _ => {}
            }
        } else if far {
            match self.item_at(e.position, window) {
                Some(Item::Node(to)) => {
                    let edit = edit::add_edge(&self.doc.source, &from, &to);
                    self.apply(edit, window, cx);
                }
                // Released on another subgraph: the node moves into it (WYSIWY-16). Elsewhere
                // in the drawing: a new node, linked, where it was released (WYSIWY-15).
                None if self.viewport.get().contains(&e.position) => {
                    let scope = self.subgraph_at(e.position, window);
                    if scope.is_some() && scope != self.parent_of(&from) {
                        let src = &self.doc.source;
                        match edit::move_to_subgraph(src, &from, scope.as_deref()) {
                            Some(edit) => self.apply(edit, window, cx),
                            None => self.refused("move", Some(refusal::MOVE_TO), cx),
                        }
                    } else {
                        self.add_linked(Shape::Rect, Link::From(from), scope, window, cx);
                    }
                }
                _ => {}
            }
        }
        cx.notify();
    }

    /// The line drawn while connecting: from the middle of the source node to the pointer.
    pub(crate) fn rubber_band(
        &mut self,
        window: &Window,
    ) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let Some(Drag::Connect { from, start, at }) = &self.drag else {
            return None;
        };
        let moved = *at - *start;
        if f32::from(moved.x).hypot(f32::from(moved.y)) <= DRAG {
            return None;
        }
        let (from, at) = (Item::Node(from.clone()), *at);
        Some((self.item_center(&from, window)?, at))
    }

    pub(crate) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_some() {
            return;
        }
        if self.doc.sequence().is_some() {
            self.sequence_delete(window, cx);
            return;
        }
        if self.doc.is_state() {
            self.state_delete(window, cx);
            return;
        }
        if self.doc.is_class() {
            self.class_delete(window, cx);
            return;
        }
        if let Some(why) = self.doc.read_only() {
            self.refused("delete", Some(why), cx);
            return;
        }
        let nodes: Vec<&str> = self
            .selection
            .iter()
            .filter_map(|i| match i {
                Item::Node(id) => Some(id.as_str()),
                Item::Edge(_) => None,
            })
            .collect();
        let edges: Vec<usize> = self
            .selection
            .iter()
            .filter_map(|i| match i {
                Item::Edge(e) => Some(*e),
                Item::Node(_) => None,
            })
            .collect();
        let edit = edit::delete(&self.doc.source, &nodes, &edges);
        self.select(Vec::new(), cx);
        if let Some(edit) = edit {
            self.apply(edit, window, cx);
        }
    }

    pub(crate) fn rename(&mut self, _: &Rename, window: &mut Window, cx: &mut Context<Self>) {
        // Enter in the label field reaches here too.
        if self.label.is_some() {
            return;
        }
        if let [item] = self.selection.as_slice() {
            let item = item.clone();
            self.edit_label(item, window, cx);
        }
    }

    pub(crate) fn deselect(&mut self, _: &Deselect, _: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_none() {
            self.select(Vec::new(), cx);
        }
    }

    /// Adds a node labeled [`NEW_NODE`] in `shape`, linked as `link` says, in subgraph `scope`
    /// (or at the top level), as one change; then selects it and opens its label (WYSIWY-11,
    /// WYSIWY-15).
    fn add_linked(
        &mut self,
        shape: Shape,
        link: Link,
        scope: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Shapes, links into a new node, and subgraphs do not apply: a participant, with a
        // message from the linked one.
        if self.doc.sequence().is_some() {
            let from = match link {
                Link::From(from) => Some(from),
                Link::None | Link::To(_) => None,
            };
            self.add_participant(from, window, cx);
            return;
        }
        // Shapes and subgraphs do not apply: a state, with a transition to or from the linked
        // one.
        if self.doc.is_state() {
            self.add_state(link, window, cx);
            return;
        }
        // Shapes and namespaces do not apply: a class, with a relation to or from the linked
        // one.
        if self.doc.is_class() {
            self.add_class(link, window, cx);
            return;
        }
        if let Some(why) = self.doc.read_only() {
            self.refused("add a node", Some(why), cx);
            return;
        }
        let src = self.doc.source.clone();
        let (id, add) = edit::add_node(&src, NEW_NODE);
        let mut out = add.apply(&src);
        if shape != Shape::Rect
            && let Some(e) = edit::set_node_shape(&out, &id, shape)
        {
            out = e.apply(&out);
        }
        match link {
            Link::None => {}
            Link::From(from) => out = edit::add_edge(&out, &from, &id).apply(&out),
            Link::To(to) => out = edit::add_edge(&out, &id, &to).apply(&out),
        }
        if scope.is_some()
            && let Some(e) = edit::move_to_subgraph(&out, &id, scope.as_deref())
        {
            out = e.apply(&out);
        }
        if let Some(edit) = TextEdit::between(&src, &out) {
            self.apply(edit, window, cx);
        }
        let item = Item::Node(id);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// Links a new node from the one selected node, if there is one.
    fn link_from_selection(&self) -> Link {
        match self.selection.as_slice() {
            [Item::Node(from)] => Link::From(from.clone()),
            _ => Link::None,
        }
    }

    pub(crate) fn add_node(&mut self, _: &AddNode, window: &mut Window, cx: &mut Context<Self>) {
        let link = self.link_from_selection();
        self.add_linked(Shape::Rect, link, None, window, cx);
    }

    pub(crate) fn add_shape(&mut self, a: &AddShape, window: &mut Window, cx: &mut Context<Self>) {
        let link = self.link_from_selection();
        self.add_linked(a.shape, link, None, window, cx);
    }

    /// The subgraph drawn at window position `p`, when no node or edge is.
    pub fn subgraph_at(&mut self, p: Point<Pixels>, window: &Window) -> Option<String> {
        let at = self.placement(window);
        let q = Pt::new(
            f32::from(p.x - at.origin.x) / at.zoom,
            f32::from(p.y - at.origin.y) / at.zoom,
        );
        match self.scene(window).hit(q, SLOP / at.zoom)? {
            Owner::Cluster(i) => self.doc.chart.subgraphs.get(i).map(|s| s.id.clone()),
            _ => None,
        }
    }

    /// The window bounds of subgraph `id` (top left, bottom right).
    pub fn subgraph_bounds(
        &mut self,
        id: &str,
        window: &Window,
    ) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let i = self.doc.chart.subgraphs.iter().position(|s| s.id == id)?;
        let (lo, hi) = self.scene(window).bounds(Owner::Cluster(i))?;
        let at = self.placement(window);
        Some((at.map(lo), at.map(hi)))
    }

    /// The subgraph node `id` is a member of.
    pub fn parent_of(&self, id: &str) -> Option<String> {
        self.doc
            .chart
            .subgraphs
            .iter()
            .find(|s| s.members.iter().any(|m| m == id))
            .map(|s| s.id.clone())
    }

    pub(crate) fn set_direction(
        &mut self,
        a: &SetDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let edit = edit::set_direction(&self.doc.source, a.subgraph.as_deref(), a.direction);
        if let Some(edit) = edit {
            self.apply(edit, window, cx);
        }
    }

    /// Says in the status bar why `action` changed nothing (WYSIWY-19).
    pub(crate) fn refused(&mut self, action: &str, reason: Option<&str>, cx: &mut Context<Self>) {
        if let Some(reason) = reason {
            self.notice = Some(refusal::notice(action, reason));
            cx.notify();
        }
    }

    pub(crate) fn group(&mut self, _: &Group, window: &mut Window, cx: &mut Context<Self>) {
        let nodes = self.selected_nodes();
        let ids: Vec<&str> = nodes.iter().map(String::as_str).collect();
        match edit::group(&self.doc.source, &ids, GROUP) {
            Some((_, edit)) => self.apply(edit, window, cx),
            None => {
                let why = refusal::group(&self.doc.source, &ids);
                self.refused("group", why, cx);
            }
        }
    }

    pub(crate) fn ungroup(&mut self, a: &Ungroup, window: &mut Window, cx: &mut Context<Self>) {
        match edit::ungroup(&self.doc.source, &a.id) {
            Some(edit) => self.apply(edit, window, cx),
            None => {
                let why = refusal::ungroup(&self.doc.source, &self.doc.chart, &a.id);
                self.refused("ungroup", why, cx);
            }
        }
    }

    pub(crate) fn keep_together(
        &mut self,
        _: &KeepTogether,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let [a, b] = self.selected_nodes().as_slice() {
            let edit = edit::keep_together(&self.doc.source, a, b);
            self.apply(edit, window, cx);
        }
    }

    pub(crate) fn move_to(&mut self, a: &MoveTo, window: &mut Window, cx: &mut Context<Self>) {
        let nodes = self.selected_nodes();
        let target = a.subgraph.clone();
        let lang = self.doc.lang();
        let mut stuck = false;
        self.change(window, cx, |src| {
            for id in &nodes {
                match edit::move_to_subgraph(src, id, target.as_deref()) {
                    Some(e) => *src = e.apply(src),
                    // Already there is not a refusal.
                    None => {
                        let now = chart(src, lang)
                            .subgraphs
                            .iter()
                            .find(|s| s.members.iter().any(|m| m == id))
                            .map(|s| s.id.clone());
                        stuck |= now != target;
                    }
                }
            }
        });
        if stuck {
            self.refused("move", Some(refusal::MOVE_TO), cx);
        }
    }

    pub(crate) fn move_earlier(
        &mut self,
        _: &MoveEarlier,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_item(false, window, cx);
    }

    pub(crate) fn move_later(
        &mut self,
        _: &MoveLater,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_item(true, window, cx);
    }

    /// Moves the one selected item's statement (WYSIWY-16); a moved edge stays selected.
    fn move_item(&mut self, later: bool, window: &mut Window, cx: &mut Context<Self>) {
        let src = &self.doc.source;
        let (edit, edge) = match self.selection.as_slice() {
            [Item::Node(id)] => (edit::move_node(src, id, later), None),
            [Item::Edge(i)] => (
                edit::move_edge(src, *i, later),
                self.doc.chart.edges.get(*i).cloned(),
            ),
            _ => return,
        };
        let Some(edit) = edit else {
            let why = refusal::move_item(&self.doc.source, &self.selection, later);
            self.refused(if later { "move later" } else { "move earlier" }, why, cx);
            return;
        };
        self.apply(edit, window, cx);
        if let Some(old) = edge {
            let same = |e: &flowchart::Edge| {
                e.from == old.from && e.to == old.to && e.label == old.label && e.id == old.id
            };
            if let Some(i) = self.doc.chart.edges.iter().position(same) {
                self.select(vec![Item::Edge(i)], cx);
            }
        }
    }

    pub(crate) fn add_node_before(
        &mut self,
        _: &AddNodeBefore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let [Item::Node(to)] = self.selection.as_slice() else {
            return;
        };
        let link = Link::To(to.clone());
        self.add_linked(Shape::Rect, link, None, window, cx);
    }

    /// The selected node ids.
    pub fn selected_nodes(&self) -> Vec<String> {
        self.selection
            .iter()
            .filter_map(|i| match i {
                Item::Node(id) => Some(id.clone()),
                Item::Edge(_) => None,
            })
            .collect()
    }

    /// The selected edge indexes.
    pub fn selected_edges(&self) -> Vec<usize> {
        self.selection
            .iter()
            .filter_map(|i| match i {
                Item::Edge(e) => Some(*e),
                Item::Node(_) => None,
            })
            .collect()
    }

    /// Applies the edits `f` makes, one after another, as one drawing change.
    fn change(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut String)) {
        let src = self.doc.source.clone();
        let mut out = src.clone();
        f(&mut out);
        if let Some(edit) = TextEdit::between(&src, &out) {
            self.apply(edit, window, cx);
        }
    }

    /// Sets the line of each selected edge to what `f` makes of it.
    fn change_lines(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(Line) -> Line,
    ) {
        let edges = self.selected_edges();
        let lang = self.doc.lang();
        self.change(window, cx, |out| {
            for i in edges {
                let Some(e) = chart(out, lang).edges.get(i).map(Line::of) else {
                    continue;
                };
                if let Some(edit) = edit::set_edge_line(out, i, f(e)) {
                    *out = edit.apply(out);
                }
            }
        });
    }

    pub(crate) fn set_shape(&mut self, a: &SetShape, window: &mut Window, cx: &mut Context<Self>) {
        let nodes = self.selected_nodes();
        self.change(window, cx, |out| {
            for id in nodes {
                if let Some(edit) = edit::set_node_shape(out, &id, a.shape) {
                    *out = edit.apply(out);
                }
            }
        });
    }

    pub(crate) fn set_stroke(
        &mut self,
        a: &SetStroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stroke = a.stroke;
        self.change_lines(window, cx, |l| Line { stroke, ..l });
    }

    pub(crate) fn set_arrows(
        &mut self,
        a: &SetArrows,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (start, end) = (a.start, a.end);
        self.change_lines(window, cx, |l| Line { start, end, ..l });
    }

    pub(crate) fn longer_edge(
        &mut self,
        _: &LongerEdge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_length(true, window, cx);
    }

    pub(crate) fn shorter_edge(
        &mut self,
        _: &ShorterEdge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_length(false, window, cx);
    }

    /// Makes the selected edges one step longer, or shorter, or says why not.
    fn change_length(&mut self, longer: bool, window: &mut Window, cx: &mut Context<Self>) {
        let lines: Vec<Line> = self
            .selected_edges()
            .iter()
            .filter_map(|&i| self.doc.chart.edges.get(i).map(Line::of))
            .collect();
        if let Some(why) = refusal::length(&lines, longer) {
            self.refused(if longer { "lengthen" } else { "shorten" }, Some(why), cx);
            return;
        }
        self.change_lines(window, cx, |l| Line {
            length: if longer {
                l.length.saturating_add(1)
            } else {
                l.length.saturating_sub(1)
            },
            ..l
        });
    }

    pub(crate) fn reverse_edge(
        &mut self,
        _: &ReverseEdge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // An edge in a chain moves to the end, so later indexes go first.
        let mut edges = self.selected_edges();
        edges.sort_unstable_by(|a, b| b.cmp(a));
        self.change(window, cx, |out| {
            for i in edges {
                if let Some(edit) = edit::reverse_edge(out, i) {
                    *out = edit.apply(out);
                }
            }
        });
        self.prune();
    }

    pub(crate) fn connect_to(
        &mut self,
        a: &ConnectTo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nodes = self.selected_nodes();
        self.change(window, cx, |out| {
            for from in nodes {
                *out = edit::add_edge(out, &from, &a.id).apply(out);
            }
        });
    }

    pub(crate) fn connect_from(
        &mut self,
        a: &ConnectFrom,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nodes = self.selected_nodes();
        self.change(window, cx, |out| {
            for to in nodes {
                *out = edit::add_edge(out, &a.id, &to).apply(out);
            }
        });
    }

    /// A right press selects what is under it (unless it is already selected) before the
    /// drawing's menu opens (WYSIWY-14).
    pub(crate) fn right_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.drawing_focus, cx);
        self.drag = None;
        self.menu_subgraph = None;
        match self.item_at(e.position, window) {
            Some(item) if !self.selection.contains(&item) => self.select(vec![item], cx),
            Some(_) => {}
            None => {
                self.select(Vec::new(), cx);
                // A press on a subgraph opens its menu (WYSIWY-16).
                self.menu_subgraph = self.subgraph_at(e.position, window);
            }
        }
    }

    /// Opens the label field over `item` (WYSIWY-10).
    pub fn edit_label(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let text = if self.doc.sequence().is_some() {
            self.sequence_label(&item)
        } else if self.doc.is_class() {
            self.class_label(&item)
        } else {
            if let Some(why) = self.doc.read_only()
                && !self.doc.is_state()
            {
                self.refused("rename", Some(why), cx);
                return;
            }
            // A state diagram's start and end states have no label.
            if self.doc.is_state()
                && let Item::Node(id) = &item
                && id.contains(":[*]")
            {
                return;
            }
            let chart = &self.doc.chart;
            match &item {
                Item::Node(id) => chart.node(id).map(|n| n.label.text.clone()),
                Item::Edge(i) => chart
                    .edges
                    .get(*i)
                    .map(|e| e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default()),
            }
        };
        let Some(text) = text else {
            return;
        };
        let text = text.replace('\n', "<br>");
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        // The field takes focus after the event that opened it, which may focus the drawing.
        let field = input.clone();
        cx.defer_in(window, move |_, window, cx| {
            field.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        });
        let subscription = cx.subscribe_in(&input, window, |v, _, e: &InputEvent, window, cx| {
            match e {
                InputEvent::PressEnter { .. } => v.commit_label(window, cx),
                // Focus has moved on; leave it there.
                InputEvent::Blur => {
                    v.label = None;
                    cx.notify();
                }
                InputEvent::Change | InputEvent::Focus => {}
            }
        });
        self.label = Some(LabelEdit {
            item,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Applies the label field's text and closes it.
    pub fn commit_label(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(label) = self.label.take() else {
            return;
        };
        let text = label.input.read(cx).value().replace("<br>", "\n");
        let src = &self.doc.source;
        if self.doc.sequence().is_some() {
            if let Some(edit) = self.sequence_rename(&label.item, &text) {
                self.record(edit, window, cx);
            }
        } else if self.doc.is_state() {
            if let Some(edit) = self.state_rename(&label.item, &text) {
                self.record(edit, window, cx);
            }
        } else if self.doc.is_class() {
            if let Some(edit) = self.class_rename(&label.item, &text) {
                self.record(edit, window, cx);
            }
        } else {
            let edit = match &label.item {
                Item::Node(id) => edit::set_node_label(src, id, &text),
                Item::Edge(i) => edit::set_edge_label(src, *i, &text),
            };
            if let Some(edit) = edit {
                self.apply(edit, window, cx);
            }
        }
        window.focus(&self.drawing_focus, cx);
        cx.notify();
    }

    /// Closes the label field without applying it, returning focus to the drawing.
    pub fn cancel_label(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.label.take().is_some() {
            window.focus(&self.drawing_focus, cx);
            cx.notify();
        }
    }

    /// Where the label field goes: over the item, at least `min` wide.
    pub(crate) fn label_field(
        &mut self,
        window: &Window,
        min: Pixels,
    ) -> Option<(Entity<InputState>, Point<Pixels>, Pixels)> {
        let label = self.label.as_ref()?;
        let (input, item) = (label.input.clone(), label.item.clone());
        let (lo, hi) = self.item_bounds(&item, window)?;
        let origin = self.viewport.get().origin;
        let width = (hi.x - lo.x).max(min);
        let mid = point((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        Some((
            input,
            point(mid.x - width / 2.0, mid.y - px(16.0)) - origin,
            width,
        ))
    }
}
