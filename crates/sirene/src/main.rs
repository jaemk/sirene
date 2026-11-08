use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

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
#[command(
    name = "sirene",
    version,
    about,
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Mermaid files to view.
    files: Vec<PathBuf>,
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
        /// Pixels per diagram unit.
        #[arg(long, default_value_t = 2.0)]
        scale: f32,
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
            scale,
            output,
        }) => {
            let output = output.or_else(|| {
                file.as_ref()
                    .filter(|f| f.as_os_str() != "-")
                    .map(|f| f.with_extension("png"))
            });
            if output.is_none() && std::io::stdout().is_terminal() {
                eprintln!("error: refusing to write a PNG to a terminal; use --output");
                return ExitCode::FAILURE;
            }
            render_with(file, output, |l| {
                let scene = scene::flowchart(&l.flowchart, &FontMetrics, &theme.theme());
                png::write(&scene, scale)
            })
        }
    }
}

#[cfg(feature = "gui")]
fn gui(files: Vec<PathBuf>) -> ExitCode {
    sirene_gui::run(files);
    ExitCode::SUCCESS
}

#[cfg(not(feature = "gui"))]
fn gui(_: Vec<PathBuf>) -> ExitCode {
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
            std::fs::read_to_string(&path)
                .map(|src| (name.clone(), src))
                .map_err(|e| format!("cannot read {name}: {e}"))
        }
        None => {
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
    let lowered = flowchart::parse(&src).lower();
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
    let written = render(&lowered).and_then(|bytes| match &output {
        Some(path) => {
            std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
        }
        None => std::io::stdout()
            .write_all(&bytes)
            .map_err(|e| format!("cannot write <stdout>: {e}")),
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
