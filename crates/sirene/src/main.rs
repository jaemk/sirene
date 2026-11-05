use clap::Parser;

/// Mermaid diagram editor and viewer.
#[derive(Debug, Parser)]
#[command(name = "sirene", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
