//! A Visio page as a scene (VISIO-11).

use super::{Scene, TextMetrics, Theme};
use crate::visio::Visio;

/// The page `v` selects, drawn on the white page Visio draws on; the theme's colors do not apply
/// (VISIO-1).
pub fn visio(v: &Visio, metrics: &impl TextMetrics, _theme: &Theme) -> crate::Result<Scene> {
    v.scene(metrics)
}
