//! Conversion of a flowchart into a layout graph.

use std::collections::HashMap;

use super::model::{Flowchart, Label, Node, Subgraph};
use crate::layout::{ClusterSpec, EdgeSpec, Graph, Item, NodeSpec, Size};

/// Sizes of flowchart elements in layout cells.
pub trait Measure {
    fn node(&self, node: &Node) -> Size;
    /// The size of an edge label, or `None` when it takes no room.
    fn edge_label(&self, label: &Label) -> Option<Size>;
    /// The size of a subgraph's title, starting on its top border; empty for no title.
    fn subgraph_title(&self, subgraph: &Subgraph) -> Size;
}

impl Flowchart {
    /// The layout graph of this chart: node `i` is `nodes[i]`, cluster `i` is `subgraphs[i]`,
    /// and edge `i` is `edges[i]`.
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
                .zip(node_parent)
                .map(|(n, parent)| NodeSpec {
                    size: measure.node(n),
                    parent,
                })
                .collect(),
            clusters: self
                .subgraphs
                .iter()
                .zip(cluster_parent)
                .map(|(s, parent)| ClusterSpec {
                    parent,
                    direction: s.direction,
                    title: measure.subgraph_title(s),
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
                })
                .collect(),
        }
    }
}
