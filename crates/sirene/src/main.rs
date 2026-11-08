use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::Severity;
use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart::{self, Lowered};
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{self, Approx, Theme};

/// Mermaid diagram editor and viewer.
///
/// With no command, opens each FILE in a viewer window, or an empty window.
#[derive(Debug, Parser)]
#[command(name = "sirene", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Mermaid files to view.
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

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a diagram as text.
    Ascii {
        /// Mermaid file to read; stdin when omitted or `-`.
        file: Option<PathBuf>,
        /// Characters to draw with.
        #[arg(long, value_enum, default_value_t = CharsetArg::Unicode)]
        charset: CharsetArg,
    },
    /// Print a diagram as SVG.
    Svg {
        /// Mermaid file to read; stdin when omitted or `-`.
        file: Option<PathBuf>,
        /// Colors to draw with.
        #[arg(long, value_enum, default_value_t = ThemeArg::Light)]
        theme: ThemeArg,
        /// Write the SVG here instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Write a diagram as a PNG image.
    Png {
        /// Mermaid file to read; stdin when omitted or `-`.
        file: Option<PathBuf>,
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
        #[arg(short, long)]
        output: Option<PathBuf>,
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

fn run(cli: Cli) -> ExitCode {
    match cli.command {
        None => gui(cli.files),
        Some(Command::Ascii { file, charset }) => {
            let charset = match charset {
                CharsetArg::Unicode => Charset::Unicode,
                CharsetArg::Ascii => Charset::Ascii,
            };
            render_with(file, None, |l| {
                Ok(ascii::render(&l.flowchart, &Options { charset }).into_bytes())
            })
        }
        Some(Command::Svg {
            file,
            theme,
            output,
        }) => render_with(file, output, |l| {
            let scene = scene::flowchart(&l.flowchart, &Approx, &theme.theme());
            Ok(scene::svg::write(&scene).into_bytes())
        }),
        Some(Command::Png {
            file,
            theme,
            quality,
            scale,
            output,
        }) => {
            let quality = quality.quality();
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
            render_with(file, output, |l| {
                let scene = scene::flowchart(&l.flowchart, &FontMetrics, &theme.theme());
                // A given scale is kept (and may be too large); the quality's own shrinks to fit.
                let scale = scale.unwrap_or_else(|| png::fit(&scene, quality.scale()));
                png::write(&scene, png::Options { scale, quality })
            })
        }
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

/// Reports diagnostics on stderr and writes `render`'s output to `output` (stdout when `None`),
/// failing if there were errors.
fn render_with(
    file: Option<PathBuf>,
    output: Option<PathBuf>,
    render: impl FnOnce(&Lowered) -> Result<Vec<u8>, String>,
) -> ExitCode {
    if let (Some(src), Some(out)) = (&file, &output)
        && same_file(src, out)
    {
        eprintln!(
            "error: refusing to overwrite the input {}; use --output",
            src.display()
        );
        return ExitCode::FAILURE;
    }
    let (name, src) = match read(file) {
        Ok(read) => read,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    log::debug!("read {name}: {} bytes", src.len());
    let start = Instant::now();
    let lowered = flowchart::parse(&src).lower();
    log::debug!("parsed and lowered in {:.1?}", start.elapsed());
    let mut failed = false;
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(&src);
        let severity = match d.severity {
            Severity::Error => {
                failed = true;
                "error"
            }
            Severity::Warning => "warning",
        };
        eprintln!("{name}:{line}:{col}: {severity}: {}", d.message);
    }
    let start = Instant::now();
    let written = render(&lowered).and_then(|bytes| {
        log::debug!("rendered {} bytes in {:.1?}", bytes.len(), start.elapsed());
        let target = output
            .as_ref()
            .map_or_else(|| "<stdout>".to_owned(), |p| p.display().to_string());
        log::debug!("writing {target}");
        match &output {
            Some(path) => std::fs::write(path, bytes)
                .map_err(|e| format!("cannot write {}: {e}", path.display())),
            None => std::io::stdout()
                .write_all(&bytes)
                .map_err(|e| format!("cannot write <stdout>: {e}")),
        }
    });
    if let Err(message) = written {
        eprintln!("error: {message}");
        return ExitCode::FAILURE;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
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
}
