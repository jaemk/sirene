//! PlantUML mindmaps and work breakdown structures (PMIND-*): a tree of `*` lines, lowered to
//! the mindmap model for `@startmindmap` and to a top-down graph for `@startwbs`.

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{self, Arrow, Direction, Edge, Flowchart, Label, Style};
use crate::lines::MAX_DEPTH;
use crate::mindmap::{self, Mindmap};
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str = "expected a node line such as `* root` or `** child`";

/// One node line.
#[derive(Debug, Default, PartialEq)]
struct Item {
    /// 1 for the root.
    depth: usize,
    /// `*_`: drawn without a box.
    boxless: bool,
    color: Option<String>,
    /// A WBS `(A)` alias, which arrows name.
    alias: Option<String>,
    text: String,
    range: TextRange,
}

/// The tree's lines, read alike by both kinds.
struct Reader {
    src: String,
    d: Vec<Diagnostic>,
    title: Option<String>,
    direction: Option<(Direction, TextRange)>,
    items: Vec<Item>,
    /// WBS `(A) -> (B)` arrows.
    arrows: Vec<(String, String, TextRange)>,
    /// Indentation columns of the open single-`*` lines, for the indented form.
    indents: Vec<usize>,
}

impl Reader {
    /// The column `line` starts at.
    fn column(&self, line: &Line) -> usize {
        let start = usize::from(line.range.start());
        let line_start = self.src[..start].rfind('\n').map_or(0, |i| i + 1);
        self.src[line_start..start].chars().count()
    }

    fn statement(&mut self, line: &Line, cursor: &mut Cursor, wbs: bool) {
        let t = line.text.as_str();
        match lines::squeeze(t).as_str() {
            "left side" | "right side" => return,
            _ => {}
        }
        match lines::common(line, cursor, &mut self.d) {
            Common::No => {}
            Common::Done => return,
            Common::Title(title) => {
                self.title = Some(title);
                return;
            }
            Common::Direction(d) => {
                self.direction = Some((d, line.range));
                return;
            }
        }
        if wbs && t.starts_with('(') && self.arrow(line) {
            return;
        }
        let Some(c) = t.chars().next().filter(|c| matches!(c, '*' | '+' | '-')) else {
            return self.d.push(Diagnostic::new(line.range, STATEMENT));
        };
        let stars = t.len() - t.trim_start_matches(c).len();
        let depth = if c == '*' && stars == 1 {
            // `* root` then indented `* child` lines: the indentation nests them.
            let col = self.column(line);
            while self.indents.last().is_some_and(|&i| i >= col) {
                self.indents.pop();
            }
            self.indents.push(col);
            self.indents.len()
        } else {
            stars
        };
        let mut item = Item {
            depth,
            range: line.range,
            ..Item::default()
        };
        let mut rest = &t[stars..];
        loop {
            if let Some(r) = rest.strip_prefix('_') {
                item.boxless = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix(['<', '>']) {
                // The side a WBS node's children go to is the layout's.
                rest = r;
            } else if let Some(r) = rest.strip_prefix('[') {
                let Some(end) = r.find(']') else { break };
                let spec = &r[..end];
                match lines::color(spec) {
                    Some(c) => item.color = Some(c),
                    None => self.d.push(Diagnostic::warning(
                        line.range_of(spec),
                        format!("unknown color `{spec}`; drawing the default"),
                    )),
                }
                rest = &r[end + 1..];
            } else if wbs && rest.starts_with('(') {
                let Some(end) = rest.find(')') else { break };
                item.alias = Some(rest[1..end].trim().to_owned());
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        let rest = rest.trim();
        // A stereotype names a style, which is not drawn.
        let rest = match rest.rfind("<<") {
            Some(at) if rest.ends_with(">>") && at > 0 => rest[..at].trim_end(),
            _ => rest,
        };
        item.text = match rest.strip_prefix(':') {
            Some(first) => self.multiline(line, first, cursor, &mut item),
            None => lines::text(rest),
        };
        self.items.push(item);
    }

    /// The text of a `:first` ... `last;` node, which may span lines.
    fn multiline(
        &mut self,
        line: &Line,
        first: &str,
        cursor: &mut Cursor,
        item: &mut Item,
    ) -> String {
        if let Some(text) = first.strip_suffix(';') {
            return lines::text(text);
        }
        let mut text = vec![first.to_owned()];
        loop {
            let Some(next) = cursor.next() else {
                self.d.push(Diagnostic::new(
                    line.range,
                    "unclosed `:` text: expected a line ending in `;`",
                ));
                break;
            };
            item.range = item.range.cover(next.range);
            match next.text.strip_suffix(';') {
                Some(last) => {
                    text.push(last.to_owned());
                    break;
                }
                None => text.push(next.text.clone()),
            }
        }
        lines::text(text.join("\n").trim())
    }

    /// A WBS `(A) -> (B)` arrow; `false` when the line is not one.
    fn arrow(&mut self, line: &Line) -> bool {
        let Some((from, to)) = line.text.split_once("->") else {
            return false;
        };
        let alias = |s: &str| {
            let s = s.trim().trim_end_matches('-');
            s.trim()
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .map(|s| s.trim().to_owned())
        };
        let (Some(from), Some(to)) = (alias(from), alias(to)) else {
            return false;
        };
        self.arrows.push((from, to, line.range));
        true
    }
}

/// Reads the tree's lines.
fn read(root: &SyntaxNode, wbs: bool) -> Reader {
    let mut r = Reader {
        src: root.to_string(),
        d: Vec::new(),
        title: None,
        direction: None,
        items: Vec::new(),
        arrows: Vec::new(),
        indents: Vec::new(),
    };
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        r.statement(&line, &mut cursor, wbs);
    }
    r
}

/// The parent of each item, by depth; `Err` with a diagnostic for an item with none, which is
/// left out, and its subtree with it.
fn parents(items: &[Item], d: &mut Vec<Diagnostic>) -> Vec<Option<Option<usize>>> {
    let mut out = Vec::with_capacity(items.len());
    // The open items, as (depth, index), with depth rising.
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut too_deep = false;
    for (i, item) in items.iter().enumerate() {
        // The stack is cut only once the item is kept, as a later line may still sit under
        // the rest.
        let under = open.partition_point(|&(depth, _)| depth < item.depth);
        let parent = under.checked_sub(1).map(|k| open[k].1);
        if i > 0 && parent.is_none() {
            d.push(Diagnostic::new(
                item.range,
                format!(
                    "`{}` has no parent: a mindmap has one root, so give this line more `*` \
                     than the root",
                    item.text.replace('\n', " ")
                ),
            ));
            out.push(None);
            // Mark the dropped line open so its children are dropped too.
            open.truncate(under);
            open.push((item.depth, i));
            continue;
        }
        if under > MAX_DEPTH {
            if !too_deep {
                too_deep = true;
                d.push(Diagnostic::new(
                    item.range,
                    crate::lines::too_deep("nodes", MAX_DEPTH),
                ));
            }
            out.push(None);
            open.truncate(under);
            open.push((item.depth, i));
            continue;
        }
        out.push(Some(parent));
        open.truncate(under);
        open.push((item.depth, i));
    }
    out
}

/// A `@startmindmap` document as a mindmap (PMIND-2).
pub(crate) fn lower_mindmap(root: &SyntaxNode) -> (Mindmap, Vec<Diagnostic>) {
    let mut r = read(root, false);
    if let Some((_, range)) = r.direction {
        r.d.push(Diagnostic::warning(
            range,
            "a mindmap's direction is the layout's; ignoring it",
        ));
    }
    let parents = parents(&r.items, &mut r.d);
    let mut map = Mindmap {
        title: r.title.take(),
        nodes: Vec::new(),
    };
    // Item index to node index.
    let mut at: Vec<Option<usize>> = Vec::with_capacity(r.items.len());
    for (item, parent) in r.items.iter().zip(parents) {
        let parent = match parent {
            Some(Some(p)) => match at[p] {
                Some(n) => Some(n),
                None => {
                    at.push(None);
                    continue;
                }
            },
            Some(None) => None,
            None => {
                at.push(None);
                continue;
            }
        };
        if item.color.is_some() {
            r.d.push(Diagnostic::warning(
                item.range,
                "node colors are not drawn in mindmaps; ignoring it",
            ));
        }
        let i = map.nodes.len();
        let (depth, section) = match parent {
            Some(p) => {
                let p = &map.nodes[p];
                (p.depth + 1, p.section.or(Some(p.children.len())))
            }
            None => (0, None),
        };
        map.nodes.push(mindmap::Node {
            id: None,
            label: Label::plain(&item.text),
            shape: if item.boxless {
                mindmap::Shape::Default
            } else {
                mindmap::Shape::Rounded
            },
            icon: None,
            classes: Vec::new(),
            parent,
            children: Vec::new(),
            depth,
            section,
        });
        if let Some(p) = parent {
            map.nodes[p].children.push(i);
        }
        at.push(Some(i));
    }
    (map, r.d)
}

/// A `@startwbs` document as a top-down tree of boxes (PMIND-3).
pub(crate) fn lower_wbs(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut r = read(root, true);
    let parents = parents(&r.items, &mut r.d);
    let mut chart = Flowchart {
        direction: r.direction.map_or(Direction::TopBottom, |(d, _)| d),
        title: r.title.take(),
        ..Flowchart::default()
    };
    let mut ids: Vec<Option<String>> = Vec::with_capacity(r.items.len());
    // Every node id taken, for duplicate aliases and arrow lookups.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, (item, parent)) in r.items.iter().zip(parents).enumerate() {
        let parent = match parent {
            Some(Some(p)) => match &ids[p] {
                Some(id) => Some(id.clone()),
                None => {
                    ids.push(None);
                    continue;
                }
            },
            Some(None) => None,
            None => {
                ids.push(None);
                continue;
            }
        };
        let mut id = item.alias.clone().unwrap_or_else(|| format!("wbs:{i}"));
        if item.alias.is_some() && seen.contains(&id) {
            r.d.push(Diagnostic::new(
                item.range,
                format!("alias `({id})` is already used; keeping the first"),
            ));
            id = format!("wbs:{i}");
        }
        // A fallback id may collide with an alias written like it, e.g. `(wbs:2)`.
        while seen.contains(&id) {
            id.push('_');
        }
        seen.insert(id.clone());
        let shape = if item.boxless {
            flowchart::Shape::Text
        } else {
            flowchart::Shape::Rect
        };
        let mut node = flowchart::Node::new(&id, Label::plain(&item.text), shape);
        node.mentions.push(item.range);
        node.styles.extend(item.color.clone().map(|value| Style {
            property: "fill".into(),
            value,
        }));
        chart.nodes.push(node);
        if let Some(p) = parent {
            let mut edge = Edge::new(p, &id, item.range);
            edge.end = None;
            chart.edges.push(edge);
        }
        ids.push(Some(id));
    }
    for (from, to, range) in std::mem::take(&mut r.arrows) {
        let known = |a: &str| seen.contains(a);
        if let Some(missing) = [&from, &to].into_iter().find(|a| !known(a)) {
            r.d.push(Diagnostic::new(
                range,
                format!("no node has the alias `({missing})`"),
            ));
            continue;
        }
        let mut edge = Edge::new(from, to, range);
        edge.end = Some(Arrow::Point);
        edge.constraint = false;
        chart.edges.push(edge);
    }
    (chart, r.d)
}

#[cfg(test)]
mod tests {
    use crate::diagram::Diagram;

    fn lower(src: &str) -> (Diagram, Vec<String>) {
        let (diagram, d) = super::super::parse(src).lower();
        (diagram, d.into_iter().map(|d| d.message).collect())
    }

    #[test]
    fn mindmaps() {
        let (diagram, d) = lower(
            "@startmindmap\ntitle Plan\n* Root\n** A\n***_ a1\n** :B\nsecond line;\n\
             left side\n-- C <<rose>>\n--- c1\n@endmindmap\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let Diagram::Mindmap(m) = diagram else {
            panic!("not a mindmap")
        };
        assert_eq!(m.title.as_deref(), Some("Plan"));
        let labels: Vec<_> = m.nodes.iter().map(|n| n.label.text.as_str()).collect();
        assert_eq!(labels, ["Root", "A", "a1", "B\nsecond line", "C", "c1"]);
        assert_eq!(m.nodes[0].children, [1, 3, 4]);
        assert_eq!(m.nodes[2].shape, crate::mindmap::Shape::Default);
        assert_eq!(m.nodes[1].shape, crate::mindmap::Shape::Rounded);
        assert_eq!(m.nodes[5].section, Some(2));
        // The indented form nests by indentation.
        let (diagram, d) = lower("@startmindmap\n* r\n  * a\n    * b\n  * c\n@endmindmap\n");
        assert!(d.is_empty(), "{d:?}");
        let Diagram::Mindmap(m) = diagram else {
            panic!("not a mindmap")
        };
        assert_eq!(m.nodes[0].children, [1, 3]);
        assert_eq!(m.nodes[1].children, [2]);
    }

    #[test]
    fn wbs() {
        let (diagram, d) = lower(
            "@startwbs\n* Project\n**[#pink] Design\n***(A) Draft\n***(B) Review\n\
             **< Build\n***_ Code\n(A) -> (B)\n@endwbs\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let g = diagram.into_graph();
        let labels: Vec<_> = g.nodes.iter().map(|n| n.label.text.as_str()).collect();
        assert_eq!(
            labels,
            ["Project", "Design", "Draft", "Review", "Build", "Code"]
        );
        assert_eq!(g.nodes[1].styles[0].value, "#ffc0cb");
        assert_eq!(g.nodes[5].shape, crate::flowchart::Shape::Text);
        let edges: Vec<_> = g
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        assert_eq!(
            edges,
            [
                ("wbs:0", "wbs:1"),
                ("wbs:1", "A"),
                ("wbs:1", "B"),
                ("wbs:0", "wbs:4"),
                ("wbs:4", "wbs:5"),
                ("A", "B"),
            ]
        );
        assert!(!g.edges[5].constraint);
    }

    #[test]
    fn errors() {
        let (_, d) = lower("@startmindmap\n* a\n* b\nwhat\n**[#nope] c\n** :open\n@endmindmap\n");
        assert_eq!(
            d,
            [
                "`b` has no parent: a mindmap has one root, so give this line more `*` than \
                 the root",
                super::STATEMENT,
                "unknown color `#nope`; drawing the default",
                "unclosed `:` text: expected a line ending in `;`",
            ]
        );
        let (_, d) = lower("@startwbs\n* a\n(X) -> (Y)\n@endwbs\n");
        assert_eq!(d, ["no node has the alias `(X)`"]);
    }

    #[test]
    fn dropped_line_drops_its_children() {
        let (diagram, d) = lower("@startmindmap\n* r\n** a\n* b\n** c\n@endmindmap\n");
        assert_eq!(d.len(), 1, "{d:?}");
        let Diagram::Mindmap(m) = diagram else {
            panic!("not a mindmap")
        };
        let labels: Vec<_> = m.nodes.iter().map(|n| n.label.text.as_str()).collect();
        assert_eq!(labels, ["r", "a"]);
        assert_eq!(m.nodes[0].children, [1]);
    }

    #[test]
    fn duplicate_alias() {
        let (diagram, d) = lower("@startwbs\n* P\n**(A) x\n**(A) y\n***(B) z\n@endwbs\n");
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("already used"), "{d:?}");
        let g = diagram.into_graph();
        let ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["wbs:0", "A", "wbs:2", "B"]);
    }

    #[test]
    fn too_deep_subtree_is_dropped() {
        let mut src = String::from("@startmindmap\n");
        for depth in 1..=70 {
            src.push_str(&format!("{} n{depth}\n", "*".repeat(depth)));
        }
        src.push_str("** back\n@enduml\n@endmindmap\n");
        let (diagram, d) = lower(&src);
        let too_deep = d.iter().filter(|m| m.contains("deep")).count();
        assert_eq!(too_deep, 1, "{d:?}");
        let Diagram::Mindmap(m) = diagram else {
            panic!("not a mindmap")
        };
        // Depths 1..=65 are kept, the rest drop, and a shallow line after them attaches to the root.
        assert_eq!(m.nodes.len(), 66);
        assert_eq!(m.nodes[65].label.text, "back");
        assert_eq!(m.nodes[65].parent, Some(0));
        assert!(m.nodes.iter().all(|n| n.label.text != "n70"));
    }

    #[test]
    fn orphan_subtree_does_not_reattach() {
        let (diagram, d) = lower("@startmindmap\n* r\n** a\n* b\n** c\n*** d\n** e\n@endmindmap\n");
        assert_eq!(d.len(), 1, "{d:?}");
        let Diagram::Mindmap(m) = diagram else {
            panic!("not a mindmap")
        };
        let labels: Vec<_> = m.nodes.iter().map(|n| n.label.text.as_str()).collect();
        assert_eq!(labels, ["r", "a"]);
        // The same in WBS.
        let (diagram, d) = lower("@startwbs\n* r\n** a\n* b\n** c\n@endwbs\n");
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(diagram.into_graph().nodes.len(), 2);
    }

    #[test]
    fn alias_colliding_with_fallback_id() {
        // `(wbs:2)` is taken before the unaliased third line asks for `wbs:2`.
        let (diagram, d) = lower("@startwbs\n* P\n**(wbs:2) x\n** y\n** z\n@endwbs\n");
        let g = diagram.into_graph();
        let mut ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), 4, "{d:?}");
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4, "node ids must be unique: {d:?}");
        // The edges still reach distinct children.
        assert_eq!(g.edges.len(), 3);
        // Alias after the fallback that used the id: reported as a duplicate.
        let (diagram, d) = lower("@startwbs\n* P\n** y\n**(wbs:1) x\n@endwbs\n");
        let g = diagram.into_graph();
        assert_eq!(d.len(), 1, "{d:?}");
        let mut ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn duplicate_alias_fallback_collides_with_alias() {
        // The duplicate `(A)` on line 3 falls back to `wbs:3`, which an alias already holds.
        let (diagram, _) = lower("@startwbs\n* P\n**(wbs:3) x\n**(A) y\n**(A) z\n@endwbs\n");
        let g = diagram.into_graph();
        let mut ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn arrows_to_duplicate_aliases_hit_the_first() {
        let (diagram, d) = lower("@startwbs\n* P\n**(A) x\n**(A) y\n(A) -> (A)\n@endwbs\n");
        assert_eq!(d.len(), 1, "{d:?}");
        let g = diagram.into_graph();
        let last = g.edges.last().unwrap();
        assert_eq!((last.from.as_str(), last.to.as_str()), ("A", "A"));
    }

    #[test]
    fn many_arrows_are_bounded() {
        let mut src = String::from("@startwbs\n* P\n");
        for i in 0..3000 {
            src.push_str(&format!("**({i}) n\n"));
        }
        for i in 0..3000 {
            src.push_str(&format!("({i}) -> (0)\n"));
        }
        src.push_str("@endwbs\n");
        let start = std::time::Instant::now();
        let (_, d) = lower(&src);
        assert!(d.is_empty(), "{d:?}");
        assert!(start.elapsed().as_secs() < 20);
    }
}
