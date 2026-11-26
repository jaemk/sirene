//! Phase timings are logged under `sirene_core::profile` only when that target is enabled
//! (ERRORH-8).

use std::sync::Mutex;

use sirene_core::ascii::Options;
use sirene_core::diagram;
use sirene_core::profile;
use sirene_core::scene::{Approx, Theme};

struct Collect(Mutex<Vec<String>>);

impl log::Log for Collect {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target() == profile::TARGET
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata())
            && let Ok(mut r) = self.0.lock()
        {
            r.push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

static RECORDS: Collect = Collect(Mutex::new(Vec::new()));

fn names() -> Vec<String> {
    let records = RECORDS.0.lock().map(|r| r.clone()).unwrap_or_default();
    records
        .iter()
        .map(|m| {
            let (name, _) = profile::parse(m).unwrap_or_else(|| panic!("not a phase: {m}"));
            name.to_string()
        })
        .collect()
}

fn draw() {
    let lowered = diagram::lower("flowchart TD\n  a --> b --> c\n  a --> c\n");
    lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
    lowered.diagram.ascii(&Options::UNICODE).unwrap();
}

#[test]
fn phases_are_logged_only_when_enabled() {
    log::set_logger(&RECORDS).unwrap();

    log::set_max_level(log::LevelFilter::Debug);
    draw();
    assert!(names().is_empty(), "{:?}", names());

    log::set_max_level(log::LevelFilter::Trace);
    draw();
    let names = names();
    for phase in [
        "lower",
        "scene",
        "ascii",
        "layout",
        "layout.build",
        "layout.order",
        "layout.ports",
        "layout.position",
        "layout.tracks",
    ] {
        assert!(names.iter().any(|n| n == phase), "{phase} in {names:?}");
    }
    // Inner phases end, and log, before the phase that holds them.
    let at = |p: &str| names.iter().position(|n| n == p).unwrap();
    assert!(at("layout.order") < at("layout"));
    assert!(at("layout") < at("scene"));
}
