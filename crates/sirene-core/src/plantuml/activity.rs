//! PlantUML activity diagrams (PACT-*), in the newer syntax, lowered to the graph model: each
//! statement adds a node joined to the open ends of the flow before it.

use std::collections::HashMap;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, keyword, parens};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{
    Edge, Flowchart, Label, Node, Note, NoteSide, Shape, Stroke, Style, Subgraph,
};
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str = "expected an activity statement such as `:action;`, `start`, \
                                    `if (...) then`, or `while (...)`";

/// The most edges an activity diagram draws; past it the rest are dropped with one error.
const MAX_EDGES: usize = 100_000;

/// An open end of the flow: the next node is joined to it, with the label of the branch and
/// the arrow statement that ended its branch.
#[derive(Debug, Clone)]
struct Tail {
    node: String,
    label: Option<String>,
    arrow: Option<Arrow>,
}

impl Tail {
    fn plain(node: &str) -> Self {
        Self::labeled(node, None)
    }

    fn labeled(node: &str, label: Option<String>) -> Self {
        Self {
            node: node.to_owned(),
            label,
            arrow: None,
        }
    }
}

/// Drops repeated tails (same node and label), keeping the first.
fn dedupe(tails: &mut Vec<Tail>) {
    let mut seen = std::collections::HashSet::new();
    tails.retain(|t| seen.insert((t.node.clone(), t.label.clone())));
}

/// A statement that is open until its closer.
enum Open {
    If {
        decision: String,
        ends: Vec<Tail>,
        otherwise: bool,
        range: TextRange,
    },
    Switch {
        decision: String,
        ends: Vec<Tail>,
        cases: usize,
        range: TextRange,
    },
    While {
        decision: String,
        breaks: Vec<Tail>,
        range: TextRange,
    },
    Repeat {
        start: String,
        backward: Option<String>,
        breaks: Vec<Tail>,
        range: TextRange,
    },
    /// `fork` (with its bar) or `split` (without one), and the ends of its finished branches.
    Fork {
        bar: Option<String>,
        from: Vec<Tail>,
        ends: Vec<Tail>,
        range: TextRange,
    },
    /// A `partition` or other grouping, closed by `}` or `end group`.
    Group {
        subgraph: usize,
        brace: bool,
        range: TextRange,
    },
}

impl Open {
    fn range(&self) -> TextRange {
        match self {
            Open::If { range, .. }
            | Open::Switch { range, .. }
            | Open::While { range, .. }
            | Open::Repeat { range, .. }
            | Open::Fork { range, .. }
            | Open::Group { range, .. } => *range,
        }
    }

    /// The keyword and closer, for messages.
    fn names(&self) -> (&'static str, &'static str) {
        match self {
            Open::If { .. } => ("if", "endif"),
            Open::Switch { .. } => ("switch", "endswitch"),
            Open::While { .. } => ("while", "endwhile"),
            Open::Repeat { .. } => ("repeat", "repeat while"),
            Open::Fork { bar: Some(_), .. } => ("fork", "end fork"),
            Open::Fork { bar: None, .. } => ("split", "end split"),
            Open::Group { brace: true, .. } => ("partition", "}"),
            Open::Group { brace: false, .. } => ("group", "end group"),
        }
    }
}

/// The style of the next edges, from an arrow statement (`-> text;`, `-[#red,dashed]->`).
#[derive(Debug, Clone, Default)]
struct Arrow {
    label: Option<String>,
    stroke: Option<Stroke>,
    styles: Vec<Style>,
}

impl Arrow {
    fn is_empty(&self) -> bool {
        self.label.is_none() && self.stroke.is_none() && self.styles.is_empty()
    }
}

struct Lower {
    chart: Flowchart,
    d: Vec<Diagnostic>,
    tails: Vec<Tail>,
    arrow: Arrow,
    stack: Vec<Open>,
    count: usize,
    /// The current swimlane's subgraph.
    lane: Option<usize>,
    lanes: HashMap<String, usize>,
    /// The last node added, which notes attach to.
    last: Option<String>,
    /// Whether the `MAX_EDGES` error was reported.
    capped: bool,
}

pub(crate) fn lower(root: &SyntaxNode) -> (Flowchart, Vec<Diagnostic>) {
    let mut l = Lower::new();
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        l.statement(&line, &mut cursor);
        // The rest of an older-syntax diagram would only add errors after the first one.
        if line.text.starts_with("(*)") {
            break;
        }
    }
    for open in std::mem::take(&mut l.stack) {
        l.d.push(unclosed(&open));
    }
    (l.chart, l.d)
}

fn unclosed(open: &Open) -> Diagnostic {
    let (kw, closer) = open.names();
    Diagnostic::new(
        open.range(),
        format!("unclosed `{kw}`: expected `{closer}`"),
    )
}

/// Whether `body` (an action's last line) ends in `>` as part of a tag such as `</b>` or
/// `<i>`, which does not close the action.
fn ends_in_tag(body: &str) -> bool {
    let Some(b) = body.strip_suffix('>') else {
        return false;
    };
    let name = b.len()
        - b.chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
    let n = b[name..].chars().count();
    if !(1..=5).contains(&n) {
        return false;
    }
    let b = &b[..name];
    let b = b.strip_suffix('/').unwrap_or(b);
    b.ends_with('<')
}

/// Where `line` closes an action: the byte offset of its closing character and the character.
fn action_end(line: &str) -> Option<(usize, char)> {
    let b = line.trim_end();
    // A stereotype such as `<<input>>` may follow the closing `;`.
    let b = match b.rfind("<<") {
        Some(i) if b.ends_with(">>") && b[..i].trim_end().ends_with(';') => b[..i].trim_end(),
        _ => b,
    };
    if ends_in_tag(b) {
        return None;
    }
    b.chars()
        .last()
        .filter(|c| ACTION_ENDS.contains(c))
        .map(|c| (b.len() - c.len_utf8(), c))
}

/// The shape an action's closing character gives it (PACT-2).
fn action_shape(end: char) -> Shape {
    match end {
        '|' => Shape::FramedRect,
        '<' => Shape::LeanLeft,
        '>' | '/' => Shape::LeanRight,
        ']' | '}' => Shape::Rect,
        _ => Shape::Rounded,
    }
}

const ACTION_ENDS: &[char] = &[';', '|', '<', '>', '/', ']', '}'];

/// A `(label)` after a keyword such as `then`, `is`, `else`, or `endwhile`.
fn label_after<'a>(s: &'a str, kw: &str) -> (Option<String>, &'a str) {
    match keyword(s, kw) {
        Some(rest) => match parens(rest) {
            Some((label, rest)) => (Some(lines::text(label.trim())), rest),
            None => (None, rest),
        },
        None => (None, s),
    }
}

/// A bare `(label)`.
fn label(s: &str) -> Option<String> {
    parens(s.trim()).map(|(l, _)| lines::text(l.trim()))
}

impl Lower {
    fn new() -> Self {
        Self {
            chart: Flowchart::default(),
            d: Vec::new(),
            tails: Vec::new(),
            arrow: Arrow::default(),
            stack: Vec::new(),
            count: 0,
            lane: None,
            lanes: HashMap::new(),
            last: None,
            capped: false,
        }
    }

    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::new(range, message));
    }

    fn warn(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::warning(range, message));
    }

    /// Adds `id` to the innermost group, else the current lane.
    fn place(&mut self, id: &str) {
        let group = self.stack.iter().rev().find_map(|o| match o {
            Open::Group { subgraph, .. } => Some(*subgraph),
            _ => None,
        });
        if let Some(i) = group.or(self.lane) {
            self.chart.subgraphs[i].members.push(id.to_owned());
        }
    }

    /// A new node, not yet joined to the flow.
    fn node(&mut self, shape: Shape, text: &str, range: TextRange) -> String {
        self.count += 1;
        let id = format!("a{}", self.count);
        let mut node = Node::new(id.clone(), Label::plain(text), shape);
        node.mentions.push(range);
        self.chart.nodes.push(node);
        self.place(&id);
        id
    }

    /// Adds `edge` unless the diagram has `MAX_EDGES` already, reporting that once.
    fn push_edge(&mut self, edge: Edge) {
        if self.chart.edges.len() < MAX_EDGES {
            self.chart.edges.push(edge);
        } else if !self.capped {
            self.capped = true;
            self.error(
                edge.range,
                format!("more than {MAX_EDGES} edges; dropping the rest"),
            );
        }
    }

    fn edge(&mut self, from: &str, to: &str, label: Option<String>, range: TextRange) {
        let mut edge = Edge::new(from, to, range);
        edge.label = label.map(Label::plain);
        self.push_edge(edge);
    }

    /// Joins every open end to `to`, with the pending arrow's label and style, else the arrow
    /// that ended the tail's branch.
    fn join(&mut self, to: &str, range: TextRange) {
        let pending = std::mem::take(&mut self.arrow);
        for tail in std::mem::take(&mut self.tails) {
            let baked = tail.arrow.unwrap_or_default();
            let mut edge = Edge::new(tail.node, to, range);
            edge.label = pending
                .label
                .clone()
                .or(baked.label)
                .or(tail.label)
                .map(Label::plain);
            if let Some(stroke) = pending.stroke.or(baked.stroke) {
                edge.stroke = stroke;
            }
            edge.styles = if pending.styles.is_empty() {
                baked.styles
            } else {
                pending.styles.clone()
            };
            self.push_edge(edge);
        }
    }

    /// Ends a branch: the pending arrow statement styles only this branch's open ends.
    fn bake(&mut self) {
        let arrow = std::mem::take(&mut self.arrow);
        if arrow.is_empty() {
            return;
        }
        for tail in &mut self.tails {
            tail.arrow = Some(arrow.clone());
        }
    }

    /// Pops the blocks above the innermost one `is` accepts, reporting each as unclosed and
    /// merging its open ends into the flow. `false`, with an error, when no block matches.
    fn unwind(&mut self, line: &Line, is: fn(&Open) -> bool, missing: &str) -> bool {
        if !self.stack.iter().any(is) {
            self.error(line.range, missing.to_owned());
            return false;
        }
        while let Some(open) = self.stack.pop_if(|o| !is(o)) {
            self.d.push(unclosed(&open));
            let mut ends = match open {
                Open::If { ends, .. } | Open::Switch { ends, .. } | Open::Fork { ends, .. } => ends,
                Open::While { breaks, .. } | Open::Repeat { breaks, .. } => breaks,
                Open::Group { .. } => Vec::new(),
            };
            self.bake();
            ends.append(&mut self.tails);
            dedupe(&mut ends);
            self.tails = ends;
        }
        true
    }

    /// A new node joined to the flow, which becomes its only open end.
    fn add(&mut self, shape: Shape, text: &str, range: TextRange) -> String {
        let id = self.node(shape, text, range);
        self.join(&id, range);
        self.tails = vec![Tail::plain(&id)];
        self.last = Some(id.clone());
        id
    }

    fn push(&mut self, open: Open, range: TextRange) -> bool {
        if self.stack.len() >= MAX_DEPTH {
            self.error(range, crate::lines::too_deep("blocks", MAX_DEPTH));
            return false;
        }
        self.stack.push(open);
        true
    }

    fn statement(&mut self, line: &Line, cursor: &mut Cursor) {
        let t = line.text.as_str();
        let squeezed = line.squeezed();
        let w = line.word();
        if t.starts_with(':') || (t.starts_with('#') && t.contains(':') && !t.starts_with("#:")) {
            self.action(line, cursor, false);
            return;
        }
        if t.starts_with("(*)") {
            return self.error(
                line.range,
                "the older activity syntax (`(*) --> \"x\"`) is not supported; use `start` and `:action;`",
            );
        }
        if t.starts_with('|') {
            return self.lane(line);
        }
        if t.starts_with('-') && t.contains('>') {
            return self.arrow(line);
        }
        if t == "}" {
            return self.close_group(line, true);
        }
        if t == "{" && matches!(self.stack.last(), Some(Open::Group { brace: true, .. })) {
            return;
        }
        if let Some((name, rest)) = parens(t) {
            if rest.is_empty() {
                // A connector.
                self.add(Shape::Circle, &lines::text(name.trim()), line.range);
                return;
            }
            // `(no) elseif (cond) then (yes)`: the label is on the edge to the new test.
            let after = keyword(rest, "elseif")
                .or_else(|| keyword(rest, "else").and_then(|r| keyword(r, "if")));
            if let Some(after) = after {
                let label = Some(lines::text(name.trim()));
                return self.else_if(line, after, label);
            }
        }
        match squeezed.as_str() {
            "start" => {
                self.add(Shape::SmallCircle, "", line.range);
                return;
            }
            "stop" | "end" => {
                self.add(Shape::FramedCircle, "", line.range);
                self.tails.clear();
                return;
            }
            "kill" | "detach" => {
                self.tails.clear();
                return;
            }
            "break" => return self.brk(line),
            "endif" | "end if" => return self.end_if(line),
            "endswitch" | "end switch" => return self.end_switch(line),
            "fork" => return self.fork(line, true),
            "split" => return self.fork(line, false),
            "fork again" | "split again" => return self.fork_again(line),
            "end group" | "endgroup" => return self.close_group(line, false),
            _ => {}
        }
        if squeezed.starts_with("end fork")
            || squeezed.starts_with("end merge")
            || squeezed == "end split"
        {
            return self.end_fork(line, &squeezed);
        }
        if squeezed.starts_with("endwhile") || squeezed.starts_with("end while") {
            return self.end_while(line);
        }
        if squeezed.starts_with("repeat while") || squeezed.starts_with("repeatwhile") {
            return self.repeat_while(line);
        }
        if squeezed.starts_with("else if") {
            return self.else_if(line, t[4..].trim_start()[2..].trim_start(), None);
        }
        match w.as_str() {
            "if" => return self.if_(line),
            "elseif" => return self.else_if(line, line.after_word(), None),
            "else" => return self.else_(line),
            "switch" => return self.switch(line),
            "case" => return self.case(line),
            "while" => return self.while_(line),
            "repeat" => return self.repeat(line, cursor),
            "backward" => return self.backward(line, cursor),
            "partition" | "group" | "package" | "rectangle" | "card" | "folder" | "frame" => {
                return self.group(line);
            }
            "note" | "floating" => return self.note(line, cursor),
            _ => {}
        }
        match lines::common(line, cursor, &mut self.d) {
            Common::No => self.error(line.range, STATEMENT),
            Common::Done => {}
            Common::Title(title) => self.chart.title = Some(title),
            Common::Direction(d) => self.chart.direction = d,
        }
    }

    /// `:text;`, over one or more lines, with an optional `#color` before it. A `backward`
    /// action is returned without joining it to the flow.
    fn action(&mut self, line: &Line, cursor: &mut Cursor, detached: bool) -> Option<String> {
        let t = line.text.as_str();
        let colon = t.find(':')?;
        let color = &t[..colon];
        let mut body = t[colon + 1..].to_owned();
        let mut range = line.range;
        // Only the newest line can close the action.
        let mut end = action_end(&body);
        while end.is_none() {
            let Some(next) = cursor.next() else {
                break;
            };
            range = range.cover(next.range);
            body.push('\n');
            let at = body.len();
            body.push_str(&next.text);
            end = action_end(&next.text).map(|(i, c)| (at + i, c));
        }
        let Some((at, end)) = end else {
            self.error(line.range, "unclosed action: expected `;` at its end");
            return None;
        };
        let text: Vec<&str> = body[..at].lines().map(str::trim).collect();
        let text = lines::text(&text.join("\n"));
        let shape = action_shape(end);
        let id = if detached {
            self.node(shape, &text, range)
        } else {
            self.add(shape, &text, range)
        };
        if !color.is_empty() {
            match lines::color(color) {
                Some(c) => {
                    let node = self.chart.nodes.last_mut().expect("just added");
                    node.styles.push(Style {
                        property: "fill".into(),
                        value: c,
                    });
                }
                None => self.warn(
                    line.range_of(color),
                    format!("unknown color `{color}`; drawing the default"),
                ),
            }
        }
        Some(id)
    }

    /// `-> text;` or `-[#color,dashed]->`: the label and style of the next edges.
    fn arrow(&mut self, line: &Line) {
        let t = line.text.as_str();
        let head = t.find('>').expect("checked");
        let shaft = &t[..head];
        let mut arrow = Arrow::default();
        if let (Some(open), Some(close)) = (shaft.find('['), shaft.rfind(']'))
            && open < close
        {
            for part in shaft[open + 1..close].split(',').map(str::trim) {
                match part.to_ascii_lowercase().as_str() {
                    "dashed" => arrow.stroke = Some(Stroke::Dotted),
                    "dotted" => arrow.stroke = Some(Stroke::Dotted),
                    "bold" | "thickness=2" | "thickness=3" => arrow.stroke = Some(Stroke::Thick),
                    "hidden" => arrow.stroke = Some(Stroke::Invisible),
                    "" | "plain" => {}
                    p if p.starts_with('#') => match lines::color(p) {
                        Some(c) => arrow.styles.push(Style {
                            property: "stroke".into(),
                            value: c,
                        }),
                        None => self.warn(
                            line.range_of(part),
                            format!("unknown color `{part}`; drawing the default"),
                        ),
                    },
                    _ => self.warn(
                        line.range_of(part),
                        format!("arrow style `{part}` is not drawn; ignoring it"),
                    ),
                }
            }
        }
        let rest = t[head + 1..].trim();
        let rest = rest.strip_suffix(';').unwrap_or(rest).trim();
        if !rest.is_empty() {
            arrow.label = Some(lines::text(rest));
        }
        self.arrow = arrow;
    }

    /// `|Lane|`, `|#color|Lane|`: the following nodes go in that swimlane (PACT-7).
    fn lane(&mut self, line: &Line) {
        let inner = line.text.trim_matches('|');
        let name = inner.rsplit('|').next().unwrap_or(inner).trim();
        if name.is_empty() {
            return self.error(line.range, "expected a swimlane name such as `|Lane|`");
        }
        let name = match name.find(']') {
            // `|[#color]Lane|`
            Some(i) if name.starts_with('[') => name[i + 1..].trim(),
            _ => name,
        };
        self.chart.lanes = true;
        let i = match self.lanes.get(name) {
            Some(&i) => i,
            None => {
                let i = self.subgraph(name, line.range);
                self.lanes.insert(name.to_owned(), i);
                i
            }
        };
        self.lane = Some(i);
    }

    fn subgraph(&mut self, title: &str, range: TextRange) -> usize {
        let i = self.chart.subgraphs.len();
        self.chart.subgraphs.push(Subgraph {
            id: format!("g{}", i + 1),
            title: Label::plain(lines::text(title)),
            direction: None,
            members: Vec::new(),
            classes: Vec::new(),
            styles: Vec::new(),
            metadata: Vec::new(),
            range,
            region: false,
        });
        i
    }

    /// `partition Name {`, `group Name`, and the other groupings: the following nodes go in
    /// a box with the title (PACT-7).
    fn group(&mut self, line: &Line) {
        let rest = line.after_word();
        let (rest, brace) = match rest.strip_suffix('{') {
            Some(r) => (r.trim_end(), true),
            None => (rest, false),
        };
        // A `#color` or `<<stereotype>>` after the name is not drawn.
        let name = rest
            .split(" #")
            .next()
            .unwrap_or(rest)
            .split("<<")
            .next()
            .unwrap_or(rest)
            .trim();
        let name = lines::unquote(name);
        let i = self.subgraph(name, line.range);
        let id = self.chart.subgraphs[i].id.clone();
        self.place(&id);
        // `partition Name` may put its `{` on the next line; `group` closes with `end group`.
        let brace = brace || line.word() != "group";
        self.push(
            Open::Group {
                subgraph: i,
                brace,
                range: line.range,
            },
            line.range,
        );
    }

    fn close_group(&mut self, line: &Line, brace: bool) {
        match self.stack.last() {
            Some(Open::Group { brace: b, .. }) if *b == brace => {
                self.stack.pop();
            }
            _ => {
                let what = if brace { "`}`" } else { "`end group`" };
                self.error(line.range, format!("{what} without a group to close"));
            }
        }
    }

    fn if_(&mut self, line: &Line) {
        let Some((cond, rest)) = parens(line.after_word()) else {
            return self.error(line.range, "expected `(condition)` after `if`");
        };
        let mut text = lines::text(cond.trim());
        let (is, rest) = label_after(rest, "is");
        if let Some(is) = is {
            text = format!("{text} {is}");
        }
        let (yes, _) = label_after(rest, "then");
        let decision = self.add(Shape::Diamond, &text, line.range);
        self.tails = vec![Tail::labeled(&decision, yes)];
        self.push(
            Open::If {
                decision,
                ends: Vec::new(),
                otherwise: false,
                range: line.range,
            },
            line.range,
        );
    }

    /// `elseif (cond) then (yes)`, with `no` from a leading `(no)` on the edge to the new test.
    fn else_if(&mut self, line: &Line, rest: &str, no: Option<String>) {
        match self.stack.last() {
            Some(Open::If {
                otherwise: false, ..
            }) => {}
            Some(Open::If { .. }) => {
                return self.error(line.range, "`elseif` after `else`; expected `endif`");
            }
            _ => return self.error(line.range, "`elseif` belongs in an `if`"),
        }
        let Some((cond, rest)) = parens(rest) else {
            return self.error(line.range, "expected `(condition)` after `elseif`");
        };
        self.bake();
        let Some(Open::If { decision, ends, .. }) = self.stack.last_mut() else {
            unreachable!()
        };
        ends.append(&mut self.tails);
        let from = decision.clone();
        let (yes, _) = label_after(rest, "then");
        let next = self.node(Shape::Diamond, &lines::text(cond.trim()), line.range);
        self.edge(&from, &next, no, line.range);
        if let Some(Open::If { decision, .. }) = self.stack.last_mut() {
            *decision = next.clone();
        }
        self.last = Some(next.clone());
        self.tails = vec![Tail::labeled(&next, yes)];
    }

    fn else_(&mut self, line: &Line) {
        if matches!(
            self.stack.last(),
            Some(Open::If {
                otherwise: false,
                ..
            })
        ) {
            self.bake();
        }
        let Some(Open::If {
            decision,
            ends,
            otherwise,
            ..
        }) = self.stack.last_mut()
        else {
            return self.error(line.range, "`else` belongs in an `if`");
        };
        if *otherwise {
            let range = line.range;
            return self.error(range, "`if` already has an `else`");
        }
        *otherwise = true;
        ends.append(&mut self.tails);
        let node = decision.clone();
        self.tails = vec![Tail::labeled(&node, label(line.after_word()))];
    }

    fn end_if(&mut self, line: &Line) {
        let is = |o: &Open| matches!(o, Open::If { .. });
        if !self.unwind(line, is, "`endif` without an `if` to close") {
            return;
        }
        self.bake();
        let Some(Open::If {
            decision,
            mut ends,
            otherwise,
            ..
        }) = self.stack.pop()
        else {
            unreachable!()
        };
        ends.append(&mut self.tails);
        if !otherwise {
            ends.push(Tail::plain(&decision));
        }
        self.tails = ends;
    }

    fn switch(&mut self, line: &Line) {
        let Some((cond, _)) = parens(line.after_word()) else {
            return self.error(line.range, "expected `(value)` after `switch`");
        };
        let decision = self.add(Shape::Diamond, &lines::text(cond.trim()), line.range);
        self.tails.clear();
        self.push(
            Open::Switch {
                decision,
                ends: Vec::new(),
                cases: 0,
                range: line.range,
            },
            line.range,
        );
    }

    fn case(&mut self, line: &Line) {
        if matches!(self.stack.last(), Some(Open::Switch { .. })) {
            self.bake();
        }
        let Some(Open::Switch {
            decision,
            ends,
            cases,
            ..
        }) = self.stack.last_mut()
        else {
            return self.error(line.range, "`case` belongs in a `switch`");
        };
        if *cases > 0 {
            ends.append(&mut self.tails);
        }
        *cases += 1;
        let node = decision.clone();
        self.tails = vec![Tail::labeled(&node, label(line.after_word()))];
    }

    fn end_switch(&mut self, line: &Line) {
        let is = |o: &Open| matches!(o, Open::Switch { .. });
        if !self.unwind(line, is, "`endswitch` without a `switch` to close") {
            return;
        }
        self.bake();
        let Some(Open::Switch {
            decision,
            mut ends,
            cases,
            ..
        }) = self.stack.pop()
        else {
            unreachable!()
        };
        ends.append(&mut self.tails);
        if cases == 0 {
            ends.push(Tail::plain(&decision));
        }
        self.tails = ends;
    }

    fn while_(&mut self, line: &Line) {
        let Some((cond, rest)) = parens(line.after_word()) else {
            return self.error(line.range, "expected `(condition)` after `while`");
        };
        let (yes, _) = label_after(rest, "is");
        let decision = self.add(Shape::Diamond, &lines::text(cond.trim()), line.range);
        self.tails = vec![Tail::labeled(&decision, yes)];
        self.push(
            Open::While {
                decision,
                breaks: Vec::new(),
                range: line.range,
            },
            line.range,
        );
    }

    fn end_while(&mut self, line: &Line) {
        let is = |o: &Open| matches!(o, Open::While { .. });
        if !self.unwind(line, is, "`endwhile` without a `while` to close") {
            return;
        }
        let Some(Open::While {
            decision,
            mut breaks,
            ..
        }) = self.stack.pop()
        else {
            unreachable!()
        };
        // Back to the test.
        self.join(&decision, line.range);
        let squeezed = line.squeezed();
        let rest = if squeezed.starts_with("end while") {
            line.after_word().trim_start()[5..].trim_start()
        } else {
            line.after_word()
        };
        let mut tails = vec![Tail::labeled(&decision, label(rest))];
        tails.append(&mut breaks);
        self.tails = tails;
    }

    fn repeat(&mut self, line: &Line, cursor: &mut Cursor) {
        let rest = line.after_word();
        let start = if rest.starts_with(':') || rest.starts_with('#') {
            let action = Line {
                text: rest.to_owned(),
                range: line.range_of(rest),
            };
            match self.action(&action, cursor, false) {
                Some(id) => id,
                None => return,
            }
        } else {
            self.add(Shape::Diamond, "", line.range)
        };
        self.push(
            Open::Repeat {
                start,
                backward: None,
                breaks: Vec::new(),
                range: line.range,
            },
            line.range,
        );
    }

    fn backward(&mut self, line: &Line, cursor: &mut Cursor) {
        if !matches!(self.stack.last(), Some(Open::Repeat { .. })) {
            return self.error(line.range, "`backward` belongs in a `repeat`");
        }
        let rest = line.after_word();
        let action = Line {
            text: rest.to_owned(),
            range: line.range_of(rest),
        };
        let Some(id) = self.action(&action, cursor, true) else {
            return;
        };
        if let Some(Open::Repeat { backward, .. }) = self.stack.last_mut() {
            *backward = Some(id);
        }
    }

    fn repeat_while(&mut self, line: &Line) {
        let is = |o: &Open| matches!(o, Open::Repeat { .. });
        if !self.unwind(line, is, "`repeat while` without a `repeat` to close") {
            return;
        }
        let Some(Open::Repeat {
            start,
            backward,
            mut breaks,
            ..
        }) = self.stack.pop()
        else {
            unreachable!()
        };
        let t = line.text.as_str();
        let at = t.to_ascii_lowercase().find("while").expect("checked") + 5;
        let rest = t[at..].trim_start();
        let (cond, rest) = parens(rest).unwrap_or(("", rest));
        let (yes, rest) = label_after(rest, "is");
        let (no, _) = label_after(rest, "not");
        let decision = self.add(Shape::Diamond, &lines::text(cond.trim()), line.range);
        match backward {
            Some(back) => {
                self.edge(&decision, &back, yes, line.range);
                self.edge(&back, &start, None, line.range);
            }
            None => self.edge(&decision, &start, yes, line.range),
        }
        let mut tails = vec![Tail::labeled(&decision, no)];
        tails.append(&mut breaks);
        self.tails = tails;
    }

    fn brk(&mut self, line: &Line) {
        let target = self.stack.iter_mut().rev().find_map(|o| match o {
            Open::While { breaks, .. } | Open::Repeat { breaks, .. } => Some(breaks),
            _ => None,
        });
        // Without a loop the flow goes on as if the `break` were not there.
        match target {
            Some(breaks) => breaks.append(&mut self.tails),
            None => self.error(line.range, "`break` belongs in a `while` or `repeat`"),
        }
    }

    fn fork(&mut self, line: &Line, bar: bool) {
        let (bar, from) = if bar {
            (Some(self.add(Shape::Fork, "", line.range)), Vec::new())
        } else {
            (None, self.tails.clone())
        };
        self.push(
            Open::Fork {
                bar,
                from,
                ends: Vec::new(),
                range: line.range,
            },
            line.range,
        );
    }

    fn fork_again(&mut self, line: &Line) {
        if matches!(self.stack.last(), Some(Open::Fork { .. })) {
            self.bake();
        }
        let Some(Open::Fork {
            bar, from, ends, ..
        }) = self.stack.last_mut()
        else {
            return self.error(line.range, "`fork again` belongs in a `fork` or `split`");
        };
        // Empty `split` branches repeat the same ends; keep one of each.
        ends.append(&mut self.tails);
        dedupe(ends);
        self.tails = match bar {
            Some(bar) => vec![Tail::plain(bar)],
            None => from.clone(),
        };
    }

    /// `end fork`, `end merge`, or `end split` (`closer`, squeezed).
    fn end_fork(&mut self, line: &Line, closer: &str) {
        let merge = closer.starts_with("end merge");
        let (used, opener) = if closer.starts_with("end split") {
            ("end split", "split")
        } else if merge {
            ("end merge", "fork")
        } else {
            ("end fork", "fork")
        };
        let is = |o: &Open| matches!(o, Open::Fork { .. });
        if !self.unwind(line, is, &format!("`{used}` without a `{opener}` to close")) {
            return;
        }
        self.bake();
        let Some(Open::Fork { bar, mut ends, .. }) = self.stack.pop() else {
            unreachable!()
        };
        ends.append(&mut self.tails);
        dedupe(&mut ends);
        self.tails = ends;
        if bar.is_some() {
            let shape = if merge { Shape::Diamond } else { Shape::Fork };
            self.add(shape, "", line.range);
        }
    }

    /// `note left: text`, `note right` ... `end note`, and `floating note`: beside the last
    /// node (PACT-8).
    fn note(&mut self, line: &Line, cursor: &mut Cursor) {
        let mut rest = line.text.as_str();
        if let Some(r) = keyword(rest, "floating") {
            rest = r;
        }
        let Some(r) = keyword(rest, "note") else {
            return self.error(line.range, STATEMENT);
        };
        let w = lines::word(r).to_ascii_lowercase();
        let (side, r) = match w.as_str() {
            "left" => (NoteSide::Before, r[4..].trim_start()),
            "right" => (NoteSide::After, r[5..].trim_start()),
            _ => (NoteSide::After, r),
        };
        // A `#color` after the side is not drawn.
        let r = match r.strip_prefix('#') {
            Some(c) => c[lines::word(c).len()..].trim_start(),
            None => r,
        };
        let (text, range) = match r.strip_prefix(':') {
            Some(text) => (lines::text(text.trim()), line.range),
            None => {
                let (body, end) = cursor.until(&["end note", "endnote"]);
                let Some(end) = end else {
                    return self.error(line.range, "unclosed `note`: expected `end note`");
                };
                let text: Vec<&str> = body.iter().map(|l| l.text.as_str()).collect();
                (lines::text(&text.join("\n")), line.range.cover(end.range))
            }
        };
        let Some(target) = self.last.clone() else {
            return self.warn(
                line.range,
                "a note needs an activity before it; ignoring it",
            );
        };
        self.chart.notes.push(Note {
            target,
            side,
            label: Label::plain(text),
            range,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_src(body: &str) -> (Flowchart, Vec<String>) {
        let src = format!("@startuml\n{body}\n@enduml\n");
        let (diagram, d) = super::super::parse(&src).lower();
        let chart = diagram.into_graph();
        (chart, d.into_iter().map(|d| d.message).collect())
    }

    fn edges(c: &Flowchart) -> Vec<String> {
        let name = |id: &str| {
            let n = c.node(id).expect("node");
            if n.label.text.is_empty() {
                format!("{:?}", n.shape)
            } else {
                n.label.text.clone()
            }
        };
        c.edges
            .iter()
            .map(|e| match &e.label {
                Some(l) => format!("{}>{}:{}", name(&e.from), name(&e.to), l.text),
                None => format!("{}>{}", name(&e.from), name(&e.to)),
            })
            .collect()
    }

    #[test]
    fn flow() {
        let (c, d) = lower_src("start\n:a;\n-> go;\n:b\nc;\nstop");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&c),
            ["SmallCircle>a", "a>b\nc:go", "b\nc>FramedCircle"]
        );
    }

    #[test]
    fn branches() {
        let (c, d) = lower_src(
            "if (x?) then (yes)\n:a;\nelseif (y?) then (ok)\n:b;\nelse (no)\n:c;\nendif\n:d;",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&c),
            [
                "x?>a:yes", "x?>y?", "y?>b:ok", "y?>c:no", "a>d", "b>d", "c>d"
            ]
        );
        let (c, _) = lower_src("if (x) then\n:a;\nendif\n:b;");
        assert_eq!(edges(&c), ["x>a", "a>b", "x>b"]);
    }

    #[test]
    fn loops() {
        let (c, d) = lower_src("while (more?) is (yes)\n:a;\nendwhile (no)\n:b;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["more?>a:yes", "a>more?", "more?>b:no"]);
        let (c, d) = lower_src(
            "repeat :a;\n:b;\nbackward :c;\nrepeat while (again?) is (yes) not (no)\n:d;",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&c),
            ["a>b", "b>again?", "again?>c:yes", "c>a", "again?>d:no"]
        );
        let (c, _) = lower_src("while (x)\n:a;\nbreak\nendwhile\n:b;");
        assert_eq!(edges(&c), ["x>a", "x>b", "a>b"]);
    }

    #[test]
    fn forks() {
        let (c, d) = lower_src("fork\n:a;\nfork again\n:b;\nend fork\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&c),
            ["Fork>a", "Fork>b", "a>Fork", "b>Fork", "Fork>c"]
        );
        let (c, _) = lower_src(":s;\nsplit\n:a;\nsplit again\n:b;\nend split\n:c;");
        assert_eq!(edges(&c), ["s>a", "s>b", "a>c", "b>c"]);
    }

    #[test]
    fn groups_and_lanes() {
        let (c, d) = lower_src("|A|\nstart\n|B|\n:x;\npartition P {\n:y;\n}\n|A|\nstop");
        assert!(d.is_empty(), "{d:?}");
        assert!(c.lanes);
        let titles: Vec<&str> = c.subgraphs.iter().map(|s| s.title.text.as_str()).collect();
        assert_eq!(titles, ["A", "B", "P"]);
        assert_eq!(c.subgraphs[0].members, ["a1", "a4"]);
        assert_eq!(c.subgraphs[1].members, ["a2", "g3"]);
        assert_eq!(c.subgraphs[2].members, ["a3"]);
    }

    #[test]
    fn errors() {
        let (_, d) = lower_src("if (x) then\n:a;");
        assert_eq!(d, ["unclosed `if`: expected `endif`"]);
        let (_, d) = lower_src("endif\nfoo bar\n:a");
        assert_eq!(
            d,
            [
                "`endif` without an `if` to close",
                STATEMENT,
                "unclosed action: expected `;` at its end"
            ]
        );
        let (_, d) = lower_src("(*) --> \"a\"\nif \"c\" then\n-->[y] \"d\"\nelse\nendif");
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("older activity syntax"), "{d:?}");
    }

    #[test]
    fn elseif_after_else_is_an_error() {
        let (c, d) = lower_src("if (x) then\n:a;\nelse\n:b;\nelseif (y) then\n:c;\nendif\n:d;");
        assert_eq!(d, ["`elseif` after `else`; expected `endif`"]);
        // The line is skipped; `:c` continues the `else` branch.
        assert_eq!(edges(&c), ["x>a", "x>b", "b>c", "a>d", "c>d"]);
    }

    #[test]
    fn malformed_elseif_keeps_the_flow() {
        let (c, d) = lower_src("if (x) then\n:a;\nelseif y\n:b;\nendif\n:c;");
        assert_eq!(d, ["expected `(condition)` after `elseif`"]);
        assert_eq!(edges(&c), ["x>a", "a>b", "b>c", "x>c"]);
    }

    #[test]
    fn break_outside_a_loop_keeps_the_flow() {
        let (c, d) = lower_src(":a;\nbreak\n:b;");
        assert_eq!(d, ["`break` belongs in a `while` or `repeat`"]);
        assert_eq!(edges(&c), ["a>b"]);
    }

    #[test]
    fn arrow_style_closing_before_opening_does_not_panic() {
        let (c, d) = lower_src(":a;\n-][>\n:b;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["a>b"]);
    }

    #[test]
    fn sequential_empty_splits_stay_bounded() {
        let body = ":s;\n".to_owned() + &"split\nsplit again\nend split\n".repeat(22) + ":e;";
        let started = std::time::Instant::now();
        let (c, d) = lower_src(&body);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["s>e"]);
        assert!(started.elapsed().as_secs() < 5);
    }

    #[test]
    fn edges_are_capped_with_one_error() {
        let mut l = Lower::new();
        let range = TextRange::default();
        let join = |l: &mut Lower, to: &str| {
            l.tails = (0..MAX_EDGES / 2 + 1)
                .map(|i| Tail::plain(&format!("t{i}")))
                .collect();
            l.join(to, range);
        };
        join(&mut l, "x");
        join(&mut l, "y");
        join(&mut l, "z");
        assert_eq!(l.chart.edges.len(), MAX_EDGES);
        assert_eq!(l.d.len(), 1);
        assert!(l.d[0].message.contains("edges"), "{:?}", l.d[0].message);
    }

    #[test]
    fn arrow_before_else_labels_only_its_branch() {
        let (c, d) = lower_src("if (x) then (yes)\n:a;\n-> done;\nelse (no)\n:b;\nendif\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["x>a:yes", "x>b:no", "a>c:done", "b>c"]);
        let (c, _) = lower_src(
            "if (x) then\n:a;\n-> one;\nelseif (y) then\n:b;\n-> two;\nelse\n:c;\nendif\n:d;",
        );
        assert_eq!(
            edges(&c),
            ["x>a", "x>y", "y>b", "y>c", "a>d:one", "b>d:two", "c>d"]
        );
        let (c, _) = lower_src("switch (v)\ncase (1)\n:a;\n-> one;\ncase (2)\n:b;\nendswitch\n:c;");
        assert_eq!(edges(&c), ["v>a:1", "v>b:2", "a>c:one", "b>c"]);
        let (c, _) = lower_src("fork\n:a;\n-> one;\nfork again\n:b;\nend fork");
        assert_eq!(edges(&c), ["Fork>a", "Fork>b", "a>Fork:one", "b>Fork"]);
    }

    #[test]
    fn arrow_before_endif_styles_one_merging_edge() {
        let (c, d) = lower_src("if (x) then\n:a;\nelse\n:b;\n-[#red,dashed]->\nendif\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["x>a", "x>b", "a>c", "b>c"]);
        let styled: Vec<bool> = c
            .edges
            .iter()
            .map(|e| e.stroke == Stroke::Dotted && !e.styles.is_empty())
            .collect();
        assert_eq!(styled, [false, false, false, true]);
        // An arrow after the closer still styles every merging edge.
        let (c, _) = lower_src("if (x) then\n:a;\nelse\n:b;\nendif\n-> all;\n:c;");
        assert_eq!(edges(&c), ["x>a", "x>b", "a>c:all", "b>c:all"]);
    }

    #[test]
    fn leading_label_before_elseif() {
        let (c, d) =
            lower_src("if (x) then (yes)\n:a;\n(no) elseif (y) then (ok)\n:b;\nendif\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            edges(&c),
            ["x>a:yes", "x>y:no", "y>b:ok", "a>c", "b>c", "y>c"]
        );
        let (c, d) = lower_src("if (x) then\n:a;\n(no) else if (y) then\n:b;\nendif");
        assert!(d.is_empty(), "{d:?}");
        assert!(edges(&c).contains(&"x>y:no".to_owned()), "{:?}", edges(&c));
    }

    #[test]
    fn closing_tag_does_not_close_an_action() {
        let (c, d) = lower_src(":This is <b>bold</b>\nand more;\n:b;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["This is <b>bold</b>\nand more>b"]);
        assert_eq!(c.nodes[0].shape, Shape::Rounded);
        let (c, _) = lower_src(":x <i>\ny;");
        assert_eq!(c.nodes[0].shape, Shape::Rounded);
        // A plain trailing `>` still closes.
        let (c, _) = lower_src(":send>");
        assert_eq!(c.nodes[0].shape, Shape::LeanRight);
    }

    #[test]
    fn long_multiline_action_is_linear() {
        let body = ":x\n".to_owned() + &"abcdefghi\n".repeat(50_000) + "done;";
        let started = std::time::Instant::now();
        let (c, d) = lower_src(&body);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(c.nodes.len(), 1);
        // Rescanning the growing body per line took many seconds here.
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn closer_reports_the_unclosed_inner_block() {
        let (c, d) = lower_src("start\nif (x) then\nwhile (y)\n:a;\nendif\n:b;");
        assert_eq!(d, ["unclosed `while`: expected `endwhile`"]);
        assert!(edges(&c).contains(&"a>b".to_owned()), "{:?}", edges(&c));
        let (_, d) = lower_src("while (y)\nfork\n:a;\nendwhile");
        assert_eq!(d, ["unclosed `fork`: expected `end fork`"]);
        let (_, d) = lower_src("start\nend split");
        assert_eq!(d, ["`end split` without a `split` to close"]);
        let (_, d) = lower_src("start\nend fork");
        assert_eq!(d, ["`end fork` without a `fork` to close"]);
        let (_, d) = lower_src("if (x) then\nend split\nendif");
        assert_eq!(d, ["`end split` without a `split` to close"]);
    }

    #[test]
    fn closer_reports_unclosed_repeat_switch_and_group() {
        let (_, d) = lower_src("start\nif (x) then\nrepeat\n:a;\nendif\n:b;");
        assert_eq!(d, ["unclosed `repeat`: expected `repeat while`"]);
        let (c, d) = lower_src("if (x) then\nswitch (v)\ncase (1)\n:a;\nendif\n:b;");
        assert_eq!(d, ["unclosed `switch`: expected `endswitch`"]);
        assert!(edges(&c).contains(&"a>b".to_owned()), "{:?}", edges(&c));
        let (c, d) = lower_src("fork\npartition P {\n:a;\nend fork\n:b;");
        assert_eq!(d, ["unclosed `partition`: expected `}`"]);
        assert!(edges(&c).contains(&"a>Fork".to_owned()), "{:?}", edges(&c));
        let (_, d) = lower_src("while (y)\ngroup G\n:a;\nendwhile");
        assert_eq!(d, ["unclosed `group`: expected `end group`"]);
        // A repeat popped by `endwhile` keeps its breaks as open ends.
        let (c, d) = lower_src("while (y)\nrepeat\nbreak\nendwhile\n:z;");
        assert_eq!(d, ["unclosed `repeat`: expected `repeat while`"]);
        assert!(c.edges.len() >= 2);
    }

    #[test]
    fn deduped_tails_with_different_arrows_keep_the_first() {
        let (c, d) = lower_src(":s;\nsplit\n-> one;\nsplit again\n-> two;\nend split\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["s>c:one"]);
        // Distinct nodes keep their own arrows.
        let (c, _) = lower_src("fork\n:a;\n-> one;\nfork again\n:b;\n-> two;\nend fork");
        assert_eq!(edges(&c), ["Fork>a", "Fork>b", "a>Fork:one", "b>Fork:two"]);
    }

    #[test]
    fn elseif_without_an_if_is_an_error() {
        let (_, d) = lower_src("start\n(no) elseif (y) then\n:a;");
        assert_eq!(d, ["`elseif` belongs in an `if`"]);
        let (_, d) = lower_src("start\n(no) else if (y) then\n:a;");
        assert_eq!(d, ["`elseif` belongs in an `if`"]);
        let (_, d) = lower_src("while (w)\n(no) elseif (y) then\nendwhile");
        assert_eq!(d, ["`elseif` belongs in an `if`"]);
        let (_, d) = lower_src("start\nelseif (y) then");
        assert_eq!(d, ["`elseif` belongs in an `if`"]);
    }

    #[test]
    fn leading_label_without_elseif_is_not_a_connector_or_elseif() {
        let (_, d) = lower_src("start\n(no) foo");
        assert_eq!(d, [STATEMENT]);
        let (_, d) = lower_src("start\n(no) elsewhere");
        assert_eq!(d, [STATEMENT]);
    }

    #[test]
    fn ends_in_tag_edges() {
        assert!(ends_in_tag("x <b>"));
        assert!(ends_in_tag("x </b>"));
        assert!(ends_in_tag("x <abcde>"));
        // Six-character names and empty tags are not tags.
        assert!(!ends_in_tag("x <abcdef>"));
        assert!(!ends_in_tag("x <>"));
        assert!(!ends_in_tag("x </>"));
        assert!(!ends_in_tag("send>"));
        assert!(!ends_in_tag("a < b>"));
        assert!(!ends_in_tag("a/b>"));
        assert!(!ends_in_tag(""));
        assert!(!ends_in_tag(">"));
        // Non-ASCII names count in characters, not bytes.
        assert!(ends_in_tag("x <\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}>"));
        assert!(!ends_in_tag("x <\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}>"));
        assert!(ends_in_tag("x <\u{65e5}\u{672c}>"));
        assert!(!ends_in_tag("\u{65e5}\u{672c}>"));
        let (c, _) = lower_src(":x <abcdef>");
        assert_eq!(c.nodes[0].shape, Shape::LeanRight);
        let (c, _) = lower_src(":x <>");
        assert_eq!(c.nodes[0].shape, Shape::LeanRight);
    }

    #[test]
    fn action_end_handles_stereotypes_and_tags() {
        assert_eq!(action_end("a;"), Some((1, ';')));
        assert_eq!(action_end("a; <<input>>"), Some((1, ';')));
        assert_eq!(action_end("a;<<input>>  "), Some((1, ';')));
        assert_eq!(action_end("a <b>"), None);
        assert_eq!(action_end("a"), None);
        assert_eq!(action_end(""), None);
        assert_eq!(action_end("\u{e9};"), Some((2, ';')));
    }

    #[test]
    fn stereotype_on_a_later_action_line() {
        let (c, d) = lower_src(":a\nb; <<input>>\n:c;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(c.nodes.len(), 2);
        assert_eq!(c.nodes[0].label.text, "a\nb");
        assert_eq!(c.nodes[0].shape, Shape::Rounded);
        // Only the newest line closes it: a `;` mid-body does not.
        let (c, d) = lower_src(":a;b\nc;");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(c.nodes.len(), 1);
        assert_eq!(c.nodes[0].label.text, "a;b\nc");
    }

    #[test]
    fn arrow_before_the_first_case_is_dropped_not_leaked() {
        let (c, d) = lower_src("switch (v)\n-> early;\ncase (1)\n:a;\ncase (2)\n:b;\nendswitch");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(edges(&c), ["v>a:1", "v>b:2"]);
    }

    #[test]
    fn edge_cap_applies_to_direct_edges() {
        let mut l = Lower::new();
        let range = TextRange::default();
        l.chart.edges = (0..MAX_EDGES).map(|_| Edge::new("p", "q", range)).collect();
        l.edge("a", "b", None, range);
        l.edge("a", "b", Some("x".into()), range);
        assert_eq!(l.chart.edges.len(), MAX_EDGES);
        assert_eq!(l.d.len(), 1);

        // `elseif` still adds its node and tails past the cap.
        let mut l = Lower::new();
        l.chart.edges = (0..MAX_EDGES).map(|_| Edge::new("p", "q", range)).collect();
        let at = |t: &str| Line {
            text: t.to_owned(),
            range,
        };
        l.statement(&at("if (x) then"), &mut Cursor::new(Vec::new()));
        l.statement(&at("elseif (y) then"), &mut Cursor::new(Vec::new()));
        assert_eq!(l.chart.edges.len(), MAX_EDGES);
        assert_eq!(l.chart.nodes.len(), 2);
        assert_eq!(l.tails.len(), 1);
        assert_eq!(l.d.len(), 1);

        // `repeat while` too, without a second error.
        let mut l = Lower::new();
        l.chart.edges = (0..MAX_EDGES).map(|_| Edge::new("p", "q", range)).collect();
        for t in ["repeat", ":a;", "repeat while (c)", ":z;"] {
            l.statement(&at(t), &mut Cursor::new(Vec::new()));
        }
        assert_eq!(l.chart.edges.len(), MAX_EDGES);
        assert_eq!(l.chart.nodes.len(), 4);
        assert_eq!(l.d.len(), 1);
        assert!(l.d[0].message.contains("edges"));
    }
}
