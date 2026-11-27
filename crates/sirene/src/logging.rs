//! Configurable stderr logging without changing diagram output on stdout.

use std::io::Write;
use std::time::Instant;

use log::{Level, LevelFilter, Log, Metadata, Record};
use sirene_core::{Error, Result};

#[derive(Debug)]
struct Directive {
    target: String,
    level: LevelFilter,
}

#[derive(Debug, Default)]
pub(crate) struct Filter {
    directives: Vec<Directive>,
}

impl Filter {
    /// Flags set the general sirene level; explicit target directives remain more specific.
    fn configure(verbose: u8, sirene: Option<&str>, rust: Option<&str>) -> Result<Self> {
        let mut filter = Self::parse(sirene.or(rust).unwrap_or(""))?;
        if verbose > 0 {
            filter.directives.retain(|d| !d.target.is_empty());
            filter.directives.push(Directive {
                target: String::new(),
                level: if verbose == 1 {
                    LevelFilter::Debug
                } else {
                    LevelFilter::Trace
                },
            });
        }
        Ok(filter)
    }

    fn parse(value: &str) -> Result<Self> {
        let mut directives = Vec::new();
        if value.trim().is_empty() {
            return Ok(Self::default());
        }
        for raw in value.split(',') {
            let raw = raw.trim();
            let (target, level) = match raw.split_once('=') {
                Some((target, level)) if !target.trim().is_empty() => (target.trim(), level.trim()),
                Some(_) => {
                    return Err(Error::new(
                        "invalid log filter: expected TARGET=LEVEL or LEVEL",
                    ));
                }
                None => ("", raw),
            };
            if !target
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-'))
            {
                return Err(Error::new(
                    "invalid log target: use module names separated by ::",
                ));
            }
            let level = level.parse::<LevelFilter>().map_err(|e| {
                Error::with_source(
                    "invalid log level: use off, error, warn, info, debug, or trace",
                    e,
                )
            })?;
            directives.push(Directive {
                target: target.into(),
                level,
            });
        }
        Ok(Self { directives })
    }

    fn enabled(&self, target: &str, level: Level) -> bool {
        // Longest prefix wins; a later directive wins a tie, as in common Rust filters.
        self.directives
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                if d.target.is_empty() {
                    matches!(
                        target.split("::").next(),
                        Some("sirene" | "sirene_core" | "sirene_gui")
                    )
                } else {
                    target.strip_prefix(&d.target).is_some_and(|suffix| {
                        suffix.is_empty() || suffix.starts_with("::") || d.target.ends_with("::")
                    })
                }
            })
            .max_by_key(|(i, d)| (d.target.len(), *i))
            .is_some_and(|(_, d)| level <= d.level)
    }

    fn max_level(&self) -> LevelFilter {
        self.directives
            .iter()
            .map(|d| d.level)
            .max()
            .unwrap_or(LevelFilter::Off)
    }
}

struct Stderr {
    start: Instant,
    filter: Filter,
}

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ms = self.start.elapsed().as_secs_f64() * 1000.0;
        // Logging is best effort. A broken diagnostic stream must never panic or recurse.
        let _ = writeln!(
            std::io::stderr().lock(),
            "[{ms:8.1}ms] {:5} {}: {}",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        let _ = std::io::stderr().lock().flush();
    }
}

fn env(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(Error::with_source(format!("{name} must be UTF-8"), e)),
    }
}

pub fn init(verbose: u8) -> Result<()> {
    let sirene = env("SIRENE_LOG")?;
    let rust = if sirene.is_none() {
        env("RUST_LOG")?
    } else {
        None
    };
    let filter = Filter::configure(verbose, sirene.as_deref(), rust.as_deref())?;
    let max = filter.max_level();
    if max == LevelFilter::Off {
        return Ok(());
    }
    log::set_boxed_logger(Box::new(Stderr {
        start: Instant::now(),
        filter,
    }))
    .map_err(|e| {
        Error::with_source(
            "cannot initialize logging: a logger is already installed",
            e,
        )
    })?;
    log::set_max_level(max);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ERRORH-7: quiet defaults, environment precedence, and explicit flag precedence.
    #[test]
    fn precedence_and_levels() {
        let quiet = Filter::configure(0, None, None).unwrap();
        assert!(!quiet.enabled("sirene", Level::Error));
        let filter = Filter::configure(0, Some("off"), Some("trace")).unwrap();
        assert!(!filter.enabled("sirene", Level::Error));
        let filter = Filter::configure(0, Some(""), Some("trace")).unwrap();
        assert!(!filter.enabled("sirene", Level::Error));
        let filter = Filter::configure(0, None, Some("info")).unwrap();
        assert!(!filter.enabled("other", Level::Info));
        assert!(filter.enabled("sirene", Level::Info));
        assert!(!filter.enabled("sirene", Level::Debug));
        let filter = Filter::configure(1, Some("off"), None).unwrap();
        assert!(filter.enabled("sirene_core", Level::Debug));
        assert!(!filter.enabled("gpui", Level::Error));
        assert!(!filter.enabled("sirene", Level::Trace));
        assert!(
            Filter::configure(2, None, None)
                .unwrap()
                .enabled("sirene", Level::Trace)
        );
        let filter = Filter::configure(2, Some("off,sirene_core=off,gpui=info"), None).unwrap();
        assert!(filter.enabled("sirene", Level::Trace));
        assert!(!filter.enabled("sirene_core::layout", Level::Error));
        assert!(filter.enabled("gpui::platform", Level::Info));
    }

    #[test]
    fn general_levels_only_cover_sirene_crates() {
        let filter = Filter::parse("debug").unwrap();
        for target in [
            "sirene",
            "sirene::update",
            "sirene_core",
            "sirene_core::layout",
            "sirene_gui",
            "sirene_gui::viewer",
        ] {
            assert!(filter.enabled(target, Level::Debug), "{target}");
        }
        for target in [
            "sirene_plugin",
            "sirene_core_extra::layout",
            "sirene_gui_extra",
            "sirenefoo",
        ] {
            assert!(!filter.enabled(target, Level::Error), "{target}");
        }

        let filter = Filter::parse("debug,sirene_plugin=info").unwrap();
        assert!(filter.enabled("sirene_plugin::module", Level::Info));
        assert!(!filter.enabled("sirene_plugin::module", Level::Debug));
    }

    #[test]
    fn explicit_targets_match_module_boundaries() {
        let filter = Filter::parse("debug,sirene=off,sirene_core::flowchart::=trace").unwrap();
        assert!(!filter.enabled("sirene::update", Level::Error));
        assert!(filter.enabled("sirene_core", Level::Debug));
        assert!(filter.enabled("sirene_gui::viewer", Level::Debug));
        assert!(!filter.enabled("sirene_core::flowchart", Level::Trace));
        assert!(filter.enabled("sirene_core::flowchart::parser", Level::Trace));
    }

    #[test]
    fn specific_filters_and_invalid_configuration() {
        let filter = Filter::parse("warn,sirene=debug,sirene::update=off,sirene=info").unwrap();
        assert!(!filter.enabled("foreign", Level::Warn));
        assert!(!filter.enabled("foreign", Level::Info));
        assert!(filter.enabled("sirene::main", Level::Info));
        assert!(!filter.enabled("sirene::main", Level::Debug));
        assert!(!filter.enabled("sirene::update", Level::Error));
        for raw in [
            "wat",
            "sirene=wat",
            "=info",
            "sirene=debug,",
            "a=b=info",
            "a b=info",
        ] {
            assert!(Filter::parse(raw).is_err(), "{raw}");
        }
    }
}
