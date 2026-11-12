//! PlantUML timing diagrams (PTIM-*): signals, `@` moments, state changes, messages,
//! constraints, highlights, and notes, lowered to the timing model (TIMING-1).

use std::collections::HashMap;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line};
use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;
use crate::timing::{
    Change, Clock, Constraint, Highlight, MAX_PERIODS, Message, Note, Signal, SignalKind, Timing,
    Value,
};

pub(crate) const STATEMENT: &str = "expected a signal such as `robust \"Web\" as W`, a moment \
     such as `@100`, or a change such as `W is Idle`";

const KINDS: &[(&str, SignalKind)] = &[
    ("robust", SignalKind::Robust),
    ("concise", SignalKind::Concise),
    ("rectangle", SignalKind::Rectangle),
    ("binary", SignalKind::Binary),
    ("clock", SignalKind::Clock),
    ("analog", SignalKind::Analog),
];

/// The largest moment, period, or value read; past it, float steps lose whole units.
const LIMIT: f64 = 1e15;

/// `s` as a number within +-LIMIT (no `inf` or `NaN`).
fn real(s: &str) -> Option<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() <= LIMIT)
}

/// Days from 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A moment written as a date (`2019/07/02`) or a time of day (`1:15:00`), in seconds.
fn clock_time(s: &str) -> Option<f64> {
    let int = |p: &str| {
        (!p.is_empty() && p.len() <= 4 && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<i64>().ok())
            .flatten()
    };
    if s.contains('/') {
        let p: Vec<&str> = s.split('/').collect();
        let [y, m, d] = p[..] else { return None };
        let (y, m, d) = (int(y)?, int(m)?, int(d)?);
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        return Some(days(y, m, d) as f64 * 86_400.0);
    }
    if s.contains(':') {
        let p: Vec<&str> = s.split(':').collect();
        let (h, m, sec) = match p[..] {
            [h, m] => (int(h)?, int(m)?, 0),
            [h, m, sec] => (int(h)?, int(m)?, int(sec)?),
            _ => return None,
        };
        if m > 59 || sec > 59 {
            return None;
        }
        return Some((h * 3600 + m * 60 + sec) as f64);
    }
    None
}

struct Lower {
    t: Timing,
    d: Vec<Diagnostic>,
    now: f64,
    /// Each signal's index in `t.signals`.
    ids: HashMap<String, usize>,
    /// Named moments; a later `as :name` replaces an earlier one.
    anchors: HashMap<String, f64>,
    /// Where each clock was declared.
    clocks: Vec<(usize, TextRange)>,
    /// The signal of a participant-oriented `@W` block.
    focus: Option<String>,
    /// Each signal's `has` states, by name, as indexes into its `states`.
    states: HashMap<usize, HashMap<String, usize>>,
}

impl Lower {
    /// A moment: `100`, `+50`, `-5`, `:name`, `:name+5`, `clk*2`, `2019/07/02`, or `1:15:00`.
    /// Relative moments count from `base`.
    fn moment(&mut self, s: &str, base: f64, range: TextRange) -> Option<f64> {
        let at = self.moment_of(s, base, range)?;
        if at.abs() > LIMIT {
            self.d.push(Diagnostic::new(
                range,
                format!("the moment `{}` is out of range", s.trim()),
            ));
            return None;
        }
        Some(at)
    }

    fn moment_of(&mut self, s: &str, base: f64, range: TextRange) -> Option<f64> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix(':') {
            let rest = rest.trim();
            if let Some(&at) = self.anchors.get(rest) {
                return Some(at);
            }
            // `:name+5`: the offset follows the last sign, so names may hold `-`.
            if let Some(i) = rest.rfind(['+', '-'])
                && let Some(&at) = self.anchors.get(rest[..i].trim())
            {
                return self.number(&rest[i..], range).map(|o| at + o);
            }
            let name = rest.rfind(['+', '-']).map_or(rest, |i| rest[..i].trim());
            self.d.push(Diagnostic::new(
                range,
                format!("no moment is named `:{name}`"),
            ));
            return None;
        }
        if let Some((id, n)) = s.split_once('*') {
            let clock = self.signal(id.trim()).and_then(|sig| sig.clock);
            let Some(c) = clock else {
                self.d.push(Diagnostic::new(
                    range,
                    format!("`{}` is not a clock", id.trim()),
                ));
                return None;
            };
            return self.number(n, range).map(|k| c.offset + k * c.period);
        }
        if let Some(rest) = s.strip_prefix('+') {
            return self.number(rest, range).map(|o| base + o);
        }
        if let Some(at) = clock_time(s) {
            self.t.marks.push((at, s.to_owned()));
            return Some(at);
        }
        self.number(s, range)
    }

    fn number(&mut self, s: &str, range: TextRange) -> Option<f64> {
        match real(s) {
            Some(v) => Some(v),
            None => {
                self.d.push(Diagnostic::new(
                    range,
                    format!(
                        "expected a moment such as `100` or `+50`, not `{}`",
                        s.trim()
                    ),
                ));
                None
            }
        }
    }

    /// `robust "Label" as ID`, with `with period N pulse N offset N` for a clock and
    /// `between A and B` for an analog signal.
    fn declare(&mut self, line: &Line, kind: SignalKind, rest: &str) {
        let Some((id, label, mut after)) = lines::name(rest) else {
            return self
                .d
                .push(Diagnostic::new(line.range, "expected a signal name"));
        };
        let mut s = Signal::new(id, label, kind);
        if let Some(open) = after.find("<<") {
            after = after[..open].trim_end();
        }
        let words: Vec<&str> = after.split_whitespace().collect();
        let mut i = 0;
        let mut clock = Clock {
            period: 0.0,
            pulse: None,
            offset: 0.0,
        };
        // A bad `period` or `pulse` was already reported.
        let mut bad_clock = false;
        while i < words.len() {
            let w = words[i].to_ascii_lowercase();
            let arg = words.get(i + 1).and_then(|v| real(v));
            match (w.as_str(), arg) {
                ("with", _) => i += 1,
                ("period", Some(v)) if v > 0.0 => {
                    clock.period = v;
                    i += 2;
                }
                ("pulse", Some(v)) if v >= 0.0 => {
                    clock.pulse = Some(v);
                    i += 2;
                }
                ("period" | "pulse", _) => {
                    let what = if w == "period" {
                        "a clock's period is a number above 0"
                    } else {
                        "a clock's pulse is a number of 0 or more"
                    };
                    self.d.push(Diagnostic::new(line.range_of(words[i]), what));
                    bad_clock = true;
                    i += 2;
                }
                ("offset", Some(v)) => {
                    clock.offset = v;
                    i += 2;
                }
                ("between", Some(lo)) => {
                    let hi = (words.get(i + 2).map(|w| w.eq_ignore_ascii_case("and"))
                        == Some(true))
                    .then(|| words.get(i + 3).and_then(|v| real(v)))
                    .flatten();
                    match hi {
                        Some(hi) if hi > lo => s.scale = Some((lo, hi)),
                        _ => self.d.push(Diagnostic::new(
                            line.range,
                            "expected `between LOW and HIGH`",
                        )),
                    }
                    i += 4;
                }
                _ => {
                    self.d.push(Diagnostic::new(
                        line.range_of(words[i]),
                        format!("unexpected `{}`", words[i]),
                    ));
                    break;
                }
            }
        }
        if kind == SignalKind::Clock {
            if clock.period > 0.0 {
                s.clock = Some(clock);
            } else if !bad_clock {
                self.d
                    .push(Diagnostic::new(line.range, "a clock needs `with period N`"));
            }
        }
        if self.ids.contains_key(&s.id) {
            self.d.push(Diagnostic::new(
                line.range,
                format!("`{}` is already declared", s.id),
            ));
            return;
        }
        let i = self.t.signals.len();
        if s.clock.is_some() {
            self.clocks.push((i, line.range));
        }
        self.ids.insert(s.id.clone(), i);
        self.t.signals.push(s);
    }

    fn signal(&self, id: &str) -> Option<&Signal> {
        self.ids.get(id).map(|&i| &self.t.signals[i])
    }

    fn signal_mut(&mut self, id: &str, range: TextRange) -> Option<&mut Signal> {
        let at = self.ids.get(id).copied();
        if at.is_none() {
            self.d
                .push(Diagnostic::new(range, format!("no signal is named `{id}`")));
        }
        at.map(|i| &mut self.t.signals[i])
    }

    /// `ID is VALUE [#color] [: note]` at `at`.
    fn change(&mut self, id: &str, value: &str, at: f64, range: TextRange) {
        let (value, note) = lines::caption(value);
        let (value, color) = match value.find('#') {
            Some(i) => (value[..i].trim(), lines::color(value[i..].trim())),
            None => (value, None),
        };
        let v = match value {
            "{hidden}" => Value::Hidden,
            "{-}" => Value::Undefined,
            v if v.starts_with('{') && v.ends_with('}') => Value::Undefined,
            v => Value::State(lines::unquote(v).trim().to_owned()),
        };
        if let Value::State(s) = &v
            && s.is_empty()
        {
            return self
                .d
                .push(Diagnostic::new(range, "expected a state after `is`"));
        }
        let Some(sig) = self.signal_mut(id, range) else {
            return;
        };
        if sig.kind == SignalKind::Clock {
            return self.d.push(Diagnostic::warning(
                range,
                "a clock's states follow its period; ignoring the change",
            ));
        }
        if sig.kind == SignalKind::Analog
            && let Value::State(s) = &v
            && real(s).is_none()
        {
            return self.d.push(Diagnostic::new(
                range,
                format!("an analog signal's value is a number, not `{s}`"),
            ));
        }
        sig.changes.push(Change {
            at,
            value: v,
            color,
            note,
        });
    }

    /// `A -> B : text`, `A@100 -> B@+50 : text`.
    fn message(&mut self, t: &str, range: TextRange) -> bool {
        let Some((left, right)) = t.split_once("->") else {
            return false;
        };
        if left.ends_with('<') {
            return false;
        }
        let (right, label) = lines::caption(right);
        let label = label.unwrap_or_default();
        let now = self.now;
        let end = |s: &str| -> (String, Option<String>) {
            match s.trim().split_once('@') {
                Some((id, at)) => (id.trim().to_owned(), Some(at.trim().to_owned())),
                None => (s.trim().to_owned(), None),
            }
        };
        let (from, from_at) = end(left);
        let (to, to_at) = end(right);
        if from.is_empty() || to.is_empty() {
            return false;
        }
        let from_at = match from_at {
            Some(s) => self.moment(&s, now, range),
            None => Some(now),
        };
        let Some(from_at) = from_at else { return true };
        let to_at = match to_at {
            Some(s) => self.moment(&s, from_at, range),
            None => Some(from_at),
        };
        let Some(to_at) = to_at else { return true };
        for id in [&from, &to] {
            if self.signal(id).is_none() {
                self.d
                    .push(Diagnostic::new(range, format!("no signal is named `{id}`")));
                return true;
            }
        }
        self.t.messages.push(Message {
            from,
            from_at,
            to,
            to_at,
            label,
        });
        true
    }

    /// `A@0 <-> @50 : {50 ms}`, `@200 <-> @+150 : {150 ms}`.
    fn constraint(&mut self, t: &str, range: TextRange) -> bool {
        let Some((left, right)) = t.split_once("<->") else {
            return false;
        };
        let (right, label) = lines::caption(right);
        let label = label.unwrap_or_default();
        let label = label
            .strip_prefix('{')
            .and_then(|l| l.strip_suffix('}'))
            .map_or(label.clone(), |l| l.trim().to_owned());
        let Some((sig, from)) = left.trim().split_once('@') else {
            self.d
                .push(Diagnostic::new(range, "expected `@TIME <-> @TIME`"));
            return true;
        };
        let Some(to) = right.trim().strip_prefix('@') else {
            self.d
                .push(Diagnostic::new(range, "expected `@TIME <-> @TIME`"));
            return true;
        };
        let now = self.now;
        let Some(from) = self.moment(from, now, range) else {
            return true;
        };
        let Some(to) = self.moment(to, from, range) else {
            return true;
        };
        let sig = sig.trim();
        let signal = if sig.is_empty() {
            None
        } else if self.signal(sig).is_some() {
            Some(sig.to_owned())
        } else {
            self.d.push(Diagnostic::new(
                range,
                format!("no signal is named `{sig}`"),
            ));
            return true;
        };
        self.t.constraints.push(Constraint {
            signal,
            from,
            to,
            label,
        });
        true
    }

    /// `highlight A to B [#color[;line:...]] [: caption]`.
    fn highlight(&mut self, rest: &str, range: TextRange) {
        let (rest, label) = lines::caption(rest);
        let label = label.unwrap_or_default();
        let lower = rest.to_ascii_lowercase();
        let Some(to_at) = lower.find(" to ") else {
            return self
                .d
                .push(Diagnostic::new(range, "expected `highlight FROM to TO`"));
        };
        let from = &rest[..to_at];
        let after = rest[to_at + 4..].trim();
        let (to, color) = match after.find('#') {
            Some(i) => (
                &after[..i],
                lines::color(after[i..].split(';').next().unwrap_or("")),
            ),
            None => (after, None),
        };
        let now = self.now;
        let Some(from) = self.moment(from, now, range) else {
            return;
        };
        let Some(to) = self.moment(to, from, range) else {
            return;
        };
        self.t.highlights.push(Highlight {
            from,
            to,
            color,
            label,
        });
    }

    /// `ID has a,b,c` or `ID has "Label" as name`.
    fn has(&mut self, id: &str, rest: &str, range: TextRange) {
        let Some(&si) = self.ids.get(id) else {
            self.d
                .push(Diagnostic::new(range, format!("no signal is named `{id}`")));
            return;
        };
        let sig = &mut self.t.signals[si];
        let at = self.states.entry(si).or_default();
        let mut states = Vec::new();
        for part in rest.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (label, name) = match lines::keyword_split(part, "as") {
                Some((l, n)) => (lines::unquote(l.trim()).to_owned(), n.trim().to_owned()),
                None => (
                    lines::unquote(part).to_owned(),
                    lines::unquote(part).to_owned(),
                ),
            };
            states.push((name, label));
        }
        for (name, label) in states {
            match at.get(&name) {
                Some(&i) => sig.states[i].1 = label,
                None => {
                    at.insert(name.clone(), sig.states.len());
                    sig.states.push((name, label));
                }
            }
        }
    }

    fn note(&mut self, line: &Line, cursor: &mut Cursor) {
        let rest = line.after_word();
        let lower = rest.to_ascii_lowercase();
        let (above, after) = if let Some(r) = lower.strip_prefix("top of") {
            (true, &rest[rest.len() - r.len()..])
        } else if let Some(r) = lower.strip_prefix("bottom of") {
            (false, &rest[rest.len() - r.len()..])
        } else {
            return self.d.push(Diagnostic::new(
                line.range,
                "expected `note top of X` or `note bottom of X`",
            ));
        };
        let (id, text) = match after.split_once(':') {
            Some((id, text)) => (id.trim(), lines::text(text.trim())),
            None => {
                let (body, end) = cursor.until(&["end note", "endnote"]);
                if end.is_none() {
                    self.d.push(Diagnostic::new(
                        line.range,
                        "unclosed `note`: expected `end note`",
                    ));
                }
                let text: Vec<&str> = body.iter().map(|l| l.text.as_str()).collect();
                (after.trim(), text.join("\n"))
            }
        };
        let id = lines::id_of(id);
        if let Some(sig) = self.signal_mut(&id, line.range) {
            sig.notes.push(Note { above, text });
        }
    }

    fn line(&mut self, line: &Line, cursor: &mut Cursor) {
        let t = line.text.as_str();
        let w = line.word();
        // `compact` before a declaration is a layout hint.
        let (t, w) = match lines::keyword(t, "compact") {
            Some(rest) if !rest.is_empty() => (rest, lines::word(rest).to_ascii_lowercase()),
            _ => (t, w),
        };
        if let Some(&(_, kind)) = KINDS.iter().find(|(k, _)| *k == w) {
            let rest = t[w.len()..].trim();
            return self.declare(line, kind, rest);
        }
        match line.squeezed().as_str() {
            "hide time-axis" | "hide time axis" => {
                self.t.hide_axis = true;
                return;
            }
            "mode compact" => return,
            "manual time-axis" => {
                return self.d.push(Diagnostic::warning(
                    line.range,
                    "`manual time-axis` is not drawn; the axis is numbered",
                ));
            }
            _ => {}
        }
        if t.contains("<->") {
            self.constraint(t, line.range);
            return;
        }
        if let Some(rest) = t.strip_prefix('@') {
            // `@W` starts a participant-oriented block; `@100 as :name` is a moment.
            let first = rest.split_whitespace().next().unwrap_or("");
            if !first.contains('*') && self.signal(first).is_some() {
                self.focus = Some(first.to_owned());
                return;
            }
            self.focus = None;
            let (at, name) = match lines::keyword_split(rest, "as") {
                Some((at, name)) => (at, Some(name.trim())),
                None => (rest, None),
            };
            let now = self.now;
            if let Some(at) = self.moment(at, now, line.range) {
                self.now = at;
                if let Some(name) = name {
                    let name = name.strip_prefix(':').unwrap_or(name).trim().to_owned();
                    self.anchors.insert(name, at);
                }
            }
            return;
        }
        if w == "highlight" {
            return self.highlight(line.after_word(), line.range);
        }
        if w == "note" {
            return self.note(line, cursor);
        }
        if self.message(t, line.range) {
            return;
        }
        // `ID is VALUE`, `ID has ...`, or in an `@ID` block `TIME is VALUE`.
        let Some((head, value)) = lines::keyword_split(t, "is") else {
            if let Some((id, rest)) = lines::keyword_split(t, "has") {
                return self.has(lines::unquote(id.trim()), rest, line.range);
            }
            return self.d.push(Diagnostic::new(line.range, STATEMENT));
        };
        let head = head.trim();
        if let Some(id) = self.focus.clone() {
            let now = self.now;
            if let Some(at) = self.moment(head, now, line.range) {
                self.now = at;
                self.change(&id, value, at, line.range);
            }
            return;
        }
        let (id, at) = match head.split_once('@') {
            Some((id, at)) => {
                let now = self.now;
                (id.trim(), self.moment(at, now, line.range))
            }
            None => (head, Some(self.now)),
        };
        if let Some(at) = at {
            self.change(lines::unquote(id), value, at, line.range);
        }
    }
}

pub(crate) fn lower(root: &SyntaxNode) -> (Timing, Vec<Diagnostic>) {
    let mut l = Lower {
        t: Timing::default(),
        d: Vec::new(),
        now: 0.0,
        ids: HashMap::new(),
        anchors: HashMap::new(),
        clocks: Vec::new(),
        focus: None,
        states: HashMap::new(),
    };
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        if !matches!(
            line.squeezed().as_str(),
            "hide time-axis" | "hide time axis"
        ) {
            match lines::common(&line, &mut cursor, &mut l.d) {
                Common::No => {}
                Common::Title(t) => {
                    l.t.title = Some(t);
                    continue;
                }
                Common::Done | Common::Direction(_) => continue,
            }
        }
        l.line(&line, &mut cursor);
    }
    // A clock draws at most MAX_PERIODS periods (TIMING-3).
    if let Some((lo, hi)) = l.t.range() {
        for &(i, range) in &l.clocks {
            if let Some(c) = l.t.signals[i].clock
                && (hi - lo) / c.period > MAX_PERIODS
            {
                l.d.push(Diagnostic::warning(
                    range,
                    format!(
                        "the diagram spans more than {MAX_PERIODS} periods of `{}`; only the \
                         first {MAX_PERIODS} are drawn",
                        l.t.signals[i].id
                    ),
                ));
            }
        }
    }
    (l.t, l.d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_times() {
        assert_eq!(clock_time("1:15:00"), Some(4500.0));
        assert_eq!(clock_time("1970/01/02"), Some(86_400.0));
        assert_eq!(days(2019, 7, 2), 18_079);
        assert_eq!(clock_time("1:75"), None);
        assert_eq!(clock_time("2019/13/01"), None);
        assert_eq!(clock_time("100"), None);
    }

    fn lower_src(body: &str) -> (Timing, Vec<String>) {
        let src = format!("@startuml\n{body}\n@enduml\n");
        let (diagram, d) = super::super::parse(&src).lower();
        let crate::diagram::Diagram::Timing(t) = diagram else {
            panic!("not a timing diagram")
        };
        (t, d.into_iter().map(|d| d.message).collect())
    }

    #[test]
    fn anchors() {
        let (t, d) = lower_src(
            "concise A\n@0 as :a\n@50 as :a\n@100 as :t-1\n@:t-1-10\nA is x\n@:a+5\nA is y",
        );
        assert!(d.is_empty(), "{d:?}");
        let at: Vec<f64> = t.signals[0].changes.iter().map(|c| c.at).collect();
        // `:t-1-10` is 10 before `:t-1`, and the later `:a` wins.
        assert_eq!(at, [90.0, 55.0]);
        let (_, d) = lower_src("concise A\n@:b-5\nA is x");
        assert_eq!(d, ["no moment is named `:b`"]);
    }

    #[test]
    fn bad_clock_settings() {
        let (_, d) = lower_src("clock C with period 0");
        assert_eq!(d, ["a clock's period is a number above 0"]);
        let (t, d) = lower_src("clock C with period 4 pulse -1");
        assert_eq!(d, ["a clock's pulse is a number of 0 or more"]);
        assert!(t.signals[0].clock.is_some());
        let (_, d) = lower_src("clock C");
        assert_eq!(d, ["a clock needs `with period N`"]);
    }

    #[test]
    fn long_clocks_warn() {
        let (_, d) = lower_src("clock C with period 1\nconcise A\n@0\nA is x\n@5000\nA is y");
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("more than 1000 periods of `C`"), "{d:?}");
    }

    #[test]
    fn analog_values_trimmed() {
        let (t, d) = lower_src("analog V\n@0\nV is \" 5\"");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.signals[0].changes[0].value, Value::State("5".into()));
    }

    #[test]
    fn has_states_relabel_in_place() {
        let (t, d) =
            lower_src("robust W\nW has a,b\nW has \"Bee\" as b,c\nrobust X\nX has b\nY has z");
        assert_eq!(d, ["no signal is named `Y`"]);
        let s = |i: usize| -> Vec<(&str, &str)> {
            t.signals[i]
                .states
                .iter()
                .map(|(n, l)| (n.as_str(), l.as_str()))
                .collect()
        };
        assert_eq!(s(0), [("a", "a"), ("b", "Bee"), ("c", "c")]);
        assert_eq!(s(1), [("b", "b")]);
    }

    #[test]
    fn reals() {
        assert_eq!(real(" 2.5 "), Some(2.5));
        assert_eq!(real("inf"), None);
        assert_eq!(real("NaN"), None);
        assert_eq!(real("1e300"), None);
    }
}
