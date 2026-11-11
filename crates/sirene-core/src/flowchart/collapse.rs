//! Collapsed subgraphs (FLOWMO-15): a subgraph with `@{ view: collapsed }` is drawn as one
//! node in place of its contents. Follows Mermaid's `flowDb.getData`.

use std::collections::HashMap;

use super::model::{Flowchart, Label, Node, Shape, Subgraph};

/// A flowchart as drawn with its collapsed subgraphs folded into nodes, and where each of its
/// elements came from in the original.
#[derive(Debug, Clone, PartialEq)]
pub struct Collapsed {
    pub chart: Flowchart,
    /// For each node: the original node, or the original subgraph it stands for.
    pub nodes: Vec<Origin>,
    /// For each subgraph, the original subgraph's index.
    pub subgraphs: Vec<usize>,
    /// For each edge, the original edge's index.
    pub edges: Vec<usize>,
}

/// What a drawn node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Node(usize),
    Subgraph(usize),
}

impl Subgraph {
    /// Whether the subgraph is drawn as one node (`@{ view: collapsed }`).
    pub fn is_collapsed(&self) -> bool {
        self.metadata
            .iter()
            .rev()
            .find(|(k, _)| k == "view")
            .is_some_and(|(_, v)| v == "collapsed")
    }
}

impl Flowchart {
    /// The chart with collapsed subgraphs folded into nodes, or `None` when none is collapsed.
    pub fn collapsed(&self) -> Option<Collapsed> {
        if !self.subgraphs.iter().any(Subgraph::is_collapsed) {
            return None;
        }
        let index: HashMap<&str, usize> = self
            .subgraphs
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i))
            .collect();
        let mut parent: HashMap<&str, &str> = HashMap::new();
        for s in &self.subgraphs {
            for m in &s.members {
                if index.contains_key(m.as_str()) {
                    parent.entry(m.as_str()).or_insert(s.id.as_str());
                }
            }
        }
        // The outermost collapsed subgraph at or above `id`.
        let outermost = |id: &str| -> Option<usize> {
            let (mut found, mut seen, mut current) = (None, Vec::new(), Some(id));
            while let Some(c) = current {
                if seen.contains(&c) {
                    break;
                }
                seen.push(c);
                if self.subgraphs[index[c]].is_collapsed() {
                    found = Some(index[c]);
                }
                current = parent.get(c).copied();
            }
            found
        };
        // Each hidden id and the visible collapsed subgraph that replaces it.
        let mut hidden: HashMap<&str, &str> = HashMap::new();
        for s in &self.subgraphs {
            let Some(top) = outermost(&s.id) else {
                continue;
            };
            let top = self.subgraphs[top].id.as_str();
            if s.id != top {
                hidden.insert(&s.id, top);
            }
            for m in &s.members {
                if m != top {
                    hidden.insert(m, top);
                }
            }
        }
        let visible = |id: &str| -> String { hidden.get(id).copied().unwrap_or(id).to_owned() };

        let group = |i: usize| -> Node {
            let s = &self.subgraphs[i];
            Node {
                id: s.id.clone(),
                label: if s.title.text.is_empty() {
                    Label::plain(s.id.clone())
                } else {
                    s.title.clone()
                },
                shape: Shape::Collapsed,
                sections: Vec::new(),
                classes: s.classes.clone(),
                styles: s.styles.clone(),
                click: None,
                tooltip: None,
                metadata: s.metadata.clone(),
                mentions: Vec::new(),
            }
        };
        let groups: Vec<usize> = (0..self.subgraphs.len())
            .filter(|&i| {
                let s = &self.subgraphs[i];
                s.is_collapsed() && !hidden.contains_key(s.id.as_str())
            })
            .collect();

        let mut out = Collapsed {
            chart: Flowchart {
                nodes: Vec::new(),
                edges: Vec::new(),
                subgraphs: Vec::new(),
                notes: Vec::new(),
                ..self.clone()
            },
            nodes: Vec::new(),
            subgraphs: Vec::new(),
            edges: Vec::new(),
        };
        // A collapsed subgraph's node goes where its first hidden node was, keeping source order.
        let mut placed = vec![false; self.subgraphs.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            match hidden.get(n.id.as_str()) {
                None => {
                    out.chart.nodes.push(n.clone());
                    out.nodes.push(Origin::Node(i));
                }
                Some(top) => {
                    let g = index[top];
                    if !placed[g] {
                        placed[g] = true;
                        out.chart.nodes.push(group(g));
                        out.nodes.push(Origin::Subgraph(g));
                    }
                }
            }
        }
        for g in groups {
            if !placed[g] {
                out.chart.nodes.push(group(g));
                out.nodes.push(Origin::Subgraph(g));
            }
        }
        for (i, s) in self.subgraphs.iter().enumerate() {
            if s.is_collapsed() || hidden.contains_key(s.id.as_str()) {
                continue;
            }
            out.chart.subgraphs.push(s.clone());
            out.subgraphs.push(i);
        }
        for (i, e) in self.edges.iter().enumerate() {
            let (from, to) = (visible(&e.from), visible(&e.to));
            let folded = hidden.contains_key(e.from.as_str()) || hidden.contains_key(e.to.as_str());
            if from == to && folded {
                continue;
            }
            out.chart.edges.push(super::Edge {
                from,
                to,
                ..e.clone()
            });
            out.edges.push(i);
        }
        for note in &self.notes {
            out.chart.notes.push(super::Note {
                target: visible(&note.target),
                ..note.clone()
            });
        }
        Some(out)
    }
}
