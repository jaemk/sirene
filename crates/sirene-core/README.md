# sirene-core

Diagram parsing, layout, and rendering for [sirene](https://github.com/jaemk/sirene): Mermaid,
Graphviz DOT, and PlantUML sources read into lossless syntax trees with diagnostics, diagram
models, layouts, and output as ASCII, SVG, or (with the `png` feature) PNG. It renders files
without the `sirene` CLI, with the same output.

```rust
use std::path::Path;
use sirene_core::diagram::{self, Lang};
use sirene_core::scene::Theme;
# fn main() -> sirene_core::Result<()> {

let path = Path::new("login.mmd");
# let dir = std::env::temp_dir().join(format!("sirene-readme-{}", std::process::id()));
# std::fs::create_dir_all(&dir).unwrap();
# let path = &dir.join(path);
# std::fs::write(path, "flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n").unwrap();
// UTF-8 of at most 64 MiB, with errors naming the file.
let src = &sirene_core::source::read(path)?;
// The extension picks DOT (`.dot`, `.gv`) or PlantUML (`.puml`, ...); otherwise the source's
// header picks the diagram type.
let lowered = diagram::lower_as(src, Lang::of_path(path));
for d in &lowered.diagnostics {
    let (line, col) = d.line_col(src);
    eprintln!("{line}:{col}: {}", d.message);
}

// What `sirene text`, `sirene svg`, and `sirene png` write.
let text = lowered.ascii(&Default::default())?;
assert!(text.contains("Start"));
let svg = lowered.svg(&Theme::LIGHT)?;
assert!(svg.starts_with("<svg"));
#[cfg(feature = "png")]
{
    use sirene_core::scene::png::Quality;
    let png = lowered.png(&Theme::LIGHT, Quality::Fine, None)?;
    assert!(png.starts_with(b"\x89PNG"));
}

// Each diagram block of a Markdown file.
let md = "# Notes\n\n```dot\ndigraph { a -> b }\n```\n";
for block in sirene_core::markdown::mermaid_blocks(md) {
    let lowered = diagram::lower_as(&block.code, Some(block.lang));
    assert!(lowered.svg(&Theme::DARK)?.contains("<svg"));
}
# std::fs::remove_dir_all(&dir).unwrap();
# Ok(())
# }
```

- Reading never fails: problems are `Diagnostic`s with byte ranges (`Diagnostic::line_col` gives
  1-based positions, `diagnostic::LineIndex` many of them). The diagram is still drawn.
- A diagram's own `theme` and `themeVariables` apply over the theme passed in.
- `Lowered::scene` gives the display list for other renderers, with text measured by any
  `scene::TextMetrics`.
- `scene::png` (feature `png`) writes PNGs with embedded Noto Sans faces, so images are the same
  on every machine.
- `flowchart::parse` and `flowchart::edit` give the flowchart syntax tree and turn changes to
  the diagram into minimal source edits.
- Rendering returns `sirene_core::Result`. Errors retain typed causes through
  `std::error::Error::source()`; use `error.report()` to display the full causal chain.

The behavior is specified in the repository's `spec/` directory.
