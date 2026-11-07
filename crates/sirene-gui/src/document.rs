//! A diagram file and its parsed contents (GUISHE-3, WYSIWY-3..5, WYSIWY-7).

use std::ops::Range;
use std::path::{Path, PathBuf};

use sirene_core::Severity;
use sirene_core::flowchart::{self, Flowchart};

/// The text of a new document (WYSIWY-7).
pub const STARTER: &str = "flowchart TD\n    A --> B\n";

/// A diagnostic located by byte range and by 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub range: Range<usize>,
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub message: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(
            f,
            "{}:{}: {severity}: {}",
            self.line, self.column, self.message
        )
    }
}

/// What a poll of the file found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disk {
    /// Nothing new.
    Unchanged,
    /// The document took the file's new text.
    Reloaded,
    /// The file changed under unsaved edits, which were kept.
    Conflict,
}

#[derive(Debug, Clone)]
pub struct Document {
    /// The file shown, if any.
    pub path: Option<PathBuf>,
    pub source: String,
    /// The text last read from or written to the file, or the text an Untitled document opened
    /// with.
    pub saved: String,
    pub chart: Flowchart,
    pub problems: Vec<Problem>,
    /// Why the file could not be read, when it could not.
    pub read_error: Option<String>,
    /// Why the last save failed, until the next save or reload.
    pub write_error: Option<String>,
    /// The file changed on disk while the document had unsaved edits.
    pub disk_changed: bool,
}

impl Document {
    /// A new Untitled document with the starter text.
    pub fn untitled() -> Self {
        Self::from_source(None, STARTER.to_owned())
    }

    /// A document showing `source`, which counts as saved.
    pub fn from_source(path: Option<PathBuf>, source: String) -> Self {
        let (chart, problems) = parse(&source);
        Self {
            path,
            saved: source.clone(),
            source,
            chart,
            problems,
            read_error: None,
            write_error: None,
            disk_changed: false,
        }
    }

    /// Reads and parses `path`. A file that cannot be read gives an empty document that
    /// records the error.
    pub fn open(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(src) => Self::from_source(Some(path.to_owned()), src),
            Err(e) => Self {
                read_error: Some(format!("cannot read {}: {e}", path.display())),
                ..Self::from_source(Some(path.to_owned()), String::new())
            },
        }
    }

    /// Replaces the text and re-parses it, returning whether it changed.
    pub fn set_source(&mut self, source: String) -> bool {
        if source == self.source {
            return false;
        }
        (self.chart, self.problems) = parse(&source);
        self.source = source;
        true
    }

    /// Whether the text differs from the saved text (WYSIWY-3).
    pub fn is_edited(&self) -> bool {
        self.source != self.saved
    }

    /// Checks the file for changes (WYSIWY-5): an unedited document takes the new text, an
    /// edited one keeps its text and records the conflict.
    pub fn poll(&mut self) -> Disk {
        let Some(path) = &self.path else {
            return Disk::Unchanged;
        };
        let next = Self::open(path);
        if next.saved == self.saved && next.read_error == self.read_error {
            return Disk::Unchanged;
        }
        if !self.is_edited() {
            *self = next;
            return Disk::Reloaded;
        }
        if self.disk_changed {
            return Disk::Unchanged;
        }
        self.disk_changed = true;
        Disk::Conflict
    }

    /// Re-reads the file, dropping unsaved edits. Returns whether the text changed.
    pub fn reload(&mut self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        let next = Self::open(path);
        let changed = next.source != self.source;
        *self = next;
        changed
    }

    /// Writes the text to the file through a temporary file in the same directory
    /// (WYSIWY-4). On failure the document stays edited and records the error.
    pub fn save(&mut self) -> Result<(), String> {
        let result = match &self.path {
            Some(path) => write_atomic(path, &self.source)
                .map_err(|e| format!("cannot write {}: {e}", path.display())),
            None => Err("no file to save to".to_owned()),
        };
        match &result {
            Ok(()) => {
                self.saved = self.source.clone();
                self.read_error = None;
                self.write_error = None;
                self.disk_changed = false;
            }
            Err(e) => self.write_error = Some(e.clone()),
        }
        result
    }

    /// Saves to `path`, which the document then shows.
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        self.path = Some(path);
        self.save()
    }

    /// The file name, or `Untitled`.
    pub fn name(&self) -> String {
        self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// The window title: the name, marked when edited.
    pub fn title(&self) -> String {
        if self.is_edited() {
            format!("{} (edited)", self.name())
        } else {
            self.name()
        }
    }

    /// One line for the status bar: a write error, a change on disk, a read error, the first
    /// diagnostic (errors first) and a count, or else the title.
    pub fn status(&self) -> String {
        if let Some(e) = &self.write_error {
            return e.clone();
        }
        if self.disk_changed {
            return format!(
                "{} changed on disk; Reload discards your edits",
                self.name()
            );
        }
        if let Some(e) = &self.read_error {
            return e.clone();
        }
        let first = self
            .problems
            .iter()
            .find(|p| p.severity == Severity::Error)
            .or(self.problems.first());
        match first {
            None => self.title(),
            Some(p) if self.problems.len() == 1 => p.to_string(),
            Some(p) => format!("{p} (+{} more)", self.problems.len() - 1),
        }
    }
}

fn parse(source: &str) -> (Flowchart, Vec<Problem>) {
    let lowered = flowchart::parse(source).lower();
    let problems = lowered
        .diagnostics
        .iter()
        .map(|d| {
            let (line, column) = d.line_col(source);
            Problem {
                range: d.range.into(),
                line,
                column,
                severity: d.severity,
                message: d.message.clone(),
            }
        })
        .collect();
    (lowered.flowchart, problems)
}

/// Writes `text` to a temporary file beside `path`, then renames it over `path`.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy());
    let tmp = path.with_file_name(format!(".{name}.sirene-save"));
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_document() {
        let d = Document::untitled();
        assert_eq!(d.name(), "Untitled");
        assert_eq!(d.title(), "Untitled");
        assert_eq!(d.chart.nodes.len(), 2);
        assert!(!d.is_edited());
        assert_eq!(d.status(), "Untitled");
        let mut d = d;
        assert_eq!(d.poll(), Disk::Unchanged);
        assert!(!d.reload());
        assert_eq!(d.save(), Err("no file to save to".into()));
        assert_eq!(d.status(), "no file to save to");
    }

    #[test]
    fn empty_text_has_an_error() {
        let d = Document::from_source(None, String::new());
        assert!(d.chart.nodes.is_empty());
        assert_eq!(
            d.status(),
            "1:1: error: expected `flowchart` or `graph` header"
        );
        assert_eq!(d.problems[0].range, 0..0);
    }

    #[test]
    fn edits_reparse_and_mark_the_document() {
        let mut d = Document::untitled();
        assert!(!d.set_source(STARTER.into()));
        assert!(d.set_source("flowchart TD\nA --> B --> C\n".into()));
        assert_eq!(d.chart.nodes.len(), 3);
        assert!(d.is_edited());
        assert_eq!(d.title(), "Untitled (edited)");
        assert_eq!(d.status(), "Untitled (edited)");
        assert!(d.set_source(STARTER.into()));
        assert!(!d.is_edited());
    }

    #[test]
    fn opens_and_polls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA --> B\n").unwrap();
        let mut d = Document::open(&path);
        assert_eq!(d.name(), "a.mmd");
        assert_eq!(d.chart.nodes.len(), 2);
        assert_eq!(d.status(), "a.mmd");
        assert_eq!(d.poll(), Disk::Unchanged);

        std::fs::write(&path, "flowchart LR\nA --> B --> C\n").unwrap();
        assert_eq!(d.poll(), Disk::Reloaded);
        assert_eq!(d.chart.nodes.len(), 3);

        std::fs::remove_file(&path).unwrap();
        assert_eq!(d.poll(), Disk::Reloaded);
        assert!(d.read_error.is_some());
        assert!(d.status().starts_with("cannot read "));
        assert_eq!(d.poll(), Disk::Unchanged);
    }

    #[test]
    fn disk_changes_under_edits_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA\n").unwrap();
        let mut d = Document::open(&path);
        d.set_source("flowchart LR\nA --> Mine\n".into());
        std::fs::write(&path, "flowchart LR\nA --> Theirs\n").unwrap();
        assert_eq!(d.poll(), Disk::Conflict);
        assert_eq!(d.source, "flowchart LR\nA --> Mine\n");
        assert_eq!(
            d.status(),
            "a.mmd changed on disk; Reload discards your edits"
        );
        assert_eq!(d.poll(), Disk::Unchanged);

        assert!(d.reload());
        assert_eq!(d.source, "flowchart LR\nA --> Theirs\n");
        assert!(!d.disk_changed);
        assert!(!d.is_edited());
        assert!(!d.reload());
    }

    #[test]
    fn saves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA\n").unwrap();
        let mut d = Document::open(&path);
        d.set_source("flowchart LR\nA --> B\n".into());
        std::fs::write(&path, "flowchart LR\nX\n").unwrap();
        assert_eq!(d.poll(), Disk::Conflict);
        assert_eq!(d.save(), Ok(()));
        assert!(!d.is_edited());
        assert!(!d.disk_changed);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "flowchart LR\nA --> B\n"
        );
        assert_eq!(d.poll(), Disk::Unchanged);
        let leftovers = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1);

        let mut d = Document::untitled();
        let other = dir.path().join("b.mmd");
        assert_eq!(d.save_as(other.clone()), Ok(()));
        assert_eq!(d.name(), "b.mmd");
        assert_eq!(std::fs::read_to_string(&other).unwrap(), STARTER);
    }

    #[test]
    fn failed_saves_keep_the_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Document::untitled();
        d.set_source("flowchart LR\nA --> B\n".into());
        let path = dir.path().join("missing").join("a.mmd");
        let err = d.save_as(path).unwrap_err();
        assert!(err.starts_with("cannot write "), "{err}");
        assert!(d.is_edited());
        assert_eq!(d.status(), err);
        assert_eq!(d.title(), "a.mmd (edited)");
    }

    #[test]
    fn status_prefers_errors() {
        let d = Document::from_source(
            None,
            "flowchart\nclick Z cb\nlinkStyle 9 color:red\n".into(),
        );
        let status = d.status();
        assert!(status.starts_with("3:"), "{status}");
        assert!(status.ends_with("(+1 more)"), "{status}");
        let d = Document::from_source(None, "flowchart\nclick Z cb\n".into());
        assert_eq!(
            d.status(),
            "2:7: warning: `click` refers to unknown node `Z`"
        );
        assert_eq!(d.problems[0].range, 16..17);
    }
}
