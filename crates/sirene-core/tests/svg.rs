//! Snapshots of every valid flowchart fixture drawn as SVG (DISPLA-*).

use std::fs;
use std::path::Path;

use sirene_core::flowchart;
use sirene_core::scene::{self, Approx, Theme};

fn fixtures() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flowchart/valid");
    let mut out: Vec<(String, String)> = fs::read_dir(root)
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p
                .file_stem()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&p).expect("readable fixture"))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn svg_snapshots() {
    for (name, src) in fixtures() {
        let chart = flowchart::parse(&src).lower().flowchart;
        let out = scene::svg::write(&scene::flowchart(&chart, &Approx, &Theme::LIGHT));
        insta::assert_snapshot!(format!("svg__{name}"), out, &src);
    }
}
