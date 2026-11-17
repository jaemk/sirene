//! Drawing edits on a Mermaid state diagram (WYSIWY-22). States and transitions are selected
//! as the flowchart's nodes and edges are; their source edits come from
//! `sirene_core::state::edit`.

use gpui::{Context, Window};
use sirene_core::flowchart::edit::TextEdit;
use sirene_core::state::edit;

use crate::drawing::{Item, Link};
use crate::viewer::Viewer;

/// Label of states made by AddNode.
pub const NEW_STATE: &str = "New state";

impl Viewer {
    /// Adds a state, with a transition to or from the linked state, as one change; then selects
    /// it and opens its label.
    pub(crate) fn add_state(&mut self, link: Link, window: &mut Window, cx: &mut Context<Self>) {
        let src = self.doc.source.clone();
        let (id, add) = edit::add_state(&src, NEW_STATE);
        let mut out = add.apply(&src);
        let linked = match &link {
            Link::None => None,
            Link::From(from) => edit::add_transition(&out, from, &id),
            Link::To(to) => edit::add_transition(&out, &id, to),
        };
        if let Some(e) = linked {
            out = e.apply(&out);
        }
        if let Some(e) = TextEdit::between(&src, &out) {
            self.record(e, window, cx);
        }
        let item = Item::Node(id);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// Adds a transition from `from` to `to`, then selects it and opens its label. A transition
    /// into a start state or out of an end state is refused.
    pub(crate) fn add_transition(
        &mut self,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = edit::add_transition(&self.doc.source, from, to) else {
            self.refused("connect", Some(edit::TRANSITION_REFUSED), cx);
            return;
        };
        self.record(e, window, cx);
        let Some(n) = self.doc.chart.edges.len().checked_sub(1) else {
            return;
        };
        let item = Item::Edge(n);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// The edit giving `item` the label `text`, on a state diagram.
    pub(crate) fn state_rename(&self, item: &Item, text: &str) -> Option<TextEdit> {
        let src = &self.doc.source;
        match item {
            Item::Node(id) => edit::set_state_label(src, id, text),
            Item::Edge(i) => edit::set_transition_label(src, *i, text),
        }
    }

    /// Deletes the selected states and transitions.
    pub(crate) fn state_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let states = self.selected_nodes();
        let states: Vec<&str> = states.iter().map(String::as_str).collect();
        let edges = self.selected_edges();
        let e = edit::delete(&self.doc.source, &states, &edges);
        self.select(Vec::new(), cx);
        if let Some(e) = e {
            self.record(e, window, cx);
        }
    }
}
