//! Lowers a ZenUML tree to the sequence diagram model (MZEN-2): sync calls activate their
//! target for their body and reply to their caller; calls with no caller come from the starter.

use std::collections::{HashMap, HashSet};

use rowan::{TextRange, TextSize};

use super::parser::unquote;
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::lines::split_br;
use crate::sequence::{
    Arrow, BlockKind, Event, Head, LineStyle, Message, Participant, ParticipantKind, Sequence,
};
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// `errors` are where the parser reported errors; a statement with one on its own line is
/// skipped.
pub(crate) fn lower(root: &SyntaxNode, mut errors: Vec<TextSize>) -> (Sequence, Vec<Diagnostic>) {
    errors.sort();
    let mut l = Lower {
        errors,
        ..Lower::default()
    };
    // The implicit starter, id "": dropped at the end if nothing calls from it.
    l.ids.insert(String::new(), 0);
    l.seq.participants.push(Participant {
        id: String::new(),
        label: Vec::new(),
        kind: ParticipantKind::Hidden,
        group: None,
    });
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            l.seq.title = text::frontmatter_title(tok.text());
        }
    }
    l.statements(root);
    if let Some(r) = l.reply.take() {
        l.unused_reply(r);
    }
    l.finish()
}

const SYNC: Arrow = Arrow {
    line: LineStyle::Solid,
    head: Head::Filled,
    both: false,
};
const ASYNC: Arrow = Arrow {
    line: LineStyle::Solid,
    head: Head::Open,
    both: false,
};
const REPLY: Arrow = Arrow {
    line: LineStyle::Dotted,
    head: Head::Open,
    both: false,
};

/// A call whose body is being lowered.
struct Frame {
    caller: usize,
    target: usize,
    returned: bool,
}

#[derive(Default)]
struct Lower {
    seq: Sequence,
    diagnostics: Vec<Diagnostic>,
    starter: Option<usize>,
    frames: Vec<Frame>,
    /// A pending `@return`, for the next async message.
    reply: Option<TextRange>,
    /// The parser's error offsets, in order.
    errors: Vec<TextSize>,
    /// Each participant's index by id.
    ids: HashMap<String, usize>,
    /// The participants a message or create event has named so far.
    used: HashSet<usize>,
}

fn tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .collect()
}

fn is_name(t: &SyntaxToken) -> bool {
    matches!(t.kind(), IDENT | STRING)
}

fn name(t: &SyntaxToken) -> String {
    unquote(t.kind(), t.text()).trim().to_owned()
}

fn display(s: &str) -> Vec<String> {
    split_br(&text::decode_entities(s))
}

/// The text between a statement's parentheses, if it has them.
fn args(node: &SyntaxNode) -> Option<String> {
    let ts = tokens(node);
    let open = ts.iter().position(|t| t.kind() == L_PAREN)?;
    Some(
        ts[open + 1..]
            .iter()
            .take_while(|t| t.kind() != R_PAREN)
            .map(|t| t.text())
            .collect::<String>()
            .trim()
            .to_owned(),
    )
}

impl Lower {
    /// Whether the statement's own line, up to any `{`, holds a syntax error.
    fn head_error(&self, node: &SyntaxNode) -> bool {
        let start = node.text_range().start();
        let end = node
            .children_with_tokens()
            .find(|e| e.kind() == L_BRACE)
            .map_or(node.text_range().end(), |e| e.text_range().start());
        let i = self.errors.partition_point(|&e| e < start);
        self.errors.get(i).is_some_and(|&e| e <= end)
    }

    /// Adds `e` to the events, noting the participants it names.
    fn push(&mut self, e: Event) {
        match &e {
            Event::Message(m) => {
                self.used.insert(m.from);
                self.used.insert(m.to);
            }
            Event::Create(p) => {
                self.used.insert(*p);
            }
            _ => {}
        }
        self.seq.events.push(e);
    }

    fn warning(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(range, message));
    }

    fn ensure(&mut self, id: &str) -> usize {
        if let Some(&i) = self.ids.get(id) {
            return i;
        }
        self.ids.insert(id.to_owned(), self.seq.participants.len());
        self.seq.participants.push(Participant {
            id: id.to_owned(),
            label: display(id),
            kind: ParticipantKind::Participant,
            group: None,
        });
        self.seq.participants.len() - 1
    }

    /// Who the next call comes from: the target of the call being lowered, or the starter.
    fn current(&self) -> usize {
        self.frames
            .last()
            .map_or(self.starter.unwrap_or(0), |f| f.target)
    }

    fn message(&mut self, from: usize, to: usize, arrow: Arrow, text: &str) -> Message {
        Message {
            from,
            to,
            arrow,
            text: display(text),
            activate: false,
            deactivate: false,
            number: None,
        }
    }

    fn unused_reply(&mut self, r: TextRange) {
        self.warning(
            r,
            "`@return` is for the next message such as `A->B: x`; ignoring it",
        );
    }

    fn statements(&mut self, node: &SyntaxNode) {
        for stmt in node.children() {
            // A fragment checks each branch.
            let checked = stmt.kind() != FRAGMENT;
            if stmt.kind() == ERROR || stmt.kind() == LABEL || checked && self.head_error(&stmt) {
                continue;
            }
            if stmt.kind() != MESSAGE_STMT
                && stmt.kind() != TITLE_STMT
                && let Some(r) = self.reply.take()
            {
                self.unused_reply(r);
            }
            match stmt.kind() {
                TITLE_STMT => {
                    let t = tokens(&stmt)
                        .iter()
                        .find(|t| t.kind() == TEXT)
                        .map(|t| t.text().trim().to_owned());
                    self.seq.title = t.filter(|s| !s.is_empty());
                }
                PARTICIPANT_STMT => self.participant(&stmt),
                ANNOTATION_STMT => self.reply = Some(stmt.text_range()),
                MESSAGE_STMT => self.async_message(&stmt),
                CALL => self.call(&stmt),
                SEQ_STMT => self.ret(&stmt),
                FRAGMENT => self.fragment(&stmt),
                _ => {}
            }
        }
    }

    /// `@Actor A as Label`, `A as Label`, or `@Starter(A)`.
    fn participant(&mut self, stmt: &SyntaxNode) {
        let ts = tokens(stmt);
        let mut i = 0;
        let mut kind = None;
        if ts.first().is_some_and(|t| t.kind() == AT) {
            let annotator = &ts[1];
            if annotator.text() == "Starter" {
                if let Some(n) = ts.iter().skip(2).find(|t| is_name(t)) {
                    let p = self.ensure(&name(n));
                    self.starter = Some(p);
                }
                return;
            }
            match ParticipantKind::from_type(annotator.text()) {
                Some(k) => kind = Some(k),
                None => self.warning(
                    annotator.text_range(),
                    format!(
                        "`@{}` is not drawn; drawing a participant",
                        annotator.text()
                    ),
                ),
            }
            i = 2;
        }
        let Some(id) = ts.get(i).filter(|t| is_name(t)) else {
            return;
        };
        let p = self.ensure(&name(id));
        if let Some(k) = kind {
            self.seq.participants[p].kind = k;
        }
        if let Some(label) = ts
            .iter()
            .position(|t| t.kind() == AS_KW)
            .and_then(|a| ts.get(a + 1))
        {
            self.seq.participants[p].label = display(&name(label));
        }
    }

    /// `A->B: text`, dotted after `@return`.
    fn async_message(&mut self, stmt: &SyntaxNode) {
        let ts = tokens(stmt);
        let names: Vec<&SyntaxToken> = ts.iter().filter(|t| is_name(t)).collect();
        let [from, to] = names[..] else {
            return;
        };
        let (from, to) = (self.ensure(&name(from)), self.ensure(&name(to)));
        let arrow = if self.reply.take().is_some() {
            REPLY
        } else {
            ASYNC
        };
        let text = ts
            .iter()
            .find(|t| t.kind() == TEXT)
            .map_or("", |t| t.text());
        let m = self.message(from, to, arrow, text);
        self.push(Event::Message(m));
    }

    /// `[a =] [A->]B.method(args) [{ ... }]` or `[a =] new B(args) [{ ... }]`.
    fn call(&mut self, stmt: &SyntaxNode) {
        let ts: Vec<SyntaxToken> = tokens(stmt)
            .into_iter()
            .take_while(|t| t.kind() != L_BRACE)
            .collect();
        let new = ts.iter().any(|t| t.kind() == ZEN_KW);
        let dot = ts.iter().position(|t| t.kind() == DOT);
        let names: Vec<&SyntaxToken> = ts[..dot.unwrap_or(ts.len())]
            .iter()
            .filter(|t| is_name(t))
            .collect();
        let has_arrow = ts.iter().any(|t| t.kind() == ARROW);
        let (caller, target) = match (has_arrow, &names[..]) {
            (true, [a, b, ..]) => (self.ensure(&name(a)), self.ensure(&name(b))),
            (_, [b, ..]) => (self.current(), self.ensure(&name(b))),
            _ => return,
        };
        let args = args(stmt);
        let text = if new {
            "<<create>>".to_owned()
        } else {
            let method = dot
                .and_then(|d| ts.get(d + 1))
                .map_or(String::new(), |t| t.text().to_owned());
            match &args {
                Some(a) => format!("{method}({a})"),
                None => method,
            }
        };
        if new && !self.used.contains(&target) {
            self.push(Event::Create(target));
        }
        let mut m = self.message(caller, target, SYNC, &text);
        m.activate = true;
        self.push(Event::Message(m));
        self.frames.push(Frame {
            caller,
            target,
            returned: false,
        });
        self.statements(stmt);
        let frame = self.frames.pop().expect("pushed above");
        let assignee = stmt
            .children()
            .find(|n| n.kind() == LABEL)
            .map(|n| n.text().to_string());
        match assignee {
            Some(a) if !frame.returned => {
                let var = a.split_whitespace().last().unwrap_or_default().to_owned();
                let mut m = self.message(target, caller, REPLY, &var);
                m.deactivate = true;
                self.push(Event::Message(m));
            }
            _ => self.push(Event::Deactivate(target)),
        }
    }

    /// `return x`: a reply from the call being lowered to its caller.
    fn ret(&mut self, stmt: &SyntaxNode) {
        let Some(frame) = self.frames.last_mut() else {
            self.warning(stmt.text_range(), "`return` outside a call; ignoring it");
            return;
        };
        frame.returned = true;
        let (from, to) = (frame.target, frame.caller);
        let text = tokens(stmt)
            .iter()
            .find(|t| t.kind() == TEXT)
            .map_or(String::new(), |t| t.text().to_owned());
        let m = self.message(from, to, REPLY, &text);
        self.push(Event::Message(m));
    }

    /// `if` / `else`, a loop, `opt`, `par`, or `try` / `catch` / `finally`.
    fn fragment(&mut self, stmt: &SyntaxNode) {
        let branches: Vec<SyntaxNode> = stmt.children().filter(|n| n.kind() == BRANCH).collect();
        let Some(first) = branches.first() else {
            return;
        };
        if self.head_error(first) {
            return;
        }
        let words = |b: &SyntaxNode| -> Vec<String> {
            tokens(b)
                .iter()
                .filter(|t| t.kind() == ZEN_KW)
                .map(|t| t.text().to_owned())
                .collect()
        };
        let kind = match words(first)[0].as_str() {
            "if" => BlockKind::Alt,
            "opt" => BlockKind::Opt,
            "par" => BlockKind::Par,
            "try" => BlockKind::Try,
            _ => BlockKind::Loop,
        };
        self.seq
            .events
            .push(Event::Start(kind, args(first).unwrap_or_default()));
        self.statements(first);
        for b in &branches[1..] {
            if self.head_error(b) {
                break;
            }
            let cond = args(b).unwrap_or_default();
            let label = match words(b)[..] {
                [ref w] if w == "catch" => format!("catch {cond}").trim().to_owned(),
                [ref w] if w == "finally" => "finally".to_owned(),
                _ => cond,
            };
            self.push(Event::Divider(label));
            self.statements(b);
        }
        self.push(Event::End);
    }

    fn finish(mut self) -> (Sequence, Vec<Diagnostic>) {
        let used = self.seq.events.iter().any(|e| match e {
            Event::Message(m) => m.from == 0 || m.to == 0,
            _ => false,
        });
        if !used {
            self.seq.participants.remove(0);
            for e in &mut self.seq.events {
                match e {
                    Event::Message(m) => {
                        m.from -= 1;
                        m.to -= 1;
                    }
                    Event::Note(n) => n.participants.iter_mut().for_each(|p| *p -= 1),
                    Event::Activate(p)
                    | Event::Deactivate(p)
                    | Event::Create(p)
                    | Event::Destroy(p) => *p -= 1,
                    _ => {}
                }
            }
        }
        (self.seq, self.diagnostics)
    }
}
