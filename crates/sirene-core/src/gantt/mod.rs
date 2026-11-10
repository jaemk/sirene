//! Mermaid gantt charts (MGANTT-*): tasks on a time axis, grouped in sections.

mod lower;
mod parser;
pub mod time;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;
pub use time::{Interval, Time, Unit};

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// A task's tags (MGANTT-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tags {
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub name: String,
    pub id: Option<String>,
    pub tags: Tags,
    pub start: Time,
    pub end: Time,
    /// Where the bar ends: before `end` when excluded days pushed the end later (MGANTT-3).
    pub drawn_end: Time,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// `None` for the tasks before the first `section`.
    pub name: Option<String>,
    pub tasks: Vec<Task>,
}

/// How today is marked (MGANTT-5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TodayMarker {
    #[default]
    Default,
    Off,
    /// CSS such as `stroke-width:5px,stroke:#0f0`.
    Style(String),
}

/// A row of the drawing: tasks of one section that share it (MGANTT-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub section: usize,
    /// Indexes into the section's tasks.
    pub tasks: Vec<usize>,
}

/// A gantt chart (MGANTT-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gantt {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub date_format: String,
    pub axis_format: String,
    pub tick_interval: Option<Interval>,
    pub top_axis: bool,
    /// `displayMode: compact`: a section's tasks share rows when they do not overlap.
    pub compact: bool,
    /// The day weeks start on for ticks, 0 for Sunday.
    pub weekday: i64,
    /// The ISO weekday the weekend starts on: 5 (Friday) or 6 (Saturday).
    pub weekend: i64,
    /// Lowercase.
    pub excludes: Vec<String>,
    /// Lowercase.
    pub includes: Vec<String>,
    pub inclusive_end_dates: bool,
    pub today_marker: TodayMarker,
    /// The time the chart was read, for the today marker.
    pub now: Time,
    pub sections: Vec<Section>,
}

impl Default for Gantt {
    fn default() -> Self {
        Gantt {
            title: None,
            acc_title: None,
            acc_descr: None,
            date_format: "YYYY-MM-DD".to_owned(),
            axis_format: "%Y-%m-%d".to_owned(),
            tick_interval: None,
            top_axis: false,
            compact: false,
            weekday: 0,
            weekend: 6,
            excludes: Vec::new(),
            includes: Vec::new(),
            inclusive_end_dates: false,
            today_marker: TodayMarker::Default,
            now: 0,
            sections: Vec::new(),
        }
    }
}

impl Gantt {
    /// Every task in order.
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.sections.iter().flat_map(|s| &s.tasks)
    }

    /// The axis: the earliest start and the latest end, never empty.
    pub fn range(&self) -> Option<(Time, Time)> {
        let start = self.tasks().map(|t| t.start).min()?;
        let end = self.tasks().map(|t| t.end).max()?.max(start);
        Some((start, if end > start { end } else { start + time::DAY }))
    }

    /// Whether `t`'s day is excluded (Mermaid's `isInvalidDate`, MGANTT-3).
    pub fn is_excluded(&self, t: Time) -> bool {
        let written = time::format_dayjs(t, &self.date_format).to_lowercase();
        if self.includes.contains(&written) {
            return false;
        }
        // ISO weekday: Monday 1 to Sunday 7.
        let iso = match time::weekday(t) {
            0 => 7,
            n => n,
        };
        if self.excludes.iter().any(|e| e == "weekends")
            && (iso == self.weekend || iso == self.weekend + 1)
        {
            return true;
        }
        let day = time::weekday_name(time::weekday(t)).to_lowercase();
        self.excludes.contains(&day) || self.excludes.contains(&written)
    }

    /// The axis ticks (MGANTT-4).
    pub fn ticks(&self) -> Vec<Time> {
        let Some((start, end)) = self.range() else {
            return Vec::new();
        };
        let interval = self
            .tick_interval
            .unwrap_or_else(|| time::auto_interval(start, end));
        time::ticks(start, end, interval, self.weekday)
    }

    /// A tick's label.
    pub fn label(&self, t: Time) -> String {
        time::format_d3(t, &self.axis_format)
    }

    /// The days to grey out: the start of each excluded day from the axis start (MGANTT-5).
    pub fn excluded_days(&self) -> Vec<Time> {
        let Some((start, end)) = self.range() else {
            return Vec::new();
        };
        if self.excludes.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut d = start;
        while d <= end && out.len() < 100_000 {
            if self.is_excluded(d) {
                out.push(d);
            }
            d += time::DAY;
        }
        out
    }

    /// The rows to draw, in order; `vert` tasks take none (MGANTT-5).
    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        for (s, section) in self.sections.iter().enumerate() {
            let first = out.len();
            for (i, t) in section.tasks.iter().enumerate() {
                if t.tags.vert {
                    continue;
                }
                if self.compact {
                    let free = out[first..].iter_mut().find(|r: &&mut Row| {
                        r.tasks.iter().all(|&j| {
                            section.tasks[j].end <= t.start || t.end <= section.tasks[j].start
                        })
                    });
                    if let Some(row) = free {
                        row.tasks.push(i);
                        continue;
                    }
                }
                out.push(Row {
                    section: s,
                    tasks: vec![i],
                });
            }
        }
        out
    }
}

/// The result of parsing gantt source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Gantt`], read now (UTC).
    pub fn lower(&self) -> (Gantt, Vec<Diagnostic>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        self.lower_at(now)
    }

    /// Lowers the tree to a [`Gantt`] read at `now`, which fills date fields a format lacks and
    /// places the today marker. The returned diagnostics include the parser's, ordered by
    /// position.
    pub fn lower_at(&self, now: Time) -> (Gantt, Vec<Diagnostic>) {
        let (gantt, lowering) = lower::lower(&self.syntax(), now);
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a gantt chart: {} sections, {} tasks; {} diagnostics",
            gantt.sections.len(),
            gantt.tasks().count(),
            diagnostics.len()
        );
        (gantt, diagnostics)
    }
}

/// Parses gantt source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
