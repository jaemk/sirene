//! The diagrams in `examples/` parse without diagnostics and render in every format.

use std::path::Path;

use sirene_core::ascii::{self, Options};
use sirene_core::flowchart;
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{self, Theme, svg};

#[test]
fn examples_render_cleanly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .collect();
    paths.sort();
    assert!(paths.len() >= 6, "{paths:?}");
    for path in paths {
        let src = std::fs::read_to_string(&path).unwrap();
        let lowered = flowchart::parse(&src).lower();
        assert!(
            lowered.diagnostics.is_empty(),
            "{}: {:?}",
            path.display(),
            lowered.diagnostics
        );
        let chart = lowered.flowchart;
        assert!(!chart.nodes.is_empty(), "{}", path.display());
        for theme in [Theme::LIGHT, Theme::DARK] {
            let s = scene::flowchart(&chart, &FontMetrics, &theme);
            assert!(svg::write(&s).starts_with("<svg"), "{}", path.display());
            png::write(&s, png::Options::new(png::Quality::Normal))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        assert!(!ascii::render(&chart, &Options::default()).trim().is_empty());
    }
}
