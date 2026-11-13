//! Every fixture, cut short at each line and halfway through each line, still draws: an
//! unfinished diagram (the editor's state while typing) never panics in any renderer.

use std::path::{Path, PathBuf};

use sirene_core::ascii::Options;
use sirene_core::diagram::{self, Lang};
use sirene_core::scene::{Approx, Theme, svg};

fn fixtures(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fixtures(&path, out);
        } else if Lang::of_path(&path).is_some() {
            out.push(path);
        }
    }
}

/// Byte offsets at each line start and at the char nearest the middle of each line.
fn cuts(src: &str) -> Vec<usize> {
    let mut cuts = vec![src.len()];
    let mut start = 0;
    for line in src.split_inclusive('\n') {
        cuts.push(start);
        let mid = (start..=start + line.len() / 2)
            .rev()
            .find(|&i| src.is_char_boundary(i))
            .unwrap();
        cuts.push(mid);
        start += line.len();
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts
}

#[test]
fn fixture_prefixes_draw() {
    let mut paths = Vec::new();
    fixtures(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        &mut paths,
    );
    paths.sort();
    assert!(paths.len() > 200, "{}", paths.len());
    for path in &paths {
        let src = std::fs::read_to_string(path).unwrap();
        let lang = Lang::of_path(path);
        let draw = |text: &str, what: &str| {
            let run = std::panic::catch_unwind(|| {
                let lowered = diagram::lower_as(text, lang);
                let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
                svg::write(&s);
                lowered.diagram.ascii(&Options::default());
            });
            assert!(run.is_ok(), "{} {what}:\n{text}", path.display());
        };
        for cut in cuts(&src) {
            draw(&src[..cut], &format!("cut at byte {cut}"));
        }
        let lines: Vec<&str> = src.split_inclusive('\n').collect();
        for skip in 0..lines.len() {
            let text: String = lines
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != skip)
                .map(|(_, line)| *line)
                .collect();
            draw(&text, &format!("without line {}", skip + 1));
        }
    }
}
