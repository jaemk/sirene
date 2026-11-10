//! Collects a sequence diagram's participants and events (MSEQ-2).

use rowan::TextRange;

use super::{
    Arrow, BlockKind, Event, Group, Message, Note, NoteSide, Participant, ParticipantKind,
    Sequence, Text,
};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::scene::parse_color;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Sequence, Vec<Diagnostic>) {
    let mut l = Lower::default();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            l.title = text::frontmatter_title(tok.text());
        }
    }
    for stmt in root.children() {
        // A statement the parser could not read is reported there and skipped here.
        if stmt.descendants().any(|n| n.kind() == ERROR) {
            continue;
        }
        match stmt.kind() {
            PARTICIPANT_STMT => l.participant(&stmt),
            MESSAGE_STMT => l.message(&stmt),
            NOTE_STMT => l.note(&stmt),
            SEQ_STMT => l.keyword(&stmt),
            TITLE_STMT => l.title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            ACC_TITLE_STMT => l.seq.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => l.seq.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    l.finish()
}

/// A `box` or block waiting for its `end`.
enum Open {
    Group(TextRange),
    Block(BlockKind, TextRange),
}

#[derive(Default)]
struct Lower {
    seq: Sequence,
    diagnostics: Vec<Diagnostic>,
    title: Option<String>,
    stack: Vec<Open>,
    /// Where each participant's open activations began.
    active: Vec<Vec<TextRange>>,
    /// The next `autonumber` number and the step, while numbering is on.
    numbering: Option<(u32, u32)>,
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn tokens(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == kind)
        .collect()
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

/// Display text: entity codes decoded and split at `<br>`.
fn display(s: &str) -> Text {
    crate::lines::split_br(&text::decode_entities(s))
}

/// The color and title of `box TEXT`: a leading CSS color, if any, then the title.
fn group(s: &str) -> Group {
    let s = s.trim();
    let color_len = if s.len() >= 3 && s[..3].eq_ignore_ascii_case("rgb") {
        s.find(')').map_or(0, |i| i + 1)
    } else {
        let word = s.find(char::is_whitespace).unwrap_or(s.len());
        if parse_color(&s[..word]).is_some() {
            word
        } else {
            0
        }
    };
    let title = s[color_len..].trim();
    Group {
        color: (color_len > 0).then(|| s[..color_len].to_owned()),
        title: (!title.is_empty()).then(|| title.to_owned()),
    }
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

    /// The participant named `id`, added on first mention.
    fn ensure(&mut self, id: &str) -> usize {
        if let Some(i) = self.find(id) {
            return i;
        }
        self.seq.participants.push(Participant {
            id: id.to_owned(),
            label: display(id),
            kind: ParticipantKind::Participant,
            group: None,
        });
        self.active.push(Vec::new());
        self.seq.participants.len() - 1
    }

    fn current_group(&self) -> Option<usize> {
        match self.stack.last() {
            Some(Open::Group(_)) => Some(self.seq.groups.len() - 1),
            _ => None,
        }
    }

    fn participant(&mut self, stmt: &SyntaxNode) {
        let keywords = tokens(stmt, SEQ_KW);
        let create = keywords
            .first()
            .is_some_and(|k| k.text().eq_ignore_ascii_case("create"));
        let actor = keywords
            .last()
            .is_some_and(|k| k.text().eq_ignore_ascii_case("actor"));
        let Some(id) = text_of(stmt, IDENT) else {
            return;
        };
        let mut kind = if actor {
            ParticipantKind::Actor
        } else {
            ParticipantKind::Participant
        };
        let mut label = text_of(stmt, TEXT).filter(|s| !s.is_empty());
        if let Some(body) = token(stmt, META_BODY) {
            let (entries, _) = text::metadata(body.text());
            for (key, value) in entries {
                match key.trim_matches('"') {
                    "type" => match ParticipantKind::from_type(&value) {
                        Some(k) => kind = k,
                        None => self.warning(
                            body.text_range(),
                            format!("unknown participant type `{value}`; drawing a box"),
                        ),
                    },
                    "alias" if label.is_none() => label = Some(value),
                    _ => {}
                }
            }
        }
        if create && self.find(&id).is_some() {
            self.error(
                stmt.text_range(),
                format!("`{id}` already exists; `create` comes before its first mention"),
            );
            return;
        }
        let group = self.current_group();
        let i = self.ensure(&id);
        let p = &mut self.seq.participants[i];
        p.kind = kind;
        if let Some(label) = label {
            p.label = display(&label);
        }
        if group.is_some() {
            p.group = group;
        }
        if create {
            self.seq.events.push(Event::Create(i));
        }
    }

    fn message(&mut self, stmt: &SyntaxNode) {
        let ids = tokens(stmt, IDENT);
        let (Some(from), Some(to), Some(arrow)) = (
            ids.first(),
            ids.get(1),
            token(stmt, ARROW).and_then(|a| Arrow::parse(a.text())),
        ) else {
            return;
        };
        let from = self.ensure(from.text().trim());
        let to = self.ensure(to.text().trim());
        let activation = token(stmt, ACTIVATION);
        let activate = activation.as_ref().is_some_and(|a| a.text() == "+");
        let mut deactivate = activation.as_ref().is_some_and(|a| a.text() == "-");
        if deactivate && self.active[from].pop().is_none() {
            let id = self.seq.participants[from].id.clone();
            let range = activation.map_or(stmt.text_range(), |a| a.text_range());
            self.error(range, format!("`{id}` is not active"));
            deactivate = false;
        }
        if activate {
            self.active[to].push(stmt.text_range());
        }
        let number = self.numbering.map(|(n, step)| {
            self.numbering = Some((n.saturating_add(step), step));
            n
        });
        let text = text_of(stmt, TEXT).map_or_else(Vec::new, |t| display(&t));
        self.seq.events.push(Event::Message(Message {
            from,
            to,
            arrow,
            text,
            activate,
            deactivate,
            number,
        }));
    }

    fn note(&mut self, stmt: &SyntaxNode) {
        let side = match token(stmt, SIDE).map(|s| s.text().to_ascii_lowercase()) {
            Some(s) if s == "left" => NoteSide::Left,
            Some(s) if s == "right" => NoteSide::Right,
            _ => NoteSide::Over,
        };
        let mut names = tokens(stmt, IDENT);
        let most = if side == NoteSide::Over { 2 } else { 1 };
        if names.len() > most {
            let message = if most == 1 {
                "a note beside a participant names one; use `note over A,B` for two"
            } else {
                "a note goes over one or two participants"
            };
            self.error(names[most].text_range(), message);
            names.truncate(most);
        }
        let participants = names.iter().map(|n| self.ensure(n.text())).collect();
        let text = text_of(stmt, TEXT).map_or_else(Vec::new, |t| display(&t));
        self.seq.events.push(Event::Note(Note {
            side,
            participants,
            text,
        }));
    }

    fn keyword(&mut self, stmt: &SyntaxNode) {
        let Some(kw) = token(stmt, SEQ_KW) else {
            return;
        };
        let kw = kw.text().to_ascii_lowercase();
        let range = stmt.text_range();
        let label = text_of(stmt, TEXT).unwrap_or_default();
        let name = text_of(stmt, IDENT);
        let kind = match kw.as_str() {
            "loop" => Some(BlockKind::Loop),
            "alt" => Some(BlockKind::Alt),
            "opt" => Some(BlockKind::Opt),
            "par" | "par_over" => Some(BlockKind::Par),
            "critical" => Some(BlockKind::Critical),
            "break" => Some(BlockKind::Break),
            "rect" => Some(BlockKind::Rect),
            _ => None,
        };
        if let Some(kind) = kind {
            self.stack.push(Open::Block(kind, range));
            self.seq.events.push(Event::Start(kind, label));
            return;
        }
        match kw.as_str() {
            "activate" => {
                let Some(name) = name else { return };
                let i = self.ensure(&name);
                self.active[i].push(range);
                self.seq.events.push(Event::Activate(i));
            }
            "deactivate" => {
                let Some(name) = name else { return };
                let i = self.ensure(&name);
                if self.active[i].pop().is_none() {
                    self.error(range, format!("`{name}` is not active"));
                } else {
                    self.seq.events.push(Event::Deactivate(i));
                }
            }
            "destroy" => {
                let Some(name) = name else { return };
                let i = self.ensure(&name);
                self.seq.events.push(Event::Destroy(i));
            }
            "autonumber" => self.autonumber(&label, range),
            "box" => {
                if self.stack.iter().any(|o| matches!(o, Open::Block(..))) {
                    self.error(range, "a `box` cannot be inside a block");
                } else if self.current_group().is_some() {
                    self.error(range, "a `box` cannot be inside another `box`");
                }
                self.seq.groups.push(group(&label));
                self.stack.push(Open::Group(range));
            }
            "else" | "and" | "option" => {
                let owner = match kw.as_str() {
                    "else" => BlockKind::Alt,
                    "and" => BlockKind::Par,
                    _ => BlockKind::Critical,
                };
                match self.stack.last() {
                    Some(Open::Block(k, _)) if *k == owner => {
                        self.seq.events.push(Event::Divider(label));
                    }
                    _ => self.error(
                        range,
                        format!("`{kw}` belongs in an `{}` block", owner.keyword()),
                    ),
                }
            }
            "end" => match self.stack.pop() {
                Some(Open::Block(..)) => self.seq.events.push(Event::End),
                Some(Open::Group(_)) => {}
                None => self.error(range, "`end` without a block or `box` to close"),
            },
            _ => self.warning(
                range,
                format!("`{kw}` is not supported in sequence diagrams; ignoring it"),
            ),
        }
    }

    /// `autonumber`, `autonumber off`, `autonumber START`, or `autonumber START STEP`.
    fn autonumber(&mut self, args: &str, range: TextRange) {
        let words: Vec<&str> = args.split_whitespace().collect();
        let number = |i: usize, default: u32| -> Option<u32> {
            words.get(i).map_or(Some(default), |w| w.parse().ok())
        };
        match words.as_slice() {
            ["off"] => self.numbering = None,
            _ if words.len() <= 2 => match (number(0, 1), number(1, 1)) {
                (Some(start), Some(step)) => self.numbering = Some((start, step)),
                _ => self.error(range, "expected `off`, or a start number and a step"),
            },
            _ => self.error(range, "expected `off`, or a start number and a step"),
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
                Open::Group(range) => self.error(range, "unclosed `box`: expected `end`"),
            }
        }
        for i in 0..self.active.len() {
            if let Some(&range) = self.active[i].last() {
                let id = self.seq.participants[i].id.clone();
                self.warning(range, format!("`{id}` is still active at the end"));
            }
        }
        self.seq.title = self.title;
        (self.seq, self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups() {
        let g = group("Aqua Alice & John");
        assert_eq!(g.color.as_deref(), Some("Aqua"));
        assert_eq!(g.title.as_deref(), Some("Alice & John"));
        let g = group("rgb(33, 66, 99) Team");
        assert_eq!(g.color.as_deref(), Some("rgb(33, 66, 99)"));
        assert_eq!(g.title.as_deref(), Some("Team"));
        let g = group("Backend services");
        assert_eq!(g.color, None);
        assert_eq!(g.title.as_deref(), Some("Backend services"));
        assert_eq!(group(""), Group::default());
    }
}
