//! Drawing edits on a Mermaid sequence diagram (WYSIWY-21). Participants are `Item::Node` by
//! id and messages `Item::Edge` by their place among the diagram's messages.

use std::ops::Range;

use gpui::{Context, Window};
use sirene_core::flowchart::edit::TextEdit;
use sirene_core::mindmap;
use sirene_core::scene::Owner;
use sirene_core::sequence::edit;

use crate::SetMessageArrow;
use crate::drawing::{Item, NEW_NODE};
use crate::viewer::Viewer;

impl Viewer {
    /// Sets the arrow of the selected messages (WYSIWY-21).
    pub(crate) fn set_message_arrow(
        &mut self,
        a: &SetMessageArrow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.doc.sequence().is_none() {
            return;
        }
        let src = self.doc.source.clone();
        let mut out = src.clone();
        for n in self.selected_edges() {
            if let Some(e) = edit::set_message_arrow(&out, n, &a.arrow) {
                out = e.apply(&out);
            }
        }
        if let Some(e) = TextEdit::between(&src, &out) {
            self.record(e, window, cx);
        }
    }

    /// The mindmap node `item` names, by index (WYSIWY-26).
    fn mindmap_node(&self, item: &Item) -> Option<usize> {
        let map = self.doc.mindmap()?;
        match item {
            Item::Node(id) => id.parse().ok().filter(|&i| i < map.nodes.len()),
            Item::Edge(_) => None,
        }
    }

    /// The item scene owner `owner` draws, on a sequence diagram or a mindmap.
    pub(crate) fn sequence_item(&self, owner: Owner) -> Option<Item> {
        if let Some(map) = self.doc.mindmap() {
            return match owner {
                Owner::Node(i) if i < map.nodes.len() => Some(Item::Node(i.to_string())),
                _ => None,
            };
        }
        let seq = self.doc.sequence()?;
        match owner {
            Owner::Node(i) => seq.participants.get(i).map(|p| Item::Node(p.id.clone())),
            Owner::Edge(n) => (n < seq.messages().count()).then_some(Item::Edge(n)),
            Owner::Cluster(_) | Owner::Diagram => None,
        }
    }

    /// The scene owner of `item`, while it exists, on a sequence diagram.
    pub(crate) fn sequence_owner(&self, item: &Item) -> Option<Owner> {
        if self.doc.mindmap().is_some() {
            return self.mindmap_node(item).map(Owner::Node);
        }
        let seq = self.doc.sequence()?;
        match item {
            Item::Node(id) => seq
                .participants
                .iter()
                .position(|p| p.id == *id)
                .map(Owner::Node),
            Item::Edge(n) => (*n < seq.messages().count()).then_some(Owner::Edge(*n)),
        }
    }

    /// The source of `item`, on a sequence diagram.
    pub(crate) fn sequence_range(&self, item: &Item) -> Option<Range<usize>> {
        let src = &self.doc.source;
        if self.doc.mindmap().is_some() {
            return mindmap::edit::node_range(src, self.mindmap_node(item)?);
        }
        match item {
            Item::Node(id) => edit::participant_range(src, id),
            Item::Edge(n) => edit::message_range(src, *n),
        }
    }

    /// The text the label field starts with for `item`, on a sequence diagram.
    pub(crate) fn sequence_label(&self, item: &Item) -> Option<String> {
        if let Some(map) = self.doc.mindmap() {
            return Some(map.nodes[self.mindmap_node(item)?].label.text.clone());
        }
        let seq = self.doc.sequence()?;
        let lines = match item {
            Item::Node(id) => &seq.participants.iter().find(|p| p.id == *id)?.label,
            Item::Edge(n) => &seq.messages().nth(*n)?.text,
        };
        Some(lines.join("\n"))
    }

    /// The edit giving `item` the label `text`, on a sequence diagram.
    pub(crate) fn sequence_rename(&self, item: &Item, text: &str) -> Option<TextEdit> {
        let src = &self.doc.source;
        if self.doc.mindmap().is_some() {
            return mindmap::edit::set_label(src, self.mindmap_node(item)?, text);
        }
        match item {
            Item::Node(id) => edit::set_participant_label(src, id, text),
            Item::Edge(n) => edit::set_message_text(src, *n, text),
        }
    }

    /// Adds a participant, with a message to it from `from` when given, as one change; then
    /// selects it and opens its label.
    pub(crate) fn add_participant(
        &mut self,
        from: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let src = self.doc.source.clone();
        // On a mindmap: a child of `from`, of the one selected node, or of the root.
        if let Some(map) = self.doc.mindmap() {
            let selected = match self.selection.as_slice() {
                [item] => self.mindmap_node(item),
                _ => None,
            };
            let parent = from
                .and_then(|f| self.mindmap_node(&Item::Node(f)))
                .or(selected)
                .or((!map.nodes.is_empty()).then_some(0));
            let Some((i, e)) = mindmap::edit::add_node(&src, parent, NEW_NODE) else {
                return;
            };
            self.record(e, window, cx);
            let item = Item::Node(i.to_string());
            self.select(vec![item.clone()], cx);
            self.edit_label(item, window, cx);
            return;
        }
        let (id, add) = edit::add_participant(&src, NEW_NODE);
        let mut out = add.apply(&src);
        if let Some(from) = &from {
            out = edit::add_message(&out, from, &id, "").apply(&out);
        }
        if let Some(e) = TextEdit::between(&src, &out) {
            self.record(e, window, cx);
        }
        let item = Item::Node(id);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// Adds a message from `from` to `to` after the last statement; then selects it and opens
    /// its text.
    pub(crate) fn add_message(
        &mut self,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.doc.sequence().is_none() {
            return;
        }
        let e = edit::add_message(&self.doc.source, from, to, "");
        self.record(e, window, cx);
        let Some(seq) = self.doc.sequence() else {
            return;
        };
        let Some(n) = seq.messages().count().checked_sub(1) else {
            return;
        };
        let item = Item::Edge(n);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// Deletes the selected participants and messages.
    pub(crate) fn sequence_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.doc.mindmap().is_some() {
            let nodes: Vec<usize> = (self.selection.iter())
                .filter_map(|i| self.mindmap_node(i))
                .collect();
            let e = mindmap::edit::delete(&self.doc.source, &nodes);
            self.select(Vec::new(), cx);
            if let Some(e) = e {
                self.record(e, window, cx);
            }
            return;
        }
        let participants = self.selected_nodes();
        let participants: Vec<&str> = participants.iter().map(String::as_str).collect();
        let messages = self.selected_edges();
        let e = edit::delete(&self.doc.source, &participants, &messages);
        self.select(Vec::new(), cx);
        if let Some(e) = e {
            self.record(e, window, cx);
        }
    }
}
