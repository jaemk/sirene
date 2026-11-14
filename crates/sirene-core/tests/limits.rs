//! Sources built to exhaust the stack, the heap, or the clock: deep nesting and long chains in
//! every language are read and drawn, with one error where a limit cuts in (LANGS-8).

use std::fmt::Write as _;

use sirene_core::ascii::Options;
use sirene_core::diagram;
use sirene_core::scene::{Approx, Theme};

/// Deeper than any parser recurses on a test thread's stack without a limit.
const DEEP: usize = 20_000;

/// Reads and draws `src` both ways, and gives each distinct diagnostic message with how often
/// it came, in order of first appearance.
fn draw(src: &str) -> Vec<(String, usize)> {
    let l = diagram::lower(src);
    let s = l.diagram.scene(&Approx, &Theme::LIGHT);
    assert!(s.width.is_finite() && s.height.is_finite());
    let text = l.diagram.ascii(&Options::default());
    assert!(text.is_empty() || text.ends_with('\n'));
    let mut out: Vec<(String, usize)> = Vec::new();
    for d in l.diagnostics {
        match out.iter_mut().find(|(m, _)| *m == d.message) {
            Some(entry) => entry.1 += 1,
            None => out.push((d.message, 1)),
        }
    }
    out
}

/// Draws `src` and checks its diagnostics are `expected`, once each.
#[track_caller]
fn check(src: &str, expected: &[&str]) {
    let expected: Vec<(String, usize)> = expected.iter().map(|m| ((*m).to_owned(), 1)).collect();
    assert_eq!(draw(src), expected);
}

/// `open` and `close` lines nested `DEEP` deep around `body`, under `header`.
fn nested(header: &str, open: impl Fn(usize) -> String, body: &str, close: &str) -> String {
    nest(header, DEEP, open, body, close)
}

fn nest(
    header: &str,
    depth: usize,
    open: impl Fn(usize) -> String,
    body: &str,
    close: &str,
) -> String {
    let mut s = format!("{header}\n");
    for i in 0..depth {
        s.push_str(&open(i));
        s.push('\n');
    }
    s.push_str(body);
    s.push('\n');
    for _ in 0..depth {
        s.push_str(close);
        s.push('\n');
    }
    s
}

/// A block past the limit is kept as text. Every prefix of a source nested past it, with
/// closers in comments, strings, and directives, parses to a tree that holds it exactly.
#[test]
fn skipped_blocks_keep_their_text() {
    type Text = fn(&str) -> String;
    let body = "%% } end\n%%{init: {\"a\": \"}\nend\"}}%%\nx[\"} end \u{e9}\"]; end\n{ \"}\" }";
    let cases: [(Text, String); 6] = [
        (
            |s| sirene_core::flowchart::parse(s).syntax().to_string(),
            nest(
                "flowchart TD",
                67,
                |i| format!("subgraph s{i}"),
                body,
                "end",
            ),
        ),
        (
            |s| sirene_core::state::parse(s).syntax().to_string(),
            nest(
                "stateDiagram-v2",
                67,
                |i| format!("state s{i} {{"),
                body,
                "}",
            ),
        ),
        (
            |s| sirene_core::class::parse(s).syntax().to_string(),
            nest(
                "classDiagram",
                67,
                |i| format!("namespace n{i} {{"),
                body,
                "}",
            ),
        ),
        (
            |s| sirene_core::c4::parse(s).syntax().to_string(),
            nest(
                "C4Context",
                67,
                |i| format!("Boundary(b{i}, \"B\") {{"),
                body,
                "}",
            ),
        ),
        (
            |s| sirene_core::zenuml::parse(s).syntax().to_string(),
            nest("zenuml", 67, |_| "if(x) {".into(), body, "}"),
        ),
        (
            |s| sirene_core::block::parse(s).syntax().to_string(),
            nest("block-beta", 15, |i| format!("block:b{i}"), body, "end"),
        ),
    ];
    for (text, src) in cases {
        for (i, _) in src.char_indices().chain([(src.len(), ' ')]) {
            let prefix = &src[..i];
            assert_eq!(text(prefix), prefix);
            if i % 16 == 0 {
                diagram::lower(prefix);
            }
        }
    }
}

/// A line per level, each indented one space more than the last.
fn indented(header: &str, depth: usize, line: impl Fn(usize) -> String) -> String {
    let mut s = format!("{header}\n");
    for i in 0..depth {
        let _ = writeln!(s, "{}{}", " ".repeat(i + 1), line(i));
    }
    s
}

#[test]
fn flowchart_subgraphs() {
    check(
        &nested("flowchart TD", |i| format!("subgraph s{i}"), "a", "end"),
        &["subgraphs nested more than 64 deep"],
    );
    // Left open: each subgraph that was read, and the limit at the first that was not.
    let mut open = String::from("flowchart TD\n");
    for i in 0..DEEP {
        let _ = writeln!(open, "subgraph s{i}");
    }
    assert_eq!(
        draw(&open),
        [
            ("unclosed `subgraph`: expected `end`".to_owned(), 65),
            ("subgraphs nested more than 64 deep".to_owned(), 1),
        ]
    );
}

#[test]
fn flowchart_chain() {
    let mut s = String::from("flowchart TD\n");
    for i in 0..20_000 {
        let _ = writeln!(s, "n{i} --> n{}", i + 1);
    }
    check(&s, &[]);
}

/// One long label on every edge of an `&` product is left out past the label budget, so a
/// short source does not copy and measure it tens of thousands of times.
#[test]
fn label_product() {
    let side = |p: &str| {
        (0..300)
            .map(|i| format!("{p}{i}"))
            .collect::<Vec<_>>()
            .join(" & ")
    };
    let label = "x".repeat(30_000);
    let src = format!("flowchart LR\n{} -->|{label}| {}\n", side("a"), side("b"));
    let l = diagram::lower(&src);
    let messages: Vec<_> = l.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        ["edge labels total more than 16 MiB; leaving out the rest"]
    );
    // Under the budget, the label is kept on every edge.
    let src = format!("flowchart LR\na & b -->|{label}| c & d\n");
    assert!(diagram::lower(&src).diagnostics.is_empty());
}

/// Metadata past `MAX_META_KEYS` per element is left out, reported once.
#[test]
fn metadata_keys() {
    let keys = |p: &str| {
        (0..300)
            .map(|i| format!("{p}{i}: v"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut src = String::from("flowchart LR\n");
    for i in 0..2_000 {
        src.push_str(&format!("a@{{ {} }}\n", keys(&format!("k{i}_"))));
    }
    let l = diagram::lower(&src);
    let messages: Vec<_> = l.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        ["an element has more than 256 metadata keys; leaving out the rest"]
    );
    assert!(
        diagram::lower("flowchart LR\na@{ a: 1, b: 2 }\na@{ b: 3 }\n")
            .diagnostics
            .is_empty()
    );
}

/// `classDef`, `class` and `linkStyle` lines naming many targets stop copying past the style
/// budget, reported once.
#[test]
fn style_product() {
    let names = vec!["a"; 200_000].join(",");
    let long = "x".repeat(400_000);
    let budget = "styles and classes applied total more than 64 MiB; leaving out the rest";
    for src in [
        format!("flowchart LR\na\nclassDef {names} stroke:{long}\n"),
        format!("flowchart LR\na\nclass {names} {long}\n"),
        format!(
            "flowchart LR\na --> b\nlinkStyle {} stroke:{long}\n",
            vec!["0"; 200_000].join(",")
        ),
    ] {
        let l = diagram::lower(&src);
        let messages: Vec<_> = l.diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, [budget]);
    }
    let src =
        "flowchart LR\na --> b\nclassDef c,d fill:#f00\nclass a,b c\nlinkStyle 0 stroke:red\n";
    assert!(diagram::lower(src).diagnostics.is_empty());
}

/// Subgraphs that list one another nest at most 65 deep, in either declaration order.
#[test]
fn subgraph_chain() {
    let chain = |next: fn(usize) -> usize| {
        let mut src = String::from("flowchart LR\n");
        for i in 1..20_000 {
            src.push_str(&format!("subgraph s{i}\ns{}\nend\n", next(i)));
        }
        diagram::lower(&src).diagnostics
    };
    // Each subgraph lists the next before it is declared, so each opens deeper in the chain.
    let down = chain(|i| i + 1);
    // Each subgraph lists the one before, so each wraps the chain so far.
    let up = chain(|i| i - 1);
    for d in [&down, &up] {
        assert!(!d.is_empty());
        assert!(
            d.iter()
                .all(|d| d.message.ends_with("nests subgraphs more than 65 deep")),
            "{:?}",
            d.first()
        );
    }
    assert!(
        diagram::lower("flowchart LR\nsubgraph a\nb\nend\nsubgraph c\na\nend\n")
            .diagnostics
            .is_empty()
    );
}

/// A `&` product past the edge cap is left out, reported once.
#[test]
fn flowchart_edge_product() {
    let side = |p: &str| {
        (0..400)
            .map(|i| format!("{p}{i}"))
            .collect::<Vec<_>>()
            .join(" & ")
    };
    let line = format!("{} --> {}\n", side("a"), side("b"));
    let src = format!("flowchart TD\n{line}{line}");
    check(
        &src,
        &["a flowchart has at most 100000 edges; leaving out the rest"],
    );
}

/// Spaces past the block cell cap are left out, reported once.
#[test]
fn block_spaces() {
    let src = format!("block-beta\ncolumns 1000\n{}", "space:1000\n".repeat(101));
    check(
        &src,
        &["a block diagram has at most 100000 cells; leaving out more spaces"],
    );
}

/// Many cells in groups nested to the limit are sized in time.
#[test]
fn block_deep_cells() {
    let cells: Vec<String> = (0..20_000).map(|i| format!("c{i}")).collect();
    let src = nest(
        "block-beta",
        8,
        |i| format!("block:b{i}"),
        &cells.join(" "),
        "end",
    );
    check(&src, &[]);
}

/// Unclosed strings and metadata on many lines do not each rescan the rest.
#[test]
fn unclosed_lines() {
    let lower = |src: &str| diagram::lower(src).diagnostics.len();
    let lines = |line: &str| format!("x\n{}", line.repeat(50_000));
    assert!(lower(&format!("block-beta\n{}", lines("a[\"x\n"))) >= 1);
    assert!(lower(&format!("block-beta\n{}", lines("a --\"x\n"))) >= 1);
    assert!(lower(&format!("mindmap\n{}", lines("  a((\"`x\n"))) >= 1);
    assert!(lower(&format!("mindmap\n{}", lines("  a@{\n"))) >= 1);
    for line in [
        "%%{x\n",
        "a[\"`x]\n",
        "a@{ label: \"x\n",
        "a@{\n",
        "accDescr {\n",
    ] {
        assert!(
            lower(&format!("flowchart TD\n{}", lines(line))) >= 1,
            "{line:?}"
        );
    }
    assert!(lower(&format!("stateDiagram-v2\n{}", lines("%%{x\n"))) >= 1);
}

/// C4 boundaries and relation styles, and ER and requirement classes, are looked up near
/// linearly.
#[test]
fn c4_er_requirement_lookups() {
    let mut s = String::from("C4Context\n");
    for i in 0..5_000 {
        let _ = writeln!(s, "Boundary(b{i}, \"B\") {{\nPerson(p{i}, \"P\")\n}}");
    }
    check(&s, &[]);
    let mut s = String::from("C4Context\nPerson(a, \"A\")\nPerson(b, \"B\")\n");
    for _ in 0..5_000 {
        s.push_str("Rel(a, b, \"r\")\n");
    }
    for _ in 0..5_000 {
        s.push_str("UpdateRelStyle(a, b, $lineColor=\"red\")\n");
    }
    check(&s, &[]);
    let names = (0..20_000)
        .map(|i| format!("c{i}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        diagram::lower(&format!("erDiagram\nA\nclassDef {names} fill:#f00\n"))
            .diagnostics
            .is_empty()
    );
    assert!(
        diagram::lower(&format!("requirementDiagram\nclassDef {names} fill:#f00\n"))
            .diagnostics
            .is_empty()
    );
}

/// Branch lookups stay near linear.
#[test]
fn git_checkouts() {
    let mut s = String::from("gitGraph\ncommit\n");
    for i in 0..20_000 {
        let _ = writeln!(s, "branch b{i}");
    }
    for _ in 0..20_000 {
        s.push_str("checkout b0\n");
    }
    assert!(diagram::lower(&s).diagnostics.is_empty());
    let order = format!("gitGraph\nbranch a order: {}\n", "9".repeat(400));
    check(&order, &["`order` takes a whole number"]);
}

/// Member lookups stay near linear: one subgraph with many members, and many subgraphs each
/// listed again by the next.
#[test]
fn flowchart_members() {
    let mut s = String::from("flowchart TD\nsubgraph big\n");
    for i in 0..20_000 {
        let _ = writeln!(s, "n{i}");
    }
    s.push_str("end\n");
    check(&s, &[]);
    let mut s = String::from("flowchart TD\n");
    for i in 0..2_000 {
        let _ = writeln!(s, "subgraph s{i}\na{i} --> b{i}\nend");
    }
    for i in 1..2_000 {
        let _ = writeln!(s, "subgraph t{i}\ns{}\nend", i - 1);
    }
    check(&s, &[]);
}

/// `a0 & ... --> b0 & ...` and DOT's `{...} -> {...}` make an edge per pair: 3,600 edges
/// through one gap, past the track preference limit.
#[test]
fn edge_products() {
    let side = |p: &str, sep: &str| {
        (0..60)
            .map(|i| format!("{p}{i}"))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let src = format!(
        "flowchart TD\n{} --> {}\n",
        side("a", " & "),
        side("b", " & ")
    );
    check(&src, &[]);
    let src = format!(
        "digraph {{ {{{}}} -> {{{}}} }}\n",
        side("a", " "),
        side("b", " ")
    );
    check(&src, &[]);
}

#[test]
fn flowchart_shapes() {
    draw(&format!(
        "flowchart TD\na{}x{}\n",
        "(".repeat(DEEP),
        ")".repeat(DEEP)
    ));
    draw(&format!(
        "flowchart TD\na{}x{}\n",
        "[".repeat(DEEP),
        "]".repeat(DEEP)
    ));
    draw(&format!(
        "flowchart TD\na{}x{}\n",
        "{".repeat(DEEP),
        "}".repeat(DEEP)
    ));
    draw(&format!(
        "flowchart TD\na[\"`{}x{}`\"]\n",
        "*".repeat(DEEP),
        "*".repeat(DEEP)
    ));
    draw(&format!("flowchart TD\na[\"`{}x`\"]\n", "**_".repeat(DEEP)));
}

#[test]
fn state_composites() {
    check(
        &nested("stateDiagram-v2", |i| format!("state s{i} {{"), "a", "}"),
        &["composite states nested more than 64 deep"],
    );
}

#[test]
fn sequence_blocks() {
    check(
        &nested("sequenceDiagram", |_| "loop x".into(), "A->>B: hi", "end"),
        &["blocks nested more than 64 deep"],
    );
    check(
        &nested(
            "sequenceDiagram",
            |_| "A->>+B: hi".into(),
            "A->>B: hi",
            "B-->>-A: ok",
        ),
        &["activations nested more than 64 deep"],
    );
}

#[test]
fn mindmap_levels() {
    check(
        &indented("mindmap", 3_000, |i| format!("n{i}")),
        &["nodes nested more than 64 deep"],
    );
}

#[test]
fn treemap_levels() {
    check(
        &indented("treemap-beta", 3_000, |i| format!("\"n{i}\"")),
        &[
            "section `n63` holds no items, so it is not drawn; give it a value such as \
             `\"n63\": 1`, or indent items under it",
            "sections nested more than 64 deep",
        ],
    );
}

#[test]
fn treeview_levels() {
    check(
        &indented("treeView-beta", 3_000, |i| format!("d{i}/")),
        &["entries nested more than 64 deep"],
    );
}

#[test]
fn ishikawa_levels() {
    check(
        &indented("ishikawa-beta", 3_000, |i| format!("cause {i}")),
        &["causes nested more than 64 deep"],
    );
}

#[test]
fn kanban_levels() {
    check(&indented("kanban", 3_000, |i| format!("n{i}[Task]")), &[]);
}

#[test]
fn block_blocks() {
    check(
        &nested("block-beta", |i| format!("block:b{i}"), "a", "end"),
        &["blocks nested more than 8 deep"],
    );
}

#[test]
fn c4_boundaries() {
    check(
        &nested(
            "C4Context",
            |i| format!("Boundary(b{i}, \"B\") {{"),
            "Person(a, \"A\")",
            "}",
        ),
        &["boundaries nested more than 64 deep"],
    );
}

#[test]
fn architecture_groups() {
    let mut s = String::from("architecture-beta\ngroup g0(cloud)[G]\n");
    for i in 1..DEEP {
        let _ = writeln!(s, "group g{i}(cloud)[G] in g{}", i - 1);
    }
    let _ = writeln!(s, "service s(server)[S] in g{}", DEEP - 1);
    check(
        &s,
        &["groups nested more than 64 deep; drawing a deeper group outside any group"],
    );
}

#[test]
fn zenuml_blocks() {
    check(
        &nested("zenuml", |_| "A.m() {".into(), "B.n()", "}"),
        &["blocks nested more than 64 deep"],
    );
    check(
        &nested("zenuml", |_| "if(x) {".into(), "A.m()", "}"),
        &["blocks nested more than 64 deep"],
    );
    draw(&format!(
        "zenuml\nA.m({}x{})\n",
        "f(".repeat(DEEP),
        ")".repeat(DEEP)
    ));
}

#[test]
fn railroad_expressions() {
    let (open, close) = ("( ".repeat(DEEP), " )".repeat(DEEP));
    check(
        &format!("railroad-ebnf-beta\nr = {open}\"a\"{close} ;\n"),
        &["groups nested more than 64 deep"],
    );
    let (open, close) = ("[ ".repeat(DEEP), " ]".repeat(DEEP));
    check(
        &format!("railroad-ebnf-beta\nr = {open}\"a\"{close} ;\n"),
        &["groups nested more than 64 deep"],
    );
    let (open, close) = ("optional(".repeat(DEEP), ")".repeat(DEEP));
    check(
        &format!("railroad-beta\nr = {open}terminal(\"a\"){close}\n"),
        &["expressions nested more than 64 deep"],
    );
}

#[test]
fn usecase_boundaries() {
    let d = draw(&nested(
        "usecase-beta",
        |i| format!("systemBoundary b{i}"),
        "U(\"use\")",
        "end",
    ));
    assert_eq!(d[0].0, "system boundaries cannot be nested");
    let (open, close) = ("[".repeat(DEEP), "]".repeat(DEEP));
    check(
        &format!("usecase-beta\njson P@{{ \"a\": {open}1{close} }}\n"),
        &["invalid JSON at byte 70: values nested more than 64 deep"],
    );
    let (open, close) = ("{\"a\":".repeat(DEEP), "}".repeat(DEEP));
    check(
        &format!("usecase-beta\njson P@{{ \"a\": {open}1{close} }}\n"),
        &["invalid JSON at byte 322: values nested more than 64 deep"],
    );
}

#[test]
fn class_generics_and_chains() {
    draw(&format!(
        "classDiagram\nclass A{}B{}\n",
        "~List".repeat(DEEP),
        "~".repeat(DEEP)
    ));
    check(
        &nested(
            "classDiagram",
            |i| format!("namespace n{i} {{"),
            "class A",
            "}",
        ),
        &["namespaces nested more than 64 deep"],
    );
    let mut s = String::from("classDiagram\n");
    for i in 0..2_000 {
        let _ = writeln!(s, "C{i} <|-- C{}", i + 1);
    }
    check(&s, &[]);
}

#[test]
fn gantt_dependencies() {
    let mut s = String::from("gantt\ndateFormat YYYY-MM-DD\nt0 : t0, 2026-01-01, 1d\n");
    for i in 1..DEEP {
        let _ = writeln!(s, "t{i} : t{i}, after t{}, 1d", i - 1);
    }
    check(&s, &[]);
    // Each task waits for the one after it.
    let mut s = String::from("gantt\ndateFormat YYYY-MM-DD\n");
    for i in 0..DEEP {
        let _ = writeln!(s, "t{i} : t{i}, after t{}, 1d", i + 1);
    }
    let _ = writeln!(s, "t{DEEP} : t{DEEP}, 2026-01-01, 1d");
    check(&s, &[]);
}

/// Long lists of states, sibling nodes, slices, and flows, forward and backward: each is read
/// and drawn in time near linear in its length.
#[test]
fn wide_inputs() {
    let lines = |header: &str, line: &dyn Fn(usize) -> String| {
        let mut s = format!("{header}\n");
        for i in 0..10_000 {
            let _ = writeln!(s, "{}", line(i));
        }
        s
    };
    check(
        &lines("stateDiagram-v2", &|i| format!("s{i} --> s{}", i + 1)),
        &[],
    );
    check(&lines("mindmap\nroot", &|i| format!("  c{i}")), &[]);
    check(&lines("pie", &|i| format!("\"s{i}\" : 1")), &[]);
    check(
        &lines("sankey-beta", &|i| format!("a{i},a{},1", i + 1)),
        &[],
    );
    check(
        &lines("sankey-beta", &|i| format!("a{},a{i},1", i + 1)),
        &[],
    );
}

#[test]
fn git_branches() {
    let mut s = String::from("gitGraph\ncommit\n");
    for i in 0..2_000 {
        let _ = writeln!(s, "branch b{i}\ncommit");
    }
    check(&s, &[]);
}

#[test]
fn config_nesting() {
    let (open, close) = ("{\"a\":".repeat(DEEP), "}".repeat(DEEP));
    draw(&format!("%%{{init: {open}1{close}}}%%\nflowchart TD\na\n"));
    let (open, close) = ("[".repeat(DEEP), "]".repeat(DEEP));
    draw(&format!(
        "%%{{init: {{\"a\": {open}1{close}}}}}%%\nflowchart TD\na\n"
    ));
    let yaml = indented("---\nconfig:", 3_000, |i| format!("k{i}:"));
    draw(&format!("{yaml}---\nflowchart TD\na\n"));
    draw(&format!(
        "---\nconfig: {open}1{close}\n---\nflowchart TD\na\n"
    ));
}

#[test]
fn dot_graphs() {
    let d = draw(&format!("digraph {{ {} }}", "{ a -> ".repeat(DEEP)));
    assert_eq!(d[0], ("subgraphs nested more than 64 deep".to_owned(), 1));
    draw(&format!(
        "digraph {{ a [label=\"{}\" shape=record] }}",
        "{".repeat(DEEP)
    ));
    draw(&format!(
        "digraph {{ a [label=<{}x>] }}",
        "<b>".repeat(DEEP)
    ));
}
