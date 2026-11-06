use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use sirene_core::ascii::{self, Charset, Options};
use sirene_core::{Severity, flowchart};

/// Mermaid diagram editor and viewer.
#[derive(Debug, Parser)]
#[command(name = "sirene", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CharsetArg {
    /// Box-drawing characters.
    Unicode,
    /// Printable ASCII only.
    Ascii,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        None => ExitCode::SUCCESS,
        Some(Command::Ascii { file, charset }) => {
            let charset = match charset {
                CharsetArg::Unicode => Charset::Unicode,
                CharsetArg::Ascii => Charset::Ascii,
            };
            print_ascii(file, charset)
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

fn print_ascii(file: Option<PathBuf>, charset: Charset) -> ExitCode {
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
    print!(
        "{}",
        ascii::render(&lowered.flowchart, &Options { charset })
    );
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
