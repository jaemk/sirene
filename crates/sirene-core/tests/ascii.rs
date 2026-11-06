//! Snapshots of every flowchart fixture rendered as text (ASCIIR-*).

use std::fs;
use std::path::Path;

use sirene_core::ascii::{self, Charset, Options};
use sirene_core::flowchart;

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flowchart");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
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

fn render(src: &str, charset: Charset) -> String {
    let chart = flowchart::parse(src).lower().flowchart;
    ascii::render(&chart, &Options { charset })
}

#[test]
fn unicode_snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            let out = render(&src, Charset::Unicode);
            insta::assert_snapshot!(format!("unicode__{dir}__{name}"), out, &src);
        }
    }
}

#[test]
fn ascii_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = render(&src, Charset::Ascii);
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}
