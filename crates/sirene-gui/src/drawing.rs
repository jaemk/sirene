//! Editing on the drawing: selection, labels, adding, connecting, deleting, and undo
//! (WYSIWY-9..13).

use gpui::component::input::{InputEvent, InputState};
use gpui::{
    AppContext, Context, Entity, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Subscription, Window, point, px,
};
use sirene_core::flowchart::edit::{self, TextEdit};
use sirene_core::scene::{Owner, Pt};

use crate::paint::Placement;
use crate::viewer::{Viewer, place};
use crate::{AddNode, Delete, Deselect, Redo, Rename, Undo};

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

/// The text field over an item whose label is being edited.
pub struct LabelEdit {
    pub item: Item,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

/// How far, in screen pixels, a click may miss a stroke.
const SLOP: f32 = 4.0;
/// How far the pointer moves before a press on a node becomes a drag.
const DRAG: f32 = 4.0;
/// Label of nodes made by AddNode.
pub const NEW_NODE: &str = "New node";

impl Viewer {
    /// Where the drawing lands in the drawing area from the last paint.
    pub fn placement(&mut self, window: &Window) -> Placement {
        let scene = self.scene(window);
        place(&scene, self.zoom, self.pan, self.viewport.get())
    }

    /// The item drawn at window position `p`.
    pub fn item_at(&mut self, p: Point<Pixels>, window: &Window) -> Option<Item> {
        let at = self.placement(window);
        let q = Pt::new(
            f32::from(p.x - at.origin.x) / at.zoom,
            f32::from(p.y - at.origin.y) / at.zoom,
        );
        match self.scene(window).hit(q, SLOP / at.zoom)? {
            Owner::Node(i) => Some(Item::Node(self.doc.chart.nodes[i].id.clone())),
            Owner::Edge(i) => Some(Item::Edge(i)),
            Owner::Cluster(_) | Owner::Diagram => None,
        }
    }

    /// The scene owner of `item`, while it exists.
    pub fn owner(&self, item: &Item) -> Option<Owner> {
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
            let range = match item {
                Item::Node(id) => chart.node(id).and_then(|n| n.mentions.first().copied()),
                Item::Edge(i) => chart.edges.get(*i).map(|e| e.range),
            };
            if let Some(r) = range {
                let range = usize::from(r.start())..usize::from(r.end());
                self.source
                    .update(cx, |s, cx| s.set_selected_range(range, cx));
            }
        }
        cx.notify();
    }

    /// Drops selected items that no longer exist, and a label field whose item is gone.
    pub(crate) fn prune(&mut self) {
        let selection = std::mem::take(&mut self.selection);
        self.selection = selection
            .into_iter()
            .filter(|i| self.owner(i).is_some())
            .collect();
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
        if e.click_count >= 2
            && let Some(item) = hit
        {
            self.drag = None;
            self.select(vec![item.clone()], cx);
            self.edit_label(item, window, cx);
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
        if far && let Some(Item::Node(to)) = self.item_at(e.position, window) {
            let edit = edit::add_edge(&self.doc.source, &from, &to);
            self.apply(edit, window, cx);
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

    pub(crate) fn add_node(&mut self, _: &AddNode, window: &mut Window, cx: &mut Context<Self>) {
        let src = self.doc.source.clone();
        let (id, add) = edit::add_node(&src, NEW_NODE);
        let mut out = add.apply(&src);
        if let [Item::Node(from)] = self.selection.as_slice() {
            out = edit::add_edge(&out, from, &id).apply(&out);
        }
        if let Some(edit) = TextEdit::between(&src, &out) {
            self.apply(edit, window, cx);
        }
        let item = Item::Node(id);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// Opens the label field over `item` (WYSIWY-10).
    pub fn edit_label(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let chart = &self.doc.chart;
        let text = match &item {
            Item::Node(id) => chart.node(id).map(|n| n.label.text.clone()),
            Item::Edge(i) => chart
                .edges
                .get(*i)
                .map(|e| e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default()),
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
        let edit = match &label.item {
            Item::Node(id) => edit::set_node_label(src, id, &text),
            Item::Edge(i) => edit::set_edge_label(src, *i, &text),
        };
        if let Some(edit) = edit {
            self.apply(edit, window, cx);
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
