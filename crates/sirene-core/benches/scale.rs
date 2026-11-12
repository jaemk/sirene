//! How reading, scene building, and text drawing scale with input size. Each shape runs at two
//! sizes ten times apart; near-linear phases take about ten times as long at the larger one.
//!
//! Run with `cargo bench -p sirene-core --bench scale`, or pass a filter such as `sankey`.

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sirene_core::ascii::Options;
use sirene_core::diagram;
use sirene_core::scene::{Approx, Theme};

/// `header`, then `line(i)` for each `i` below `n`.
fn lines(header: &str, n: usize, line: impl Fn(usize) -> String) -> String {
    let mut s = format!("{header}\n");
    for i in 0..n {
        let _ = writeln!(s, "{}", line(i));
    }
    s
}

/// A shape: its name, its two sizes, and its source at a size.
struct Shape {
    name: &'static str,
    sizes: [usize; 2],
    source: fn(usize) -> String,
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "flowchart_chain",
        sizes: [500, 5_000],
        source: |n| lines("flowchart TD", n, |i| format!("n{i} --> n{}", i + 1)),
    },
    Shape {
        // `n` edges between two ranks: `a0 & ... --> b0 & ...`.
        name: "flowchart_fan",
        sizes: [400, 4_000],
        source: |n| {
            let side = (n as f64).sqrt() as usize;
            let group = |p: &str| {
                (0..side)
                    .map(|i| format!("{p}{i}"))
                    .collect::<Vec<_>>()
                    .join(" & ")
            };
            format!("flowchart TD\n{} --> {}\n", group("a"), group("b"))
        },
    },
    Shape {
        name: "flowchart_subgraph",
        sizes: [1_000, 10_000],
        source: |n| lines("flowchart TD\nsubgraph big", n, |i| format!("n{i}")) + "end\n",
    },
    Shape {
        // Subgraphs of two nodes, one after another.
        name: "flowchart_clusters",
        sizes: [50, 500],
        source: |n| {
            lines("flowchart TD", n, |i| {
                format!("subgraph s{i}\na{i} --> b{i}\nend\nb{i} --> a{}", i + 1)
            })
        },
    },
    Shape {
        name: "dot_chain",
        sizes: [500, 5_000],
        source: |n| lines("digraph {", n, |i| format!("n{i} -> n{};", i + 1)) + "}\n",
    },
    Shape {
        name: "state_chain",
        sizes: [500, 5_000],
        source: |n| lines("stateDiagram-v2", n, |i| format!("s{i} --> s{}", i + 1)),
    },
    Shape {
        name: "class_chain",
        sizes: [500, 5_000],
        source: |n| lines("classDiagram", n, |i| format!("C{i} <|-- C{}", i + 1)),
    },
    Shape {
        name: "sequence_messages",
        sizes: [1_000, 10_000],
        source: |n| {
            lines("sequenceDiagram", n, |i| {
                format!("A{} ->> A{}: m", i % 20, (i + 1) % 20)
            })
        },
    },
    Shape {
        name: "mindmap_siblings",
        sizes: [1_000, 10_000],
        source: |n| lines("mindmap\nroot", n, |i| format!("  c{i}")),
    },
    Shape {
        name: "sankey_chain",
        sizes: [1_000, 10_000],
        source: |n| lines("sankey-beta", n, |i| format!("a{i},a{},1", i + 1)),
    },
    Shape {
        // One source flowing to every other node.
        name: "sankey_star",
        sizes: [100, 1_000],
        source: |n| lines("sankey-beta", n, |i| format!("a,b{i},1")),
    },
    Shape {
        name: "gantt_tasks",
        sizes: [1_000, 10_000],
        source: |n| {
            lines("gantt\ndateFormat YYYY-MM-DD", n, |i| {
                format!("t{i} : t{i}, 2026-01-01, 1d")
            })
        },
    },
];

fn scale(c: &mut Criterion) {
    for shape in SHAPES {
        let mut group = c.benchmark_group(shape.name);
        group
            .sample_size(10)
            .warm_up_time(Duration::from_millis(500))
            .measurement_time(Duration::from_secs(2));
        for n in shape.sizes {
            let src = (shape.source)(n);
            let lowered = diagram::lower(&src);
            assert!(
                lowered.diagnostics.is_empty(),
                "{} at {n}: {:?}",
                shape.name,
                lowered.diagnostics
            );
            group.bench_with_input(BenchmarkId::new("lower", n), &src, |b, src| {
                b.iter(|| diagram::lower(black_box(src)));
            });
            group.bench_with_input(BenchmarkId::new("scene", n), &lowered, |b, l| {
                b.iter(|| l.diagram.scene(&Approx, &Theme::LIGHT));
            });
            group.bench_with_input(BenchmarkId::new("ascii", n), &lowered, |b, l| {
                b.iter(|| l.diagram.ascii(&Options::default()));
            });
        }
        group.finish();
    }
}

criterion_group!(benches, scale);
criterion_main!(benches);
