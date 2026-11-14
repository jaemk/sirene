//! The diagrams in `examples/` parse without diagnostics and render in every format, and the
//! diagram blocks in the docs have no errors.

use std::path::Path;

use sirene_core::ascii::Options;
use sirene_core::diagnostic::Severity;
use sirene_core::diagram::{self, Lang};
use sirene_core::markdown;
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{Theme, svg};

#[test]
fn examples_render_cleanly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| Lang::of_path(p).is_some())
        .collect();
    paths.sort();
    let count = |lang| {
        paths
            .iter()
            .filter(|p| Lang::of_path(p) == Some(lang))
            .count()
    };
    assert!(count(Lang::Mermaid) >= 6, "{paths:?}");
    assert!(count(Lang::PlantUml) >= 4, "{paths:?}");
    assert!(count(Lang::Dot) >= 1, "{paths:?}");
    for path in &paths {
        let src = std::fs::read_to_string(path).unwrap();
        let lowered = diagram::lower_as(&src, Lang::of_path(path));
        assert!(
            lowered.diagnostics.is_empty(),
            "{}: {:?}",
            path.display(),
            lowered.diagnostics
        );
        for theme in [Theme::LIGHT, Theme::DARK] {
            let s = lowered.diagram.scene(&FontMetrics, &theme);
            assert!(svg::write(&s).starts_with("<svg"), "{}", path.display());
            png::write(&s, png::Options::new(png::Quality::Normal))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        assert!(
            !lowered.diagram.ascii(&Options::default()).trim().is_empty(),
            "{}",
            path.display()
        );
    }
}

/// Every diagram block in the guide reads without errors, so the syntax it shows works.
/// Warnings are allowed: some pages show what is read but not drawn.
#[test]
fn docs_blocks_have_no_errors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/src");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    paths.sort();
    let mut blocks = 0;
    let mut failures = Vec::new();
    for path in &paths {
        let src = std::fs::read_to_string(path).unwrap();
        for block in markdown::mermaid_blocks(&src) {
            // `{{#include ...}}` pulls in a file from `examples/`, checked above.
            if block.code.trim_start().starts_with("{{#include") {
                continue;
            }
            blocks += 1;
            let lowered = diagram::lower_as(&block.code, Some(block.lang));
            for d in lowered
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
            {
                failures.push(format!(
                    "{}:{}: {}",
                    path.file_name().unwrap().to_string_lossy(),
                    block.line,
                    d.message
                ));
            }
        }
    }
    assert!(blocks >= 35, "only {blocks} blocks found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
