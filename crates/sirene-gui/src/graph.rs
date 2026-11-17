//! Drawing edits on a diagram drawn from `Document::chart` that has its own edit module: a
//! Mermaid state (WYSIWY-22), class (WYSIWY-23), or ER (WYSIWY-24) diagram. Its nodes and edges
//! are selected as the flowchart's are; their source edits come from
//! `sirene_core::{state, class, er}::edit`.

use gpui::{Context, Window};
use sirene_core::flowchart::edit::TextEdit;
use sirene_core::{class, er, state};

use crate::document::Graph;
use crate::drawing::{Item, Link};
use crate::viewer::Viewer;
use crate::{SetCardinality, SetIdentifying};

/// Label of states made by AddNode.
pub const NEW_STATE: &str = "New state";
/// Label of classes made by AddNode.
pub const NEW_CLASS: &str = "New class";
/// Label of entities made by AddNode.
pub const NEW_ENTITY: &str = "New entity";

/// Why connecting two entities failed: one of them is gone.
const NO_ENTITY: &str = "a relationship joins two entities";

impl Graph {
    /// A new node labeled for this kind: its id and the edit.
    fn add_node(self, src: &str) -> (String, TextEdit) {
        match self {
            Graph::State => state::edit::add_state(src, NEW_STATE),
            Graph::Class => class::edit::add_class(src, NEW_CLASS),
            Graph::Er => er::edit::add_entity(src, NEW_ENTITY),
        }
    }

    /// An edge from `from` to `to`, or why there is none.
    fn connect(self, src: &str, from: &str, to: &str) -> Result<TextEdit, &'static str> {
        match self {
            Graph::State => {
                state::edit::add_transition(src, from, to).ok_or(state::edit::TRANSITION_REFUSED)
            }
            Graph::Class => {
                class::edit::add_relation(src, from, to).ok_or(class::edit::RELATION_REFUSED)
            }
            Graph::Er => er::edit::add_relationship(src, from, to).ok_or(NO_ENTITY),
        }
    }

    /// Whether node `id` has no label to edit.
    fn unlabeled(self, id: &str) -> bool {
        self == Graph::State && id.contains(":[*]")
    }
}

impl Viewer {
    /// Adds a node, with an edge to or from the linked one, as one change; then selects it and
    /// opens its label.
    pub(crate) fn graph_add(
        &mut self,
        graph: Graph,
        link: Link,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let src = self.doc.source.clone();
        let (id, add) = graph.add_node(&src);
        let mut out = add.apply(&src);
        let linked = match &link {
            Link::None => None,
            Link::From(from) => graph.connect(&out, from, &id).ok(),
            Link::To(to) => graph.connect(&out, &id, to).ok(),
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

    /// Adds an edge from `from` to `to`, then selects it and opens its label, or says why not.
    pub(crate) fn graph_connect(
        &mut self,
        graph: Graph,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let e = match graph.connect(&self.doc.source, from, to) {
            Ok(e) => e,
            Err(why) => {
                self.refused("connect", Some(why), cx);
                return;
            }
        };
        // The new edge is the one in the inserted text, which is not always the last: one at
        // a composite state's start or end goes inside the composite.
        let added = e.range.start..e.range.start + e.text.len();
        self.record(e, window, cx);
        let Some(n) = (self.doc.chart.edges.iter())
            .position(|t| added.contains(&usize::from(t.range.start())))
        else {
            return;
        };
        let item = Item::Edge(n);
        self.select(vec![item.clone()], cx);
        self.edit_label(item, window, cx);
    }

    /// The text the label field opens with for `item`, or `None` when it has no label.
    pub(crate) fn graph_label(&self, graph: Graph, item: &Item) -> Option<String> {
        match item {
            Item::Node(id) if graph.unlabeled(id) => None,
            Item::Node(id) if graph == Graph::Class => class::edit::label(&self.doc.source, id),
            Item::Node(id) => self.doc.chart.node(id).map(|n| n.label.text.clone()),
            Item::Edge(i) => self
                .doc
                .chart
                .edges
                .get(*i)
                .map(|e| e.label.as_ref().map(|l| l.text.clone()).unwrap_or_default()),
        }
    }

    /// The edit giving `item` the label `text`.
    pub(crate) fn graph_rename(&self, graph: Graph, item: &Item, text: &str) -> Option<TextEdit> {
        let src = &self.doc.source;
        match (graph, item) {
            (Graph::State, Item::Node(id)) => state::edit::set_state_label(src, id, text),
            (Graph::State, Item::Edge(i)) => state::edit::set_transition_label(src, *i, text),
            (Graph::Class, Item::Node(id)) => class::edit::set_class_label(src, id, text),
            (Graph::Class, Item::Edge(i)) => class::edit::set_relation_label(src, *i, text),
            (Graph::Er, Item::Node(id)) => er::edit::set_entity_label(src, id, text),
            (Graph::Er, Item::Edge(i)) => er::edit::set_relationship_label(src, *i, text),
        }
    }

    /// Deletes the selected nodes and edges.
    pub(crate) fn graph_delete(
        &mut self,
        graph: Graph,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nodes = self.selected_nodes();
        let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
        let edges = self.selected_edges();
        let src = &self.doc.source;
        let e = match graph {
            Graph::State => state::edit::delete(src, &nodes, &edges),
            Graph::Class => class::edit::delete(src, &nodes, &edges),
            Graph::Er => er::edit::delete(src, &nodes, &edges),
        };
        self.select(Vec::new(), cx);
        if let Some(e) = e {
            self.record(e, window, cx);
        }
    }

    /// Applies `f` to the source once per selected edge, as one change.
    fn graph_edges(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(&str, usize) -> Option<TextEdit>,
    ) {
        let src = self.doc.source.clone();
        let mut out = src.clone();
        for i in self.selected_edges() {
            if let Some(e) = f(&out, i) {
                out = e.apply(&out);
            }
        }
        if let Some(e) = TextEdit::between(&src, &out) {
            self.record(e, window, cx);
        }
    }

    /// Sets a cardinality of the selected relationships of an ER diagram (WYSIWY-24).
    pub(crate) fn set_cardinality(
        &mut self,
        a: &SetCardinality,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.doc.graph() != Some(Graph::Er) {
            return;
        }
        let (start, marker) = (a.start, a.marker);
        self.graph_edges(window, cx, |src, i| {
            er::edit::set_cardinality(src, i, start, marker)
        });
    }

    /// Makes the selected relationships of an ER diagram identifying or not (WYSIWY-24).
    pub(crate) fn set_identifying(
        &mut self,
        a: &SetIdentifying,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.doc.graph() != Some(Graph::Er) {
            return;
        }
        let identifying = a.identifying;
        self.graph_edges(window, cx, |src, i| {
            er::edit::set_identifying(src, i, identifying)
        });
    }
}
