//! Renders each file in the repository's `examples/`, and a gantt chart (which reads the date),
//! as ASCII, SVG, and PNG. Built for
//! `wasm32-unknown-unknown` and run by `scripts/wasm-smoke.mjs`, it checks that nothing in
//! reading, layout, or drawing needs what that target lacks (a clock, files, threads): any
//! panic traps (WORKSP-11).

use sirene_core::diagram::{self, Lang};
use sirene_core::scene::Theme;
use sirene_core::scene::png::Quality;

const EXAMPLES: &[(&str, &str)] = &[
    (
        "architecture.mmd",
        include_str!("../../examples/architecture.mmd"),
    ),
    (
        "build-graph.dot",
        include_str!("../../examples/build-graph.dot"),
    ),
    (
        "checkout.puml",
        include_str!("../../examples/checkout.puml"),
    ),
    (
        "ci-pipeline.mmd",
        include_str!("../../examples/ci-pipeline.mmd"),
    ),
    (
        "fulfillment.puml",
        include_str!("../../examples/fulfillment.puml"),
    ),
    ("login.mmd", include_str!("../../examples/login.mmd")),
    (
        "order-lifecycle.mmd",
        include_str!("../../examples/order-lifecycle.mmd"),
    ),
    (
        "request-timing.puml",
        include_str!("../../examples/request-timing.puml"),
    ),
    ("shapes.mmd", include_str!("../../examples/shapes.mmd")),
    (
        "shop-model.puml",
        include_str!("../../examples/shop-model.puml"),
    ),
    ("styling.mmd", include_str!("../../examples/styling.mmd")),
    (
        "gantt.mmd",
        "gantt\n  title Plan\n  section A\n  Task :a1, 2026-01-01, 3d\n",
    ),
];

fn main() -> sirene_core::Result<()> {
    for (name, src) in EXAMPLES {
        let lowered = diagram::lower_as(src, Lang::of_path(std::path::Path::new(name)));
        assert!(!lowered.ascii(&Default::default())?.is_empty(), "{name}");
        assert!(lowered.svg(&Theme::LIGHT)?.starts_with("<svg"), "{name}");
        let png = lowered.png(&Theme::DARK, Quality::Draft, None)?;
        assert!(png.starts_with(b"\x89PNG"), "{name}");
    }
    Ok(())
}
