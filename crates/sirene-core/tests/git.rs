//! Mermaid git graphs (MGIT-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the graph each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::git::{self, CommitType, Direction, GitGraph};
use sirene_core::scene::{Approx, Owner, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/git");
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

fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(out, "{line}:{col}: {severity}: {}", d.message).unwrap();
    }
    out
}

/// Each fixture's diagnostics and drawing; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(git::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Git, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
                assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = diagram::lower(&src).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn graph(src: &str) -> GitGraph {
    let lowered = diagram::lower(src);
    assert!(
        lowered.diagnostics.is_empty(),
        "{:?}\n{src}",
        lowered.diagnostics
    );
    match lowered.diagram {
        Diagram::Git(g) => g,
        other => panic!("not a git graph: {other:?}"),
    }
}

fn names(g: &GitGraph) -> Vec<&str> {
    g.branches.iter().map(|b| b.name.as_str()).collect()
}

/// MGIT-2: commits follow their branch's head, and `branch` starts at the current head.
#[test]
fn commits_and_branches() {
    let g = graph(
        "gitGraph\n  commit id: \"a\" msg: \"first\"\n  branch dev\n  commit id: \"b\" type: REVERSE\n  \
         switch main\n  commit id: \"c\" type: HIGHLIGHT tag: \"t1\" tag: \"t2\"\n",
    );
    assert_eq!(names(&g), ["main", "dev"]);
    let ids: Vec<&str> = g.commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    let (a, b, c) = (&g.commits[0], &g.commits[1], &g.commits[2]);
    assert_eq!((a.branch, b.branch, c.branch), (0, 1, 0));
    assert_eq!(a.parents, [] as [usize; 0]);
    assert_eq!(b.parents, [0]);
    assert_eq!(c.parents, [0]);
    assert_eq!(a.message.as_deref(), Some("first"));
    assert_eq!(b.kind, CommitType::Reverse);
    assert_eq!(c.kind, CommitType::Highlight);
    assert_eq!(c.tags, ["t1", "t2"]);
    assert_eq!(g.direction, Direction::LeftRight);
    // `msg:` is optional before a commit's message.
    let g = graph("gitGraph\n  commit \"hello\"\n  commit id: \"x\" \"bye\" tag: \"t\"\n");
    assert_eq!(g.commits[0].message.as_deref(), Some("hello"));
    assert_eq!(g.commits[1].message.as_deref(), Some("bye"));
    assert_eq!(g.commits[1].parents, [0]);
    // Generated ids are stable and differ per commit.
    let g1 = graph("gitGraph\n  commit\n  commit\n");
    let g2 = graph("gitGraph\n  commit\n  commit\n");
    assert_eq!(g1, g2);
    assert_ne!(g1.commits[0].id, g1.commits[1].id);
    assert!(g1.commits[0].id.starts_with("0-") && g1.commits[0].id.len() == 9);
    for (src, d) in [
        ("gitGraph TB:\n", Direction::TopBottom),
        ("gitGraph BT\n", Direction::BottomTop),
        ("gitGraph:\n", Direction::LeftRight),
    ] {
        assert_eq!(graph(src).direction, d);
    }
}

#[test]
fn merges_and_cherry_picks() {
    let g = graph(
        "gitGraph\n  commit id: \"a\"\n  branch dev\n  commit id: \"b\"\n  checkout main\n  \
         merge dev id: \"m\" tag: \"v1\"\n  branch rel\n  commit id: \"r\"\n  cherry-pick id: \"b\"\n  \
         cherry-pick id: \"m\" parent: \"b\" tag: \"mine\"\n",
    );
    let m = &g.commits[2];
    assert!(m.is_merge());
    assert_eq!(m.parents, [0, 1]);
    assert_eq!(m.tags, ["v1"]);
    let pick = &g.commits[4];
    assert_eq!(pick.cherry_pick.as_deref(), Some("b"));
    assert_eq!(pick.tags, ["cherry-pick:b"]);
    assert_eq!(pick.parents, [3]);
    // A given tag replaces the generated one.
    assert_eq!(g.commits[5].tags, ["mine"]);
}

/// MGIT-2: `main` first, then branches without an order as `0.k`, then ordered ones.
#[test]
fn branch_order() {
    let g = graph(
        "gitGraph\n  commit\n  branch late order: 3\n  branch early order: 1\n  branch plain\n  \
         branch zero order: 0\n",
    );
    assert_eq!(names(&g), ["main", "zero", "plain", "early", "late"]);
    // The tenth branch without an order is `0.10`, before the second (`0.2`).
    let mut src = "gitGraph\n  commit\n".to_owned();
    for k in 1..=10 {
        writeln!(src, "  branch b{k}").unwrap();
    }
    let g = graph(&src);
    assert_eq!(&names(&g)[..4], ["main", "b1", "b10", "b2"]);
}

/// MGIT-3: each problem is an error on its statement, and the rest is drawn.
#[test]
fn errors() {
    let errors = |src: &str| -> Vec<String> {
        let l = diagram::lower(src);
        assert!(l.diagnostics.iter().all(|d| d.severity == Severity::Error));
        l.diagnostics.into_iter().map(|d| d.message).collect()
    };
    let g = "gitGraph\n  commit id: \"a\"\n";
    for (tail, want) in [
        ("branch main\n", "branch `main` already exists"),
        ("checkout x\n", "unknown branch `x`"),
        ("merge x\n", "unknown branch `x`"),
        ("merge main\n", "cannot merge `main` into itself"),
        (
            "branch d\n  merge main\n",
            "`main` and `d` have the same head",
        ),
        (
            "branch d\n  checkout main\n  merge d\n",
            "`d` and `main` have the same head",
        ),
        ("commit id: \"a\"\n", "commit id `a` is already used"),
        ("commit order: 1\n", "`order` does not apply to `commit`"),
        (
            "commit type: X\n",
            "expected `NORMAL`, `REVERSE`, or `HIGHLIGHT`",
        ),
        ("branch d order: x\n", "`order` takes a whole number"),
        ("cherry-pick id: \"q\"\n", "unknown commit `q`"),
        (
            "cherry-pick id: \"a\"\n",
            "commit `a` is already on this branch",
        ),
        (
            "cherry-pick tag: \"t\"\n",
            "`cherry-pick` needs `id: \"...\"`",
        ),
    ] {
        assert_eq!(errors(&format!("{g}  {tail}")), [want], "{tail}");
    }
    assert_eq!(
        errors("gitGraph\n  branch d\n  commit id: \"x\"\n  checkout main\n  merge d\n"),
        ["cannot merge into `main`, which has no commits"]
    );
    assert_eq!(
        errors(
            "gitGraph\n  commit\n  branch d\n  checkout main\n  merge d\n  branch e\n  merge d\n"
        ),
        [
            "`d` and `main` have the same head",
            "`d` and `e` have the same head"
        ]
    );
    // Merging the same branch into two others is fine.
    assert!(
        errors(
            "gitGraph\n  commit\n  branch d\n  branch e\n  checkout d\n  commit\n  checkout e\n  \
             merge d\n  checkout main\n  merge d\n"
        )
        .is_empty()
    );
    let merged = "gitGraph\n  commit id: \"a\"\n  branch d\n  commit id: \"b\"\n  checkout main\n  \
                  commit id: \"c\"\n  merge d id: \"m\"\n  branch e\n  commit id: \"x\"\n";
    assert_eq!(
        errors(&format!("{merged}  cherry-pick id: \"m\"\n")),
        ["`m` is a merge: name one of its parents with `parent:`"]
    );
    assert_eq!(
        errors(&format!("{merged}  cherry-pick id: \"m\" parent: \"a\"\n")),
        ["`a` is not a parent of `m`"]
    );
    // A commit from another branch may be picked, as Mermaid allows.
    assert!(
        errors(
            "gitGraph\n  commit id: \"a\"\n  branch e\n  checkout main\n  branch f\n  checkout e\n  \
             cherry-pick id: \"a\"\n"
        )
        .is_empty()
    );
    assert_eq!(
        errors(
            "gitGraph\n  branch e\n  commit id: \"a\"\n  checkout main\n  cherry-pick id: \"a\"\n"
        ),
        ["cannot cherry-pick onto `main`, which has no commits"]
    );
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(errors("gitGraph\n  branch\n"), ["expected a branch name"]);
    assert_eq!(
        errors("gitGraph\n  push\n"),
        ["expected `commit`, `branch`, `checkout`, `merge`, or `cherry-pick`"]
    );
    assert_eq!(
        errors("gitGraph\n  commit foo\n"),
        ["expected an attribute such as `id: \"...\"`"]
    );
}

/// MGIT-4: lanes in order, commits along them, labels for ids and tags.
#[test]
fn drawing() {
    let src = "gitGraph\n  commit id: \"a\"\n  branch dev\n  commit id: \"b\" tag: \"v1\"\n  \
               checkout main\n  merge dev\n";
    let lowered = diagram::lower(src);
    let s = lowered.diagram.scene(&Approx, &Theme::LIGHT);
    let text_at = |want: &str| {
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == want => Some(*center),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {want}"))
    };
    let (main, dev) = (text_at("main"), text_at("dev"));
    assert!(main.y < dev.y, "lanes top to bottom");
    let (a, b) = (text_at("a"), text_at("b"));
    assert!(a.x < b.x, "commits left to right");
    assert!(text_at("v1").y < b.y, "tags above, ids below");
    // Top to bottom puts lanes left to right and commits down.
    let tb = diagram::lower(&src.replace("gitGraph", "gitGraph TB:")).diagram;
    let s = tb.scene(&Approx, &Theme::LIGHT);
    let at = |want: &str| {
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == want => Some(*center),
                _ => None,
            })
            .unwrap()
    };
    assert!(at("main").x < at("dev").x);
    assert!(at("a").y < at("b").y);
    // Bottom to top mirrors it: later commits are higher.
    let bt = diagram::lower(&src.replace("gitGraph", "gitGraph BT:")).diagram;
    let s = bt.scene(&Approx, &Theme::LIGHT);
    let pos = |want: &str| {
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == want => Some(*center),
                _ => None,
            })
            .unwrap()
    };
    assert!(pos("a").y > pos("b").y);
    assert!(pos("main").y > pos("a").y, "branch names below");
    // ASCII flips rows for bottom to top.
    let down = tb.ascii(&Options::default());
    let up = bt.ascii(&Options::default());
    let lines = |s: &str| s.lines().map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(lines(&down)[0], "main");
    assert_eq!(lines(&up).last().map(String::as_str), Some("main"));
}
