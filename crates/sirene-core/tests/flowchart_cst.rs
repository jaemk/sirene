//! Snapshot tests of the flowchart syntax tree over fixture files.
//!
//! Every fixture must round-trip byte for byte (FLOWCH-1). Fixtures under `valid/` must parse
//! without diagnostics; fixtures under `errors/` must produce at least one.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::flowchart;

fn render(src: &str) -> (String, usize) {
    let parse = flowchart::parse(src);
    assert_eq!(
        parse.syntax().to_string(),
        src,
        "tree text differs from source"
    );
    let mut out = parse.debug_tree();
    out.push_str("--- diagnostics ---\n");
    for d in parse.diagnostics() {
        writeln!(out, "{:?}: {}", d.range, d.message).unwrap();
    }
    (out, parse.diagnostics().len())
}

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/flowchart")
        .join(dir);
    let mut out: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no fixtures in {}", dir.display());
    out
}

#[test]
fn valid_fixtures() {
    for (name, src) in fixtures("valid") {
        let (out, errors) = render(&src);
        assert_eq!(errors, 0, "{name} has diagnostics:\n{out}");
        insta::assert_snapshot!(format!("valid__{name}"), out);
    }
}

#[test]
fn error_fixtures() {
    for (name, src) in fixtures("errors") {
        let (out, errors) = render(&src);
        assert!(errors > 0, "{name} has no diagnostics");
        insta::assert_snapshot!(format!("errors__{name}"), out);
    }
}

#[test]
fn empty_input() {
    let (out, errors) = render("");
    assert_eq!(errors, 1);
    assert!(out.contains("expected `flowchart` or `graph` header"));
}

#[test]
fn crlf_line_endings() {
    let src = "flowchart LR\r\n    A --> B\r\n    %% comment\r\n    B --> C\r\n";
    let parse = flowchart::parse(src);
    assert_eq!(parse.syntax().to_string(), src);
    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
}

#[test]
fn no_trailing_newline() {
    let parse = flowchart::parse("flowchart LR\n    A --> B[label]");
    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
}

#[test]
fn comment_after_semicolon_is_not_a_comment() {
    let parse = flowchart::parse("flowchart LR\n    A;%% not a comment\n");
    assert_eq!(parse.diagnostics().len(), 1);
}
