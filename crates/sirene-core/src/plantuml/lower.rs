//! Collects a PlantUML sequence diagram's participants and events (PSEQ-2 to PSEQ-6).

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

/// A `box` or frame waiting for its `end`.
enum Open {
    Box(TextRange),
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
    /// Where each participant's open activations began.
    active: Vec<Vec<TextRange>>,
    /// Each open activation made by a message or after one: the participant and its caller,
    /// for `return`.
    calls: Vec<(usize, usize)>,
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
    /// Warnings given once.
    told_o: bool,
    told_style: bool,
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
    // `-[style]>` and `-[style]->`: the dashes on either side of the style, which is dropped.
    let (s, dashes, styled) = match (s.find('['), s.rfind(']')) {
        (Some(a), Some(b)) if a < b => {
            let before = s[..a].matches('-').count();
            let after = s[b + 1..].matches('-').count();
            (
                format!("{}{}", &s[..a], &s[b + 1..]),
                before.max(after),
                true,
            )
        }
        _ => (s.to_owned(), s.matches('-').count(), false),
    };
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

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.seq.participants.iter().position(|p| p.id == id)
    }

    fn push_participant(&mut self, id: &str, label: Text, kind: ParticipantKind) -> usize {
        self.seq.participants.push(Participant {
            id: id.to_owned(),
            label,
            kind,
            group: None,
        });
        self.active.push(Vec::new());
        self.order.push(None);
        self.mentioned.push(false);
        self.seq.participants.len() - 1
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

    fn current_group(&self) -> Option<usize> {
        match self.stack.last() {
            Some(Open::Box(_)) => Some(self.seq.groups.len() - 1),
            _ => None,
        }
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
        if let Some(k) = self.calls.iter().rposition(|&(p, _)| p == i) {
            self.calls.remove(k);
        }
        self.active[i].len() < MAX_DEPTH
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
        let activate = modifiers.contains("++") && self.activate(to, range);
        if modifiers.contains("++") {
            self.calls.push((to, from));
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
                } else if let Some(Event::Message(m)) =
                    self.last_message.map(|k| &self.seq.events[k])
                {
                    let (lo, hi) = if self.rank(m.from) <= self.rank(m.to) {
                        (m.from, m.to)
                    } else {
                        (m.to, m.from)
                    };
                    if side == NoteSide::Left { lo } else { hi }
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
                "include" | "includeurl" | "includesub" | "import"
            ) {
                self.error(
                    range,
                    format!("`!{directive}` reads other files and is not supported"),
                );
            } else {
                self.warning(
                    range,
                    format!("`!{directive}` is not supported; ignoring the line"),
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
                    self.calls.push((i, from));
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
                let Some(&(callee, caller)) = self.calls.last() else {
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
                self.stack.push(Open::Box(range));
            }
            "else" => match self.stack.last() {
                Some(Open::Block(..)) => self.seq.events.push(Event::Divider(label)),
                _ => self.error(range, "`else` belongs in a frame such as `alt`"),
            },
            "end" if kws.len() > 1 => match self.stack.last() {
                Some(Open::Box(_)) => {
                    self.stack.pop();
                }
                _ => self.error(range, "`end box` without a `box` to close"),
            },
            "end" => match self.stack.pop() {
                Some(Open::Block(..)) => self.seq.events.push(Event::End),
                Some(Open::Box(_)) => {}
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
                let (n, step) = self.paused.take().unwrap_or((1, 1));
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
                Open::Box(range) => self.error(range, "unclosed `box`: expected `end box`"),
            }
        }
        for i in 0..self.active.len() {
            if let Some(&range) = self.active[i].last() {
                let id = self.seq.participants[i].id.clone();
                self.warning(range, format!("`{id}` is still active at the end"));
            }
        }
        self.sort();
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
        assert_eq!(a("-[#red]->"), (Solid, Filled, false, false));
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
}
