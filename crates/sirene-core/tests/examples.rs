//! The diagrams in `examples/` parse without diagnostics and render in every format.

use std::path::Path;

use sirene_core::ascii::Options;
use sirene_core::diagram::{self, Lang};
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
