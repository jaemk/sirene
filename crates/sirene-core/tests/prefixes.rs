//! Every fixture, example, and diagram block of the guide, cut short at each line and halfway
//! through each line, with a line left out, or with non-ASCII text around a line, still draws:
//! an unfinished diagram (the editor's state while typing) never panics in any renderer. Whole,
//! each fixture draws with only ASCII characters under `Charset::Ascii` (ASCIIR-2).

use std::path::{Path, PathBuf};

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lang};
use sirene_core::markdown;
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

/// The examples, and the guide's inline diagram blocks (an `{{#include}}` is an example).
fn examples_and_guide() -> Vec<(String, String, Option<Lang>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    let mut examples = Vec::new();
    fixtures(&root.join("examples"), &mut examples);
    examples.sort();
    for path in examples {
        let src = std::fs::read_to_string(&path).unwrap();
        out.push((path.display().to_string(), src, Lang::of_path(&path)));
    }
    let mut pages: Vec<_> = std::fs::read_dir(root.join("docs/src"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    pages.sort();
    for path in pages {
        let src = std::fs::read_to_string(&path).unwrap();
        for block in markdown::mermaid_blocks(&src) {
            if !block.code.trim_start().starts_with("{{#include") {
                let name = format!("{}:{}", path.display(), block.line);
                out.push((name, block.code, Some(block.lang)));
            }
        }
    }
    out
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

/// Draws `src` cut short, without each line, and with non-ASCII text around each line.
fn edits_draw(name: &str, src: &str, lang: Option<Lang>) {
    let draw = |text: &str, what: &str| {
        let run = std::panic::catch_unwind(|| {
            let lowered = diagram::lower_as(text, lang);
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            lowered.diagram.ascii(&Options::UNICODE).unwrap();
            svg::write(&s)
        });
        let doc = run.unwrap_or_else(|_| panic!("{name} {what}:\n{text}"));
        // The SVG is well-formed XML, whatever the labels hold.
        if let Err(e) = roxmltree::Document::parse(&doc) {
            panic!("{name} {what}: {e}\n{text}\n{doc}");
        }
    };
    for cut in cuts(src) {
        draw(&src[..cut], &format!("cut at byte {cut}"));
    }
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    for i in 0..lines.len() {
        let mut edited = lines.clone();
        edited.remove(i);
        draw(&edited.concat(), &format!("without line {}", i + 1));
        let wide = format!("é{}", lines[i].replace('\n', "日\n"));
        let mut edited = lines.clone();
        edited[i] = &wide;
        draw(
            &edited.concat(),
            &format!("with non-ASCII around line {}", i + 1),
        );
    }
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
        edits_draw(&path.display().to_string(), &src, lang);
        // With the ASCII charset, no drawing characters (box drawing, blocks, shapes, arrows,
        // guillemets, dots) are used except those in the source's own text.
        let text = diagram::lower_as(&src, lang)
            .diagram
            .ascii(&Options {
                charset: Charset::Ascii,
            })
            .unwrap();
        let drawing = |c: char| matches!(c, '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{25ff}' | '«' | '»' | '·' | '…');
        let stray: String = text
            .chars()
            .filter(|&c| drawing(c) && !src.contains(c))
            .collect();
        assert!(stray.is_empty(), "{}: {stray:?}\n{text}", path.display());
    }
}

#[test]
fn example_and_guide_prefixes_draw() {
    let sources = examples_and_guide();
    assert!(sources.len() >= 40, "only {} sources", sources.len());
    for (name, src, lang) in &sources {
        edits_draw(name, src, *lang);
    }
}
