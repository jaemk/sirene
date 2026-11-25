#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::diagram::{self, Lang, MAX_DIAGNOSTICS};
use sirene_core::scene::Theme;

// Read arbitrary bytes as a Visio package and draw every page: errors are fine, panics are not,
// and the diagnostics stay bounded.
fuzz_target!(|data: &[u8]| {
    let lowered = diagram::lower_bytes(data, Some(Lang::Visio));
    assert!(lowered.diagnostics.len() <= MAX_DIAGNOSTICS + 1);
    for page in 0..lowered.page_count().min(4) {
        if let Some(l) = lowered.with_page(page) {
            let _ = l.svg(&Theme::LIGHT);
        }
    }
});
