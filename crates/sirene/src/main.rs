use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{self, Approx, Theme};

/// Mermaid diagram editor and viewer.
///
/// With no command, opens each FILE in an editor window, or a new document with a starter
/// flowchart.
#[derive(Debug, Parser)]
#[command(name = "sirene", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Mermaid files to edit.
    files: Vec<PathBuf>,
    /// Print what each step does, with timings, to stderr.
    #[arg(short, long, global = true)]
    verbose: bool,
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
        /// Mermaid files, directories of them, or Markdown files (their mermaid blocks); `-`
        /// for stdin.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Print a diagram as text.
    Ascii {
        /// Mermaid file to read; stdin when omitted or `-`.
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
        /// Mermaid file or directory to read; stdin when omitted or `-`. A directory's `.mmd`
        /// and `.mermaid` files are written as `.svg` beside them, or under --output. A
        /// Markdown file's mermaid blocks are written as NAME-1.svg, NAME-2.svg, ...
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
        /// Write the SVG here instead of stdout; for a directory, the directory to write into.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Write a diagram as a PNG image, or one per diagram in a directory or Markdown file.
    Png {
        /// Mermaid file or directory to read; stdin when omitted or `-`. A directory's `.mmd`
        /// and `.mermaid` files are written as `.png` beside them, or under --output. A
        /// Markdown file's mermaid blocks are written as NAME-1.png, NAME-2.png, ...
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
        /// Where to write the PNG; defaults to FILE with a .png extension, or stdout for stdin.
        /// For a directory, the directory to write into.
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
    let near = |cmd: &str| {
        let (a, b): (Vec<char>, Vec<char>) = (name.chars().collect(), cmd.chars().collect());
        // Levenshtein distance, one row at a time.
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.iter().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let next = (prev + usize::from(ca != cb))
                    .min(row[j] + 1)
                    .min(row[j + 1] + 1);
                prev = row[j + 1];
                row[j + 1] = next;
            }
        }
        row[b.len()] <= 2
    };
    ["ascii", "svg", "png", "check", "update", "help"]
        .into_iter()
        .find(|cmd| near(cmd))
        .map(|cmd| (name.to_owned(), cmd))
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
            if let Some(md) = file
                .as_ref()
                .filter(|f| sirene_core::markdown::is_markdown(f))
            {
                return render_markdown(md, output.as_deref(), "svg", link, watch, &render);
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
            if let Some(md) = file
                .as_ref()
                .filter(|f| sirene_core::markdown::is_markdown(f))
            {
                return render_markdown(md, output.as_deref(), "png", link, watch, &render);
            }
            if file.as_ref().is_some_and(|f| f.is_dir()) {
                return convert(file, output, Some("png"), watch, render);
            }
            let output = output.or_else(|| {
                let default = file
                    .as_ref()
                    .filter(|f| f.as_os_str() != "-")
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

/// Renders Markdown file `md`'s mermaid blocks (CLI-12); with `watch`, again whenever it changes,
/// never returning.
fn render_markdown(
    md: &Path,
    out: Option<&Path>,
    ext: &str,
    link: bool,
    watch: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>, String>,
) -> ExitCode {
    if !watch {
        return if markdown::render(md, out, ext, link, render) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    eprintln!("watching {} (Ctrl-C to stop)", md.display());
    let mut seen = None;
    loop {
        let now = stamp(md);
        if now != seen {
            markdown::render(md, out, ext, link, render);
            // Adding links changes the file; that is not a change to render again.
            seen = stamp(md);
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
            std::fs::read_to_string(&path)
                .map(|src| (name.clone(), src))
                .map_err(|e| format!("cannot read {name}: {e}"))
        }
        None => {
            log::debug!("reading <stdin>");
            let mut src = String::new();
            std::io::stdin()
                .read_to_string(&mut src)
                .map(|_| ("<stdin>".to_owned(), src))
                .map_err(|e| format!("cannot read <stdin>: {e}"))
        }
    }
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
            eprintln!("error: no .mmd or .mermaid files in {}", d.display());
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

/// The `.mmd` and `.mermaid` files under `dir`, sorted, each with its output: the same path
/// relative to `out`, with extension `ext`. Hidden entries and symlinked directories are skipped.
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
            } else if path.is_file()
                && path
                    .extension()
                    .is_some_and(|x| x == "mmd" || x == "mermaid")
            {
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
    let lowered = diagram::lower(&src);
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
                std::fs::write(path, bytes)
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
        for ok in [
            &["./asci"][..],
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
                (PathBuf::from("x/a.mermaid"), PathBuf::from("o/x/a.png")),
                (PathBuf::from("x/y/c.mmd"), PathBuf::from("o/x/y/c.png")),
            ]
        );
        assert!(scan(&dir.join("missing"), dir, "png").is_empty());
    }
}
