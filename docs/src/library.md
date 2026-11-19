# Rust library

[`sirene-core`](https://crates.io/crates/sirene-core) reads, lays out, and draws diagrams
without the CLI, with the same output as `sirene ascii`, `sirene svg`, and `sirene png`. API
reference: [docs.rs/sirene-core](https://docs.rs/sirene-core).

```toml
[dependencies]
sirene-core = { version = "0.6", features = ["png"] }  # `png` only for PNG output
```

```rust
use std::path::Path;
use sirene_core::diagram::{self, Lang};
use sirene_core::scene::Theme;

fn main() -> sirene_core::Result<()> {
    let path = Path::new("login.mmd");
    // UTF-8 of at most 64 MiB, with errors naming the file.
    let src = &sirene_core::source::read(path)?;
    // The extension picks DOT (`.dot`, `.gv`) or PlantUML (`.puml`, ...); otherwise the
    // source's header picks the diagram type.
    let lowered = diagram::lower_as(src, Lang::of_path(path));
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        eprintln!("{line}:{col}: {}", d.message);
    }

    // What `sirene ascii`, `sirene svg`, and `sirene png` write.
    println!("{}", lowered.ascii(&Default::default())?);
    let svg: String = lowered.svg(&Theme::LIGHT)?;
    let png: Vec<u8> = lowered.png(&Theme::DARK, sirene_core::scene::png::Quality::Fine, None)?;

    // Each diagram block of a Markdown file.
    let md = "# Notes\n\n```dot\ndigraph { a -> b }\n```\n";
    for block in sirene_core::markdown::mermaid_blocks(md) {
        let svg = diagram::lower_as(&block.code, Some(block.lang)).svg(&Theme::DARK)?;
    }
    Ok(())
}
```

- Reading never fails: problems are `Diagnostic`s with byte ranges and a `Severity` (an error
  is what Mermaid would refuse to draw). The diagram is still drawn. `diagnostic::LineIndex`
  finds the lines and columns of many diagnostics in one pass.
- A diagram's own `theme` and `themeVariables` apply over the theme passed in.
- `Lowered::scene` gives the display list for other renderers, with text measured by any
  `scene::TextMetrics`.
- `scene::png` (feature `png`) draws text with embedded Noto Sans faces, so images are the same
  on every machine.
- `flowchart::parse` and `flowchart::edit` give the flowchart syntax tree and turn changes to the
  diagram into minimal source edits; the editor uses them.
- Errors are `sirene_core::Error`; `error.report()` formats the full chain of causes.
- `sirene-core` builds for `wasm32-unknown-unknown`. For web pages, use the prebuilt module
  instead: see [Web pages](embedding.md).
