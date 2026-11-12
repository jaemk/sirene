//! A diagram file and its parsed contents (GUISHE-3, WYSIWY-3..5, WYSIWY-7).

use std::ops::Range;
use std::path::{Path, PathBuf};

use sirene_core::Severity;
use sirene_core::diagram::{self, Diagram, Kind, Lang};
use sirene_core::flowchart::Flowchart;
use sirene_core::markdown::{Block, is_markdown, mermaid_blocks};
use sirene_core::scene::{Scene, TextMetrics, Theme};
use sirene_core::theme::ThemeConfig;

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

/// A Markdown file, edited one mermaid block at a time (WYSIWY-20).
#[derive(Debug, Clone)]
pub struct Markdown {
    /// The whole file's text, with the edits made to its blocks.
    pub text: String,
    /// The mermaid blocks of `text`.
    pub blocks: Vec<Block>,
    /// The block shown, an index into `blocks`.
    pub block: usize,
}

#[derive(Debug, Clone)]
pub struct Document {
    /// The file shown, if any.
    pub path: Option<PathBuf>,
    /// The diagram's text: the file's, or for a Markdown file the shown block's code.
    pub source: String,
    /// The text last read from or written to the file, or the text an Untitled document opened
    /// with.
    pub saved: String,
    /// The Markdown file around the diagram, for a `.md` or `.markdown` path.
    pub markdown: Option<Markdown>,
    /// The diagram as a graph: the one drawn, selected, and edited. Empty for the other types.
    pub chart: Flowchart,
    /// A diagram of a type not lowered to the graph model, drawn read-only (LANGS-2).
    pub other: Option<Diagram>,
    pub problems: Vec<Problem>,
    /// The diagram's own theme settings: Mermaid's frontmatter and directives, or DOT's
    /// `bgcolor`.
    pub theme: ThemeConfig,
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

    /// A document showing `source`, which counts as saved. For a Markdown `path`, `source` is the
    /// file's text and the document shows its first mermaid block.
    pub fn from_source(path: Option<PathBuf>, source: String) -> Self {
        let markdown = path
            .as_deref()
            .filter(|p| is_markdown(p))
            .map(|_| Markdown {
                blocks: mermaid_blocks(&source),
                text: source.clone(),
                block: 0,
            });
        let (code, lang) = match &markdown {
            Some(md) => md
                .blocks
                .first()
                .map_or_else(Default::default, |b| (b.code.clone(), Some(b.lang))),
            None => (source.clone(), path.as_deref().and_then(Lang::of_path)),
        };
        let (chart, other, problems, theme) = parse(&code, lang);
        Self {
            path,
            saved: source,
            source: code,
            markdown,
            chart,
            other,
            problems,
            theme,
            read_error: None,
            write_error: None,
            disk_changed: false,
        }
    }

    /// The language the shown diagram is read as: its Markdown block's, or its file extension's,
    /// and otherwise detected from the source (DOT-5).
    pub fn lang(&self) -> Option<Lang> {
        match &self.markdown {
            Some(md) => md.blocks.get(md.block).map(|b| b.lang),
            None => self.path.as_deref().and_then(Lang::of_path),
        }
    }

    /// Why drawing edits are off for this diagram's type, when they are (LANGS-5).
    pub fn read_only(&self) -> Option<&'static str> {
        match Kind::of_lang(&self.source, self.lang()) {
            Kind::Flowchart => None,
            Kind::Sequence => Some("drawing edits are not supported for sequence diagrams yet"),
            Kind::State => Some("drawing edits are not supported for state diagrams yet"),
            Kind::Class => Some("drawing edits are not supported for class diagrams yet"),
            Kind::Er => Some("drawing edits are not supported for ER diagrams yet"),
            Kind::Git => Some("drawing edits are not supported for git graphs yet"),
            Kind::Timeline => Some("drawing edits are not supported for timelines yet"),
            Kind::Pie => Some("drawing edits are not supported for pie charts yet"),
            Kind::Gantt => Some("drawing edits are not supported for gantt charts yet"),
            Kind::Mindmap => Some("drawing edits are not supported for mindmaps yet"),
            Kind::Journey => Some("drawing edits are not supported for user journeys yet"),
            Kind::Quadrant => Some("drawing edits are not supported for quadrant charts yet"),
            Kind::XyChart => Some("drawing edits are not supported for XY charts yet"),
            Kind::Kanban => Some("drawing edits are not supported for kanban boards yet"),
            Kind::Packet => Some("drawing edits are not supported for packet diagrams yet"),
            Kind::Radar => Some("drawing edits are not supported for radar charts yet"),
            Kind::Sankey => Some("drawing edits are not supported for sankey diagrams yet"),
            Kind::Treemap => Some("drawing edits are not supported for treemaps yet"),
            Kind::Block => Some("drawing edits are not supported for block diagrams yet"),
            Kind::Architecture => {
                Some("drawing edits are not supported for architecture diagrams yet")
            }
            Kind::C4 => Some("drawing edits are not supported for C4 diagrams yet"),
            Kind::Info => Some("info diagrams have nothing to edit"),
            Kind::Ishikawa => Some("drawing edits are not supported for ishikawa diagrams yet"),
            Kind::Venn => Some("drawing edits are not supported for venn diagrams yet"),
            Kind::Wardley => Some("drawing edits are not supported for Wardley maps yet"),
            Kind::TreeView => Some("drawing edits are not supported for tree views yet"),
            Kind::UseCase => Some("drawing edits are not supported for use case diagrams yet"),
            Kind::Railroad => Some("drawing edits are not supported for railroad diagrams yet"),
            Kind::Swimlane => Some("drawing edits are not supported for swimlane diagrams yet"),
            Kind::Cynefin => Some("drawing edits are not supported for Cynefin diagrams yet"),
            Kind::EventModel => {
                Some("drawing edits are not supported for event modeling diagrams yet")
            }
            Kind::Agentflow => Some("drawing edits are not supported for agentflow diagrams yet"),
            Kind::ZenUml => Some("drawing edits are not supported for ZenUML diagrams yet"),
            Kind::Dot => Some("drawing edits are not supported for DOT yet"),
            Kind::PlantUml => Some("drawing edits are not supported for PlantUML yet"),
            Kind::Requirement => {
                Some("drawing edits are not supported for requirement diagrams yet")
            }
        }
    }

    /// The drawing of whichever diagram the document holds.
    pub fn scene(&self, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
        // The diagram's own theme settings win over the app's (MTHEME-1).
        let theme = &self.theme.apply(theme);
        match &self.other {
            Some(d) => d.scene(metrics, theme),
            None => sirene_core::scene::flowchart(&self.chart, metrics, theme),
        }
    }

    /// The text a save writes: the diagram's, or the whole Markdown file's.
    pub fn text(&self) -> &str {
        match &self.markdown {
            Some(md) => &md.text,
            None => &self.source,
        }
    }

    /// Shows mermaid block `i` of a Markdown file. Returns whether the shown block changed.
    pub fn show_block(&mut self, i: usize) -> bool {
        let Some(md) = &mut self.markdown else {
            return false;
        };
        let Some(block) = md.blocks.get(i) else {
            return false;
        };
        if i == md.block {
            return false;
        }
        md.block = i;
        self.source = block.code.clone();
        (self.chart, self.other, self.problems, self.theme) = parse(&self.source, Some(block.lang));
        true
    }

    /// For a Markdown file: the shown block (from 1), the number of blocks, and the shown
    /// block's first line in the file.
    pub fn block_info(&self) -> Option<(usize, usize, usize)> {
        let md = self.markdown.as_ref()?;
        let block = md.blocks.get(md.block)?;
        Some((md.block + 1, md.blocks.len(), block.line))
    }

    /// Reads and parses `path`. A file that cannot be read gives an empty document that
    /// records the error.
    pub fn open(path: &Path) -> Self {
        let doc = Self::read(path);
        match &doc.read_error {
            Some(e) => log::debug!("{e}"),
            None => log::debug!(
                "opened {}: {} bytes, {} problems",
                path.display(),
                doc.source.len(),
                doc.problems.len()
            ),
        }
        doc
    }

    fn read(path: &Path) -> Self {
        let (text, error) = Self::read_text(path);
        Self {
            read_error: error,
            ..Self::from_source(Some(path.to_owned()), text)
        }
    }

    /// The file's text, or an empty text and the read error.
    fn read_text(path: &Path) -> (String, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => (text, None),
            Err(e) => (
                String::new(),
                Some(format!("cannot read {}: {e}", path.display())),
            ),
        }
    }

    /// Replaces the text and re-parses it, returning whether it changed.
    pub fn set_source(&mut self, source: String) -> bool {
        if source == self.source {
            return false;
        }
        (self.chart, self.other, self.problems, self.theme) = parse(&source, self.lang());
        if let Some(md) = &mut self.markdown {
            // The code goes back in its block; a file with no block gets one at the end.
            md.text = match md.blocks.get(md.block) {
                Some(block) => block.replace_code(&md.text, &source),
                None => {
                    let mut text = md.text.clone();
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    let code = source.trim_end_matches('\n');
                    text.push_str(&format!("```mermaid\n{code}\n```\n"));
                    text
                }
            };
            md.blocks = mermaid_blocks(&md.text);
            md.block = md.block.min(md.blocks.len().saturating_sub(1));
        }
        self.source = source;
        true
    }

    /// Whether the text differs from the saved text (WYSIWY-3).
    pub fn is_edited(&self) -> bool {
        self.text() != self.saved
    }

    /// Checks the file for changes (WYSIWY-5): an unedited document takes the new text, an
    /// edited one keeps its text and records the conflict.
    pub fn poll(&mut self) -> Disk {
        let Some(path) = &self.path else {
            return Disk::Unchanged;
        };
        // Compare the text before parsing it: this runs every 500ms.
        let (text, error) = Self::read_text(path);
        if text == self.saved && error == self.read_error {
            return Disk::Unchanged;
        }
        let next = Self {
            read_error: error,
            ..Self::from_source(Some(path.clone()), text)
        };
        if !self.is_edited() {
            log::debug!(
                "{} changed on disk; took the new text ({} bytes){}",
                path.display(),
                next.source.len(),
                next.read_error
                    .as_deref()
                    .map_or_else(String::new, |e| format!(": {e}"))
            );
            self.replace(next);
            return Disk::Reloaded;
        }
        if self.disk_changed {
            return Disk::Unchanged;
        }
        log::debug!(
            "{} changed on disk under unsaved edits; keeping the edits",
            path.display()
        );
        self.disk_changed = true;
        Disk::Conflict
    }

    /// Re-reads the file, dropping unsaved edits. Returns whether the text changed.
    pub fn reload(&mut self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        log::debug!("reloading {}", path.display());
        let next = Self::open(path);
        let before = self.source.clone();
        self.replace(next);
        self.source != before
    }

    /// Becomes `next`, a new reading of the same file, still showing the same Markdown block.
    fn replace(&mut self, next: Self) {
        let block = self.markdown.as_ref().map_or(0, |md| md.block);
        *self = next;
        self.show_block(block);
    }

    /// Writes the text to the file through a temporary file in the same directory
    /// (WYSIWY-4). On failure the document stays edited and records the error.
    pub fn save(&mut self) -> Result<(), String> {
        let result = match &self.path {
            Some(path) => write_atomic(path, self.text())
                .map_err(|e| format!("cannot write {}: {e}", path.display())),
            None => Err("no file to save to".to_owned()),
        };
        match &result {
            Ok(()) => log::debug!("saved {} bytes to {}", self.text().len(), self.name()),
            Err(e) => log::debug!("save failed: {e}"),
        }
        match &result {
            Ok(()) => {
                self.saved = self.text().to_owned();
                self.read_error = None;
                self.write_error = None;
                self.disk_changed = false;
            }
            Err(e) => self.write_error = Some(e.clone()),
        }
        result
    }

    /// Saves to `path`, which the document then shows. Saving a diagram as Markdown writes it in
    /// a mermaid block; saving a Markdown file's block as a diagram writes the block alone.
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        match (&self.markdown, is_markdown(&path)) {
            (Some(_), false) => self.markdown = None,
            (None, true) => {
                let code = self.source.trim_end_matches('\n');
                let text = format!("```mermaid\n{code}\n```\n");
                self.markdown = Some(Markdown {
                    blocks: mermaid_blocks(&text),
                    text,
                    block: 0,
                });
            }
            _ => {}
        }
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
        if self
            .markdown
            .as_ref()
            .is_some_and(|md| md.blocks.is_empty())
        {
            return format!(
                "{} has no mermaid blocks; editing adds one at the end",
                self.name()
            );
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

/// The graph (empty for other types), any other diagram, the problems in `source`, and its theme
/// settings, read as `lang` when given.
fn parse(
    source: &str,
    lang: Option<Lang>,
) -> (Flowchart, Option<Diagram>, Vec<Problem>, ThemeConfig) {
    let lowered = diagram::lower_as(source, lang);
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
    match lowered.diagram {
        Diagram::Graph(chart) => (chart, None, problems, lowered.theme),
        other => (Flowchart::default(), Some(other), problems, lowered.theme),
    }
}

/// Writes `text` to a temporary file beside `path`, then renames it over `path`. A symlink is
/// followed, so the link stays and its target changes; the file keeps its permissions, owner, and
/// group; and the text is on disk before the rename. A read-only file is refused. When the owner
/// cannot be kept (another user's file), the file is written in place instead.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    write_atomic_with(path, text, keep_owner)
}

/// Gives `file` the owner and group in `meta`, when they differ.
#[cfg(unix)]
fn keep_owner(file: &std::fs::File, meta: &std::fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let own = file.metadata()?;
    if (own.uid(), own.gid()) == (meta.uid(), meta.gid()) {
        return Ok(());
    }
    std::os::unix::fs::fchown(file, Some(meta.uid()), Some(meta.gid()))
}

#[cfg(not(unix))]
fn keep_owner(_: &std::fs::File, _: &std::fs::Metadata) -> std::io::Result<()> {
    Ok(())
}

fn write_atomic_with(
    path: &Path,
    text: &str,
    keep_owner: impl Fn(&std::fs::File, &std::fs::Metadata) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::io::Write;

    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    log::debug!("writing {}", path.display());
    let meta = std::fs::metadata(&path).ok();
    if meta.as_ref().is_some_and(|m| m.permissions().readonly()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the file is read-only",
        ));
    }
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!(".{name}.sirene-save"));
    // A leftover temporary file, or anything planted there, is replaced, never written through.
    let _ = std::fs::remove_file(&tmp);
    // `Ok(false)`: the owner cannot be kept.
    let staged = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        if let Some(meta) = &meta {
            file.set_permissions(meta.permissions())?;
            if keep_owner(&file, meta).is_err() {
                return Ok(false);
            }
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(true)
    })();
    let result = match staged {
        Ok(true) => {
            log::debug!("renaming {} over it", tmp.display());
            std::fs::rename(&tmp, &path)
        }
        Ok(false) => {
            log::debug!("cannot keep the file's owner; writing in place");
            let _ = std::fs::remove_file(&tmp);
            write_in_place(&path, text)
        }
        Err(e) => Err(e),
    };
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Overwrites `path` itself, keeping its inode and so its owner.
fn write_in_place(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
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

    /// The drawing takes the diagram's own theme: DOT's `bgcolor` as well as Mermaid's settings,
    /// and an edit re-reads it.
    #[test]
    fn scene_uses_the_diagram_theme() {
        use sirene_core::scene::{Approx, Color};
        let mut d = Document::from_source(None, "digraph { bgcolor=red; a }".into());
        let light = Theme::LIGHT;
        assert_eq!(d.scene(&Approx, &light).background, Color::rgb(0xff0000));
        d.set_source("digraph { a }".into());
        assert_eq!(d.scene(&Approx, &light).background, light.background);
        d.set_source(
            "%%{init: {'themeVariables': {'background': '#00ff00'}}}%%\nflowchart\nA\n".into(),
        );
        assert_eq!(d.scene(&Approx, &light).background, Color::rgb(0x00ff00));
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

    /// A save keeps the file's mode, writes through a symlink, and never writes through
    /// whatever sits at the temporary name.
    #[cfg(unix)]
    #[test]
    fn saves_keep_modes_and_links() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.mmd");
        std::fs::write(&target, "flowchart LR\nA\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("link.mmd");
        symlink(&target, &link).unwrap();
        // Planted at the temporary name: must not receive the text.
        let victim = dir.path().join("victim");
        std::fs::write(&victim, "keep").unwrap();
        symlink(&victim, dir.path().join(".real.mmd.sirene-save")).unwrap();

        let mut d = Document::open(&link);
        d.set_source("flowchart LR\nA --> B\n".into());
        assert_eq!(d.save(), Ok(()));
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "flowchart LR\nA --> B\n"
        );
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["link.mmd", "real.mmd", "victim"]);
        assert_eq!(d.poll(), Disk::Unchanged);
    }

    /// A read-only file is refused and left alone.
    #[test]
    fn read_only_files_are_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA\n").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();
        let mut d = Document::open(&path);
        d.set_source("flowchart LR\nA --> B\n".into());
        let err = d.save().unwrap_err();
        assert_eq!(
            err,
            format!("cannot write {}: the file is read-only", path.display())
        );
        assert!(d.is_edited());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "flowchart LR\nA\n");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    /// When the owner cannot be kept, the file is overwritten in place: same inode, so same
    /// owner, and no temporary file left.
    #[cfg(unix)]
    #[test]
    fn saves_in_place_when_the_owner_cannot_be_kept() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA --> B --> C\n").unwrap();
        let inode = std::fs::metadata(&path).unwrap().ino();
        let refuse = |_: &std::fs::File, _: &std::fs::Metadata| {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        };
        write_atomic_with(&path, "flowchart LR\nA\n", refuse).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "flowchart LR\nA\n");
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        // A normal save replaces the inode, keeping the owner.
        write_atomic(&path, "flowchart LR\nB\n").unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert_ne!(meta.ino(), inode);
        let dir_meta = std::fs::metadata(dir.path()).unwrap();
        assert_eq!((meta.uid(), meta.gid()), (dir_meta.uid(), dir_meta.gid()));

        // The in-place write's own failure is reported.
        let gone = dir.path().join("gone").join("a.mmd");
        assert!(write_in_place(&gone, "x").is_err());
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
