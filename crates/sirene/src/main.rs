use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lang, Lowered};
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{self, Approx, Theme};

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
    /// Print what each step does, with timings, to stderr.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Read every diagram as this language, instead of by its file extension, its Markdown
    /// block's name, or its first line.
    #[arg(long, global = true, value_enum)]
    lang: Option<LangArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangArg {
    Mermaid,
    /// Graphviz DOT.
    Dot,
    /// PlantUML.
    Plantuml,
}

/// The language `--lang` forces, set once at startup (DOT-5).
static FORCED_LANG: std::sync::OnceLock<Option<Lang>> = std::sync::OnceLock::new();

/// The language to read a diagram as: `--lang`, else `named` (by the file's extension or the
/// Markdown block's info string), else detected from the source.
fn lang(named: Option<Lang>) -> Option<Lang> {
    FORCED_LANG.get().copied().flatten().or(named)
}

/// Parses and lowers the diagram read from `path` (stdin for `None`) in its language.
fn lower_file(path: Option<&Path>, src: &str) -> Lowered {
    diagram::lower_as(src, lang(path.and_then(Lang::of_path)))
}

/// Writes log records to stderr as `[elapsed] target: message` (`--verbose`).
struct Stderr {
    start: Instant,
}

impl log::Log for Stderr {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        // Only our own crates; gpui and its dependencies log a lot at debug level.
        if !record.target().starts_with("sirene") {
            return;
        }
        let ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(
            std::io::stderr().lock(),
            "[{ms:8.1}ms] {}: {}",
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

fn verbose(on: bool) {
    if on
        && log::set_boxed_logger(Box::new(Stderr {
            start: Instant::now(),
        }))
        .is_ok()
    {
        log::set_max_level(log::LevelFilter::Debug);
    }
}

mod markdown;
mod update;

#[derive(Debug, Subcommand)]
enum Command {
    /// Check diagrams for errors without drawing them; prints only diagnostics.
    Check {
        /// Diagram files, directories of them, or Markdown files (their diagram blocks); `-`
        /// for stdin.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Print a diagram, or each diagram block of a Markdown file, as text.
    Ascii {
        /// Diagram or Markdown file to read; stdin when omitted or `-`.
        file: Option<PathBuf>,
        /// Characters to draw with.
        #[arg(long, value_enum, default_value_t = CharsetArg::Unicode)]
        charset: CharsetArg,
        /// Print again each time FILE changes, until interrupted.
        #[arg(short, long)]
        watch: bool,
    },
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CharsetArg {
    /// Box-drawing characters.
    Unicode,
    /// Printable ASCII only.
    Ascii,
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
    let cli = Cli::parse();
    verbose(cli.verbose);
    let _ = FORCED_LANG.set(cli.lang.map(|l| match l {
        LangArg::Mermaid => Lang::Mermaid,
        LangArg::Dot => Lang::Dot,
        LangArg::Plantuml => Lang::PlantUml,
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
    log::debug!("exit with {status}");
    code
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

fn run(cli: Cli) -> ExitCode {
    if cli.command.is_none()
        && let Some((name, cmd)) = misspelled_command(&cli.files)
    {
        eprintln!(
            "error: no file or command named '{name}'; did you mean 'sirene {cmd}'? \
             (to open a new file with that name, use './{name}')"
        );
        return ExitCode::from(2);
    }
    match cli.command {
        None => gui(cli.files),
        Some(Command::Update { check, yes, to }) => update::run(update::Args { check, yes, to }),
        Some(Command::Check { files }) => {
            // Every file is checked, whatever the ones before it had.
            let failed = files.iter().filter(|f| !markdown::check(f)).count();
            if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(Command::Ascii {
            file,
            charset,
            watch,
        }) => {
            let charset = match charset {
                CharsetArg::Unicode => Charset::Unicode,
                CharsetArg::Ascii => Charset::Ascii,
            };
            if let Some(md) = markdown_file(&file) {
                return markdown_blocks(md, watch, || markdown::print_ascii(md, charset));
            }
            convert(file, None, None, watch, |l| {
                Ok(l.diagram.ascii(&Options { charset }).into_bytes())
            })
        }
        Some(Command::Svg {
            file,
            watch,
            link,
            theme,
            output,
        }) => {
            let render = |l: &Lowered| {
                let scene = l.diagram.scene(&Approx, &l.theme.apply(&theme.theme()));
                Ok(scene::svg::write(&scene).into_bytes())
            };
            let Ok(output) = stdout_dash(&file, output) else {
                return ExitCode::from(2);
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
        }) => {
            let quality = quality.quality();
            let render = |l: &Lowered| {
                let scene = l
                    .diagram
                    .scene(&FontMetrics, &l.theme.apply(&theme.theme()));
                // A given scale is kept (and may be too large); the quality's own shrinks to fit.
                let scale = scale.unwrap_or_else(|| png::fit(&scene, quality.scale()));
                png::write(&scene, png::Options { scale, quality })
            };
            let to_stdout = output.as_deref() == Some(Path::new("-"));
            let Ok(output) = stdout_dash(&file, output) else {
                return ExitCode::from(2);
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
                eprintln!("error: refusing to write a PNG to a terminal; use --output");
                return ExitCode::FAILURE;
            }
            convert(file, output, Some("png"), watch, render)
        }
    }
}

/// `output` with `-` read as stdout (`None`), as `-` is stdin for FILE; an error for a directory
/// or Markdown `file`, which write many images.
fn stdout_dash(file: &Option<PathBuf>, output: Option<PathBuf>) -> Result<Option<PathBuf>, ()> {
    if output.as_deref() != Some(Path::new("-")) {
        return Ok(output);
    }
    if markdown_file(file).is_some() || file.as_ref().is_some_and(|f| f.is_dir()) {
        eprintln!("error: `--output -` writes one diagram to stdout; give a directory instead");
        return Err(());
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
fn markdown_blocks(md: &Path, watch: bool, run: impl Fn() -> bool) -> ExitCode {
    if !watch {
        return if run() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    eprintln!("watching {} (Ctrl-C to stop)", md.display());
    // The first pass always runs, so a missing file is reported.
    let mut seen = None;
    loop {
        let now = stamp(md);
        if seen.as_ref() != Some(&now) {
            run();
            // Adding links changes the file; that is not a change to render again.
            seen = Some(stamp(md));
        }
        std::thread::sleep(POLL);
    }
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
    eprintln!("error: built without the GUI; see sirene --help");
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
fn read(file: Option<PathBuf>) -> Result<(String, String), String> {
    match file.filter(|f| f.as_os_str() != "-") {
        Some(path) => {
            let name = path.display().to_string();
            log::debug!("reading {name}");
            read_text(&path).map(|src| (name, src))
        }
        None => {
            log::debug!("reading <stdin>");
            let src = read_capped(std::io::stdin().lock())
                .map_err(|e| format!("cannot read <stdin>: {e}"))?;
            utf8(src)
                .map(|src| ("<stdin>".to_owned(), src))
                .map_err(|e| format!("cannot read <stdin>: {e}"))
        }
    }
}

/// The largest source read, in bytes.
const MAX_INPUT: u64 = 64 << 20;

/// Reads at most `MAX_INPUT` bytes from `r`, or an error past that.
fn read_capped(r: impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    r.take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err(format!("the text is over {} MiB", MAX_INPUT >> 20));
    }
    Ok(bytes)
}

/// Reads the file at `path` as UTF-8 text, with an error naming it.
pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    std::fs::File::open(path)
        .map_err(|e| e.to_string())
        .and_then(read_capped)
        .and_then(utf8)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over `path`, so a failed
/// write leaves the old file whole. The file keeps its permissions. A symlink at `path` is
/// replaced, not written through, unless `follow`, when its target is written instead. Anything
/// other than a regular file, such as `/dev/stdout`, is written in place.
pub(crate) fn write_file(path: &Path, bytes: &[u8], follow: bool) -> std::io::Result<()> {
    let link = std::fs::symlink_metadata(path).ok();
    let path = match &link {
        Some(m) if m.file_type().is_symlink() && follow => std::fs::canonicalize(path)?,
        _ => path.to_owned(),
    };
    let meta = std::fs::symlink_metadata(&path).ok();
    if meta
        .as_ref()
        .is_some_and(|m| !m.is_file() && !m.file_type().is_symlink())
    {
        return std::fs::write(&path, bytes);
    }
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!(".{name}.sirene-tmp"));
    // A leftover temporary file, or anything planted there, is replaced, never written through.
    let _ = std::fs::remove_file(&tmp);
    let staged = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        if let Some(m) = meta.as_ref().filter(|m| m.is_file()) {
            file.set_permissions(m.permissions())?;
        }
        file.write_all(bytes)?;
        file.sync_all()
    })();
    let result = staged.and_then(|()| std::fs::rename(&tmp, &path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// `bytes` as UTF-8, or an error naming UTF-16 (by its byte order mark) or the first bad byte.
fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err("the text is UTF-16; sirene reads UTF-8".into());
    }
    String::from_utf8(bytes).map_err(|e| {
        let at = e.utf8_error().valid_up_to();
        format!("the text is not UTF-8 (an invalid byte at offset {at})")
    })
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
    render: impl Fn(&Lowered) -> Result<Vec<u8>, String>,
) -> ExitCode {
    let file = file.filter(|f| f.as_os_str() != "-");
    let dir = file.clone().filter(|f| f.is_dir());
    let jobs = || match (&dir, ext) {
        (Some(d), Some(ext)) => distinct(scan(d, output.as_deref().unwrap_or(d), ext)),
        _ => (vec![(file.clone(), output.clone())], Vec::new()),
    };
    if let Some(d) = &dir {
        if ext.is_none() {
            eprintln!(
                "error: {} is a directory; ascii reads one file",
                d.display()
            );
            return ExitCode::FAILURE;
        }
        if let Err(e) = std::fs::read_dir(d) {
            eprintln!("error: cannot read {}: {e}", d.display());
            return ExitCode::FAILURE;
        }
        if let Some(out) = &output
            && out.exists()
            && !out.is_dir()
        {
            eprintln!(
                "error: {} is not a directory; a DIR renders into one",
                out.display()
            );
            return ExitCode::FAILURE;
        }
    } else if let (Some(src), Some(out)) = (&file, &output)
        && same_file(src, out)
    {
        eprintln!(
            "error: refusing to overwrite the input {}; use --output",
            src.display()
        );
        return ExitCode::FAILURE;
    }
    let make_dirs = dir.is_some();

    if !watch {
        let (jobs, skipped) = jobs();
        if let Some(d) = &dir
            && jobs.is_empty()
        {
            eprintln!("{}", no_diagrams(d));
            return ExitCode::FAILURE;
        }
        for line in &skipped {
            eprintln!("{line}");
        }
        log::debug!("rendering {} file(s)", jobs.len());
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
        eprintln!("error: --watch needs a FILE or directory, not stdin");
        return ExitCode::FAILURE;
    };
    eprintln!("watching {} (Ctrl-C to stop)", watched.display());
    let mut seen = Seen::new();
    let mut reported = Vec::new();
    loop {
        let (jobs, skipped) = jobs();
        if skipped != reported {
            for line in &skipped {
                eprintln!("{line}");
            }
            reported = skipped;
        }
        for line in watch_pass(&mut seen, &jobs, make_dirs, &render) {
            eprintln!("{line}");
        }
        std::thread::sleep(POLL);
    }
}

/// What a file looked like when last rendered: its modification time and length.
type Stamp = (SystemTime, u64);

/// The stamp of each watched input when it was last rendered.
type Seen = HashMap<PathBuf, Option<Stamp>>;

/// Renders each of `jobs` whose input changed since `seen`, and returns the status lines.
fn watch_pass(
    seen: &mut Seen,
    jobs: &[Job],
    make_dirs: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>, String>,
) -> Vec<String> {
    seen.retain(|p, _| jobs.iter().any(|(i, _)| i.as_ref() == Some(p)));
    let mut lines = Vec::new();
    for (input, output) in jobs {
        let input = input.as_ref().expect("a watched input is a path");
        let now = stamp(input);
        if seen.get(input) == Some(&now) {
            continue;
        }
        log::debug!("{} changed", input.display());
        seen.insert(input.clone(), now);
        let done = render_one(Some(input), output.as_deref(), make_dirs, render);
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

/// The message for a directory with no diagram files.
pub(crate) fn no_diagrams(dir: &Path) -> String {
    format!(
        "error: no .mmd, .mermaid, .dot, .gv, .puml, .plantuml, .pu, or .iuml files in {}",
        dir.display()
    )
}

/// The diagram files under `dir` (`.mmd`, `.mermaid`, `.dot`, `.gv`, `.puml`, `.plantuml`,
/// `.pu`, and `.iuml`, in any case; LANGS-1), sorted, each with its output: the same path
/// relative to `out`, with extension `ext`. Hidden entries and symlinked directories are
/// skipped.
fn scan(dir: &Path, out: &Path, ext: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = match std::fs::read_dir(&d) {
            Ok(entries) => entries,
            Err(e) => {
                log::debug!("cannot list {}: {e}", d.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path);
            } else if path.is_file() && Lang::of_path(&path).is_some() {
                let rel = path.strip_prefix(dir).expect("found under dir");
                let target = out.join(rel).with_extension(ext);
                found.push((path, target));
            }
        }
    }
    found.sort();
    found
}

/// An input (stdin when `None`) and its output (stdout when `None`).
type Job = (Option<PathBuf>, Option<PathBuf>);

/// `found` as jobs, without each input whose output an earlier input already writes (`a.mmd`
/// beside `a.mermaid`), plus an error line for each input left out.
fn distinct(found: Vec<(PathBuf, PathBuf)>) -> (Vec<Job>, Vec<String>) {
    let mut writers: HashMap<PathBuf, PathBuf> = HashMap::new();
    let mut jobs = Vec::new();
    let mut skipped = Vec::new();
    for (input, output) in found {
        if let Some(first) = writers.get(&output) {
            skipped.push(format!(
                "error: skipping {}; {} also writes {}",
                input.display(),
                first.display(),
                output.display()
            ));
            continue;
        }
        writers.insert(output.clone(), input.clone());
        jobs.push((Some(input), Some(output)));
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

/// Reports diagnostics on stderr and writes `render`'s output to `output` (stdout when `None`),
/// creating its directory first when `make_dirs` is set.
fn render_one(
    file: Option<&Path>,
    output: Option<&Path>,
    make_dirs: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>, String>,
) -> Done {
    let failed = Done {
        written: false,
        clean: false,
    };
    let (name, src) = match read(file.map(Path::to_path_buf)) {
        Ok(read) => read,
        Err(message) => {
            eprintln!("error: {message}");
            return failed;
        }
    };
    log::debug!("read {name}: {} bytes", src.len());
    let start = Instant::now();
    let lowered = lower_file(file.filter(|f| f.as_os_str() != "-"), &src);
    log::debug!("parsed and lowered in {:.1?}", start.elapsed());
    let errors = markdown::report(&name, &src, &lowered, |l, c| (l, c));
    let start = Instant::now();
    let written = render(&lowered).and_then(|bytes| {
        log::debug!("rendered {} bytes in {:.1?}", bytes.len(), start.elapsed());
        let target = output.map_or_else(|| "<stdout>".to_owned(), |p| p.display().to_string());
        log::debug!("writing {target}");
        match output {
            Some(path) => {
                if make_dirs && let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
                }
                write_file(path, &bytes, false)
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))
            }
            None => {
                let mut out = std::io::stdout().lock();
                out.write_all(&bytes)
                    .and_then(|()| out.flush())
                    .map_err(|e| format!("cannot write <stdout>: {e}"))
            }
        }
    });
    if let Err(message) = written {
        eprintln!("error: {message}");
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

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("sirene").chain(args.iter().copied())).unwrap()
    }

    /// `--verbose` goes anywhere, and files and commands parse as before.
    #[test]
    fn arguments() {
        for args in [
            &["-v", "ascii"][..],
            &["ascii", "-v"],
            &["--verbose", "ascii"],
        ] {
            let cli = parse(args);
            assert!(cli.verbose, "{args:?}");
            assert!(
                matches!(cli.command, Some(Command::Ascii { .. })),
                "{args:?}"
            );
        }
        let cli = parse(&["a.mmd", "b.mmd"]);
        assert!(cli.command.is_none() && !cli.verbose);
        assert_eq!(cli.files, [PathBuf::from("a.mmd"), PathBuf::from("b.mmd")]);
        let cli = parse(&["-v", "a.mmd"]);
        assert!(cli.command.is_none() && cli.verbose);
        assert_eq!(cli.files, [PathBuf::from("a.mmd")]);
        let cli = parse(&[]);
        assert!(cli.command.is_none() && cli.files.is_empty());
        // A file named like a command needs a path that is not one.
        let cli = parse(&["./ascii"]);
        assert_eq!(cli.files, [PathBuf::from("./ascii")]);
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

    /// The first input to claim an output keeps it; later ones are left out with a reason.
    #[test]
    fn distinct_outputs() {
        let p = PathBuf::from;
        let (jobs, skipped) = distinct(vec![
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
            ["error: skipping a.mmd; a.mermaid also writes a.svg"]
        );
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
        let rel: Vec<_> = found
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
        assert!(scan(&dir.join("missing"), dir, "png").is_empty());
    }

    #[test]
    fn reads_are_capped() {
        let big = std::io::repeat(b'a').take(MAX_INPUT + 1);
        assert!(read_capped(big).unwrap_err().contains("over 64 MiB"));
        assert_eq!(read_capped(&b"ab"[..]).unwrap(), b"ab");
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
