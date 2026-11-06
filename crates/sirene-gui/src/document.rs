//! A diagram file and its parsed contents (GUISHE-3).

use std::path::{Path, PathBuf};

use sirene_core::Severity;
use sirene_core::flowchart::{self, Flowchart};

/// A diagnostic located by 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
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

#[derive(Debug, Clone)]
pub struct Document {
    /// The file shown, if any.
    pub path: Option<PathBuf>,
    pub source: String,
    pub chart: Flowchart,
    pub problems: Vec<Problem>,
    /// Why the file could not be read, when it could not.
    pub read_error: Option<String>,
}

impl Document {
    /// A document with no file.
    pub fn empty() -> Self {
        Self::from_source(None, String::new())
    }

    pub fn from_source(path: Option<PathBuf>, source: String) -> Self {
        let lowered = flowchart::parse(&source).lower();
        let problems = lowered
            .diagnostics
            .iter()
            .map(|d| {
                let (line, column) = d.line_col(&source);
                Problem {
                    line,
                    column,
                    severity: d.severity,
                    message: d.message.clone(),
                }
            })
            .collect();
        Self {
            path,
            source,
            chart: lowered.flowchart,
            problems,
            read_error: None,
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

    /// Re-reads the file, returning whether anything changed.
    pub fn reload(&mut self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        let next = Self::open(path);
        if next.source == self.source && next.read_error == self.read_error {
            return false;
        }
        *self = next;
        true
    }

    /// The file name, or `Untitled`.
    pub fn name(&self) -> String {
        self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// One line summarizing problems: the read error, or the first diagnostic and a count.
    pub fn status(&self) -> Option<String> {
        if let Some(e) = &self.read_error {
            return Some(e.clone());
        }
        let first = self
            .problems
            .iter()
            .find(|p| p.severity == Severity::Error)
            .or(self.problems.first())?;
        let more = self.problems.len() - 1;
        Some(if more == 0 {
            first.to_string()
        } else {
            format!("{first} (+{more} more)")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document() {
        let d = Document::empty();
        assert_eq!(d.name(), "Untitled");
        assert!(d.chart.nodes.is_empty());
        assert_eq!(
            d.status(),
            Some("1:1: error: expected `flowchart` or `graph` header".into())
        );
    }

    #[test]
    fn opens_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA --> B\n").unwrap();
        let mut d = Document::open(&path);
        assert_eq!(d.name(), "a.mmd");
        assert_eq!(d.chart.nodes.len(), 2);
        assert_eq!(d.status(), None);
        assert!(!d.reload());

        std::fs::write(&path, "flowchart LR\nA --> B --> C\n").unwrap();
        assert!(d.reload());
        assert_eq!(d.chart.nodes.len(), 3);

        std::fs::remove_file(&path).unwrap();
        assert!(d.reload());
        assert!(d.read_error.is_some());
        assert!(d.status().unwrap().starts_with("cannot read "));
        assert!(!d.reload());
    }

    #[test]
    fn status_prefers_errors() {
        let d = Document::from_source(
            None,
            "flowchart\nclick Z cb\nlinkStyle 9 color:red\n".into(),
        );
        let status = d.status().unwrap();
        assert!(status.starts_with("3:"), "{status}");
        assert!(status.ends_with("(+1 more)"), "{status}");
        let d = Document::from_source(None, "flowchart\nclick Z cb\n".into());
        assert_eq!(
            d.status(),
            Some("2:7: warning: `click` refers to unknown node `Z`".into())
        );
        let mut d = Document::from_source(None, "flowchart\n".into());
        assert!(!d.reload());
    }
}
