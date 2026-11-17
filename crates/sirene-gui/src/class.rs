//! Drawing edits on a Mermaid class diagram (WYSIWY-23). Classes and relations are selected as
//! the flowchart's nodes and edges are; their source edits come from
//! `sirene_core::class::edit`.

use gpui::{Context, Window};
use sirene_core::class::edit;
use sirene_core::flowchart::edit::TextEdit;

use crate::drawing::{Item, Link};
use crate::viewer::Viewer;

/// Label of classes made by AddNode.
pub const NEW_CLASS: &str = "New class";

impl Viewer {
    /// Adds a class, with a relation to or from the linked class, as one change; then selects it
    /// and opens its label.
    pub(crate) fn add_class(&mut self, link: Link, window: &mut Window, cx: &mut Context<Self>) {
        let src = self.doc.source.clone();
        let (id, add) = edit::add_class(&src, NEW_CLASS);
        let mut out = add.apply(&src);
        let linked = match &link {
            Link::None => None,
            Link::From(from) => edit::add_relation(&out, from, &id),
            Link::To(to) => edit::add_relation(&out, &id, to),
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

    /// Adds a relation from `from` to `to`, then selects it and opens its label. One at a note
    /// is refused.
    pub(crate) fn add_relation(
        &mut self,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = edit::add_relation(&self.doc.source, from, to) else {
            self.refused("connect", Some(edit::RELATION_REFUSED), cx);
            return;
        };
        self.record(e, window, cx);
        // Relations are appended, so the new one is the last.
        let Some(n) = self.doc.chart.edges.len().checked_sub(1) else {
            return;
        };
        let item = Item::Edge(n);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// The text the label field opens with for `item`, on a class diagram: a class's label
    /// without its annotation.
    pub(crate) fn class_label(&self, item: &Item) -> Option<String> {
        match item {
            Item::Node(id) => edit::label(&self.doc.source, id),
            Item::Edge(i) => self
                .doc
                .chart
                .edges
                .get(*i)
                .map(|e| e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default()),
        }
    }

    /// The edit giving `item` the label `text`, on a class diagram.
    pub(crate) fn class_rename(&self, item: &Item, text: &str) -> Option<TextEdit> {
        let src = &self.doc.source;
        match item {
            Item::Node(id) => edit::set_class_label(src, id, text),
            Item::Edge(i) => edit::set_relation_label(src, *i, text),
        }
    }

    /// Deletes the selected classes and relations.
    pub(crate) fn class_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let classes = self.selected_nodes();
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
        let edges = self.selected_edges();
        let e = edit::delete(&self.doc.source, &classes, &edges);
        self.select(Vec::new(), cx);
        if let Some(e) = e {
            self.record(e, window, cx);
        }
    }
}
