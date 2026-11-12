//! Conversion of a flowchart into a layout graph.

use std::collections::HashMap;

use super::model::{Direction, Flowchart, Label, Node, Note, NoteSide, Shape, Subgraph};
use crate::layout::{ClusterSpec, EdgeSpec, Graph, Item, NodeSpec, NoteSpec, Size};

/// Sizes of flowchart elements in layout cells.
pub trait Measure {
    fn node(&self, node: &Node) -> Size;
    /// The size of an edge label, or `None` when it takes no room.
    fn edge_label(&self, label: &Label) -> Option<Size>;
    /// The size of a subgraph's title, starting on its top border; empty for no title.
    fn subgraph_title(&self, subgraph: &Subgraph) -> Size;
    /// The size of a note's box.
    fn note(&self, label: &Label) -> Size;
}

impl Flowchart {
    /// Each note whose target is a node or a subgraph, with that item, in the order of the
    /// layout graph's notes.
    pub fn placed_notes(&self) -> impl Iterator<Item = (Item, &Note)> {
        self.notes.iter().filter_map(|n| {
            let item = match self.nodes.iter().position(|node| node.id == n.target) {
                Some(i) => Item::Node(i),
                None => Item::Cluster(self.subgraphs.iter().position(|s| s.id == n.target)?),
            };
            Some((item, n))
        })
    }

    /// The layout graph of this chart: node `i` is `nodes[i]`, cluster `i` is `subgraphs[i]`,
    /// edge `i` is `edges[i]`, and note `i` is the `i`th note whose target is a node.
    ///
    /// # Panics
    ///
    /// If an edge endpoint is neither a node nor a subgraph, which lowering never produces.
    pub fn layout_graph(&self, measure: &impl Measure) -> Graph {
        let nodes: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let clusters: HashMap<&str, usize> = self
            .subgraphs
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i))
            .collect();
        let mut node_parent = vec![None; self.nodes.len()];
        let mut cluster_parent = vec![None; self.subgraphs.len()];
        for (i, s) in self.subgraphs.iter().enumerate() {
            for m in &s.members {
                if let Some(&c) = clusters.get(m.as_str()) {
                    cluster_parent[c] = Some(i);
                } else if let Some(&n) = nodes.get(m.as_str()) {
                    node_parent[n] = Some(i);
                }
            }
        }
        // The direction a node is laid out in: its nearest subgraph's, or the chart's.
        let direction_of = |mut parent: Option<usize>| -> Direction {
            let mut steps = 0;
            while let Some(c) = parent {
                if let Some(d) = self.subgraphs[c].direction {
                    return d;
                }
                parent = cluster_parent[c];
                steps += 1;
                if steps > self.subgraphs.len() {
                    break;
                }
            }
            self.direction
        };
        let item = |id: &str| -> Item {
            match clusters.get(id) {
                Some(&c) => Item::Cluster(c),
                None => Item::Node(*nodes.get(id).expect("edge endpoint exists")),
            }
        };
        Graph {
            direction: self.direction,
            nodes: self
                .nodes
                .iter()
                .zip(&node_parent)
                .map(|(n, &parent)| {
                    let mut size = measure.node(n);
                    // A fork bar lies across the direction of flow.
                    if n.shape == Shape::Fork
                        && matches!(
                            direction_of(parent),
                            Direction::LeftRight | Direction::RightLeft
                        )
                    {
                        size = Size::new(size.h, size.w);
                    }
                    NodeSpec { size, parent }
                })
                .collect(),
            clusters: self
                .subgraphs
                .iter()
                .zip(cluster_parent.iter().copied())
                .map(|(s, parent)| ClusterSpec {
                    parent,
                    direction: s.direction,
                    title: measure.subgraph_title(s),
                    lane: self.lanes && parent.is_none(),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|e| EdgeSpec {
                    from: item(&e.from),
                    to: item(&e.to),
                    min_len: u32::from(e.length),
                    label: e.label.as_ref().and_then(|l| measure.edge_label(l)),
                    constraint: e.constraint,
                })
                .collect(),
            notes: self
                .placed_notes()
                .map(|(target, n)| NoteSpec {
                    target,
                    size: measure.note(&n.label),
                    after: n.side == NoteSide::After,
                })
                .collect(),
            same_rank: self
                .same_rank
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .filter_map(|id| nodes.get(id.as_str()).copied())
                        .collect()
                })
                .collect(),
            end_ranks: self
                .end_ranks
                .iter()
                .map(|(end, group)| {
                    let group = group
                        .iter()
                        .filter_map(|id| nodes.get(id.as_str()).copied())
                        .collect();
                    (*end, group)
                })
                .collect(),
            ordering: self
                .ordering
                .iter()
                .filter_map(|(id, order)| Some((nodes.get(id.as_str()).copied()?, *order)))
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .filter_map(|id| nodes.get(id.as_str()).copied())
                        .collect()
                })
                .collect(),
        }
    }
}
