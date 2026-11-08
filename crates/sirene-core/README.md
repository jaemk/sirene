# sirene-core

Mermaid flowchart parsing, layout, and rendering for [sirene](https://github.com/jaemk/sirene):
a lossless syntax tree with diagnostics, a diagram model, a layered layout, and output as ASCII,
SVG, or (with the `png` feature) PNG.

```rust
use sirene_core::flowchart;
use sirene_core::scene::{self, Approx, Theme};

let src = "flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n";
let lowered = flowchart::parse(src).lower();
assert!(lowered.diagnostics.is_empty());

// Text for a terminal.
let text = sirene_core::ascii::render(&lowered.flowchart, &Default::default());
assert!(text.contains("Start"));

// A standalone SVG document, with text measured by a monospace estimate.
let drawing = scene::flowchart(&lowered.flowchart, &Approx, &Theme::LIGHT);
let svg = scene::svg::write(&drawing);
assert!(svg.starts_with("<svg"));
```

- `flowchart::parse` never fails: errors are `Diagnostic`s with byte ranges
  (`Diagnostic::line_col` gives 1-based positions), and the tree's text is the input.
- `flowchart::edit` turns changes to the diagram into minimal source edits.
- `scene::png` (feature `png`) writes PNGs with embedded Noto Sans faces, so images are the same
  on every machine.

The behavior is specified in the repository's `spec/` directory.
