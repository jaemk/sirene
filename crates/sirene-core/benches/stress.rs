//! Stress shapes for every diagram kind, each timed at two sizes four times apart. A phase that
//! scales linearly takes about 4x as long at the larger size and a quadratic one about 16x; the
//! report flags phases whose growth is above `--flag` (default 8) or that are slow outright.
//!
//! Run with `cargo bench -p sirene-core --bench stress`; arguments filter shapes by name
//! (`cargo bench -p sirene-core --bench stress -- flowchart sequence`). `--png` also times PNG
//! rasterizing (needs `--features png`), and `--base N` scales every size. `--phases` adds a line
//! per shape with the phase timings (ERRORH-8) of one run at the larger size, such as layout
//! ordering and track assignment, to find which part of a flagged phase grew.

use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sirene_core::ascii::Options;
use sirene_core::diagram::{self, Lang};
use sirene_core::profile;
use sirene_core::scene::{Approx, Theme};

/// Collects phase timings while the log level allows them; they are off while timing.
struct Phases(Mutex<Vec<(String, u128)>>);

impl log::Log for Phases {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target() == profile::TARGET
    }

    fn log(&self, record: &log::Record) {
        let message = record.args().to_string();
        if let (true, Some((name, ns))) =
            (self.enabled(record.metadata()), profile::parse(&message))
            && let Ok(mut phases) = self.0.lock()
        {
            phases.push((name.to_string(), ns));
        }
    }

    fn flush(&self) {}
}

static PHASES: Phases = Phases(Mutex::new(Vec::new()));

/// The phase timings of one run of lowering, scene building, text drawing, and with `--png`
/// PNG writing, summed per
/// phase in first-seen order, with how many times each ran.
fn phases(src: &str, lang: Option<Lang>, with_png: bool) -> String {
    if let Ok(mut p) = PHASES.0.lock() {
        p.clear();
    }
    log::set_max_level(log::LevelFilter::Trace);
    let lowered = diagram::lower_as(src, lang);
    let _ = lowered.diagram.scene(&Approx, &Theme::LIGHT);
    let _ = lowered.diagram.ascii(&Options::default());
    if with_png {
        png(&lowered, Duration::ZERO);
    }
    log::set_max_level(log::LevelFilter::Off);
    let records = PHASES.0.lock().map(|p| p.clone()).unwrap_or_default();
    let mut sums: Vec<(String, u128, usize)> = Vec::new();
    for (name, ns) in records {
        match sums.iter_mut().find(|(n, _, _)| *n == name) {
            Some(s) => {
                s.1 += ns;
                s.2 += 1;
            }
            None => sums.push((name, ns, 1)),
        }
    }
    sums.iter()
        .map(|(name, ns, runs)| {
            let ms = *ns as f64 / 1e6;
            if *runs > 1 {
                format!("{name} {ms:.2} (x{runs})")
            } else {
                format!("{name} {ms:.2}")
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A stress shape: its name, the language it is read as, and its source at a size.
struct Shape {
    name: &'static str,
    lang: Option<Lang>,
    /// Size at the base scale; the larger run uses four times this.
    size: usize,
    source: fn(usize) -> String,
}

/// `header`, then `line(i)` for each `i` below `n`, then `footer`.
fn lines(header: &str, n: usize, footer: &str, line: impl Fn(usize) -> String) -> String {
    let mut s = format!("{header}\n");
    for i in 0..n {
        let _ = writeln!(s, "{}", line(i));
    }
    s.push_str(footer);
    s
}

/// A deterministic pseudo-random number below `n` for index `i` and `salt`.
fn pick(i: usize, salt: usize, n: usize) -> usize {
    let mut x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (salt as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 29;
    (x % n.max(1) as u64) as usize
}

const M: Option<Lang> = Some(Lang::Mermaid);
const P: Option<Lang> = Some(Lang::PlantUml);
const D: Option<Lang> = Some(Lang::Dot);

#[rustfmt::skip]
const SHAPES: &[Shape] = &[
    // Flowcharts.
    Shape { name: "flowchart_chain", lang: M, size: 500, source: |n| lines("flowchart TD", n, "", |i| format!("n{i} --> n{}", i + 1)) },
    Shape { name: "flowchart_chain_lr", lang: M, size: 500, source: |n| lines("flowchart LR", n, "", |i| format!("n{i}[Node {i}] -->|e{i}| n{}", i + 1)) },
    Shape { name: "flowchart_random", lang: M, size: 300, source: |n| lines("flowchart TD", n * 2, "", |i| format!("n{} --> n{}", pick(i, 1, n), pick(i, 2, n))) },
    Shape { name: "flowchart_wide", lang: M, size: 300, source: |n| lines("flowchart TD", n, "", |i| format!("root --> c{i}")) },
    Shape { name: "flowchart_parallel_edges", lang: M, size: 200, source: |n| lines("flowchart TD", n, "", |i| format!("a -->|l{i}| b")) },
    Shape { name: "flowchart_grid", lang: M, size: 150, source: |n| {
        let side = (n as f64).sqrt() as usize + 1;
        lines("flowchart TD", n, "", |i| format!("g{i} --> g{}\ng{i} --> g{}", i + 1, i + side))
    } },
    Shape { name: "flowchart_nested_subgraphs", lang: M, size: 15, source: |n| {
        let mut s = "flowchart TD\n".to_owned();
        for i in 0..n { let _ = writeln!(s, "subgraph s{i}\nn{i} --> m{i}"); }
        for _ in 0..n { s.push_str("end\n"); }
        s
    } },
    Shape { name: "flowchart_many_subgraphs", lang: M, size: 100, source: |n| lines("flowchart TD", n, "", |i| format!("subgraph s{i}\na{i} --> b{i}\nend\nb{i} --> a{}", (i + 1) % n.max(1))) },
    Shape { name: "flowchart_cross_cluster", lang: M, size: 60, source: |n| {
        let mut s = lines("flowchart TD", n, "", |i| format!("subgraph s{i}\na{i}\nb{i}\nend"));
        for i in 0..n * 2 { let _ = writeln!(s, "a{} --> b{}", pick(i, 3, n), pick(i, 4, n)); }
        s
    } },
    Shape { name: "flowchart_long_labels", lang: M, size: 200, source: |n| lines("flowchart TD", n, "", |i| format!("n{i}[\"{}\"] --> n{}", "word ".repeat(40), i + 1)) },
    Shape { name: "flowchart_styles", lang: M, size: 300, source: |n| {
        let mut s = lines("flowchart TD", n, "", |i| format!("n{i} --> n{}", i + 1));
        for i in 0..n { let _ = writeln!(s, "classDef c{i} fill:#f{:02x},stroke:#333\nclass n{i} c{i}\nstyle n{i} color:red", i % 256); }
        for i in 0..n { let _ = writeln!(s, "linkStyle {i} stroke:blue"); }
        s
    } },
    Shape { name: "flowchart_shapes", lang: M, size: 300, source: |n| {
        const SHAPES: &[&str] = &["rect", "rounded", "stadium", "cyl", "circle", "diamond", "hex", "lean-r", "trap-b", "doc", "das", "notch-rect", "flag", "bolt"];
        lines("flowchart TD", n, "", |i| format!("n{i}@{{ shape: {}, label: \"S {i}\" }}\nn{i} --> n{}", SHAPES[i % SHAPES.len()], i + 1))
    } },
    // Everything on one line, to catch scans to the end of the line per item.
    Shape { name: "line_flowchart_chain", lang: M, size: 500, source: |n| format!("flowchart TD\n{}\n", (0..n).map(|i| format!("n{i}")).collect::<Vec<_>>().join(" --> ")) },
    Shape { name: "line_flowchart_semicolons", lang: M, size: 500, source: |n| format!("flowchart TD\n{}\n", (0..n).map(|i| format!("n{i}[\"N {i}\"] --> n{}", i + 1)).collect::<Vec<_>>().join("; ")) },
    Shape { name: "line_sequence_semicolons", lang: M, size: 500, source: |n| format!("sequenceDiagram\n{}\n", (0..n).map(|i| format!("A{} ->> A{}: m{i}", i % 5, (i + 1) % 5)).collect::<Vec<_>>().join("; ")) },
    Shape { name: "line_class_semicolons", lang: M, size: 500, source: |n| format!("classDiagram\n{}\n", (0..n).map(|i| format!("C{i} <|-- C{}", i + 1)).collect::<Vec<_>>().join("; ")) },
    Shape { name: "line_state_semicolons", lang: M, size: 500, source: |n| format!("stateDiagram-v2\n{}\n", (0..n).map(|i| format!("s{i} --> s{}", i + 1)).collect::<Vec<_>>().join("; ")) },
    Shape { name: "line_block_row", lang: M, size: 300, source: |n| format!("block-beta\ncolumns 8\n{}\n", (0..n).map(|i| format!("b{i}[\"B {i}\"]")).collect::<Vec<_>>().join(" ")) },
    Shape { name: "line_dot", lang: D, size: 500, source: |n| format!("digraph {{ {} }}\n", (0..n).map(|i| format!("n{i} -> n{} [label=\"e{i}\"];", i + 1)).collect::<Vec<_>>().join(" ")) },
    Shape { name: "line_dot_chain", lang: D, size: 500, source: |n| format!("digraph {{ {} }}\n", (0..n).map(|i| format!("n{i}")).collect::<Vec<_>>().join(" -> ")) },
    Shape { name: "flowchart_ampersand", lang: M, size: 100, source: |n| lines("flowchart TD", n, "", |i| format!("a{i} & b{i} --> c{i} & d{i} --> a{}", i + 1)) },
    // Sequence and friends.
    Shape { name: "sequence_messages", lang: M, size: 1_000, source: |n| lines("sequenceDiagram", n, "", |i| format!("A{} ->> A{}: message {i}", i % 10, (i + 3) % 10)) },
    Shape { name: "sequence_participants", lang: M, size: 100, source: |n| lines("sequenceDiagram", n, "", |i| format!("P{i} ->> P{}: m", i + 1)) },
    Shape { name: "sequence_nested_blocks", lang: M, size: 15, source: |n| {
        let mut s = "sequenceDiagram\n".to_owned();
        for i in 0..n { let _ = writeln!(s, "loop l{i}\nA ->> B: m{i}"); }
        for _ in 0..n { s.push_str("end\n"); }
        s
    } },
    Shape { name: "sequence_notes_activations", lang: M, size: 300, source: |n| lines("sequenceDiagram\nautonumber", n, "", |i| format!("A ->> +B: req {i}\nNote over A,B: note {i}\nB -->> -A: res")) },
    Shape { name: "sequence_alt_blocks", lang: M, size: 200, source: |n| lines("sequenceDiagram", n, "", |i| format!("alt a{i}\nA ->> B: x\nelse b\nB ->> A: y\nend")) },
    Shape { name: "zenuml_calls", lang: M, size: 300, source: |n| lines("zenuml", n, "", |i| format!("A.call{i}() {{\n  B.inner{i}()\n}}")) },
    // Class, state, ER, requirement.
    Shape { name: "class_chain", lang: M, size: 300, source: |n| lines("classDiagram", n, "", |i| format!("C{i} <|-- C{}", i + 1)) },
    Shape { name: "class_members", lang: M, size: 100, source: |n| lines("classDiagram", n, "", |i| format!("class C{i} {{\n  +int a{i}\n  +String b\n  +run(int x) bool\n  -helper()$ void\n}}\nC{i} --> C{}", (i + 1) % n.max(1))) },
    Shape { name: "class_namespaces", lang: M, size: 60, source: |n| lines("classDiagram", n, "", |i| format!("namespace N{i} {{\n  class A{i}\n  class B{i}\n}}\nA{i} --> B{}", pick(i, 5, n))) },
    Shape { name: "class_random", lang: M, size: 150, source: |n| lines("classDiagram", n * 2, "", |i| format!("C{} --> C{}", pick(i, 6, n), pick(i, 7, n))) },
    Shape { name: "state_chain", lang: M, size: 300, source: |n| lines("stateDiagram-v2", n, "", |i| format!("s{i} --> s{}: e{i}", i + 1)) },
    Shape { name: "state_composites", lang: M, size: 60, source: |n| lines("stateDiagram-v2", n, "", |i| format!("state C{i} {{\n  [*] --> a{i}\n  a{i} --> b{i}\n  b{i} --> [*]\n}}\nC{i} --> C{}", i + 1)) },
    Shape { name: "state_nested", lang: M, size: 15, source: |n| {
        let mut s = "stateDiagram-v2\n".to_owned();
        for i in 0..n { let _ = writeln!(s, "state S{i} {{\nx{i} --> y{i}"); }
        for _ in 0..n { s.push_str("}\n"); }
        s
    } },
    Shape { name: "state_forks", lang: M, size: 100, source: |n| lines("stateDiagram-v2", n, "", |i| format!("state f{i} <<fork>>\ns{i} --> f{i}\nf{i} --> a{i}\nf{i} --> b{i}\na{i} --> s{}\nb{i} --> s{}", i + 1, i + 1)) },
    Shape { name: "er_entities", lang: M, size: 100, source: |n| lines("erDiagram", n, "", |i| format!("E{i} {{\n  int id PK\n  string name\n  int e{} FK\n}}\nE{i} ||--o{{ E{} : has", i + 1, i + 1)) },
    Shape { name: "er_random", lang: M, size: 150, source: |n| lines("erDiagram", n * 2, "", |i| format!("E{} }}|--|{{ E{} : r{i}", pick(i, 8, n), pick(i, 9, n))) },
    Shape { name: "requirement_chain", lang: M, size: 100, source: |n| lines("requirementDiagram", n, "", |i| format!("requirement r{i} {{\n  id: {i}\n  text: requirement {i}\n  risk: high\n  verifymethod: test\n}}\nelement e{i} {{\n  type: sim\n}}\ne{i} - satisfies -> r{i}")) },
    // Charts.
    Shape { name: "gantt_tasks", lang: M, size: 500, source: |n| lines("gantt\ndateFormat YYYY-MM-DD\nsection s", n, "", |i| format!("t{i} : t{i}, 2026-01-01, {}d", i % 30 + 1)) },
    Shape { name: "gantt_after", lang: M, size: 500, source: |n| lines("gantt\ndateFormat YYYY-MM-DD\nsection s\nt0 : t0, 2026-01-01, 1d", n, "", |i| format!("t{} : t{}, after t{i}, 1d", i + 1, i + 1))  },
    Shape { name: "gantt_sections_excludes", lang: M, size: 200, source: |n| lines("gantt\ndateFormat YYYY-MM-DD\nexcludes weekends", n, "", |i| format!("section S{i}\nt{i} : 2026-01-01, {}d", i % 60 + 1)) },
    Shape { name: "pie_slices", lang: M, size: 500, source: |n| lines("pie title P", n, "", |i| format!("\"slice {i}\" : {}", i + 1)) },
    Shape { name: "journey_tasks", lang: M, size: 300, source: |n| lines("journey\ntitle J", n, "", |i| format!("section S{}\nTask {i}: {}: Me, You", i / 5, i % 5 + 1)) },
    Shape { name: "git_commits", lang: M, size: 300, source: |n| lines("gitGraph", n, "", |i| format!("commit id: \"c{i}\"")) },
    Shape { name: "git_branches", lang: M, size: 60, source: |n| lines("gitGraph\ncommit", n, "", |i| format!("branch b{i}\ncommit\ncheckout main\ncommit\nmerge b{i}")) },
    Shape { name: "mindmap_siblings", lang: M, size: 500, source: |n| lines("mindmap\nroot", n, "", |i| format!("  c{i}")) },
    Shape { name: "mindmap_deep", lang: M, size: 15, source: |n| lines("mindmap\nroot", n, "", |i| format!("{}c{i}", " ".repeat(i + 2))) },
    Shape { name: "mindmap_tree", lang: M, size: 300, source: |n| lines("mindmap\nroot", n, "", |i| format!("{}n{i}", "  ".repeat(1 + (i % 4)))) },
    Shape { name: "timeline_events", lang: M, size: 300, source: |n| lines("timeline\ntitle T", n, "", |i| format!("{} : event {i} : more {i}", 1900 + i)) },
    Shape { name: "quadrant_points", lang: M, size: 300, source: |n| lines("quadrantChart\nx-axis Low --> High\ny-axis Low --> High", n, "", |i| format!("P{i}: [{:.2}, {:.2}]", pick(i, 10, 100) as f32 / 100.0, pick(i, 11, 100) as f32 / 100.0)) },
    Shape { name: "xychart_points", lang: M, size: 1_000, source: |n| {
        let xs = (0..n).map(|i| format!("x{i}")).collect::<Vec<_>>().join(", ");
        let ys = (0..n).map(|i| (pick(i, 12, 100)).to_string()).collect::<Vec<_>>().join(", ");
        format!("xychart-beta\nx-axis [{xs}]\nbar [{ys}]\nline [{ys}]\n")
    } },
    Shape { name: "sankey_random", lang: M, size: 300, source: |n| lines("sankey-beta", n, "", |i| { let a = pick(i, 13, n / 2 + 1); format!("a{a},b{},{}", pick(i, 14, n / 2 + 1), i % 9 + 1) }) },
    Shape { name: "sankey_layers", lang: M, size: 300, source: |n| lines("sankey-beta", n, "", |i| format!("L{}_{},L{}_{},{}", i % 10, i / 10 % 10, i % 10 + 1, (i / 10 + 1) % 10, i % 5 + 1)) },
    Shape { name: "packet_fields", lang: M, size: 300, source: |n| lines("packet-beta", n, "", |i| format!("{}-{}: \"field {i}\"", i * 8, i * 8 + 7)) },
    Shape { name: "radar_axes", lang: M, size: 100, source: |n| {
        let axes = (0..n).map(|i| format!("a{i}")).collect::<Vec<_>>().join(", ");
        let vals = (0..n).map(|i| (pick(i, 15, 10)).to_string()).collect::<Vec<_>>().join(", ");
        format!("radar-beta\naxis {axes}\ncurve c1{{{vals}}}\ncurve c2{{{vals}}}\n")
    } },
    Shape { name: "treemap_wide", lang: M, size: 300, source: |n| lines("treemap-beta\n\"root\"", n, "", |i| format!("  \"leaf {i}\": {}", i % 50 + 1)) },
    Shape { name: "treemap_deep", lang: M, size: 15, source: |n| lines("treemap-beta", n, "", |i| format!("{}\"n{i}\"\n{}\"leaf{i}\": 3", "  ".repeat(i), "  ".repeat(i + 1))) },
    Shape { name: "kanban_cards", lang: M, size: 300, source: |n| lines("kanban", n, "", |i| if i % 20 == 0 { format!("col{i}[Column {i}]") } else { format!("  card{i}[Card {i} with some text]") }) },
    Shape { name: "block_columns", lang: M, size: 300, source: |n| lines("block-beta\ncolumns 8", n, "", |i| format!("b{i}[\"B {i}\"]")) },
    Shape { name: "block_edges", lang: M, size: 200, source: |n| {
        let mut s = lines("block-beta\ncolumns 6", n, "", |i| format!("b{i}"));
        for i in 0..n { let _ = writeln!(s, "b{} --> b{}", pick(i, 16, n), pick(i, 17, n)); }
        s
    } },
    Shape { name: "architecture_services", lang: M, size: 100, source: |n| {
        let mut s = lines("architecture-beta\ngroup g(cloud)[G]", n, "", |i| format!("service s{i}(server)[S {i}] in g"));
        for i in 0..n.saturating_sub(1) { let _ = writeln!(s, "s{i}:R --> L:s{}", i + 1); }
        s
    } },
    Shape { name: "c4_containers", lang: M, size: 100, source: |n| {
        let mut s = lines("C4Context\nEnterprise_Boundary(b0, \"B\") {", n, "}\n", |i| format!("  System(s{i}, \"System {i}\", \"desc {i}\")"));
        for i in 0..n.saturating_sub(1) { let _ = writeln!(s, "Rel(s{i}, s{}, \"uses\")", i + 1); }
        s
    } },
    Shape { name: "venn_sets", lang: M, size: 8, source: |n| {
        let mut s = lines("venn-beta", n, "", |i| format!("set S{i}"));
        for i in 0..n.saturating_sub(1) { let _ = writeln!(s, "union S{i},S{}", i + 1); }
        s
    } },
    Shape { name: "treeview_files", lang: M, size: 500, source: |n| lines("treeView-beta", n, "", |i| format!("{}\"f{i}\"", "  ".repeat(i % 6))) },
    Shape { name: "usecase_actors", lang: M, size: 100, source: |n| lines("usecase-beta", n, "", |i| format!("actor a{i}(\"Actor {i}\")\nu{i}(\"Use {i}\")\na{i} --> u{i}\nu{i} --> u{}", (i + 1) % n.max(1))) },
    Shape { name: "railroad_rules", lang: M, size: 100, source: |n| lines("railroad-beta", n, "", |i| format!("r{i} = sequence(terminal(\"a\"), zeroOrMore(choice(terminal(\"b\"), nonterminal(\"r{}\"))), optional(terminal(\"c\"))) ;", (i + 1) % n.max(1))) },
    Shape { name: "wardley_components", lang: M, size: 200, source: |n| {
        let mut s = lines("wardley-beta\ntitle W", n, "", |i| format!("component C{i} [{:.2}, {:.2}]", pick(i, 18, 100) as f32 / 100.0, pick(i, 19, 100) as f32 / 100.0));
        for i in 0..n.saturating_sub(1) { let _ = writeln!(s, "C{i} -> C{}", i + 1); }
        s
    } },
    Shape { name: "ishikawa_causes", lang: M, size: 200, source: |n| lines("ishikawa-beta\nProblem", n, "", |i| if i % 10 == 0 { format!("  Cat{i}") } else { format!("    cause {i}") }) },
    Shape { name: "cynefin_items", lang: M, size: 200, source: |n| lines("cynefin-beta", n, "", |i| format!("{} \"item {i}\"", ["clear", "complicated", "complex", "chaotic"][i % 4])) },
    Shape { name: "eventmodel_flow", lang: M, size: 100, source: |n| lines("eventmodeling", n, "", |i| format!("tf {:02} cmd Cmd{i}\ntf {:02} evt Evt{i}", 2 * i + 1, 2 * i + 2)) },
    // PlantUML.
    Shape { name: "puml_sequence", lang: P, size: 500, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("P{} -> P{} : message {i}", i % 10, (i + 3) % 10)) },
    Shape { name: "puml_sequence_groups", lang: P, size: 200, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("alt c{i}\nA -> B : x\nelse\nB -> A : y\nend\nnote over A : n{i}")) },
    Shape { name: "puml_class", lang: P, size: 150, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("class C{i} {{\n  +a : int\n  +run() : void\n}}\nC{i} <|-- C{}", i + 1)) },
    Shape { name: "puml_class_packages", lang: P, size: 60, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("package p{i} {{\n  class A{i}\n  class B{i}\n}}\nA{i} --> B{}", pick(i, 20, n))) },
    Shape { name: "puml_activity", lang: P, size: 200, source: |n| lines("@startuml\nstart", n, "stop\n@enduml\n", |i| format!(":step {i};\nif (c{i}?) then (yes)\n  :a{i};\nelse (no)\n  :b{i};\nendif")) },
    Shape { name: "puml_activity_lanes", lang: P, size: 200, source: |n| lines("@startuml\nstart", n, "stop\n@enduml\n", |i| format!("|L{}|\n:step {i};", i % 6)) },
    Shape { name: "puml_state", lang: P, size: 200, source: |n| lines("@startuml\n[*] --> s0", n, "@enduml\n", |i| format!("s{i} --> s{} : e{i}", i + 1)) },
    Shape { name: "puml_usecase", lang: P, size: 150, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("actor A{i}\nA{i} --> (U{i})\n(U{i}) ..> (U{}) : include", (i + 1) % n.max(1))) },
    Shape { name: "puml_component", lang: P, size: 150, source: |n| lines("@startuml", n, "@enduml\n", |i| format!("[C{i}] --> [C{}]", pick(i, 21, n))) },
    Shape { name: "puml_gantt", lang: P, size: 300, source: |n| lines("@startgantt\nProject starts 2026-01-01\n[t0] lasts 2 days", n, "@endgantt\n", |i| format!("[t{}] lasts {} days\n[t{}] starts at [t{i}]'s end", i + 1, i % 7 + 1, i + 1)) },
    Shape { name: "puml_mindmap", lang: P, size: 300, source: |n| lines("@startmindmap\n* root", n, "@endmindmap\n", |i| format!("{} n{i}", "*".repeat(2 + i % 4))) },
    Shape { name: "puml_timing", lang: P, size: 300, source: |n| lines("@startuml\nrobust \"R\" as R\nconcise \"C\" as C", n, "@enduml\n", |i| format!("@{}\nR is s{}\nC is v{}", i * 10, i % 4, i % 3)) },
    // DOT.
    Shape { name: "dot_chain", lang: D, size: 500, source: |n| lines("digraph {", n, "}\n", |i| format!("n{i} -> n{};", i + 1)) },
    Shape { name: "dot_random", lang: D, size: 300, source: |n| lines("digraph {", n * 2, "}\n", |i| format!("n{} -> n{};", pick(i, 22, n), pick(i, 23, n))) },
    Shape { name: "dot_undirected_dense", lang: D, size: 60, source: |n| lines("graph {", n * 4, "}\n", |i| format!("n{} -- n{};", pick(i, 24, n), pick(i, 25, n))) },
    Shape { name: "dot_clusters", lang: D, size: 80, source: |n| {
        let mut s = lines("digraph {", n, "", |i| format!("subgraph cluster_{i} {{ label=\"C{i}\"; a{i}; b{i}; a{i} -> b{i}; }}"));
        for i in 0..n { let _ = writeln!(s, "b{i} -> a{};", pick(i, 26, n)); }
        s + "}\n"
    } },
    Shape { name: "dot_records", lang: D, size: 200, source: |n| lines("digraph {\nnode [shape=record]", n, "}\n", |i| format!("r{i} [label=\"<p0> a{i}|{{<p1> b|c}}|<p2> d\"];\nr{i}:p2 -> r{}:p0;", i + 1)) },
    Shape { name: "dot_html", lang: D, size: 200, source: |n| lines("digraph {", n, "}\n", |i| format!("h{i} [label=<<table><tr><td>a{i}</td><td><b>b</b></td></tr></table>>];\nh{i} -> h{};", i + 1)) },
    Shape { name: "dot_attrs", lang: D, size: 300, source: |n| lines("digraph {\nrankdir=LR", n, "}\n", |i| format!("n{i} [shape=box, color=red, style=filled, fillcolor=\"#eeeeee\", label=\"Node {i}\"];\nn{i} -> n{} [label=\"e{i}\", color=blue, style=dashed];", i + 1)) },
];

/// The least time per run of `f`, repeated until `budget` is spent (at least twice).
fn time(budget: Duration, mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    let mut best = Duration::MAX;
    let mut runs = 0;
    while runs < 2 || (start.elapsed() < budget && runs < 50) {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed());
        runs += 1;
    }
    best
}

fn ms(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64() * 1e3)
}

struct Args {
    filters: Vec<String>,
    png: bool,
    phases: bool,
    base: f64,
    flag: f64,
}

fn args() -> Args {
    let mut a = Args {
        filters: Vec::new(),
        png: false,
        phases: false,
        base: 1.0,
        flag: 8.0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--bench" => {}
            "--png" => a.png = true,
            "--phases" => a.phases = true,
            "--base" => a.base = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--flag" => a.flag = it.next().and_then(|v| v.parse().ok()).unwrap_or(8.0),
            _ => a.filters.push(arg),
        }
    }
    a
}

fn main() {
    let args = args();
    if args.phases {
        let _ = log::set_logger(&PHASES);
        log::set_max_level(log::LevelFilter::Off);
    }
    let budget = Duration::from_millis(300);
    // A phase slower than this at the larger size is flagged whatever its growth.
    let slow = Duration::from_millis(500);
    println!(
        "{:<28} {:>6} {:>7} | {:>9} {:>9} {:>6} | {:>9} {:>9} {:>6} | {:>9} {:>9} {:>6}{}",
        "shape",
        "n",
        "bytes",
        "lower",
        "4x",
        "grow",
        "scene",
        "4x",
        "grow",
        "ascii",
        "4x",
        "grow",
        if args.png {
            "       png        4x   grow"
        } else {
            ""
        }
    );
    let mut flagged = Vec::new();
    for shape in SHAPES {
        if !args.filters.is_empty() && !args.filters.iter().any(|f| shape.name.contains(f.as_str()))
        {
            continue;
        }
        let n = ((shape.size as f64 * args.base).round() as usize).max(1);
        let mut times = Vec::new();
        let mut areas = Vec::new();
        let mut bytes = 0;
        let mut errors = 0;
        let mut breakdown = String::new();
        let mut pngs = Vec::new();
        for size in [n, n * 4] {
            let src = (shape.source)(size);
            bytes = src.len();
            let lowered = diagram::lower_as(&src, shape.lang);
            errors = lowered
                .diagnostics
                .iter()
                .filter(|d| d.severity == sirene_core::diagnostic::Severity::Error)
                .count();
            if size == n && errors > 0 {
                let first = &lowered.diagnostics[0];
                let (line, col) = first.line_col(&src);
                eprintln!(
                    "{}: {errors} errors, first {line}:{col}: {}",
                    shape.name, first.message
                );
            }
            let lower = time(budget, || {
                black_box(diagram::lower_as(black_box(&src), shape.lang));
            });
            let scene = time(budget, || {
                let _ = black_box(lowered.diagram.scene(&Approx, &Theme::LIGHT));
            });
            let ascii = time(budget, || {
                let _ = black_box(lowered.diagram.ascii(&Options::default()));
            });
            let mut row = vec![lower, scene, ascii];
            if args.png {
                let (t, pixels) = png(&lowered, budget);
                row.push(t);
                pngs.push(pixels);
            }
            times.push(row);
            areas.push(area(&lowered));
            if args.phases && size > n {
                breakdown = phases(&src, shape.lang, args.png);
            }
        }
        // How much the text drawing grew: ASCII time growing as fast is output-bound.
        let area_grow = areas[1] as f64 / areas[0].max(1) as f64;
        // How much the image grew: PNG time growing as fast is output-bound too.
        let pixel_grow = match pngs[..] {
            [a, b] => b as f64 / a.max(1) as f64,
            _ => 1.0,
        };
        let mut line = format!("{:<28} {:>6} {:>7}", shape.name, n, bytes);
        let phases = ["lower", "scene", "ascii", "png"];
        for (p, (small, large)) in times[0].iter().zip(&times[1]).enumerate() {
            let grow = large.as_secs_f64() / small.as_secs_f64().max(1e-7);
            // Growth is noise below a tenth of a millisecond.
            let limit = match p {
                2 => args.flag.max(2.0 * area_grow),
                3 => args.flag.max(2.0 * pixel_grow),
                _ => args.flag,
            };
            let bad = (grow > limit && *large > Duration::from_micros(500)) || *large > slow;
            let _ = write!(
                line,
                " | {:>9} {:>9} {:>5.1}{}",
                ms(*small),
                ms(*large),
                grow,
                if bad { "!" } else { " " }
            );
            if bad {
                flagged.push(format!(
                    "{} {}: {} -> {} ms ({grow:.1}x)",
                    shape.name,
                    phases[p],
                    ms(*small),
                    ms(*large)
                ));
            }
        }
        let _ = write!(line, " | text {area_grow:>5.1}x");
        if args.png {
            let _ = write!(line, " pixels {pixel_grow:>5.1}x");
        }
        if errors > 0 {
            line.push_str(&format!("  ({errors} errors at 4x)"));
        }
        println!("{line}");
        if !breakdown.is_empty() {
            println!("    {breakdown}");
        }
    }
    println!("\nflagged ({}):", flagged.len());
    for f in &flagged {
        println!("  {f}");
    }
}

/// Cells in the text drawing: its rows by its widest row.
fn area(lowered: &diagram::Lowered) -> usize {
    let text = lowered
        .diagram
        .ascii(&Options::default())
        .unwrap_or_default();
    let rows = text.lines().count();
    let cols = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    rows * cols
}

/// The time to write the PNG, at a scale that fits as the CLI's does, and its pixel count.
#[cfg(feature = "png")]
fn png(lowered: &diagram::Lowered, budget: Duration) -> (Duration, u64) {
    use sirene_core::scene::png::{self, FontMetrics, Quality};
    let Ok(scene) = lowered.diagram.scene(&FontMetrics, &Theme::LIGHT) else {
        return (Duration::ZERO, 0);
    };
    let mut options = png::Options::new(Quality::Normal);
    options.scale = png::fit(&scene, options.scale);
    if let Err(e) = png::write(&scene, options) {
        eprintln!("png: {e}");
    }
    let side = |v: f32| (v * options.scale).ceil() as u64;
    let pixels = side(scene.width) * side(scene.height);
    let t = time(budget, || {
        let _ = black_box(png::write(&scene, options));
    });
    (t, pixels)
}

#[cfg(not(feature = "png"))]
fn png(_: &diagram::Lowered, _: Duration) -> (Duration, u64) {
    (Duration::ZERO, 0)
}
