#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lang, Lowered};
use sirene_core::scene::Theme;
use sirene_core::scene::png;
use sirene_core::{Error, Result, source};

/// Mermaid, PlantUML, and Graphviz DOT diagram editor and viewer.
///
/// With no command, opens each FILE in an editor window, or a new document with a starter
/// flowchart.
#[derive(Debug, Parser)]
#[command(name = "sirene", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Diagram or Markdown files to edit.
    files: Vec<PathBuf>,
    /// Log steps to stderr; repeat for trace detail (also SIRENE_LOG).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Read every diagram as this language, instead of by its file extension, its Markdown
    /// block's name, or its first line.
    #[arg(long, global = true, value_enum)]
    lang: Option<LangArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangArg {
    Mermaid,
    /// Graphviz DOT.
    #[value(alias = "graphviz")]
    Dot,
    /// PlantUML.
    #[value(alias = "puml")]
    Plantuml,
    /// One TeX formula (`.tex`, `.latex`).
    #[value(alias = "tex")]
    Latex,
    /// A Visio drawing (`.vsdx`).
    #[value(alias = "vsdx")]
    Visio,
}

/// The language `--lang` forces, set once at startup (DOT-5).
static FORCED_LANG: std::sync::OnceLock<Option<Lang>> = std::sync::OnceLock::new();

/// The language to read a diagram as: `--lang`, else `named` (by the file's extension or the
/// Markdown block's info string), else detected from the source.
fn lang(named: Option<Lang>) -> Option<Lang> {
    FORCED_LANG.get().copied().flatten().or(named)
}

/// The page of a multi-page drawing to write, from 1, set once at startup (VISIO-11).
static PAGE: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();

/// What a diagram input held: UTF-8 text, or the bytes of a binary package such as `.vsdx`.
pub(crate) enum Source {
    Text(String),
    Bytes(Vec<u8>),
}

impl Source {
    fn len(&self) -> usize {
        match self {
            Source::Text(s) => s.len(),
            Source::Bytes(b) => b.len(),
        }
    }
}

/// Parses and lowers the diagram read from `path` (stdin for `None`) in its language.
fn lower_file(path: Option<&Path>, src: &Source) -> Lowered {
    let named = lang(path.and_then(Lang::of_path));
    log::trace!("lowering {} bytes with language {named:?}", src.len());
    let lowered = match src {
        Source::Text(s) => diagram::lower_as(s, named),
        Source::Bytes(b) => diagram::lower_bytes(b, named),
    };
    log::debug!("read as a {}", lowered.kind.name());
    lowered
}

// Diagnostics are a presentation boundary. A failed stderr write is remembered so the
// process exits unsuccessfully without attempting to report that failure recursively.
macro_rules! diagnostic {
    ($($args:tt)*) => { crate::output::diagnostic(format_args!($($args)*)) };
}

mod logging;
mod markdown;
mod output;
mod update;

#[derive(Debug, Subcommand)]
enum Command {
    /// Check diagrams for errors without drawing them; prints only diagnostics.
    Check {
        /// Diagram files, directories of them, or Markdown files (their diagram blocks); `-`
        /// for stdin.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Read stdin and files as Markdown, whatever their names.
        #[arg(long)]
        markdown: bool,
        /// Print the diagnostics to stdout as one JSON array instead of to stderr.
        #[arg(long)]
        json: bool,
    },
    /// Print a diagram, or each diagram block of a Markdown file, as plain ASCII text.
    Ascii(TextArgs),
    /// Print a diagram, or each diagram block of a Markdown file, as text with box-drawing
    /// characters (`ascii --charset unicode`).
    Text(TextArgs),
    /// Print a diagram as SVG, or write one per diagram in a directory or Markdown file.
    Svg {
        /// Diagram file or directory to read; stdin when omitted or `-`. A directory's diagram
        /// files (`.mmd`, `.puml`, `.dot`, ...) are written as `.svg` beside them, or under
        /// --output. A Markdown file's diagram blocks are written as NAME-1.svg, NAME-2.svg, ...
        file: Option<PathBuf>,
        /// Write again each time an input changes, until interrupted.
        #[arg(short, long)]
        watch: bool,
        /// For a Markdown file: add a link to each image after its block. The blocks are kept.
        #[arg(long)]
        link: bool,
        /// Colors to draw with.
        #[arg(long, value_enum, default_value_t = ThemeArg::Light)]
        theme: ThemeArg,
        /// Write the SVG here instead of stdout (`-` is stdout); for a directory, the directory to
        /// write into.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// For a Visio drawing: write page N (from 1). Without it each page is written as
        /// STEM-N.svg beside --output.
        #[arg(long, value_name = "N")]
        page: Option<usize>,
    },
    /// Write a diagram as a PNG image, or one per diagram in a directory or Markdown file.
    Png {
        /// Diagram file or directory to read; stdin when omitted or `-`. A directory's diagram
        /// files (`.mmd`, `.puml`, `.dot`, ...) are written as `.png` beside them, or under
        /// --output. A Markdown file's diagram blocks are written as NAME-1.png, NAME-2.png, ...
        file: Option<PathBuf>,
        /// Write again each time an input changes, until interrupted.
        #[arg(short, long)]
        watch: bool,
        /// For a Markdown file: add a link to each image after its block. The blocks are kept.
        #[arg(long)]
        link: bool,
        /// Colors to draw with.
        #[arg(long, value_enum, default_value_t = ThemeArg::Light)]
        theme: ThemeArg,
        /// Detail: draft (1x, no anti-aliasing), normal (2x), or fine (4x, exact text outlines).
        #[arg(long, value_enum, default_value_t = QualityArg::Fine)]
        quality: QualityArg,
        /// Pixels per diagram unit, instead of the quality's own.
        #[arg(long)]
        scale: Option<f32>,
        /// Where to write the PNG (`-` is stdout); defaults to FILE with a .png extension, or
        /// stdout for stdin. For a directory, the directory to write into.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// For a Visio drawing: write page N (from 1). Without it each page is written as
        /// STEM-N.png beside the output.
        #[arg(long, value_name = "N")]
        page: Option<usize>,
    },
    /// Replace this sirene with the latest release, or the one --to names.
    Update {
        /// Print whether an update is available, and change nothing.
        #[arg(long)]
        check: bool,
        /// Update without asking.
        #[arg(short, long)]
        yes: bool,
        /// The release to install (e.g. 0.2.0), instead of the latest. Never downgrades.
        #[arg(long, value_name = "VERSION")]
        to: Option<String>,
    },
}

/// The options of `ascii` and `text`, which differ only in the default charset.
#[derive(Debug, clap::Args)]
struct TextArgs {
    /// Diagram or Markdown file to read; stdin when omitted or `-`.
    file: Option<PathBuf>,
    /// Read stdin or FILE as Markdown, whatever its name.
    #[arg(long)]
    markdown: bool,
    /// Characters to draw with (default: ascii for `ascii`, unicode for `text`).
    #[arg(long, value_enum)]
    charset: Option<CharsetArg>,
    /// Fail when a drawing is wider than N columns. It is still printed.
    #[arg(long, value_name = "N")]
    width: Option<usize>,
    /// Print again each time FILE changes, until interrupted.
    #[arg(short, long)]
    watch: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CharsetArg {
    /// Box-drawing characters; formulas use only characters most monospace fonts draw in one cell.
    Unicode,
    /// Printable ASCII only, formulas included.
    Ascii,
    /// As unicode, with mathematical symbols, script letters, and bracket pieces in formulas,
    /// which some fonts draw wider than a cell.
    UnicodeMath,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum QualityArg {
    Draft,
    Normal,
    Fine,
}

impl QualityArg {
    fn quality(self) -> png::Quality {
        match self {
            QualityArg::Draft => png::Quality::Draft,
            QualityArg::Normal => png::Quality::Normal,
            QualityArg::Fine => png::Quality::Fine,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ThemeArg {
    Light,
    Dark,
}

impl ThemeArg {
    fn theme(self) -> Theme {
        match self {
            ThemeArg::Light => Theme::LIGHT,
            ThemeArg::Dark => Theme::DARK,
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = e.exit_code() as u8;
            return match e.print() {
                Ok(()) => ExitCode::from(code),
                Err(e) => {
                    diagnostic!(
                        "error: {}",
                        Error::with_source("cannot write command-line help or diagnostics", e)
                            .report()
                    );
                    ExitCode::FAILURE
                }
            };
        }
    };
    if let Err(e) = logging::init(cli.verbose) {
        diagnostic!("error: {}", e.report());
        return ExitCode::from(2);
    }
    let _ = FORCED_LANG.set(cli.lang.map(|l| match l {
        LangArg::Mermaid => Lang::Mermaid,
        LangArg::Dot => Lang::Dot,
        LangArg::Plantuml => Lang::PlantUml,
        LangArg::Latex => Lang::Latex,
        LangArg::Visio => Lang::Visio,
    }));
    log::debug!(
        "sirene {} ({}, {}): {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        cli.command
            .as_ref()
            .map_or_else(|| format!("open {:?}", cli.files), |c| format!("{c:?}"))
    );
    let code = run(cli);
    let status = if code == ExitCode::SUCCESS {
        "success"
    } else {
        "failure"
    };
    log::info!("exit with {status}");
    if output::failed() {
        ExitCode::FAILURE
    } else {
        code
    }
}

/// The subcommand a missing FILE was probably meant to be, e.g. `asci` for `ascii`.
fn misspelled_command(files: &[PathBuf]) -> Option<(String, &'static str)> {
    let first = files.first()?;
    let name = first.to_str()?;
    if first.exists() || name.contains(['/', '\\', '.']) {
        return None;
    }
    // One edit from a short command (so `pdf` is not `png`), two from a longer one.
    ["ascii", "svg", "png", "check", "update", "help"]
        .into_iter()
        .find(|cmd| edits(name, cmd) <= if cmd.len() < 5 { 1 } else { 2 })
        .map(|cmd| (name.to_owned(), cmd))
}

/// The edits between `a` and `b`: insertions, deletions, substitutions, and swaps of two
/// neighbors (optimal string alignment distance).
fn edits(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[a.len()][b.len()]
}

/// `ascii` and `text`: print a diagram as text, drawn with `default` unless `--charset` says
/// otherwise.
fn print_text(args: TextArgs, default: CharsetArg) -> ExitCode {
    let TextArgs {
        file,
        markdown,
        charset,
        width,
        watch,
    } = args;

    let charset = match charset.unwrap_or(default) {
        CharsetArg::Unicode => Charset::Unicode,
        CharsetArg::Ascii => Charset::Ascii,
        CharsetArg::UnicodeMath => Charset::UnicodeMath,
    };
    let stdin = PathBuf::from("-");
    let md = if markdown {
        Some(file.as_ref().unwrap_or(&stdin)).filter(|f| !f.is_dir())
    } else {
        markdown_file(&file)
    };
    if let Some(md) = md {
        if watch && md.as_os_str() == "-" {
            diagnostic!("error: --watch needs a FILE or directory, not stdin");
            return ExitCode::FAILURE;
        }
        return markdown_blocks(md, watch, || markdown::print_ascii(md, charset, width));
    }
    let wide = std::cell::Cell::new(false);
    let name = file
        .as_deref()
        .filter(|f| f.as_os_str() != "-")
        .map_or_else(|| "<stdin>".to_owned(), |f| f.display().to_string());
    let code = convert(file, None, None, watch, |l| {
        let drawing = l.ascii(&Options { charset })?;
        if too_wide(&name, &drawing, width) {
            wide.set(true);
        }
        Ok(drawing.into_bytes())
    });
    if wide.get() && code == ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }
    code
}

fn run(cli: Cli) -> ExitCode {
    if cli.command.is_none()
        && let Some((name, cmd)) = misspelled_command(&cli.files)
    {
        diagnostic!(
            "error: no file or command named '{name}'; did you mean 'sirene {cmd}'? \
             (to open a new file with that name, use './{name}')"
        );
        return ExitCode::from(2);
    }
    match cli.command {
        None => gui(cli.files),
        Some(Command::Update { check, yes, to }) => update::run(update::Args { check, yes, to }),
        Some(Command::Check {
            files,
            markdown,
            json,
        }) => {
            log::info!("checking {} input(s)", files.len());
            let mut out = if json {
                markdown::Out::json()
            } else {
                markdown::Out::text()
            };
            // Every file is checked, whatever the ones before it had.
            let failed = files
                .iter()
                .filter(|f| !markdown::check(&mut out, f, markdown))
                .count();
            if let Err(e) = out.finish() {
                diagnostic!(
                    "error: {}",
                    Error::with_source("cannot write <stdout>", e).report()
                );
                return ExitCode::FAILURE;
            }
            if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(Command::Ascii(args)) => print_text(args, CharsetArg::Ascii),
        Some(Command::Text(args)) => print_text(args, CharsetArg::Unicode),
        Some(Command::Svg {
            file,
            watch,
            link,
            theme,
            output,
            page,
        }) => {
            let _ = PAGE.set(page);
            let render = |l: &Lowered| l.svg(&theme.theme()).map(String::into_bytes);
            let output = match stdout_dash(&file, output) {
                Ok(output) => output,
                Err(e) => {
                    diagnostic!("error: {}", e.report());
                    return ExitCode::from(2);
                }
            };
            if let Some(md) = markdown_file(&file) {
                return markdown_blocks(md, watch, || {
                    markdown::render(md, output.as_deref(), "svg", link, &render)
                });
            }
            convert(file, output, Some("svg"), watch, render)
        }
        Some(Command::Png {
            file,
            watch,
            link,
            theme,
            quality,
            scale,
            output,
            page,
        }) => {
            let _ = PAGE.set(page);
            let quality = quality.quality();
            let render = |l: &Lowered| l.png(&theme.theme(), quality, scale);
            let to_stdout = output.as_deref() == Some(Path::new("-"));
            let output = match stdout_dash(&file, output) {
                Ok(output) => output,
                Err(e) => {
                    diagnostic!("error: {}", e.report());
                    return ExitCode::from(2);
                }
            };
            if let Some(md) = markdown_file(&file) {
                return markdown_blocks(md, watch, || {
                    markdown::render(md, output.as_deref(), "png", link, &render)
                });
            }
            if file.as_ref().is_some_and(|f| f.is_dir()) {
                return convert(file, output, Some("png"), watch, render);
            }
            let output = output.or_else(|| {
                let default = file
                    .as_ref()
                    .filter(|f| f.as_os_str() != "-" && !to_stdout)
                    .map(|f| f.with_extension("png"));
                if let Some(d) = &default {
                    log::debug!("no --output; writing next to the input as {}", d.display());
                }
                default
            });
            if output.is_none() && std::io::stdout().is_terminal() {
                diagnostic!("error: refusing to write a PNG to a terminal; use --output");
                return ExitCode::FAILURE;
            }
            convert(file, output, Some("png"), watch, render)
        }
    }
}

/// Whether `drawing` is wider than `width` columns, reporting it when so (CLI-14).
fn too_wide(name: &str, drawing: &str, width: Option<usize>) -> bool {
    let Some(limit) = width else {
        return false;
    };
    let columns = sirene_core::ascii::columns(drawing);
    if columns <= limit {
        return false;
    }
    diagnostic!("error: {name}: the drawing is {columns} columns wide, over --width {limit}");
    true
}

/// `output` with `-` read as stdout (`None`), as `-` is stdin for FILE; an error for a directory
/// or Markdown `file`, which write many images.
fn stdout_dash(file: &Option<PathBuf>, output: Option<PathBuf>) -> Result<Option<PathBuf>> {
    if output.as_deref() != Some(Path::new("-")) {
        return Ok(output);
    }
    if markdown_file(file).is_some() || file.as_ref().is_some_and(|f| f.is_dir()) {
        return Err(Error::new(
            "`--output -` writes one diagram to stdout; give a directory instead",
        ));
    }
    Ok(None)
}

/// `file` when it names a Markdown file (a directory named like one is a directory).
fn markdown_file(file: &Option<PathBuf>) -> Option<&PathBuf> {
    file.as_ref()
        .filter(|f| sirene_core::markdown::is_markdown(f) && !f.is_dir())
}

/// Runs `run` on Markdown file `md`'s diagram blocks (CLI-12); with `watch`, again whenever it
/// changes, never returning.
fn markdown_blocks(md: &Path, watch: bool, run: impl Fn() -> Done) -> ExitCode {
    if !watch {
        let done = run();
        return if done.written && done.clean {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    diagnostic!("watching {} (Ctrl-C to stop)", md.display());
    // The first pass always runs, so a missing file is reported.
    let mut seen = None;
    loop {
        markdown_watch_pass(md, &mut seen, &run, Instant::now());
        if output::failed() {
            return ExitCode::FAILURE;
        }
        std::thread::sleep(POLL);
    }
}

/// Retry failed Markdown renders without requiring another source edit. Record the stamp after
/// each pass because adding links can itself change the Markdown file, even on a partial failure.
fn markdown_watch_pass(
    md: &Path,
    seen: &mut Option<WatchEntry>,
    run: &impl Fn() -> Done,
    clock: Instant,
) {
    let now = stamp(md);
    let same = seen.as_ref().is_some_and(|entry| entry.stamp == now);
    if same
        && seen
            .as_ref()
            .is_some_and(|entry| entry.retry.is_none_or(|(at, _)| clock < at))
    {
        return;
    }
    let retry = if run().written {
        None
    } else {
        let delay = if same {
            seen.as_ref()
                .and_then(|entry| {
                    entry
                        .retry
                        .map(|(_, delay)| (delay * 2).min(Duration::from_secs(30)))
                })
                .unwrap_or(Duration::from_secs(2))
        } else {
            Duration::from_secs(2)
        };
        Some((clock + delay, delay))
    };
    *seen = Some(WatchEntry {
        stamp: stamp(md),
        missing: false,
        retry,
    });
}

#[cfg(feature = "gui")]
fn gui(files: Vec<PathBuf>) -> ExitCode {
    log::debug!("starting the GUI");
    sirene_gui::run(files);
    ExitCode::SUCCESS
}

#[cfg(not(feature = "gui"))]
fn gui(_: Vec<PathBuf>) -> ExitCode {
    log::debug!("this binary was built without the `gui` feature");
    diagnostic!("error: built without the GUI; see sirene --help");
    ExitCode::FAILURE
}

/// Whether `a` and `b` name the same existing file.
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Reads the source named on the command line: a file, or stdin for `None` or `-`.
fn read(file: Option<PathBuf>) -> Result<(String, String)> {
    match file.filter(|f| f.as_os_str() != "-") {
        Some(path) => {
            let name = path.display().to_string();
            log::debug!("reading {name}");
            read_text(&path).map(|src| (name, src))
        }
        None => {
            log::debug!("reading <stdin>");
            source::read_text(std::io::stdin().lock())
                .map(|src| ("<stdin>".to_owned(), src))
                .map_err(|e| e.context("cannot read <stdin>"))
        }
    }
}

/// Reads the source named on the command line as text, or as bytes when it is a package: forced
/// or named as Visio, or starting with the ZIP magic (VISIO-12).
fn read_source(file: Option<PathBuf>) -> Result<(String, Source)> {
    let named = lang(file.as_deref().and_then(Lang::of_path));
    let (name, context, reader): (String, String, Box<dyn Read>) =
        match file.filter(|f| f.as_os_str() != "-") {
            Some(path) => {
                let name = path.display().to_string();
                log::debug!("reading {name}");
                let context = format!("cannot read {name}");
                let f = std::fs::File::open(&path)
                    .map_err(|e| Error::with_source(context.clone(), e))?;
                (name, context, Box::new(f))
            }
            None => {
                log::debug!("reading <stdin>");
                (
                    "<stdin>".to_owned(),
                    "cannot read <stdin>".to_owned(),
                    Box::new(std::io::stdin().lock()),
                )
            }
        };
    read_diagram_source(reader, named)
        .map(|source| (name, source))
        .map_err(|e| e.context(context))
}

fn read_diagram_source(reader: impl Read, named: Option<Lang>) -> Result<Source> {
    let bytes = source::read_bytes(reader)?;
    if named == Some(Lang::Visio) || (named.is_none() && bytes.starts_with(b"PK\x03\x04")) {
        Ok(Source::Bytes(bytes))
    } else {
        source::text(bytes).map(Source::Text)
    }
}

/// Reads the file at `path` as UTF-8 text, with an error naming it.
pub(crate) fn read_text(path: &Path) -> Result<String> {
    source::read(path)
}

/// Missing output files are expected; other metadata failures are not permission to replace them.
fn metadata(path: &Path) -> std::io::Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over `path`, so a failed
/// write leaves the old file whole. The file keeps its permissions. A symlink at `path` is
/// replaced, not written through, unless `follow`, when its target is written instead. Anything
/// other than a regular file, such as `/dev/stdout`, is written in place.
pub(crate) fn write_file(path: &Path, bytes: &[u8], follow: bool) -> std::io::Result<()> {
    let link = metadata(path)?;
    let path = match &link {
        Some(m) if m.file_type().is_symlink() && follow => std::fs::canonicalize(path)?,
        _ => path.to_owned(),
    };
    let meta = metadata(&path)?;
    if meta
        .as_ref()
        .is_some_and(|m| !m.is_file() && !m.file_type().is_symlink())
    {
        return std::fs::write(&path, bytes);
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".sirene-");
    #[cfg(unix)]
    if !meta.as_ref().is_some_and(|m| m.is_file()) {
        use std::os::unix::fs::PermissionsExt;
        // Match ordinary output-file creation, including the caller's umask.
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut tmp = builder.tempfile_in(parent)?;
    if let Some(m) = meta.as_ref().filter(|m| m.is_file()) {
        tmp.as_file().set_permissions(m.permissions())?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)
}

/// How often `--watch` looks for changes.
const POLL: Duration = Duration::from_millis(500);

/// Renders `file` (stdin when `None` or `-`) to `output` (stdout when `None`), or with an
/// extension, every diagram under a directory `file` to the same relative path under `output`
/// (or beside it). With `watch`, renders again whenever an input changes and never returns.
fn convert(
    file: Option<PathBuf>,
    output: Option<PathBuf>,
    ext: Option<&str>,
    watch: bool,
    render: impl Fn(&Lowered) -> Result<Vec<u8>>,
) -> ExitCode {
    let file = file.filter(|f| f.as_os_str() != "-");
    let dir = file.clone().filter(|f| f.is_dir());
    let jobs = || match (&dir, ext) {
        (Some(d), Some(ext)) => {
            let scanned = scan(d, output.as_deref().unwrap_or(d), ext);
            let (jobs, mut skipped) = safe_batch(scanned.found);
            skipped.extend(
                scanned
                    .errors
                    .iter()
                    .map(|e| format!("error: {}", e.report())),
            );
            (jobs, skipped)
        }
        _ => (vec![(file.clone(), output.clone())], Vec::new()),
    };
    if let Some(d) = &dir {
        if ext.is_none() {
            diagnostic!(
                "error: {} is a directory; ascii and text read one file",
                d.display()
            );
            return ExitCode::FAILURE;
        }
        if let Err(e) = std::fs::read_dir(d) {
            diagnostic!("error: cannot read {}: {e}", d.display());
            return ExitCode::FAILURE;
        }
        if let Some(out) = &output
            && out.exists()
            && !out.is_dir()
        {
            diagnostic!(
                "error: {} is not a directory; a DIR renders into one",
                out.display()
            );
            return ExitCode::FAILURE;
        }
    } else if let (Some(src), Some(out)) = (&file, &output)
        && same_file(src, out)
    {
        diagnostic!(
            "error: refusing to overwrite the input {}; use --output",
            src.display()
        );
        return ExitCode::FAILURE;
    }
    let make_dirs = dir.is_some();

    if !watch {
        let (jobs, skipped) = jobs();
        for line in &skipped {
            diagnostic!("{line}");
        }
        if let Some(d) = &dir
            && jobs.is_empty()
        {
            if skipped.is_empty() {
                diagnostic!("{}", no_diagrams(d));
            }
            return ExitCode::FAILURE;
        }
        log::info!("rendering {} file(s)", jobs.len());
        let mut ok = skipped.is_empty();
        for (input, output) in &jobs {
            let done = render_one(input.as_deref(), output.as_deref(), make_dirs, &render);
            ok &= done.written && done.clean;
        }
        return if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    let Some(watched) = &file else {
        diagnostic!("error: --watch needs a FILE or directory, not stdin");
        return ExitCode::FAILURE;
    };
    diagnostic!("watching {} (Ctrl-C to stop)", watched.display());
    let mut seen = Seen::new();
    let mut reported = Vec::new();
    loop {
        let (jobs, skipped) = jobs();
        if skipped != reported {
            for line in &skipped {
                diagnostic!("{line}");
            }
            reported = skipped;
        }
        for line in watch_pass(&mut seen, &jobs, make_dirs, &render) {
            diagnostic!("{line}");
        }
        if output::failed() {
            return ExitCode::FAILURE;
        }
        std::thread::sleep(POLL);
    }
}

/// What a file looked like when last rendered: its modification time and length.
type Stamp = (SystemTime, u64);

/// The last observed stamp and the next retry for a failed render.
struct WatchEntry {
    stamp: Option<Stamp>,
    missing: bool,
    retry: Option<(Instant, Duration)>,
}
type Seen = HashMap<PathBuf, WatchEntry>;

/// Renders each of `jobs` whose input changed since `seen`, and returns the status lines.
fn watch_pass(
    seen: &mut Seen,
    jobs: &[Job],
    make_dirs: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>>,
) -> Vec<String> {
    watch_pass_at(seen, jobs, make_dirs, render, Instant::now())
}

fn watch_pass_at(
    seen: &mut Seen,
    jobs: &[Job],
    make_dirs: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>>,
    clock: Instant,
) -> Vec<String> {
    let inputs: std::collections::HashSet<_> =
        jobs.iter().filter_map(|(i, _)| i.as_ref()).collect();
    seen.retain(|p, _| inputs.contains(p));
    let mut lines = Vec::new();
    for (input, output) in jobs {
        let Some(input) = input.as_ref() else {
            lines.push("error: --watch needs a FILE or directory, not stdin".to_owned());
            continue;
        };
        let now = stamp(input);
        let missing = now.is_none()
            && matches!(std::fs::metadata(input), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        let prior = seen.get(input);
        let same = prior.is_some_and(|entry| entry.stamp == now && entry.missing == missing);
        if same && prior.is_some_and(|entry| entry.retry.is_none_or(|(at, _)| clock < at)) {
            continue;
        }
        log::debug!("{} changed", input.display());
        let done = render_one(Some(input), output.as_deref(), make_dirs, render);
        if done.written || missing {
            seen.insert(
                input.clone(),
                WatchEntry {
                    stamp: now,
                    missing,
                    retry: None,
                },
            );
        } else {
            let delay = if same {
                prior
                    .and_then(|entry| {
                        entry
                            .retry
                            .map(|(_, delay)| (delay * 2).min(Duration::from_secs(30)))
                    })
                    .unwrap_or(Duration::from_secs(2))
            } else {
                Duration::from_secs(2)
            };
            seen.insert(
                input.clone(),
                WatchEntry {
                    stamp: now,
                    missing,
                    retry: Some((clock + delay, delay)),
                },
            );
        }
        if done.written {
            lines.push(match output {
                Some(o) => format!("wrote {}", o.display()),
                None => format!("printed {}", input.display()),
            });
        }
    }
    lines
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Whether `path` names a diagram file a directory scan picks up. A `.tex` file is read when
/// named, but not found in a directory, where it is most likely a whole LaTeX document (LEMB-3).
fn is_diagram_file(path: &Path) -> bool {
    Lang::of_path(path).is_some_and(|lang| lang != Lang::Latex)
}

/// The message for a directory with no diagram files.
pub(crate) fn no_diagrams(dir: &Path) -> String {
    format!(
        "error: no .mmd, .mermaid, .dot, .gv, .puml, .plantuml, .pu, .iuml, or .vsdx files in {}",
        dir.display()
    )
}

/// The diagram files under `dir` (`.mmd`, `.mermaid`, `.dot`, `.gv`, `.puml`, `.plantuml`,
/// `.pu`, and `.iuml`, in any case; LANGS-1), sorted, each with its output: the same path
/// relative to `out`, with extension `ext`. Hidden entries and symlinked directories are
/// skipped.
#[derive(Default)]
struct Scan {
    found: Vec<(PathBuf, PathBuf)>,
    errors: Vec<Error>,
}

#[derive(Clone, Copy)]
enum EntryKind {
    Directory,
    File,
    Other,
}

fn directory_entries(dir: &Path) -> Result<Vec<Result<(PathBuf, EntryKind)>>> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| Error::with_source(format!("cannot read {}", dir.display()), e))?;
    Ok(entries
        .map(|entry| {
            let entry = entry.map_err(|e| {
                Error::with_source(format!("cannot read an entry in {}", dir.display()), e)
            })?;
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                return Ok((path, EntryKind::Other));
            }
            let kind = entry
                .file_type()
                .map_err(|e| Error::with_source(format!("cannot inspect {}", path.display()), e))?;
            let kind = if kind.is_dir() {
                EntryKind::Directory
            } else if kind.is_file() {
                EntryKind::File
            } else if kind.is_symlink() {
                // Only diagram-named links can be inputs. Avoid inspecting unrelated links,
                // whose targets may be missing or inaccessible.
                if !is_diagram_file(&path) {
                    return Ok((path, EntryKind::Other));
                }
                // Follow file symlinks, but never recurse into directory symlinks.
                match std::fs::metadata(&path) {
                    Ok(m) if m.is_file() => EntryKind::File,
                    Ok(_) => EntryKind::Other,
                    Err(e) => {
                        return Err(Error::with_source(
                            format!("cannot inspect {}", path.display()),
                            e,
                        ));
                    }
                }
            } else {
                EntryKind::Other
            };
            Ok((path, kind))
        })
        .collect())
}

fn scan(dir: &Path, out: &Path, ext: &str) -> Scan {
    scan_with(dir, out, ext, directory_entries)
}

/// Keeps traversal failures alongside successful discoveries, so a readable sibling never
/// hides a failed directory or entry. The listing seam also tests I/O failures deterministically.
fn scan_with(
    dir: &Path,
    out: &Path,
    ext: &str,
    list: impl Fn(&Path) -> Result<Vec<Result<(PathBuf, EntryKind)>>>,
) -> Scan {
    let mut scan = Scan::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        log::trace!("scanning {}", d.display());
        let entries = match list(&d) {
            Ok(entries) => entries,
            Err(e) => {
                scan.errors.push(e);
                continue;
            }
        };
        for entry in entries {
            let (path, kind) = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    scan.errors.push(e);
                    continue;
                }
            };
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            match kind {
                EntryKind::Directory => stack.push(path),
                EntryKind::File if is_diagram_file(&path) => match path.strip_prefix(dir) {
                    Ok(rel) => {
                        let target = out.join(rel).with_extension(ext);
                        scan.found.push((path, target));
                    }
                    Err(e) => scan.errors.push(Error::with_source(
                        "directory entry is outside the requested root",
                        e,
                    )),
                },
                _ => {}
            }
        }
    }
    scan.found.sort();
    log::debug!(
        "scanned {}: {} diagram file(s), {} traversal error(s)",
        dir.display(),
        scan.found.len(),
        scan.errors.len()
    );
    scan
}

/// An input (stdin when `None`) and its output (stdout when `None`).
type Job = (Option<PathBuf>, Option<PathBuf>);

/// Resolve an output's parent, including existing directory symlinks, without following the
/// final entry: writing replaces an output symlink instead of following it.
fn output_key(path: &Path) -> std::io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("output has no filename"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut ancestor = parent;
    let mut missing = Vec::new();
    let mut resolved = loop {
        match std::fs::canonicalize(ancestor) {
            Ok(path) => break path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // A dangling symlink is not a missing directory we can safely create.
                match std::fs::symlink_metadata(ancestor) {
                    Ok(_) => return Err(e),
                    Err(m) if m.kind() == std::io::ErrorKind::NotFound => {}
                    Err(m) => return Err(m),
                }
                let part = ancestor.file_name().ok_or(e)?;
                missing.push(part.to_owned());
                ancestor = ancestor
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
            }
            Err(e) => return Err(e),
        }
    };
    for part in missing.into_iter().rev() {
        resolved.push(part);
    }
    resolved.push(name);
    Ok(resolved)
}

fn safe_batch(found: Vec<(PathBuf, PathBuf)>) -> (Vec<Job>, Vec<String>) {
    safe_batch_by(found, cfg!(any(target_os = "macos", target_os = "windows")))
}

fn safe_batch_by(found: Vec<(PathBuf, PathBuf)>, fold: bool) -> (Vec<Job>, Vec<String>) {
    let key = |p: PathBuf| {
        if fold {
            PathBuf::from(p.to_string_lossy().to_lowercase())
        } else {
            p
        }
    };
    let mut sources = HashMap::new();
    for (input, _) in &found {
        match std::fs::canonicalize(input) {
            Ok(path) => {
                sources.insert(key(path), input.clone());
            }
            Err(e) => {
                return (
                    Vec::new(),
                    vec![format!("error: cannot resolve {}: {e}", input.display())],
                );
            }
        }
    }
    let mut writers = HashMap::new();
    let mut jobs = Vec::new();
    let mut skipped = Vec::new();
    for (input, output) in found {
        let resolved = match output_key(&output) {
            Ok(path) => key(path),
            Err(e) => {
                skipped.push(format!("error: cannot resolve {}: {e}", output.display()));
                continue;
            }
        };
        if let Some(source) = sources.get(&resolved) {
            skipped.push(format!(
                "error: skipping {}; {} would overwrite input {}",
                input.display(),
                output.display(),
                source.display()
            ));
        } else if let Some(first) = writers.get(&resolved) {
            skipped.push(format!(
                "error: skipping {}; {} also writes {}",
                input.display(),
                first,
                output.display()
            ));
        } else {
            writers.insert(resolved, input.display().to_string());
            jobs.push((Some(input), Some(output)));
        }
    }
    (jobs, skipped)
}

/// How a render went.
struct Done {
    /// The output was written.
    written: bool,
    /// The input was read and had no errors.
    clean: bool,
}

/// Writes `bytes` to `output` (stdout when `None`), creating its directory first when `make_dirs`
/// is set.
fn write_output(output: Option<&Path>, bytes: &[u8], make_dirs: bool) -> Result<()> {
    let target = output.map_or_else(|| "<stdout>".to_owned(), |p| p.display().to_string());
    log::debug!("writing {target}");
    match output {
        Some(path) => {
            if make_dirs && let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::with_source(format!("cannot create {}", parent.display()), e)
                })?;
            }
            write_file(path, bytes, false)
                .map_err(|e| Error::with_source(format!("cannot write {}", path.display()), e))
        }
        None => {
            let mut out = std::io::stdout().lock();
            out.write_all(bytes)
                .and_then(|()| out.flush())
                .map_err(|e| Error::with_source("cannot write <stdout>", e))
        }
    }
}

/// `path` with `-N` after its stem: `a.svg` and 2 give `a-2.svg`.
pub(crate) fn page_path(path: &Path, n: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "page".to_owned(), |s| s.to_string_lossy().into_owned());
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    path.with_file_name(name)
}

/// The drawings to write for `lowered` and where (VISIO-11): itself for most diagrams; for a
/// Visio drawing, the page `chosen` names, or each page as `STEM-N.EXT` beside `output`.
fn pages(
    lowered: Lowered,
    output: Option<&Path>,
    chosen: Option<usize>,
) -> Result<Vec<(Lowered, Option<PathBuf>)>> {
    let count = lowered.page_count();
    let same = |l: Lowered| vec![(l, output.map(Path::to_path_buf))];
    if let Some(n) = chosen {
        if count <= 1 && n == 1 {
            return Ok(same(lowered));
        }
        let page = n
            .checked_sub(1)
            .and_then(|i| lowered.with_page(i))
            .ok_or_else(|| {
                Error::new(format!(
                    "--page {n} is out of range; the drawing has {count} page{}",
                    if count == 1 { "" } else { "s" }
                ))
            })?;
        return Ok(same(page));
    }
    if count <= 1 {
        return Ok(same(lowered));
    }
    let Some(output) = output else {
        return Err(Error::new(format!(
            "the drawing has {count} pages; choose one with --page N, or write them to files with --output"
        )));
    };
    Ok((1..=count)
        .filter_map(|n| Some((lowered.with_page(n - 1)?, Some(page_path(output, n)))))
        .collect())
}

/// Reports diagnostics on stderr and writes `render`'s output to `output` (stdout when `None`),
/// creating its directory first when `make_dirs` is set.
fn render_one(
    file: Option<&Path>,
    output: Option<&Path>,
    make_dirs: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>>,
) -> Done {
    let failed = Done {
        written: false,
        clean: false,
    };
    let (name, src) = match read_source(file.map(Path::to_path_buf)) {
        Ok(read) => read,
        Err(message) => {
            diagnostic!("error: {}", message.report());
            return failed;
        }
    };
    log::debug!("read {name}: {} bytes", src.len());
    let start = Instant::now();
    let lowered = lower_file(file.filter(|f| f.as_os_str() != "-"), &src);
    log::debug!("parsed and lowered in {:.1?}", start.elapsed());
    let errors = markdown::report_source(&name, &src, &lowered);
    let start = Instant::now();
    let chosen = PAGE.get().copied().flatten();
    let written = pages(lowered, output, chosen).and_then(|targets| {
        for (page, target) in &targets {
            let bytes = render(page)?;
            log::debug!("rendered {} bytes in {:.1?}", bytes.len(), start.elapsed());
            write_output(target.as_deref(), &bytes, make_dirs)?;
        }
        Ok(())
    });
    if let Err(message) = written {
        diagnostic!("error: {}", message.report());
        return failed;
    }
    Done {
        written: true,
        clean: !errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_package_magic_across_short_reads() {
        let reader = (&b"PK"[..]).chain(&b"\x03\x04rest of package"[..]);
        let input = read_diagram_source(reader, None).unwrap();
        assert!(matches!(input, Source::Bytes(bytes) if bytes == b"PK\x03\x04rest of package"));
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("sirene").chain(args.iter().copied())).unwrap()
    }

    /// `--verbose` goes anywhere, and files and commands parse as before.
    #[test]
    fn arguments() {
        // `--lang` takes the names Markdown blocks and the JavaScript API use.
        for name in [
            "dot", "graphviz", "plantuml", "puml", "latex", "tex", "visio", "vsdx",
        ] {
            assert!(
                Cli::try_parse_from(["sirene", "--lang", name, "ascii"]).is_ok(),
                "{name}"
            );
        }
        assert!(matches!(parse(&["text"]).command, Some(Command::Text(_))));
        assert!(matches!(
            parse(&["text", "--charset", "ascii"]).command,
            Some(Command::Text(TextArgs {
                charset: Some(CharsetArg::Ascii),
                ..
            }))
        ));
        assert!(matches!(
            parse(&["ascii"]).command,
            Some(Command::Ascii(TextArgs { charset: None, .. }))
        ));
        for args in [
            &["-v", "ascii"][..],
            &["ascii", "-v"],
            &["--verbose", "ascii"],
        ] {
            let cli = parse(args);
            assert_eq!(cli.verbose, 1, "{args:?}");
            assert!(
                matches!(cli.command, Some(Command::Ascii { .. })),
                "{args:?}"
            );
        }
        let cli = parse(&["a.mmd", "b.mmd"]);
        assert!(cli.command.is_none() && cli.verbose == 0);
        assert_eq!(cli.files, [PathBuf::from("a.mmd"), PathBuf::from("b.mmd")]);
        let cli = parse(&["-v", "a.mmd"]);
        assert!(cli.command.is_none() && cli.verbose > 0);
        assert_eq!(cli.files, [PathBuf::from("a.mmd")]);
        let cli = parse(&[]);
        assert!(cli.command.is_none() && cli.files.is_empty());
        // A file named like a command needs a path that is not one.
        let cli = parse(&["./ascii"]);
        assert_eq!(cli.files, [PathBuf::from("./ascii")]);
        assert_eq!(parse(&["-vv", "ascii"]).verbose, 2);
        assert_eq!(parse(&["ascii", "--verbose", "--verbose"]).verbose, 2);
    }

    /// ERRORH-4: a readable sibling does not erase descendant or directory-entry failures.
    #[test]
    fn scan_retains_directory_and_entry_errors() {
        use std::error::Error as _;

        let root = Path::new("root");
        let scanned = scan_with(root, Path::new("out"), "svg", |dir| {
            if dir == root {
                Ok(vec![
                    Ok((root.join("ok.mmd"), EntryKind::File)),
                    Ok((root.join("locked"), EntryKind::Directory)),
                    Err(Error::with_source(
                        "cannot read entry",
                        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                    )),
                ])
            } else {
                Err(Error::with_source(
                    "cannot read root/locked",
                    std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                ))
            }
        });
        assert_eq!(
            scanned.found,
            [(root.join("ok.mmd"), PathBuf::from("out/ok.svg"))]
        );
        assert_eq!(scanned.errors.len(), 2);
        for error in &scanned.errors {
            assert_eq!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<std::io::Error>()
                    .unwrap()
                    .kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
    }

    #[test]
    fn watch_rejects_stdin_jobs_without_panicking() {
        let lines = watch_pass(
            &mut Seen::new(),
            &[(None, None)],
            false,
            &|_| Ok(Vec::new()),
        );
        assert_eq!(
            lines,
            ["error: --watch needs a FILE or directory, not stdin"]
        );
    }

    #[test]
    fn misspelled_commands() {
        let check = |args: &[&str]| misspelled_command(&parse(args).files);
        assert_eq!(check(&["asci", "a.mmd"]), Some(("asci".into(), "ascii")));
        assert_eq!(check(&["asciii"]), Some(("asciii".into(), "ascii")));
        assert_eq!(check(&["sgv"]), Some(("sgv".into(), "svg")));
        assert_eq!(check(&["pgn"]), Some(("pgn".into(), "png")));
        assert_eq!(check(&["hlep"]), Some(("hlep".into(), "help")));
        assert_eq!(check(&["acsii"]), Some(("acsii".into(), "ascii")));
        assert_eq!(check(&["chekc"]), Some(("chekc".into(), "check")));
        // Short names two edits from a command are new files.
        for ok in [
            &["pdf"][..],
            &["test"],
            &["pu"],
            &["gv"],
            &["hello"],
            &["./asci"],
            &["asci.mmd"],
            &["diagram"],
            &["a.mmd", "asci"],
            &[],
        ] {
            assert_eq!(check(ok), None, "{ok:?}");
        }
        // A path that exists is used, whatever its name (tests run in `crates/sirene`).
        assert!(Path::new("src").exists());
        assert_eq!(check(&["src"]), None);
    }

    /// A watch pass renders new and changed inputs only, and forgets inputs that are gone.
    #[test]
    fn watch_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let out = dir.join("out");
        let render = |l: &Lowered| {
            let nodes = l.diagram.graph().map_or(0, |g| g.nodes.len());
            Ok(format!("{nodes} nodes").into_bytes())
        };
        let jobs = || -> Vec<_> {
            scan(dir, &out, "txt")
                .found
                .into_iter()
                .map(|(i, o)| (Some(i), Some(o)))
                .collect()
        };
        let read = |name: &str| std::fs::read_to_string(out.join(name)).unwrap();
        let mut seen = Seen::new();
        assert!(watch_pass(&mut seen, &jobs(), true, &render).is_empty());

        std::fs::write(dir.join("a.mmd"), "flowchart\nA\n").unwrap();
        let lines = watch_pass(&mut seen, &jobs(), true, &render);
        assert_eq!(lines, [format!("wrote {}", out.join("a.txt").display())]);
        assert_eq!(read("a.txt"), "1 nodes");
        assert!(watch_pass(&mut seen, &jobs(), true, &render).is_empty());

        std::fs::write(dir.join("a.mmd"), "flowchart\nA --> B\n").unwrap();
        assert_eq!(watch_pass(&mut seen, &jobs(), true, &render).len(), 1);
        assert_eq!(read("a.txt"), "2 nodes");

        std::fs::remove_file(dir.join("a.mmd")).unwrap();
        assert!(watch_pass(&mut seen, &jobs(), true, &render).is_empty());
        assert!(seen.is_empty());

        // One file to stdout; a missing file reports once, then renders when it is back.
        let file = dir.join("b.mmd");
        let jobs = [(Some(file.clone()), None)];
        let mut seen = Seen::new();
        assert!(watch_pass(&mut seen, &jobs, false, &render).is_empty());
        assert!(watch_pass(&mut seen, &jobs, false, &render).is_empty());
        std::fs::write(&file, "flowchart\nB\n").unwrap();
        assert_eq!(
            watch_pass(&mut seen, &jobs, false, &render),
            [format!("printed {}", file.display())]
        );
    }

    #[test]
    fn watch_retries_failed_render_without_input_change() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.mmd");
        let output = dir.path().join("a.svg");
        std::fs::write(&input, "flowchart\nA\n").unwrap();
        let jobs = [(Some(input), Some(output.clone()))];
        let attempts = std::cell::Cell::new(0);
        let render = |_: &Lowered| {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 7 {
                Err(Error::new("temporary failure"))
            } else {
                Ok(b"ok".to_vec())
            }
        };
        let mut seen = Seen::new();
        let start = Instant::now();
        for (seconds, count) in [
            (0, 1),
            (1, 1),
            (2, 2),
            (5, 2),
            (6, 3),
            (13, 3),
            (14, 4),
            (30, 5),
            (60, 6),
            (89, 6),
        ] {
            assert!(
                watch_pass_at(
                    &mut seen,
                    &jobs,
                    false,
                    &render,
                    start + Duration::from_secs(seconds)
                )
                .is_empty()
            );
            assert_eq!(attempts.get(), count);
        }
        assert_eq!(
            watch_pass_at(
                &mut seen,
                &jobs,
                false,
                &render,
                start + Duration::from_secs(90)
            )
            .len(),
            1
        );
        assert!(
            watch_pass_at(
                &mut seen,
                &jobs,
                false,
                &render,
                start + Duration::from_secs(120)
            )
            .is_empty()
        );
        assert_eq!(std::fs::read(output).unwrap(), b"ok");
        assert_eq!(attempts.get(), 7);
    }

    /// ERRORH-4: Markdown watches retry transient failures and ignore their own link edits.
    #[test]
    fn markdown_watch_retries_and_ignores_own_changes() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.md");
        std::fs::write(&input, "source").unwrap();
        let attempts = std::cell::Cell::new(0);
        let run = || {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                // A partial pass may have inserted links before another output failed.
                std::fs::write(&input, "source with link").unwrap();
            }
            Done {
                written: attempts.get() >= 3,
                clean: false,
            }
        };
        let mut seen = None;
        let start = Instant::now();
        for (seconds, expected) in [(0, 1), (1, 1), (2, 2), (5, 2), (6, 3), (50, 3)] {
            markdown_watch_pass(
                &input,
                &mut seen,
                &run,
                start + Duration::from_secs(seconds),
            );
            assert_eq!(attempts.get(), expected);
        }
        std::fs::write(&input, "edited source with new content").unwrap();
        markdown_watch_pass(&input, &mut seen, &run, start + Duration::from_secs(51));
        assert_eq!(attempts.get(), 4);
    }

    /// The first input to claim an output keeps it; later ones are left out with a reason.
    #[test]
    fn distinct_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let p = |name| dir.path().join(name);
        for name in ["a.mermaid", "a.mmd", "b.mmd"] {
            std::fs::write(p(name), "").unwrap();
        }
        let (jobs, skipped) = safe_batch(vec![
            (p("a.mermaid"), p("a.svg")),
            (p("a.mmd"), p("a.svg")),
            (p("b.mmd"), p("b.svg")),
        ]);
        assert_eq!(
            jobs,
            [
                (Some(p("a.mermaid")), Some(p("a.svg"))),
                (Some(p("b.mmd")), Some(p("b.svg"))),
            ]
        );
        assert_eq!(
            skipped,
            [format!(
                "error: skipping {}; {} also writes {}",
                p("a.mmd").display(),
                p("a.mermaid").display(),
                p("a.svg").display()
            )]
        );
    }

    /// Outputs differing only in case collide when the filesystem folds case.
    #[test]
    fn distinct_outputs_fold_case() {
        let dir = tempfile::tempdir().unwrap();
        let p = |name| dir.path().join(name);
        for name in ["A.mmd", "a.mmd"] {
            std::fs::write(p(name), "").unwrap();
        }
        let found = || vec![(p("A.mmd"), p("A.svg")), (p("a.mmd"), p("a.svg"))];
        let (jobs, skipped) = safe_batch_by(found(), true);
        assert_eq!(jobs, [(Some(p("A.mmd")), Some(p("A.svg")))]);
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("also writes"));
        let (jobs, skipped) = safe_batch_by(found(), false);
        assert_eq!(jobs.len(), 2);
        assert!(skipped.is_empty());
    }

    #[test]
    fn scan_skips_hidden_and_other_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("x/y")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        for f in [
            "b.mmd",
            "x/a.mermaid",
            "x/y/c.mmd",
            ".git/d.mmd",
            "e.md",
            ".f.mmd",
            "x/G.PUML",
        ] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let found = scan(dir, Path::new("o"), "png");
        assert!(found.errors.is_empty());
        let rel: Vec<_> = found
            .found
            .iter()
            .map(|(i, o)| (i.strip_prefix(dir).unwrap().to_owned(), o.clone()))
            .collect();
        assert_eq!(
            rel,
            [
                (PathBuf::from("b.mmd"), PathBuf::from("o/b.png")),
                (PathBuf::from("x/G.PUML"), PathBuf::from("o/x/G.png")),
                (PathBuf::from("x/a.mermaid"), PathBuf::from("o/x/a.png")),
                (PathBuf::from("x/y/c.mmd"), PathBuf::from("o/x/y/c.png")),
            ]
        );
        assert_eq!(scan(&dir.join("missing"), dir, "png").errors.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn writes_leave_unrelated_staging_symlink_alone() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("keep");
        std::fs::write(&target, b"keep this target").unwrap();
        let staging = dir.path().join(".out.svg.sirene-tmp");
        symlink(&target, &staging).unwrap();
        let out = dir.path().join("out.svg");
        write_file(&out, b"new output", false).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"new output");
        assert_eq!(std::fs::read(&target).unwrap(), b"keep this target");
        assert!(std::fs::symlink_metadata(&staging).unwrap().is_symlink());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn writes_replace_or_follow_links() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap();
        // A new file, then over it, keeping its mode; no temporary file is left.
        let out = dir.join("out.svg");
        write_file(&out, b"1", false).unwrap();
        let ordinary = dir.join("ordinary");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&ordinary)
            .unwrap();
        assert_eq!(
            std::fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            std::fs::metadata(&ordinary).unwrap().permissions().mode() & 0o777
        );
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o600)).unwrap();
        write_file(&out, b"2", false).unwrap();
        assert_eq!(read("out.svg"), "2");
        let mode = std::fs::metadata(&out).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!dir.join(".out.svg.sirene-tmp").exists());
        // A symlinked output is replaced; its target is kept.
        std::fs::write(dir.join("target"), "keep").unwrap();
        std::os::unix::fs::symlink(dir.join("target"), dir.join("link.svg")).unwrap();
        write_file(&dir.join("link.svg"), b"svg", false).unwrap();
        assert_eq!(read("target"), "keep");
        assert_eq!(read("link.svg"), "svg");
        assert!(
            !std::fs::symlink_metadata(dir.join("link.svg"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // A followed link writes its target and stays a link.
        std::os::unix::fs::symlink(dir.join("target"), dir.join("doc.md")).unwrap();
        write_file(&dir.join("doc.md"), b"doc", true).unwrap();
        assert_eq!(read("target"), "doc");
        assert!(
            std::fs::symlink_metadata(dir.join("doc.md"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
