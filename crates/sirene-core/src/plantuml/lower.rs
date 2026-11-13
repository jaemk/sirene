//! Collects a PlantUML sequence diagram's participants and events (PSEQ-2 to PSEQ-6).

use std::collections::HashMap;

use rowan::TextRange;

use crate::diagnostic::Diagnostic;
use crate::lines::{MAX_DEPTH, too_deep};
use crate::scene::parse_color;
use crate::sequence::{
    Arrow, BlockKind, Event, Group, Head, LineStyle, Message, Note, NoteSide, Participant,
    ParticipantKind, Sequence, Text,
};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

/// The most pixels a `||N||` space takes.
const MAX_SPACE: u32 = 1000;

pub(crate) fn lower(root: &SyntaxNode) -> (Sequence, Vec<Diagnostic>) {
    let mut l = Lower::default();
    for stmt in root.children() {
        // A statement the parser could not read is reported there and skipped here.
        if stmt.descendants().any(|n| n.kind() == ERROR) {
            continue;
        }
        // Inside a block nested too deep, only the statements that open and close blocks count.
        if l.skipped > 0 && stmt.kind() != SEQ_STMT {
            continue;
        }
        match stmt.kind() {
            PARTICIPANT_STMT => l.participant(&stmt),
            MESSAGE_STMT => l.message(&stmt),
            NOTE_STMT => l.note(&stmt),
            SEQ_STMT => l.keyword(&stmt),
            TITLE_STMT => {
                let lines: Vec<String> = tokens(&stmt, TEXT)
                    .iter()
                    .map(|t| t.text().trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect();
                l.seq.title = (!lines.is_empty()).then(|| lines.join(" "));
            }
            _ => {}
        }
    }
    l.finish()
}

/// A `box` (with its group) or frame waiting for its `end`.
enum Open {
    Box(TextRange, usize),
    Block(BlockKind, TextRange),
}

/// Which side of the diagram an edge message (`[->`, `->]`) comes from or goes to.
#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Left,
    Right,
}

#[derive(Default)]
struct Lower {
    seq: Sequence,
    diagnostics: Vec<Diagnostic>,
    stack: Vec<Open>,
    /// How many frames are open past the nesting limit; their content is left out (LANGS-8).
    skipped: usize,
    deep_block: bool,
    deep_activation: bool,
    /// Each participant's index by id.
    index: HashMap<String, usize>,
    /// Where each participant's open activations began.
    active: Vec<Vec<TextRange>>,
    /// Each activation made by a message or after one: the participant, its caller, and
    /// whether it is still open, for `return`. The last entry is always open.
    calls: Vec<(usize, usize, bool)>,
    /// Each participant's open entries in `calls`, latest last.
    calls_of: Vec<Vec<usize>>,
    /// The next `autonumber` number and the step, while numbering is on, and while paused.
    numbering: Option<(u32, u32)>,
    paused: Option<(u32, u32)>,
    /// Each participant's `order`, whether a message has named it, and the hidden edges.
    order: Vec<Option<i64>>,
    mentioned: Vec<bool>,
    edges: [Option<usize>; 2],
    /// The latest message's event.
    last_message: Option<usize>,
    /// The `note across` events, filled in once every participant is known.
    across: Vec<usize>,
    /// The `note left`/`note right` events without `of` and the message each goes beside,
    /// resolved once `order` has placed the participants.
    beside: Vec<(usize, usize)>,
    /// Warnings given once.
    told_o: bool,
    told_style: bool,
    /// Whether `autoactivate on` makes each message activate its target.
    autoactivate: bool,
}

fn tokens(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == kind)
        .collect()
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node, kind).into_iter().next()
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

/// A name without its quotes.
fn unquote(t: &SyntaxToken) -> String {
    let s = t.text();
    if t.kind() == STRING {
        s[1..s.len() - 1].to_owned()
    } else {
        s.to_owned()
    }
}

/// Display text: split into lines at a literal `\n`, each trimmed.
fn display(s: &str) -> Text {
    s.split("\\n").map(|l| l.trim().to_owned()).collect()
}

/// The non-trivia tokens of `node`, in order.
fn significant(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .collect()
}

/// What a PlantUML arrow spells (PSEQ-3).
#[derive(Debug, PartialEq)]
struct Spelled {
    arrow: Arrow,
    /// The head is only at the left: the message runs from the right end to the left.
    reversed: bool,
    /// An `o` end, drawn as a filled head.
    circle: bool,
    /// A `[style]` in the dashes.
    styled: bool,
}

/// The head one side of an arrow names: `>`/`<` filled, `>>`/`<<` and the half heads open,
/// and `x` beside one a cross. An `x` alone is a head on the right (`-x`), and an `o` is
/// never one.
fn head(s: &str, right: bool) -> Option<Head> {
    let core: String = s.chars().filter(|c| !matches!(c, 'x' | 'o')).collect();
    if core.is_empty() {
        return (right && s.contains('x')).then_some(Head::Cross);
    }
    Some(if s.contains('x') {
        Head::Cross
    } else if core == ">" || core == "<" {
        Head::Filled
    } else {
        Head::Open
    })
}

fn spell(s: &str) -> Spelled {
    // `-[style]>` and `-[style]->`: the style is dropped, and the dashes on both sides count,
    // so `-[#red]->` is dashed as in PlantUML.
    let (s, styled) = match (s.find('['), s.rfind(']')) {
        (Some(a), Some(b)) if a < b => (format!("{}{}", &s[..a], &s[b + 1..]), true),
        _ => (s.to_owned(), false),
    };
    let dashes = s.matches('-').count();
    let first = s.find('-').unwrap_or(0);
    let last = s.rfind('-').unwrap_or(first);
    let line = if dashes >= 2 {
        LineStyle::Dotted
    } else {
        LineStyle::Solid
    };
    let (left, right) = (&s[..first], &s[last + 1..]);
    let circle = left.contains('o') || right.contains('o');
    let (l, r) = (head(left, false), head(right, true));
    let (head, both, reversed) = match (l, r) {
        (Some(l), None) => (l, false, true),
        (Some(_), Some(r)) => (r, true, false),
        (None, r) => (r.unwrap_or(Head::Filled), false, false),
    };
    Spelled {
        arrow: Arrow { line, head, both },
        reversed,
        circle,
        styled,
    }
}

/// The color and title of `box "Title" #color`, `box Title #color`, or `box #color`.
fn group(s: &str) -> (Group, bool) {
    let mut title = Vec::new();
    let mut color = None;
    let mut bad_color = false;
    let mut rest = s.trim();
    while !rest.is_empty() {
        if let Some(body) = rest.strip_prefix('"') {
            let end = body.find('"').unwrap_or(body.len());
            title.push(body[..end].to_owned());
            rest = body.get(end + 1..).unwrap_or("").trim_start();
            continue;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let word = &rest[..end];
        if let Some(c) = word.strip_prefix('#') {
            let named = if parse_color(word).is_some() { word } else { c };
            if parse_color(named).is_some() {
                color = Some(named.to_owned());
            } else {
                bad_color = true;
            }
        } else {
            title.push(word.to_owned());
        }
        rest = rest[end..].trim_start();
    }
    let title = title.join(" ");
    let g = Group {
        color,
        title: (!title.is_empty()).then_some(title),
    };
    (g, bad_color)
}

/// `text` without its leading `#color` words, and those words.
fn split_colors(text: &str) -> (String, String) {
    let mut rest = text.trim_start();
    let mut colors: Vec<&str> = Vec::new();
    while let Some(word) = rest.split_whitespace().next()
        && word.len() > 1
        && word.starts_with('#')
        && word[1..].chars().all(|c| c.is_ascii_alphanumeric())
        && (parse_color(word).is_some() || word[1..].starts_with(|c: char| c.is_ascii_alphabetic()))
    {
        colors.push(word);
        rest = rest[word.len()..].trim_start();
    }
    (rest.to_owned(), colors.join(" "))
}

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    fn push_participant(&mut self, id: &str, label: Text, kind: ParticipantKind) -> usize {
        let i = self.seq.participants.len();
        // The first participant with an id keeps it.
        self.index.entry(id.to_owned()).or_insert(i);
        self.seq.participants.push(Participant {
            id: id.to_owned(),
            label,
            kind,
            group: None,
        });
        self.active.push(Vec::new());
        self.calls_of.push(Vec::new());
        self.order.push(None);
        self.mentioned.push(false);
        i
    }

    /// The participant named `id`, added on first mention.
    fn ensure(&mut self, id: &str) -> usize {
        match self.find(id) {
            Some(i) => i,
            None => self.push_participant(id, display(id), ParticipantKind::Participant),
        }
    }

    /// The hidden participant at the left or right edge of the diagram.
    fn edge(&mut self, edge: Edge) -> usize {
        let at = edge as usize;
        if let Some(i) = self.edges[at] {
            return i;
        }
        let id = if edge == Edge::Left { "[" } else { "]" };
        let i = self.push_participant(id, Vec::new(), ParticipantKind::Hidden);
        self.edges[at] = Some(i);
        i
    }

    /// Where participant `i` sits, before `order` sorts them: the edges at the ends.
    fn rank(&self, i: usize) -> (u8, usize) {
        match self.edges.iter().position(|&e| e == Some(i)) {
            Some(0) => (0, i),
            Some(_) => (2, i),
            None => (1, i),
        }
    }

    /// The innermost open `box`'s group, also from inside a frame in it.
    fn current_group(&self) -> Option<usize> {
        self.stack.iter().rev().find_map(|o| match o {
            Open::Box(_, g) => Some(*g),
            Open::Block(..) => None,
        })
    }

    fn color_warning(&mut self, t: Option<SyntaxToken>, what: &str) {
        if let Some(t) = t {
            self.warning(
                t.text_range(),
                format!("{what} colors are not drawn; ignoring `{}`", t.text()),
            );
        }
    }

    fn participant(&mut self, stmt: &SyntaxNode) {
        let keywords = tokens(stmt, SEQ_KW);
        let create = keywords
            .first()
            .is_some_and(|k| k.text().eq_ignore_ascii_case("create"));
        let kind = keywords
            .iter()
            .find_map(|k| ParticipantKind::from_type(k.text()))
            .unwrap_or_default();
        let names: Vec<SyntaxToken> = significant(stmt)
            .into_iter()
            .filter(|t| matches!(t.kind(), IDENT | STRING))
            .collect();
        let Some(first) = names.first() else { return };
        // `X as Y`: the quoted one is the label; otherwise `Y` is the id and `X` the label.
        let (id, label) = match names.get(1) {
            Some(second) if second.kind() == STRING && first.kind() != STRING => {
                (unquote(first), unquote(second))
            }
            Some(second) => (unquote(second), unquote(first)),
            None => (unquote(first), unquote(first)),
        };
        self.color_warning(token(stmt, STYLE_TEXT), "participant");
        if let Some(s) = token(stmt, STEREOTYPE) {
            self.warning(
                s.text_range(),
                format!("stereotypes are not drawn; ignoring `{}`", s.text()),
            );
        }
        let existing = self.find(&id);
        if create && existing.is_some_and(|i| self.mentioned[i]) {
            self.error(
                stmt.text_range(),
                format!("`{id}` already has messages; `create` comes before its first one"),
            );
            return;
        }
        let group = self.current_group();
        let i = self.ensure(&id);
        let p = &mut self.seq.participants[i];
        if keywords
            .iter()
            .any(|k| ParticipantKind::from_type(k.text()).is_some())
            || existing.is_none()
        {
            p.kind = kind;
        }
        p.label = display(&label);
        if group.is_some() {
            p.group = group;
        }
        if let Some(n) = token(stmt, NUMBER) {
            match n.text().parse::<f64>() {
                Ok(v) => self.order[i] = Some(v as i64),
                Err(_) => self.error(n.text_range(), "expected a whole number after `order`"),
            }
        }
        if create {
            self.seq.events.push(Event::Create(i));
        }
    }

    /// Opens an activation of participant `i`; whether it is drawn (LANGS-8).
    fn activate(&mut self, i: usize, range: TextRange) -> bool {
        let drawn = self.active[i].len() < MAX_DEPTH;
        if !drawn && !std::mem::replace(&mut self.deep_activation, true) {
            self.error(range, too_deep("activations", MAX_DEPTH));
        }
        self.active[i].push(range);
        drawn
    }

    /// Closes participant `i`'s latest activation; whether a drawn one closed. Reports one that
    /// is not open.
    fn deactivate(&mut self, i: usize, range: TextRange) -> bool {
        if self.active[i].pop().is_none() {
            let id = self.seq.participants[i].id.clone();
            self.error(range, format!("`{id}` is not active"));
            return false;
        }
        // Close `i`'s latest call, then drop closed calls from the end so the last is open.
        if let Some(k) = self.calls_of[i].pop() {
            self.calls[k].2 = false;
            while self.calls.last().is_some_and(|c| !c.2) {
                self.calls.pop();
            }
        }
        self.active[i].len() < MAX_DEPTH
    }

    /// Records an activation of `callee` made by `caller`, for `return`.
    fn call(&mut self, callee: usize, caller: usize) {
        self.calls_of[callee].push(self.calls.len());
        self.calls.push((callee, caller, true));
    }

    fn number(&mut self) -> Option<u32> {
        self.numbering.map(|(n, step)| {
            self.numbering = Some((n.saturating_add(step), step));
            n
        })
    }

    fn message(&mut self, stmt: &SyntaxNode) {
        let toks = significant(stmt);
        let Some(at) = toks.iter().position(|t| t.kind() == ARROW) else {
            return;
        };
        let end = |t: &SyntaxToken, first: bool, l: &mut Self| -> usize {
            match t.kind() {
                L_BRACKET => l.edge(Edge::Left),
                R_BRACKET => l.edge(Edge::Right),
                IDENT if t.text() == "?" => l.edge(if first { Edge::Left } else { Edge::Right }),
                _ => l.ensure(&unquote(t)),
            }
        };
        let (Some(a), Some(b)) = (toks.get(at.wrapping_sub(1)), toks.get(at + 1)) else {
            return;
        };
        let a = end(a, true, self);
        let b = end(b, false, self);
        let arrow_tok = &toks[at];
        let spelled = spell(arrow_tok.text());
        if spelled.styled && !std::mem::replace(&mut self.told_style, true) {
            self.warning(
                arrow_tok.text_range(),
                "arrow styles are not drawn; drawing a plain arrow",
            );
        }
        if spelled.circle && !std::mem::replace(&mut self.told_o, true) {
            self.warning(
                arrow_tok.text_range(),
                "`o` arrow ends are drawn as filled heads",
            );
        }
        self.color_warning(token(stmt, STYLE_TEXT), "message");
        let (from, to) = if spelled.reversed { (b, a) } else { (a, b) };
        self.mentioned[from] = true;
        let modifiers = text_of(stmt, ACTIVATION).unwrap_or_default();
        let range = token(stmt, ACTIVATION).map_or(stmt.text_range(), |t| t.text_range());
        if modifiers.contains("**") {
            if self.mentioned[to] {
                let id = self.seq.participants[to].id.clone();
                self.error(
                    range,
                    format!("`{id}` already has messages; `**` creates it at its first one"),
                );
            } else {
                self.seq.events.push(Event::Create(to));
            }
        }
        self.mentioned[to] = true;
        if modifiers.contains("!!") {
            self.seq.events.push(Event::Destroy(to));
        }
        let deactivate = modifiers.contains("--") && self.deactivate(from, range);
        let opens = modifiers.contains("++") || self.autoactivate && !modifiers.contains("!!");
        let activate = opens && self.activate(to, range);
        if opens {
            self.call(to, from);
        }
        let number = self.number();
        let text = text_of(stmt, TEXT).map_or_else(Vec::new, |t| display(&t));
        self.seq.events.push(Event::Message(Message {
            from,
            to,
            arrow: spelled.arrow,
            text,
            activate,
            deactivate,
            number,
        }));
        self.last_message = Some(self.seq.events.len() - 1);
    }

    /// The text of a note or `ref`: after `:`, or its lines.
    fn note_text(stmt: &SyntaxNode) -> Text {
        tokens(stmt, TEXT)
            .iter()
            .flat_map(|t| display(t.text()))
            .collect()
    }

    fn note(&mut self, stmt: &SyntaxNode) {
        let kw = text_of(stmt, NOTE_KW)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let side = text_of(stmt, SIDE).unwrap_or_default().to_ascii_lowercase();
        let names: Vec<usize> = tokens(stmt, IDENT)
            .into_iter()
            .chain(tokens(stmt, STRING))
            .collect::<Vec<_>>()
            .iter()
            .map(|t| (t.text_range().start(), unquote(t)))
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .map(|n| self.ensure(&n))
            .collect();
        self.color_warning(token(stmt, STYLE_TEXT), "note");
        let text = Self::note_text(stmt);
        if kw == "ref" {
            self.seq.events.push(Event::Ref {
                participants: names,
                text,
            });
            return;
        }
        let (side, participants) = match side.as_str() {
            "over" => {
                let mut p = names;
                if p.len() > 2 {
                    let last = p[p.len() - 1];
                    p.truncate(1);
                    p.push(last);
                }
                (NoteSide::Over, p)
            }
            "across" => {
                self.across.push(self.seq.events.len());
                (NoteSide::Over, Vec::new())
            }
            s => {
                let side = if s == "left" {
                    NoteSide::Left
                } else {
                    NoteSide::Right
                };
                let p = if let Some(&n) = names.first() {
                    n
                } else if let Some(k) = self.last_message
                    && let Event::Message(m) = &self.seq.events[k]
                {
                    // Which end is left is known once `order` sorts the participants.
                    let from = m.from;
                    self.beside.push((self.seq.events.len(), k));
                    from
                } else {
                    self.error(
                        stmt.text_range(),
                        format!("`note {s}` without `of` goes beside the message before it"),
                    );
                    return;
                };
                (side, vec![p])
            }
        };
        self.seq.events.push(Event::Note(Note {
            side,
            participants,
            text,
        }));
    }

    fn keyword(&mut self, stmt: &SyntaxNode) {
        let kws = tokens(stmt, SEQ_KW);
        let Some(kw) = kws.first() else { return };
        let kw = kw.text().to_ascii_lowercase();
        let range = stmt.text_range();
        let label = text_of(stmt, TEXT).unwrap_or_default();
        let kind = match kw.as_str() {
            "alt" => Some(BlockKind::Alt),
            "opt" => Some(BlockKind::Opt),
            "loop" => Some(BlockKind::Loop),
            "par" => Some(BlockKind::Par),
            "break" => Some(BlockKind::Break),
            "critical" => Some(BlockKind::Critical),
            "group" => Some(BlockKind::Group),
            _ => None,
        };
        // Colors before a frame or divider label are not drawn: `alt#Gold #LightBlue text`.
        let label = if kind.is_some() || kw == "else" {
            let (rest, colors) = split_colors(&label);
            if !colors.is_empty() && self.skipped == 0 {
                self.warning(
                    range,
                    format!("frame colors are not drawn; ignoring `{colors}`"),
                );
            }
            rest
        } else {
            label
        };
        if self.skipped > 0 {
            if kind.is_some() {
                self.skipped += 1;
            } else if kw == "end" && kws.len() == 1 {
                self.skipped -= 1;
            }
            return;
        }
        if let Some(kind) = kind {
            let open = self.stack.iter().filter(|o| matches!(o, Open::Block(..)));
            if open.count() >= MAX_DEPTH {
                if !std::mem::replace(&mut self.deep_block, true) {
                    self.error(range, too_deep("frames", MAX_DEPTH));
                }
                self.skipped = 1;
                return;
            }
            self.stack.push(Open::Block(kind, range));
            self.seq.events.push(Event::Start(kind, label));
            return;
        }
        if kw.starts_with("==") {
            self.seq.events.push(Event::Separator(label));
            return;
        }
        if kw.starts_with("...") {
            let text = (!label.is_empty()).then_some(label);
            self.seq.events.push(Event::Delay(text));
            return;
        }
        if kw.starts_with("||") {
            let px = token(stmt, NUMBER)
                .map(|n| n.text().parse::<u32>().unwrap_or(MAX_SPACE).min(MAX_SPACE));
            self.seq.events.push(Event::Space(px));
            return;
        }
        if let Some(directive) = kw.strip_prefix('!') {
            if matches!(
                directive,
                "include"
                    | "includeurl"
                    | "includesub"
                    | "include_many"
                    | "include_once"
                    | "import"
            ) {
                self.error(
                    range,
                    format!("`!{directive}` reads other files and is not supported"),
                );
            } else {
                // `!$a = 1` is named by its whole first token, not an empty directive.
                let text = stmt.text().to_string();
                let shown = text.split_whitespace().next().unwrap_or("!");
                self.warning(
                    range,
                    format!("`{shown}` is not supported; ignoring the line"),
                );
            }
            return;
        }
        let name = || {
            significant(stmt)
                .into_iter()
                .find(|t| matches!(t.kind(), IDENT | STRING))
                .map(|t| unquote(&t))
        };
        match kw.as_str() {
            "activate" => {
                let Some(name) = name() else { return };
                let i = self.ensure(&name);
                self.color_warning(token(stmt, STYLE_TEXT), "activation");
                if self.activate(i, range) {
                    self.seq.events.push(Event::Activate(i));
                }
                if let Some(Event::Message(m)) = self.last_message.map(|k| &self.seq.events[k])
                    && m.to == i
                {
                    let from = m.from;
                    self.call(i, from);
                }
            }
            "deactivate" => {
                let Some(name) = name() else { return };
                let i = self.ensure(&name);
                if self.deactivate(i, range) {
                    self.seq.events.push(Event::Deactivate(i));
                }
            }
            "destroy" => {
                let Some(name) = name() else { return };
                let i = self.ensure(&name);
                // Right after a message to or from it, the lifeline ends at that message.
                match self.seq.events.last() {
                    Some(Event::Message(m)) if m.from == i || m.to == i => {
                        let at = self.seq.events.len() - 1;
                        self.seq.events.insert(at, Event::Destroy(i));
                        self.last_message = Some(at + 1);
                    }
                    _ => self.seq.events.push(Event::Destroy(i)),
                }
            }
            "return" => {
                let Some(&(callee, caller, _)) = self.calls.last() else {
                    self.error(range, "`return` needs a participant activated by a message");
                    return;
                };
                let deactivate = self.deactivate(callee, range);
                let number = self.number();
                self.seq.events.push(Event::Message(Message {
                    from: callee,
                    to: caller,
                    arrow: Arrow {
                        line: LineStyle::Dotted,
                        head: Head::Filled,
                        both: false,
                    },
                    text: display(&label),
                    activate: false,
                    deactivate,
                    number,
                }));
                self.last_message = Some(self.seq.events.len() - 1);
            }
            "autonumber" => self.autonumber(&label, range),
            "autoactivate" => match label.trim().to_ascii_lowercase().as_str() {
                "on" => self.autoactivate = true,
                "off" => self.autoactivate = false,
                _ => self.error(range, "expected `on` or `off` after `autoactivate`"),
            },
            "box" => {
                if self.stack.iter().any(|o| matches!(o, Open::Block(..))) {
                    self.error(range, "a `box` cannot be inside a frame");
                } else if self.current_group().is_some() {
                    self.error(range, "a `box` cannot be inside another `box`");
                }
                let (g, bad) = group(&label);
                if bad {
                    self.warning(range, "unknown `box` color; drawing no background");
                }
                self.seq.groups.push(g);
                self.stack.push(Open::Box(range, self.seq.groups.len() - 1));
            }
            "else" => match self.stack.last() {
                Some(Open::Block(..)) => self.seq.events.push(Event::Divider(label)),
                _ => self.error(range, "`else` belongs in a frame such as `alt`"),
            },
            "end" if kws.len() > 1 => match self.stack.last() {
                Some(Open::Box(..)) => {
                    self.stack.pop();
                }
                _ => self.error(range, "`end box` without a `box` to close"),
            },
            "end" => match self.stack.pop() {
                Some(Open::Block(..)) => self.seq.events.push(Event::End),
                Some(Open::Box(..)) => {
                    self.warning(range, "`end` closes a `box`; expected `end box`")
                }
                None => self.error(range, "`end` without a frame to close"),
            },
            "hide" | "show" if label.eq_ignore_ascii_case("footbox") => {
                self.seq.hide_footbox = kw == "hide";
            }
            "hide" | "show" => self.warning(
                range,
                format!("`{kw} {label}` is not supported; ignoring it"),
            ),
            _ => self.warning(range, format!("`{kw}` is not drawn yet; ignoring it")),
        }
    }

    /// `autonumber [start] [step] ["format"]`, `autonumber stop`, or `autonumber resume
    /// [step] ["format"]`.
    fn autonumber(&mut self, args: &str, range: TextRange) {
        let (plain, format) = match args.find('"') {
            Some(i) => (&args[..i], true),
            None => (args, false),
        };
        if format {
            self.warning(
                range,
                "`autonumber` formats are not drawn; numbering plainly",
            );
        }
        let words: Vec<&str> = plain.split_whitespace().collect();
        let bad = |l: &mut Self| {
            l.error(
                range,
                "expected `stop`, `resume`, or a start number and a step",
            );
        };
        match words.as_slice() {
            ["stop"] => self.paused = self.numbering.take().or(self.paused),
            ["resume", rest @ ..] => {
                // Numbering that is on carries on; otherwise the paused numbering resumes.
                let paused = self.paused.take();
                let (n, step) = self.numbering.or(paused).unwrap_or((1, 1));
                match rest {
                    [] => self.numbering = Some((n, step)),
                    [s] => match s.parse() {
                        Ok(step) => self.numbering = Some((n, step)),
                        Err(_) => bad(self),
                    },
                    _ => bad(self),
                }
            }
            _ if words.len() <= 2 => {
                let number =
                    |i: usize| -> Option<u32> { words.get(i).map_or(Some(1), |w| w.parse().ok()) };
                match (number(0), number(1)) {
                    (Some(start), Some(step)) => self.numbering = Some((start, step)),
                    _ => bad(self),
                }
            }
            _ => bad(self),
        }
    }

    fn finish(mut self) -> (Sequence, Vec<Diagnostic>) {
        while let Some(open) = self.stack.pop() {
            match open {
                Open::Block(kind, range) => {
                    let kw = kind.keyword();
                    self.error(range, format!("unclosed `{kw}`: expected `end`"));
                    self.seq.events.push(Event::End);
                }
                Open::Box(range, _) => self.error(range, "unclosed `box`: expected `end box`"),
            }
        }
        for i in 0..self.active.len() {
            if let Some(&range) = self.active[i].last() {
                let id = self.seq.participants[i].id.clone();
                self.warning(range, format!("`{id}` is still active at the end"));
            }
        }
        self.sort();
        // Sorted, a lower index is further left.
        for &(note, msg) in &self.beside {
            let Event::Message(m) = &self.seq.events[msg] else {
                continue;
            };
            let (lo, hi) = (m.from.min(m.to), m.from.max(m.to));
            if let Event::Note(n) = &mut self.seq.events[note] {
                n.participants = vec![if n.side == NoteSide::Left { lo } else { hi }];
            }
        }
        let shown: Vec<usize> = (0..self.seq.participants.len())
            .filter(|&i| self.seq.participants[i].kind != ParticipantKind::Hidden)
            .collect();
        for &k in &self.across {
            if let Event::Note(n) = &mut self.seq.events[k] {
                n.participants = match (shown.first(), shown.last()) {
                    (Some(&a), Some(&b)) if a != b => vec![a, b],
                    (Some(&a), _) => vec![a],
                    _ => Vec::new(),
                };
            }
        }
        // A note across no participants has nowhere to go.
        self.seq
            .events
            .retain(|e| !matches!(e, Event::Note(n) if n.participants.is_empty()));
        (self.seq, self.diagnostics)
    }

    /// Puts the left edge first, the right edge last, and the rest in `order` (PSEQ-2):
    /// stable, with a participant without one after the ordered one before it.
    fn sort(&mut self) {
        let n = self.seq.participants.len();
        let mut last = i64::MIN;
        let mut keys = Vec::with_capacity(n);
        for i in 0..n {
            let (class, _) = self.rank(i);
            let key = match self.order[i] {
                Some(v) if class == 1 => {
                    last = v;
                    v
                }
                _ => last,
            };
            keys.push((class, key, i));
        }
        keys.sort();
        let old: Vec<usize> = keys.iter().map(|&(_, _, i)| i).collect();
        if old.iter().enumerate().all(|(new, &o)| new == o) {
            return;
        }
        let mut new_of = vec![0; n];
        for (new, &o) in old.iter().enumerate() {
            new_of[o] = new;
        }
        let mut parts: Vec<Option<Participant>> = std::mem::take(&mut self.seq.participants)
            .into_iter()
            .map(Some)
            .collect();
        self.seq.participants = old
            .iter()
            .map(|&o| parts[o].take().expect("each once"))
            .collect();
        for i in self.index.values_mut() {
            *i = new_of[*i];
        }
        let m = |p: &mut usize| *p = new_of[*p];
        for e in &mut self.seq.events {
            match e {
                Event::Message(msg) => {
                    m(&mut msg.from);
                    m(&mut msg.to);
                }
                Event::Note(note) => note.participants.iter_mut().for_each(m),
                Event::Ref { participants, .. } => participants.iter_mut().for_each(m),
                Event::Activate(p)
                | Event::Deactivate(p)
                | Event::Create(p)
                | Event::Destroy(p) => m(p),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows() {
        let a = |s: &str| {
            let sp = spell(s);
            (sp.arrow.line, sp.arrow.head, sp.arrow.both, sp.reversed)
        };
        use Head::*;
        use LineStyle::*;
        assert_eq!(a("->"), (Solid, Filled, false, false));
        assert_eq!(a("-->"), (Dotted, Filled, false, false));
        assert_eq!(a("->>"), (Solid, Open, false, false));
        assert_eq!(a("-\\"), (Solid, Open, false, false));
        assert_eq!(a("-//"), (Solid, Open, false, false));
        assert_eq!(a("->x"), (Solid, Cross, false, false));
        assert_eq!(a("-x"), (Solid, Cross, false, false));
        assert_eq!(a("<-"), (Solid, Filled, false, true));
        assert_eq!(a("<<--"), (Dotted, Open, false, true));
        assert_eq!(a("x<-"), (Solid, Cross, false, true));
        assert_eq!(a("<->"), (Solid, Filled, true, false));
        assert_eq!(a("-[#red]>"), (Solid, Filled, false, false));
        // The dashes on both sides of the style count, as in PlantUML.
        assert_eq!(a("-[#red]->"), (Dotted, Filled, false, false));
        assert_eq!(a("-[#0000FF]->"), (Dotted, Filled, false, false));
        assert_eq!(a("<-[#red]-"), (Dotted, Filled, false, true));
        assert_eq!(a("--[#red]>"), (Dotted, Filled, false, false));
        assert_eq!(a("x->"), (Solid, Filled, false, false));
        assert!(spell("-[#red]>").styled);
        assert!(spell("->o").circle);
        assert!(spell("o->").circle);
        assert_eq!(a("o->"), (Solid, Filled, false, false));
    }

    #[test]
    fn boxes() {
        let (g, bad) = group("\"Internal\" #LightBlue");
        assert_eq!(g.title.as_deref(), Some("Internal"));
        assert_eq!(g.color.as_deref(), Some("LightBlue"));
        assert!(!bad);
        let (g, _) = group("Team #ccc");
        assert_eq!(g.title.as_deref(), Some("Team"));
        assert_eq!(g.color.as_deref(), Some("#ccc"));
        let (g, bad) = group("#nocolor");
        assert_eq!(g, Group::default());
        assert!(bad);
    }

    fn lowered(src: &str) -> (Sequence, Vec<Diagnostic>) {
        lower(&crate::plantuml::parse(src).syntax())
    }

    fn id(seq: &Sequence, i: usize) -> &str {
        &seq.participants[i].id
    }

    fn notes(seq: &Sequence) -> Vec<(NoteSide, Vec<&str>)> {
        seq.events
            .iter()
            .filter_map(|e| match e {
                Event::Note(n) => {
                    Some((n.side, n.participants.iter().map(|&p| id(seq, p)).collect()))
                }
                _ => None,
            })
            .collect()
    }

    fn numbers(seq: &Sequence) -> Vec<Option<u32>> {
        seq.events
            .iter()
            .filter_map(|e| match e {
                Event::Message(m) => Some(m.number),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn note_beside_message_follows_order() {
        let (seq, d) = lowered(
            "@startuml\nparticipant A order 20\nparticipant B order 10\nA -> B : hi\n\
             note left : l\nnote right : r\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(id(&seq, 0), "B");
        assert_eq!(
            notes(&seq),
            vec![(NoteSide::Left, vec!["B"]), (NoteSide::Right, vec!["A"])]
        );
    }

    #[test]
    fn note_beside_message_with_edges() {
        let (seq, _) = lowered("@startuml\nA -> ] : out\nnote right : r\nnote left : l\n@enduml\n");
        assert_eq!(
            notes(&seq),
            vec![(NoteSide::Right, vec!["]"]), (NoteSide::Left, vec!["A"])]
        );
    }

    #[test]
    fn autonumber_resume_keeps_numbering() {
        let (seq, d) =
            lowered("@startuml\nautonumber 5\nA -> B\nautonumber resume\nA -> B\n@enduml\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(numbers(&seq), vec![Some(5), Some(6)]);
        let (seq, _) = lowered(
            "@startuml\nautonumber 5 10\nA -> B\nautonumber resume 2\nA -> B\nA -> B\n@enduml\n",
        );
        assert_eq!(numbers(&seq), vec![Some(5), Some(15), Some(17)]);
        let (seq, _) = lowered(
            "@startuml\nautonumber\nA -> B\nautonumber stop\nA -> B\nautonumber resume\nA -> B\n@enduml\n",
        );
        assert_eq!(numbers(&seq), vec![Some(1), None, Some(2)]);
        let (seq, _) = lowered("@startuml\nautonumber resume\nA -> B\n@enduml\n");
        assert_eq!(numbers(&seq), vec![Some(1)]);
    }

    #[test]
    fn plain_end_closes_box_with_warning() {
        let (seq, d) =
            lowered("@startuml\nbox \"T\"\nparticipant A\nend\nparticipant B\n@enduml\n");
        assert_eq!(seq.participants[0].group, Some(0));
        assert_eq!(seq.participants[1].group, None);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].severity, crate::diagnostic::Severity::Warning);
        assert!(d[0].message.contains("`end box`"), "{}", d[0].message);
    }

    #[test]
    fn participant_in_frame_in_box_is_grouped() {
        let (seq, d) = lowered(
            "@startuml\nA -> B\nbox \"T\"\nparticipant C\nalt x\nA -> C\nparticipant D\nend\n\
             participant E\nend box\nbox \"U\"\ngroup g\nparticipant F\nend\nend box\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let group = |name: &str| {
            seq.participants
                .iter()
                .find(|p| p.id == name)
                .and_then(|p| p.group)
        };
        assert_eq!(group("B"), None);
        assert_eq!(group("C"), Some(0));
        assert_eq!(group("D"), Some(0));
        assert_eq!(group("E"), Some(0));
        assert_eq!(group("F"), Some(1));
    }

    #[test]
    fn index_tracks_participants_through_order() {
        let (seq, _) = lowered(
            "@startuml\nparticipant A order 3\nparticipant B order 1\nparticipant C order 2\n\
             A -> B\nB -> C\n@enduml\n",
        );
        let ids: Vec<&str> = seq.participants.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["B", "C", "A"]);
        let ends: Vec<(&str, &str)> = seq
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Message(m) => Some((id(&seq, m.from), id(&seq, m.to))),
                _ => None,
            })
            .collect();
        assert_eq!(ends, [("A", "B"), ("B", "C")]);
    }

    #[test]
    fn many_participants_lower_quickly() {
        // 100k participants, each named twice: linear lookup made this quadratic.
        let mut src = String::from("@startuml\n");
        for i in 0..100_000 {
            src.push_str(&format!("p{i} -> p{}\n", i + 1));
        }
        src.push_str("@enduml\n");
        // Time only the lowering, not the parse.
        let tree = crate::plantuml::parse(&src).syntax();
        let t = std::time::Instant::now();
        let (seq, d) = lower(&tree);
        assert!(d.is_empty());
        assert_eq!(seq.participants.len(), 100_001);
        assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    }

    #[test]
    fn deactivate_closes_latest_call() {
        // `return` answers the latest open call after an inner one is deactivated.
        let (seq, d) = lowered(
            "@startuml\nA -> B ++\nB -> C ++\nC -> D ++\ndeactivate C\nreturn r1\nreturn r2\n\
             @enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let returns: Vec<(&str, &str)> = seq
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Message(m) if m.text == ["r1"] || m.text == ["r2"] => {
                    Some((id(&seq, m.from), id(&seq, m.to)))
                }
                _ => None,
            })
            .collect();
        assert_eq!(returns, [("D", "C"), ("B", "A")]);
    }

    fn errors(d: &[Diagnostic]) -> Vec<&str> {
        d.iter()
            .filter(|d| d.severity == crate::diagnostic::Severity::Error)
            .map(|d| d.message.as_str())
            .collect()
    }

    #[test]
    fn note_beside_survives_destroy_insert() {
        // `destroy` inserts before the message, shifting it by one.
        let (seq, d) = lowered(
            "@startuml\nparticipant A order 2\nparticipant B order 1\nA -> B\nnote left : l0\n\
             B -> A\ndestroy A\nnote right : r\nnote left : l\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            notes(&seq),
            vec![
                (NoteSide::Left, vec!["B"]),
                (NoteSide::Right, vec!["A"]),
                (NoteSide::Left, vec!["B"]),
            ]
        );
        assert!(
            matches!(seq.events[2], Event::Destroy(_)),
            "{:?}",
            seq.events
        );
    }

    #[test]
    fn note_beside_after_destroy_and_create() {
        let (seq, d) = lowered(
            "@startuml\nparticipant A order 2\nparticipant B order 1\nA -> B\ndestroy B\n\
             note right : r\ncreate C\nC -> A\nnote left : l\nnote right : r2\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let ns = notes(&seq);
        assert_eq!(ns[0], (NoteSide::Right, vec!["A"]));
        // C has no order, so it follows B (the ordered one before it): B, C, A.
        assert_eq!(ns[1], (NoteSide::Left, vec!["C"]));
        assert_eq!(ns[2], (NoteSide::Right, vec!["A"]));
    }

    #[test]
    fn note_beside_with_across_notes() {
        let (seq, d) = lowered(
            "@startuml\nparticipant B order 1\nparticipant A order 2\nA -> B\nnote across : x\n\
             note left : l\nnote across : y\nnote right : r\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            notes(&seq),
            vec![
                (NoteSide::Over, vec!["B", "A"]),
                (NoteSide::Left, vec!["B"]),
                (NoteSide::Over, vec!["B", "A"]),
                (NoteSide::Right, vec!["A"]),
            ]
        );
    }

    #[test]
    fn note_beside_without_message_errors() {
        let (seq, d) = lowered("@startuml\nA -> B\nnote left : ok\n@enduml\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(notes(&seq).len(), 1);
        let (seq, d) = lowered("@startuml\nparticipant A\nnote right : x\n@enduml\n");
        assert_eq!(errors(&d).len(), 1, "{d:?}");
        assert!(notes(&seq).is_empty());
    }

    #[test]
    fn quoted_edge_names_stay_distinct() {
        let (seq, d) = lowered(
            "@startuml\nparticipant \"[\"\nparticipant \"]\"\n[-> A : in\nA -> ] : out\n\
             \"[\" -> \"]\" : q\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        let hidden = |i: usize| seq.participants[i].kind == ParticipantKind::Hidden;
        let ids: Vec<&str> = seq.participants.iter().map(|p| p.id.as_str()).collect();
        // Hidden left edge first, hidden right edge last.
        assert!(hidden(0) && hidden(ids.len() - 1), "{ids:?}");
        let ends: Vec<(usize, usize)> = seq
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Message(m) => Some((m.from, m.to)),
                _ => None,
            })
            .collect();
        assert!(hidden(ends[0].0) && !hidden(ends[0].1));
        assert!(!hidden(ends[1].0) && hidden(ends[1].1));
        // The quoted names are real, drawn participants.
        assert!(!hidden(ends[2].0) && !hidden(ends[2].1), "{ends:?}");
        assert_ne!(ends[2].0, ends[0].0);
    }

    #[test]
    fn activation_depth_is_capped() {
        let n = MAX_DEPTH + 1;
        let mut src = String::from("@startuml\n");
        for _ in 0..n {
            src.push_str("A -> B ++\n");
        }
        for _ in 0..n {
            src.push_str("deactivate B\n");
        }
        src.push_str("return r\n@enduml\n");
        let (seq, d) = lowered(&src);
        // One error for the overflow, one for the `return` with no open call.
        let errs = errors(&d);
        assert_eq!(errs.len(), 2, "{d:?}");
        assert!(errs[0].contains("activations nested"), "{errs:?}");
        assert!(errs[1].contains("`return`"), "{errs:?}");
        let msgs: Vec<&Message> = seq
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs.iter().filter(|m| m.activate).count(), MAX_DEPTH);
        // The overflowing call is the last one and is not drawn.
        assert!(!msgs[MAX_DEPTH].activate);
        let drawn = seq
            .events
            .iter()
            .filter(|e| matches!(e, Event::Deactivate(_)))
            .count();
        assert_eq!(drawn, MAX_DEPTH);
    }

    #[test]
    fn scale_and_skin_are_ignored() {
        let (seq, d) = lowered("@startuml\nscale 2\nskin rose\nscale -> B\n@enduml\n");
        assert!(errors(&d).is_empty(), "{d:?}");
        assert_eq!(d.len(), 2, "{d:?}");
        assert_eq!(numbers(&seq).len(), 1, "`scale -> B` is a message");
    }

    #[test]
    fn return_after_all_calls_closed_errors() {
        let (seq, d) = lowered("@startuml\nA -> B ++\ndeactivate B\nreturn r\n@enduml\n");
        assert_eq!(errors(&d).len(), 1, "{d:?}");
        assert!(errors(&d)[0].contains("`return`"));
        assert_eq!(numbers(&seq).len(), 1, "no return message");
    }

    #[test]
    fn end_with_frame_open_inside_box() {
        // `end` closes the frame, a second plain `end` warns and closes the box.
        let (seq, d) = lowered(
            "@startuml\nbox \"T\"\nalt x\nparticipant A\nend\nend\nparticipant B\n@enduml\n",
        );
        assert!(errors(&d).is_empty(), "{d:?}");
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].message.contains("`end box`"));
        assert_eq!(seq.participants[0].group, Some(0));
        assert_eq!(seq.participants[1].group, None);
    }

    #[test]
    fn end_box_with_frame_open_errors() {
        let (seq, d) =
            lowered("@startuml\nbox \"T\"\nalt x\nA -> B\nend box\nend\nparticipant C\n@enduml\n");
        let errs = errors(&d);
        assert!(errs.iter().any(|m| m.contains("`end box`")), "{d:?}");
        // The box was never closed, so it is reported at the end.
        assert!(errs.iter().any(|m| m.contains("unclosed `box`")), "{d:?}");
        assert_eq!(seq.participants.last().unwrap().group, Some(0));
        let ends = seq
            .events
            .iter()
            .filter(|e| matches!(e, Event::End))
            .count();
        assert_eq!(ends, 1);
    }

    #[test]
    fn autonumber_stop_twice_then_resume() {
        let (seq, d) = lowered(
            "@startuml\nautonumber 3\nA -> B\nautonumber stop\nautonumber stop\nA -> B\n\
             autonumber resume\nA -> B\nautonumber resume\nA -> B\n@enduml\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(numbers(&seq), vec![Some(3), None, Some(4), Some(5)]);
    }

    #[test]
    fn many_open_calls_deactivate_quickly() {
        // Each `deactivate` used to scan every open call: quadratic.
        let n = 40_000;
        let mut src = String::from("@startuml\n");
        for _ in 0..n {
            src.push_str("A -> B ++\n");
        }
        for _ in 0..n {
            src.push_str("A -> A ++\n");
        }
        for _ in 0..n {
            src.push_str("deactivate B\n");
        }
        src.push_str("return r\n@enduml\n");
        let tree = crate::plantuml::parse(&src).syntax();
        let t = std::time::Instant::now();
        let (seq, _) = lower(&tree);
        assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
        let Some(Event::Message(m)) = seq
            .events
            .iter()
            .rev()
            .find(|e| matches!(e, Event::Message(_)))
        else {
            panic!("no return");
        };
        assert_eq!((id(&seq, m.from), id(&seq, m.to)), ("A", "A"));
    }
}
