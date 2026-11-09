//! The `render` agent skill (`skills/render/SKILL.md`, AGENTS-2): its commands run as written.

use assert_cmd::Command;

fn skill() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../skills/render/SKILL.md");
    std::fs::read_to_string(path).expect("skills/render/SKILL.md")
}

#[test]
fn frontmatter() {
    let skill = skill();
    let front = skill
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .expect("frontmatter")
        .0;
    assert!(front.lines().any(|l| l == "name: render"), "{front}");
    assert!(
        front
            .lines()
            .any(|l| l.starts_with("description: ") && l.len() > 40),
        "{front}"
    );
}

/// Each line of the ```sh block, comments dropped, runs and succeeds in a directory holding the
/// files it names.
#[test]
fn commands_run() {
    let skill = skill();
    let block = skill
        .split_once("```sh\n")
        .and_then(|(_, s)| s.split_once("```"))
        .expect("a ```sh block")
        .0;
    let dir = tempfile::tempdir().unwrap();
    let flowchart = "flowchart LR\n  A[Write] --> B{Ok?} -->|yes| C[Done]\n";
    std::fs::write(dir.path().join("diagram.mmd"), flowchart).unwrap();
    std::fs::write(
        dir.path().join("notes.md"),
        format!("# Notes\n\n```mermaid\n{flowchart}```\n"),
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/a.mmd"), flowchart).unwrap();

    let mut ran = 0;
    for line in block.lines() {
        let cmd = line.split_once('#').map_or(line, |(c, _)| c).trim();
        let args: Vec<&str> = cmd.split_whitespace().collect();
        assert_eq!(args.first(), Some(&"sirene"), "{line}");
        assert!(args.len() > 2, "a command and a file: {line}");
        Command::cargo_bin("sirene")
            .unwrap()
            .current_dir(dir.path())
            .args(&args[1..])
            .assert()
            .success();
        ran += 1;
    }
    assert!(ran >= 8, "{ran}");
    for out in [
        "diagram.png",
        "dark.png",
        "diagram.svg",
        "out/notes-1.png",
        "out/a.svg",
    ] {
        assert!(dir.path().join(out).is_file(), "{out}");
    }
    // The skill never edits the Markdown file it renders.
    assert!(
        !std::fs::read_to_string(dir.path().join("notes.md"))
            .unwrap()
            .contains("![")
    );
}
