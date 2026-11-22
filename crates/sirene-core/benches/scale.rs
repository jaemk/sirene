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
use sirene_core::flowchart::EdgeOrder;
use sirene_core::layout::{self, ClusterSpec, EdgeSpec, Graph, Item, NodeSpec, Size};
use sirene_core::scene::{Approx, Theme};

#[path = "../src/layout/route.rs"]
#[allow(dead_code)]
mod route;

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
        // One titled subgraph per rank, exercising repeated diagram-center scans.
        name: "flowchart_titled_chain",
        sizes: [50, 500],
        source: |n| {
            let mut src = lines("flowchart TD", n, |i| {
                format!("subgraph s{i}[Title {i}]\na{i}\nb{i}\nend")
            });
            src.push_str(&lines("", n.saturating_sub(1), |i| {
                format!("b{i} --> a{}", i + 1)
            }));
            src
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
                b.iter(|| l.diagram.scene(&Approx, &Theme::LIGHT).unwrap());
            });
            group.bench_with_input(BenchmarkId::new("ascii", n), &lowered, |b, l| {
                b.iter(|| l.diagram.ascii(&Options::default()).unwrap());
            });
        }
        group.finish();
    }
}

fn layout_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_scale");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for n in [100, 1000] {
        let lanes = Graph {
            clusters: (0..n)
                .map(|_| ClusterSpec {
                    parent: None,
                    direction: None,
                    title: Size::new(4, 1),
                    lane: true,
                })
                .collect(),
            nodes: (0..n)
                .map(|i| NodeSpec {
                    size: Size::new(3, 1),
                    parent: Some(i),
                })
                .collect(),
            ..Graph::default()
        };
        group.bench_with_input(BenchmarkId::new("lanes", n), &lanes, |b, g| {
            b.iter(|| layout::layout(black_box(g), &sirene_core::ascii::CONFIG).unwrap())
        });
        let fan = Graph {
            nodes: (0..=n)
                .map(|_| NodeSpec {
                    size: Size::new(3, 1),
                    parent: None,
                })
                .collect(),
            edges: (1..=n)
                .map(|i| EdgeSpec {
                    from: Item::Node(0),
                    to: Item::Node(i),
                    min_len: 1,
                    label: None,
                    constraint: true,
                    ports: [None; 2],
                    anchors: [None; 2],
                })
                .collect(),
            ordering: vec![(0, EdgeOrder::Out)],
            ..Graph::default()
        };
        group.bench_with_input(BenchmarkId::new("ordered_fan", n), &fan, |b, g| {
            b.iter(|| layout::layout(black_box(g), &sirene_core::ascii::CONFIG).unwrap())
        });
    }
    group.finish();
}

fn route_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("route_scale");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for n in [1_100, 5_000] {
        let disjoint: Vec<_> = (0..n)
            .map(|i| route::Jog {
                top: i * 3,
                bottom: i * 3 + 1,
                flat: false,
            })
            .collect();
        group.bench_with_input(BenchmarkId::new("disjoint", n), &disjoint, |b, jogs| {
            b.iter(|| route::tracks(black_box(jogs)))
        });
        let overlapping: Vec<_> = (0..n)
            .map(|i| route::Jog {
                top: i,
                bottom: i + n,
                flat: false,
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::new("overlapping", n),
            &overlapping,
            |b, jogs| b.iter(|| route::tracks(black_box(jogs))),
        );
    }
    group.finish();
}

/// Header truncation should depend on label length without repeatedly allocating prefixes.
/// Lowering is outside the timed loop; both labels that fit and labels that must be cut are
/// included, with multibyte text to exercise character boundaries.
fn treemap_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("treemap_text");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for (name, unit) in [("ascii", "a"), ("unicode", "界é")] {
        for n in [8, 128, 2048] {
            let src = format!("treemap-beta\n\"{}\"\n  \"Leaf\": 1\n", unit.repeat(n));
            let lowered = diagram::lower(&src);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            group.bench_with_input(BenchmarkId::new(name, n), &lowered, |b, l| {
                b.iter(|| black_box(l).diagram.scene(&Approx, &Theme::LIGHT).unwrap());
            });
        }
    }
    group.finish();
}

fn railroad_measurement(c: &mut Criterion) {
    use sirene_core::railroad::{Expr, Railroad, Rule};
    let mut group = c.benchmark_group("railroad_measurement");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for n in [32, 128, 512] {
        for nested in [false, true] {
            let atom = || Expr::Terminal("word".into());
            let definition = if nested {
                (0..n).fold(atom(), |inner, _| Expr::Optional(Box::new(inner)))
            } else {
                Expr::Sequence((0..n).map(|_| atom()).collect())
            };
            let chart = Railroad {
                rules: vec![Rule {
                    name: "rule".into(),
                    definition,
                }],
                ..Railroad::default()
            };
            group.bench_with_input(
                BenchmarkId::new(if nested { "nested" } else { "flat" }, n),
                &chart,
                |b, chart| {
                    b.iter(|| {
                        sirene_core::scene::railroad(black_box(chart), &Approx, &Theme::LIGHT)
                    });
                },
            );
        }
    }
    group.finish();
}

/// Compare syntax parsing, theme-only reads, and full lowering on the same large documents.
fn theme_reading(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_reading");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for directives in [false, true] {
        let mut src = String::from("flowchart TD\n");
        for i in 0..5_000 {
            if directives && i % 10 == 0 {
                src.push_str("%%{init: {'theme':'dark'}}%%\n");
            }
            let _ = writeln!(src, "node{i}");
        }
        let suffix = if directives { "directives" } else { "plain" };
        group.bench_with_input(BenchmarkId::new("parse", suffix), &src, |b, s| {
            b.iter(|| sirene_core::flowchart::parse(black_box(s)))
        });
        group.bench_with_input(BenchmarkId::new("theme", suffix), &src, |b, s| {
            b.iter(|| sirene_core::theme::ThemeConfig::read(black_box(s)))
        });
        group.bench_with_input(BenchmarkId::new("lower", suffix), &src, |b, s| {
            b.iter(|| diagram::lower(black_box(s)))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scale,
    layout_scale,
    route_scale,
    treemap_text,
    railroad_measurement,
    theme_reading
);
criterion_main!(benches);
