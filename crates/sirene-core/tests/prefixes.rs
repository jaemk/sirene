//! Every fixture, cut short at each line and halfway through each line, or with a line left
//! out, still draws: an unfinished diagram (the editor's state while typing) never panics in any
//! renderer. Whole, each draws with only ASCII characters under `Charset::Ascii` (ASCIIR-2).

use std::path::{Path, PathBuf};

use sirene_core::ascii::{Charset, Options};
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
                lowered.diagram.ascii(&Options::default());
                svg::write(&s)
            });
            let doc = run.unwrap_or_else(|_| panic!("{} {what}:\n{text}", path.display()));
            // The SVG is well-formed XML, whatever the labels hold.
            if let Err(e) = roxmltree::Document::parse(&doc) {
                panic!("{} {what}: {e}\n{text}\n{doc}", path.display());
            }
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
        // With the ASCII charset, no drawing characters (box drawing, blocks, shapes, arrows,
        // guillemets, dots) are used except those in the source's own text.
        let text = diagram::lower_as(&src, lang).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        let drawing = |c: char| matches!(c, '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{25ff}' | '«' | '»' | '·' | '…');
        let stray: String = text
            .chars()
            .filter(|&c| drawing(c) && !src.contains(c))
            .collect();
        assert!(stray.is_empty(), "{}: {stray:?}\n{text}", path.display());
    }
}
