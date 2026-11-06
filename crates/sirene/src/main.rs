use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::Severity;
use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart::{self, Lowered};
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        None => {
            sirene_gui::run(cli.files);
            ExitCode::SUCCESS
        }
        Some(Command::Ascii { file, charset }) => {
            let charset = match charset {
                CharsetArg::Unicode => Charset::Unicode,
                CharsetArg::Ascii => Charset::Ascii,
            };
            print_with(file, |l| ascii::render(&l.flowchart, &Options { charset }))
        }
        Some(Command::Svg { file, theme }) => {
            let theme = match theme {
                ThemeArg::Light => Theme::LIGHT,
                ThemeArg::Dark => Theme::DARK,
            };
            print_with(file, |l| {
                scene::svg::write(&scene::flowchart(&l.flowchart, &Approx, &theme))
            })
        }
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

/// Reports diagnostics on stderr and prints `render`'s output, failing if there were errors.
fn print_with(file: Option<PathBuf>, render: impl FnOnce(&Lowered) -> String) -> ExitCode {
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
    print!("{}", render(&lowered));
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
