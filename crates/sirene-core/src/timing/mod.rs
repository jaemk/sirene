//! Timing diagrams (TIMING-*): signals whose state changes over time, with messages,
//! constraints, and highlights between them. PlantUML timing diagrams lower to this model.

use std::collections::HashMap;

/// How a signal is drawn (TIMING-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// A line stepping between levels, one per state.
    Robust,
    /// A band with each state's name in it.
    Concise,
    /// A concise band in a box.
    Rectangle,
    /// A line between high and low.
    Binary,
    /// A binary line that repeats with a period.
    Clock,
    /// A line through numeric values.
    Analog,
}

/// A signal's value from a moment on.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    State(String),
    /// `{hidden}`: nothing is drawn.
    Hidden,
    /// `{-}` or `{a,b}`: the state is unknown.
    Undefined,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub at: f64,
    pub value: Value,
    /// Fill as `#rrggbb`.
    pub color: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub period: f64,
    /// How long each period is high; half the period when `None`.
    pub pulse: Option<f64>,
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Above the signal, else below it.
    pub above: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    pub id: String,
    pub label: String,
    pub kind: SignalKind,
    /// Declared states, top first, as (name, label).
    pub states: Vec<(String, String)>,
    pub changes: Vec<Change>,
    pub clock: Option<Clock>,
    /// An analog signal's fixed scale.
    pub scale: Option<(f64, f64)>,
    pub notes: Vec<Note>,
}

impl Signal {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: SignalKind) -> Self {
        Signal {
            id: id.into(),
            label: label.into(),
            kind,
            states: Vec::new(),
            changes: Vec::new(),
            clock: None,
            scale: None,
            notes: Vec::new(),
        }
    }

    /// The levels of a robust or binary signal, top first: the declared states, then the others
    /// in the order they appear. Binary and clock signals are high over low.
    pub fn levels(&self) -> Levels<'_> {
        let binary = matches!(self.kind, SignalKind::Binary | SignalKind::Clock);
        let candidates: Vec<&str> = if binary {
            vec!["high", "low"]
        } else {
            let declared = self.states.iter().map(|(n, _)| n.as_str());
            let used = self.changes.iter().filter_map(|c| match &c.value {
                Value::State(s) => Some(s.as_str()),
                _ => None,
            });
            declared.chain(used).collect()
        };
        let mut names: Vec<&str> = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        for n in candidates {
            if !index.contains_key(n) {
                index.insert(n, names.len());
                names.push(n);
            }
        }
        let labels = self
            .states
            .iter()
            .map(|(n, l)| (n.as_str(), l.as_str()))
            .collect();
        Levels {
            names,
            index,
            labels,
            binary,
        }
    }
}

/// A signal's levels and state labels, indexed (TIMING-2).
#[derive(Debug, Clone)]
pub struct Levels<'a> {
    /// Level names, top first.
    pub names: Vec<&'a str>,
    index: HashMap<&'a str, usize>,
    labels: HashMap<&'a str, &'a str>,
    binary: bool,
}

impl<'a> Levels<'a> {
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The level of `state`, 0 at the top. A binary value other than a true word is low.
    pub fn level(&self, state: &str) -> Option<usize> {
        if self.binary {
            let high = matches!(
                state.to_ascii_lowercase().as_str(),
                "high" | "1" | "true" | "on"
            );
            return Some(usize::from(!high));
        }
        self.index.get(state).copied()
    }

    /// A state's display label.
    pub fn label<'b>(&self, name: &'b str) -> &'b str
    where
        'a: 'b,
    {
        self.labels.get(name).copied().unwrap_or(name)
    }
}

impl Signal {
    /// An analog signal's scale: its fixed one, or its values' range.
    pub fn analog_scale(&self) -> (f64, f64) {
        if let Some(s) = self.scale {
            return s;
        }
        let values = self.changes.iter().filter_map(|c| match &c.value {
            Value::State(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        });
        let (lo, hi) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
        if lo > hi {
            (0.0, 1.0)
        } else if lo == hi {
            (lo - 1.0, hi + 1.0)
        } else {
            (lo, hi)
        }
    }
}

/// A segment of a signal: from `start` to `end`, its value, and its fill.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment<'a> {
    pub start: f64,
    pub end: f64,
    pub value: Value,
    pub color: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub from_at: f64,
    pub to: String,
    pub to_at: f64,
    pub label: String,
}

/// A labeled span between two moments, on a signal or under the axis.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub signal: Option<String>,
    pub from: f64,
    pub to: f64,
    pub label: String,
}

/// A shaded span across every signal.
#[derive(Debug, Clone, PartialEq)]
pub struct Highlight {
    pub from: f64,
    pub to: f64,
    pub color: Option<String>,
    pub label: String,
}

/// A timing diagram (TIMING-1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timing {
    pub title: Option<String>,
    pub signals: Vec<Signal>,
    pub messages: Vec<Message>,
    pub constraints: Vec<Constraint>,
    pub highlights: Vec<Highlight>,
    pub hide_axis: bool,
    /// Axis labels for moments written as dates or times; numbered ticks when empty.
    pub marks: Vec<(f64, String)>,
}

/// The most clock periods drawn.
pub(crate) const MAX_PERIODS: f64 = 1000.0;

impl Timing {
    /// The span of time drawn: every change, message, constraint, and highlight, and at least
    /// one unit past the last change so its state shows.
    pub fn range(&self) -> Option<(f64, f64)> {
        let mut times: Vec<f64> = Vec::new();
        for s in &self.signals {
            times.extend(s.changes.iter().map(|c| c.at));
        }
        for m in &self.messages {
            times.extend([m.from_at, m.to_at]);
        }
        for c in &self.constraints {
            times.extend([c.from, c.to]);
        }
        for h in &self.highlights {
            times.extend([h.from, h.to]);
        }
        times.extend(self.marks.iter().map(|(t, _)| *t));
        let times: Vec<f64> = times.into_iter().filter(|t| t.is_finite()).collect();
        let (lo, hi) = times
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &t| {
                (lo.min(t), hi.max(t))
            });
        let longest = self
            .signals
            .iter()
            .filter_map(|s| s.clock)
            .map(|c| c.period)
            .fold(0.0_f64, f64::max);
        if lo > hi {
            return (longest > 0.0).then_some((0.0, longest * 4.0));
        }
        // A tail after the last moment, so the last state has room.
        let tail = ((hi - lo) / 10.0).max(longest).max(1.0);
        Some((lo, hi + tail))
    }

    /// The axis ticks: the written moments, or round numbers across `range`.
    pub fn ticks(&self, (lo, hi): (f64, f64)) -> Vec<(f64, String)> {
        if !self.marks.is_empty() {
            let mut m = self.marks.clone();
            m.sort_by(|a, b| a.0.total_cmp(&b.0));
            m.dedup_by(|a, b| a.0 == b.0);
            return m;
        }
        let step = nice_step((hi - lo) / 8.0);
        let mut out = Vec::new();
        let mut t = (lo / step).ceil() * step;
        while t <= hi + step * 1e-9 && out.len() < 100 {
            out.push((t, number(t)));
            t += step;
        }
        out
    }

    /// `signal`'s segments across `range`, in time order (TIMING-3). A clock's are its periods.
    pub fn segments<'a>(&self, signal: &'a Signal, (lo, hi): (f64, f64)) -> Vec<Segment<'a>> {
        if let Some(c) = signal.clock
            && c.period > 0.0
        {
            let pulse = c.pulse.unwrap_or(c.period / 2.0).clamp(0.0, c.period);
            let mut out = Vec::new();
            let first = ((lo - c.offset) / c.period).floor();
            let last = ((hi - c.offset) / c.period).ceil();
            // A count, not a float step: `k += 1.0` stalls once `k` is past 2^53.
            let n = (last - first).clamp(0.0, MAX_PERIODS) as usize;
            for i in 0..n {
                let start = c.offset + (first + i as f64) * c.period;
                for (a, b, v) in [
                    (start, start + pulse, "high"),
                    (start + pulse, start + c.period, "low"),
                ] {
                    let (a, b) = (a.max(lo), b.min(hi));
                    if b > a {
                        out.push(Segment {
                            start: a,
                            end: b,
                            value: Value::State(v.to_owned()),
                            color: None,
                        });
                    }
                }
            }
            return out;
        }
        let mut changes: Vec<&Change> = signal.changes.iter().collect();
        changes.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut out: Vec<Segment> = Vec::new();
        for (i, c) in changes.iter().enumerate() {
            // A later change at the same moment wins.
            if changes.get(i + 1).is_some_and(|n| n.at == c.at) {
                continue;
            }
            let end = changes
                .iter()
                .skip(i + 1)
                .map(|n| n.at)
                .find(|&t| t > c.at)
                .unwrap_or(hi);
            out.push(Segment {
                start: c.at,
                end,
                value: c.value.clone(),
                color: c.color.as_deref(),
            });
        }
        out
    }

    pub fn signal(&self, id: &str) -> Option<&Signal> {
        self.signals.iter().find(|s| s.id == id)
    }
}

/// A round step near `raw`: 1, 2, or 5 times a power of ten.
fn nice_step(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    let f = if m <= 1.0 {
        1.0
    } else if m <= 2.0 {
        2.0
    } else if m <= 5.0 {
        5.0
    } else {
        10.0
    };
    f * p
}

/// `t` without a trailing `.0`.
pub fn number(t: f64) -> String {
    let r = (t * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(at: f64, v: &str) -> Change {
        Change {
            at,
            value: Value::State(v.to_owned()),
            color: None,
            note: None,
        }
    }

    #[test]
    fn segments_and_levels() {
        let mut s = Signal::new("a", "A", SignalKind::Robust);
        s.states.push(("busy".into(), "Busy".into()));
        s.changes = vec![
            change(10.0, "idle"),
            change(0.0, "busy"),
            change(10.0, "off"),
        ];
        let t = Timing {
            signals: vec![s.clone()],
            ..Timing::default()
        };
        let range = t.range().unwrap();
        assert_eq!(range, (0.0, 11.0));
        let segs = t.segments(&s, range);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].value, Value::State("off".into()));
        let levels = s.levels();
        assert_eq!(levels.names, ["busy", "idle", "off"]);
        assert_eq!(levels.level("off"), Some(2));
        assert_eq!(levels.level("nope"), None);
        assert_eq!(levels.label("busy"), "Busy");
        assert_eq!(levels.label("idle"), "idle");
    }

    #[test]
    fn clocks() {
        let mut s = Signal::new("c", "C", SignalKind::Clock);
        s.clock = Some(Clock {
            period: 10.0,
            pulse: Some(3.0),
            offset: 0.0,
        });
        let t = Timing::default();
        let segs = t.segments(&s, (0.0, 20.0));
        let spans: Vec<(f64, f64)> = segs.iter().map(|g| (g.start, g.end)).collect();
        assert_eq!(spans, [(0.0, 3.0), (3.0, 10.0), (10.0, 13.0), (13.0, 20.0)]);
        // A huge range draws a bounded number of periods.
        assert!(t.segments(&s, (0.0, 1e12)).len() <= 2 * MAX_PERIODS as usize + 2);
        // Far from zero, where a float step would stall, it still ends.
        assert!(t.segments(&s, (1e17, 1e17 + 1e4)).len() <= 2 * MAX_PERIODS as usize + 2);
        assert_eq!(s.levels().level("1"), Some(0));
        assert_eq!(s.levels().level("low"), Some(1));
    }

    #[test]
    fn ticks() {
        let t = Timing::default();
        let ticks: Vec<String> = t.ticks((0.0, 100.0)).into_iter().map(|t| t.1).collect();
        assert_eq!(ticks, ["0", "20", "40", "60", "80", "100"]);
        assert_eq!(number(2.5), "2.5");
        assert_eq!(nice_step(0.0), 1.0);
    }
}
