//! Mermaid event modeling diagrams (MEVM-*): UI, processor, command, read model, and event
//! frames on a timeline, in swimlanes by type and namespace, with relations between them.

mod parser;

use std::collections::HashMap;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// An entity type, which picks a frame's swimlane group and colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    Ui,
    Processor,
    Command,
    ReadModel,
    Event,
}

impl EntityType {
    fn parse(s: &str) -> Option<EntityType> {
        Some(match s {
            "ui" => EntityType::Ui,
            "pcr" | "processor" => EntityType::Processor,
            "cmd" | "command" => EntityType::Command,
            "rmo" | "readmodel" => EntityType::ReadModel,
            "evt" | "event" => EntityType::Event,
            _ => return None,
        })
    }

    /// The compact keyword, e.g. `cmd`.
    pub fn short(self) -> &'static str {
        match self {
            EntityType::Ui => "ui",
            EntityType::Processor => "pcr",
            EntityType::Command => "cmd",
            EntityType::ReadModel => "rmo",
            EntityType::Event => "evt",
        }
    }

    /// The name in messages, with its article, e.g. `a read model`.
    fn name(self) -> &'static str {
        match self {
            EntityType::Ui => "a ui",
            EntityType::Processor => "a processor",
            EntityType::Command => "a command",
            EntityType::ReadModel => "a read model",
            EntityType::Event => "an event",
        }
    }

    pub fn group(self) -> Group {
        match self {
            EntityType::Ui | EntityType::Processor => Group::Automation,
            EntityType::Command | EntityType::ReadModel => Group::State,
            EntityType::Event => Group::Events,
        }
    }

    /// The types an explicit `->>` source may have, and how messages name them.
    fn sources(self) -> (&'static [EntityType], &'static str) {
        match self {
            EntityType::Command => (
                &[EntityType::Ui, EntityType::Processor],
                "a ui or processor",
            ),
            EntityType::Event => (&[EntityType::Command], "a command"),
            EntityType::ReadModel => (&[EntityType::Event], "an event"),
            EntityType::Processor | EntityType::Ui => (&[EntityType::ReadModel], "a read model"),
        }
    }
}

/// The three swimlane groups, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    /// UI/Automation: UIs and processors.
    Automation,
    /// Command/Read Model.
    State,
    Events,
}

impl Group {
    fn label(self) -> &'static str {
        match self {
            Group::Automation => "UI/Automation",
            Group::State => "Command/Read Model",
            Group::Events => "Events",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Group::Automation => "UI/A: ",
            Group::State => "C/RM: ",
            Group::Events => "Stream: ",
        }
    }
}

/// A swimlane: a group, or a group's frames in one namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lane {
    pub group: Group,
    pub namespace: Option<String>,
}

impl Lane {
    /// The label, e.g. `Events` or `Stream: Inventory`.
    pub fn label(&self) -> String {
        match &self.namespace {
            None => self.group.label().to_owned(),
            Some(ns) => format!("{}{ns}", self.group.prefix()),
        }
    }
}

/// A time or reset frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The frame number as written, e.g. `01`.
    pub id: String,
    pub kind: EntityType,
    /// Whether it is a reset frame, which takes no inferred relation.
    pub reset: bool,
    /// The name without its namespace.
    pub name: String,
    /// The data shown under the name: inline data or a referenced data block, by line.
    pub data: Vec<String>,
    /// Index into [`EventModel::lanes`].
    pub lane: usize,
}

/// A relation from one frame to another, as indices into [`EventModel::frames`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relation {
    pub from: usize,
    pub to: usize,
}

/// An event modeling diagram (MEVM-2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EventModel {
    pub title: Option<String>,
    /// Frames in timeline (source) order.
    pub frames: Vec<Frame>,
    /// Swimlanes top to bottom.
    pub lanes: Vec<Lane>,
    pub relations: Vec<Relation>,
}

/// The result of parsing event modeling source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
}

fn token(node: &SyntaxNode, kind: crate::syntax::SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

/// The namespace and name of an identifier: `Cart.ItemAdded` has namespace `Cart`; one with
/// more dots has none, as in Mermaid.
fn split_name(id: &str) -> (Option<String>, String) {
    match id.split_once('.') {
        Some((ns, name)) if !name.contains('.') => (Some(ns.to_owned()), name.to_owned()),
        _ => (None, id.to_owned()),
    }
}

/// Inline data's lines: the text inside its braces or quotes, as one line.
fn inline_data(token: &str) -> Vec<String> {
    let inner = token[1..token.len() - 1].trim();
    if inner.is_empty() {
        Vec::new()
    } else {
        vec![inner.to_owned()]
    }
}

/// A data block's lines: those between its `{` and `}` lines, without their common indent or
/// blank lines around them.
fn block_data(token: &str) -> Vec<String> {
    let mut lines: Vec<&str> = token.lines().collect();
    // The `{` line and the `}` line.
    lines.remove(0);
    lines.pop();
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| l.get(indent..).unwrap_or("").trim_end().to_owned())
        .collect()
}

/// A frame read from the tree, before lanes are assigned.
struct Raw {
    id: String,
    kind: EntityType,
    reset: bool,
    namespace: Option<String>,
    name: String,
    inline: Vec<String>,
    data_ref: Option<SyntaxToken>,
    sources: Vec<SyntaxToken>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to an [`EventModel`], with the parser's diagnostics and errors for
    /// repeated frame numbers, undefined references, and sources of the wrong type.
    pub fn lower(&self) -> (EventModel, Vec<Diagnostic>) {
        let root = self.syntax();
        let mut chart = EventModel::default();
        let mut diagnostics = self.diagnostics.clone();
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                chart.title = text::frontmatter_title(tok.text());
            }
        }
        let mut raws: Vec<Raw> = Vec::new();
        let mut data: HashMap<String, Vec<String>> = HashMap::new();
        let mut refs: Vec<SyntaxToken> = Vec::new();
        for node in root.children() {
            match node.kind() {
                TITLE_STMT => {
                    if let Some(t) = token(&node, TEXT) {
                        chart.title = Some(t.text().to_owned());
                    }
                }
                FRAME_STMT => {
                    let (Some(kw), Some(id), Some(ty), Some(name)) = (
                        token(&node, EM_KW),
                        token(&node, NUMBER),
                        token(&node, EM_TYPE).and_then(|t| EntityType::parse(t.text())),
                        token(&node, IDENT),
                    ) else {
                        continue;
                    };
                    if raws.iter().any(|r| r.id == id.text()) {
                        diagnostics.push(Diagnostic::new(
                            id.text_range(),
                            format!(
                                "frame `{}` is defined again; each frame needs its own number",
                                id.text()
                            ),
                        ));
                        continue;
                    }
                    let (namespace, short) = split_name(name.text());
                    let mut idents = tokens(&node).filter(|t| t.kind() == IDENT).skip(1);
                    raws.push(Raw {
                        id: id.text().to_owned(),
                        kind: ty,
                        reset: kw.text().starts_with('r'),
                        namespace,
                        name: short,
                        inline: token(&node, EM_DATA)
                            .map(|t| inline_data(t.text()))
                            .unwrap_or_default(),
                        data_ref: idents.next(),
                        sources: tokens(&node)
                            .filter(|t| t.kind() == NUMBER)
                            .skip(1)
                            .collect(),
                    });
                }
                DATA_STMT => {
                    if let (Some(name), Some(body)) = (token(&node, IDENT), token(&node, EM_DATA))
                        && body.text().ends_with('}')
                    {
                        data.insert(name.text().to_owned(), block_data(body.text()));
                    }
                }
                NOTE_STMT | GWT_STMT => refs.extend(token(&node, NUMBER)),
                _ => {}
            }
        }

        let index: HashMap<&str, usize> = raws
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), i))
            .collect();
        let undefined = |t: &SyntaxToken| {
            Diagnostic::new(
                t.text_range(),
                format!("frame `{}` is not defined", t.text()),
            )
        };
        for t in &refs {
            if !index.contains_key(t.text()) {
                diagnostics.push(undefined(t));
            }
        }

        // Lanes in order of first use, then grouped, each group's own lane first.
        let mut lanes: Vec<Lane> = Vec::new();
        let mut lane_of = Vec::with_capacity(raws.len());
        for r in &raws {
            let lane = Lane {
                group: r.kind.group(),
                namespace: r.namespace.clone(),
            };
            let i = match lanes.iter().position(|l| *l == lane) {
                Some(i) => i,
                None => {
                    lanes.push(lane);
                    lanes.len() - 1
                }
            };
            lane_of.push(i);
        }
        let mut order: Vec<usize> = (0..lanes.len()).collect();
        order.sort_by_key(|&i| (lanes[i].group, lanes[i].namespace.is_some()));
        let mut rank = vec![0; lanes.len()];
        for (k, &i) in order.iter().enumerate() {
            rank[i] = k;
        }
        chart.lanes = order.iter().map(|&i| lanes[i].clone()).collect();

        for (i, r) in raws.iter().enumerate() {
            let mut lines = r.inline.clone();
            if let Some(d) = &r.data_ref {
                match data.get(d.text()) {
                    Some(block) => lines = block.clone(),
                    None => diagnostics.push(Diagnostic::new(
                        d.text_range(),
                        format!("data block `{}` is not defined", d.text()),
                    )),
                }
            }
            chart.frames.push(Frame {
                id: r.id.clone(),
                kind: r.kind,
                reset: r.reset,
                name: r.name.clone(),
                data: lines,
                lane: rank[lane_of[i]],
            });
        }

        // Relations: explicit sources, or else the latest earlier frame in another lane.
        for (i, r) in raws.iter().enumerate() {
            if r.sources.is_empty() {
                if r.reset {
                    continue;
                }
                let lane = chart.frames[i].lane;
                if let Some(from) = (0..i).rev().find(|&j| chart.frames[j].lane != lane) {
                    chart.relations.push(Relation { from, to: i });
                }
                continue;
            }
            let mut from: Vec<usize> = Vec::new();
            for t in &r.sources {
                let Some(&j) = index.get(t.text()) else {
                    diagnostics.push(undefined(t));
                    continue;
                };
                if j == i {
                    diagnostics.push(Diagnostic::warning(
                        t.text_range(),
                        format!("a relation from frame `{}` to itself is not drawn", r.id),
                    ));
                    continue;
                }
                let (allowed, what) = r.kind.sources();
                let src = raws[j].kind;
                if !allowed.contains(&src) {
                    diagnostics.push(Diagnostic::new(
                        t.text_range(),
                        format!(
                            "{} can only receive input from {what}, not from `{}`",
                            r.kind.name(),
                            src.short()
                        ),
                    ));
                }
                if !from.contains(&j) {
                    from.push(j);
                }
            }
            from.sort_unstable();
            chart
                .relations
                .extend(from.into_iter().map(|from| Relation { from, to: i }));
        }
        log::debug!("lowered an event modeling diagram");
        (chart, diagnostics)
    }
}

/// Parses event modeling source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_data() {
        assert_eq!(
            split_name("Cart.Item"),
            (Some("Cart".into()), "Item".into())
        );
        assert_eq!(split_name("A.B.C"), (None, "A.B.C".into()));
        assert_eq!(split_name("Item"), (None, "Item".into()));
        assert_eq!(inline_data("{ a: string }"), ["a: string"]);
        assert_eq!(inline_data("'x'"), ["x"]);
        assert!(inline_data("{ }").is_empty());
        assert_eq!(
            block_data("{\n\n    a: 1\n      b: 2\n\n  }"),
            ["a: 1", "  b: 2"]
        );
    }
}
