//! Drawing edits on a Mermaid sequence diagram (WYSIWY-21). Participants are `Item::Node` by
//! id and messages `Item::Edge` by their place among the diagram's messages.

use std::ops::Range;

use gpui::{Context, Window};
use sirene_core::flowchart::edit::TextEdit;
use sirene_core::scene::Owner;
use sirene_core::sequence::edit;

use crate::drawing::{Item, NEW_NODE};
use crate::viewer::Viewer;

impl Viewer {
    /// The item scene owner `owner` draws, on a sequence diagram.
    pub(crate) fn sequence_item(&self, owner: Owner) -> Option<Item> {
        let seq = self.doc.sequence()?;
        match owner {
            Owner::Node(i) => seq.participants.get(i).map(|p| Item::Node(p.id.clone())),
            Owner::Edge(n) => (n < seq.messages().count()).then_some(Item::Edge(n)),
            Owner::Cluster(_) | Owner::Diagram => None,
        }
    }

    /// The scene owner of `item`, while it exists, on a sequence diagram.
    pub(crate) fn sequence_owner(&self, item: &Item) -> Option<Owner> {
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
        match item {
            Item::Node(id) => edit::participant_range(src, id),
            Item::Edge(n) => edit::message_range(src, *n),
        }
    }

    /// The text the label field starts with for `item`, on a sequence diagram.
    pub(crate) fn sequence_label(&self, item: &Item) -> Option<String> {
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
