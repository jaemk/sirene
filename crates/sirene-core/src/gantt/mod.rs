//! Mermaid gantt charts (MGANTT-*): tasks on a time axis, grouped in sections.

mod lower;
mod parser;
pub mod time;

use std::collections::HashSet;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;
pub use time::{Interval, Time, Unit};

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// The most days [`Gantt::excluded_days`] looks at.
pub const MAX_EXCLUDED_SCAN: usize = 100_000;

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
    /// Days closed one by one, as day numbers (`t.div_euclid(DAY)`); ahead of `excludes`.
    pub closed_days: HashSet<i64>,
    /// Days opened one by one, as day numbers; ahead of everything else.
    pub open_days: HashSet<i64>,
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
            closed_days: HashSet::new(),
            open_days: HashSet::new(),
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
        let n = t.div_euclid(time::DAY);
        if self.open_days.contains(&n) {
            return false;
        }
        if self.closed_days.contains(&n) {
            return true;
        }
        if self.excludes.is_empty() {
            return false;
        }
        // Only lists naming dates need the day written out.
        let dated = !self.includes.is_empty()
            || self
                .excludes
                .iter()
                .any(|e| e != "weekends" && time::weekday_of(e).is_none());
        // A date is matched as written in `dateFormat` and as `YYYY-MM-DD`, so a list of days
        // works when the format also holds a time.
        let (written, iso_date) = if dated {
            (
                time::format_dayjs(t, &self.date_format).to_lowercase(),
                time::format_dayjs(t, "YYYY-MM-DD"),
            )
        } else {
            (String::new(), String::new())
        };
        let listed =
            |list: &[String]| dated && (list.contains(&written) || list.contains(&iso_date));
        if listed(&self.includes) {
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
        self.excludes.contains(&day) || listed(&self.excludes)
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

    /// The days to grey out: the start of each excluded day from the axis start (MGANTT-5),
    /// looking at no more than [`MAX_EXCLUDED_SCAN`] days.
    pub fn excluded_days(&self) -> Vec<Time> {
        let Some((start, end)) = self.range() else {
            return Vec::new();
        };
        if self.excludes.is_empty() && self.closed_days.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut d = start;
        for _ in 0..MAX_EXCLUDED_SCAN {
            if d > end {
                break;
            }
            if self.is_excluded(d) {
                out.push(d);
            }
            let Some(next) = d.checked_add(time::DAY) else {
                break;
            };
            d = next;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(days: i64, excludes: &[&str]) -> Gantt {
        Gantt {
            excludes: excludes.iter().map(|s| (*s).to_owned()).collect(),
            sections: vec![Section {
                name: None,
                tasks: vec![Task {
                    name: "x".into(),
                    id: None,
                    tags: Tags::default(),
                    start: 0,
                    end: days.saturating_mul(time::DAY),
                    drawn_end: 0,
                }],
            }],
            ..Gantt::default()
        }
    }

    #[test]
    fn excluded_days_scan_is_capped() {
        // A date that never comes up: the scan stops after its cap, not after a match cap.
        let g = chart(100_000_000_000, &["1960-01-01"]);
        assert!(g.excluded_days().is_empty());
        let g = chart(i64::MAX / time::DAY, &["thursday"]);
        let days = g.excluded_days();
        // 1970-01-01 is a Thursday.
        assert_eq!(days[0], 0);
        assert_eq!(days.len(), MAX_EXCLUDED_SCAN.div_ceil(7));
    }

    #[test]
    fn excluded_days_near_the_end_of_time() {
        let mut g = chart(1, &["thursday"]);
        let last = (i64::MAX / time::DAY) * time::DAY;
        g.sections[0].tasks[0].start = last;
        g.sections[0].tasks[0].end = i64::MAX;
        assert!(g.excluded_days().len() <= 1);
    }

    #[test]
    fn includes_without_excludes_exclude_nothing() {
        // Mermaid: an include only lifts an exclude; with none, every day is valid.
        let mut g = chart(14, &[]);
        g.includes = vec!["1970-01-02".to_owned()];
        assert!(!g.is_excluded(time::DAY));
        assert!(!g.is_excluded(2 * time::DAY));
        assert!(g.excluded_days().is_empty());
        // An include lifts a matching exclude only.
        let mut g = chart(14, &["saturday", "sunday"]);
        g.includes = vec!["1970-01-03".to_owned()];
        assert!(!g.is_excluded(2 * time::DAY));
        assert!(g.is_excluded(3 * time::DAY));
    }

    #[test]
    fn listed_days_match_with_a_time_in_the_format() {
        // Day 2 is 1970-01-03, a Saturday; the format holds a time, the list a plain date.
        let mut g = chart(14, &["1970-01-03"]);
        g.date_format = "YYYY-MM-DD HH:mm".to_owned();
        assert!(g.is_excluded(2 * time::DAY));
        assert!(!g.is_excluded(time::DAY));
        g.excludes = vec!["saturday".to_owned()];
        g.includes = vec!["1970-01-03".to_owned()];
        assert!(!g.is_excluded(2 * time::DAY));
    }

    #[test]
    fn single_days() {
        let mut g = chart(14, &["saturday", "sunday"]);
        // Day 0 is Thursday 1970-01-01.
        g.closed_days.insert(1);
        g.open_days.insert(2);
        assert!(!g.is_excluded(0));
        assert!(g.is_excluded(time::DAY));
        // Saturday the 3rd is open; Sunday the 4th is not.
        assert!(!g.is_excluded(2 * time::DAY));
        assert!(g.is_excluded(3 * time::DAY));
        assert_eq!(
            g.excluded_days(),
            [1, 3, 9, 10].map(|d| d * time::DAY).to_vec()
        );
        // Closed days alone grey out days.
        let mut g = chart(3, &[]);
        g.closed_days.insert(2);
        assert_eq!(g.excluded_days(), [2 * time::DAY]);
    }
}
