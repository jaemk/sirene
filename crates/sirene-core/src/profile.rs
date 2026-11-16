//! Phase timing (ERRORH-8): how long each stage of reading, layout, and drawing takes, logged
//! at trace level under the target [`TARGET`]. Nothing is measured unless that target is
//! enabled, e.g. `SIRENE_LOG=sirene_core::profile=trace`.

use std::time::Instant;

/// The log target of phase timings.
pub const TARGET: &str = "sirene_core::profile";

/// Times a phase from [`Phase::start`] until dropped, then logs `phase <name> <ns> ns` at
/// trace level. A phase that runs many times (such as one gap's routes) logs once per run.
#[must_use = "a phase is timed until it is dropped"]
pub(crate) struct Phase {
    name: &'static str,
    start: Option<Instant>,
}

impl Phase {
    pub(crate) fn start(name: &'static str) -> Phase {
        let on = log::log_enabled!(target: TARGET, log::Level::Trace);
        Phase {
            name,
            start: on.then(Instant::now),
        }
    }
}

impl Drop for Phase {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            log::trace!(
                target: TARGET,
                "phase {} {} ns",
                self.name,
                start.elapsed().as_nanos()
            );
        }
    }
}

/// Reads a timing record's message back as `(phase, nanoseconds)`, or `None` for any other
/// message.
pub fn parse(message: &str) -> Option<(&str, u128)> {
    let rest = message.strip_prefix("phase ")?;
    let (name, ns) = rest.strip_suffix(" ns")?.rsplit_once(' ')?;
    Some((name, ns.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_its_messages() {
        assert_eq!(
            parse("phase layout.order 1200 ns"),
            Some(("layout.order", 1200))
        );
        assert_eq!(parse("phase x 12"), None);
        assert_eq!(parse("laid out 3 nodes"), None);
    }
}
