//! Settings kept between launches: the appearance, recent files, the window size, and the
//! source pane (GUISHE-10, GUISHE-11).

use std::path::{Path, PathBuf};

use gpui::{App, Global};

/// How many recent files are kept.
pub const RECENT: usize = 10;
/// The source pane's width when none is remembered.
pub const SOURCE_WIDTH: f32 = 480.0;
/// The source pane's least width.
pub const SOURCE_MIN: f32 = 160.0;
/// The width a new window leaves the drawing beside a remembered source pane.
pub const DRAWING_MIN: f32 = 240.0;

/// The remembered window size `want`, shrunk to fit a screen of `screen` (when known).
pub fn fit_window(want: (f32, f32), screen: Option<(f32, f32)>) -> (f32, f32) {
    match screen {
        Some((sw, sh)) => (want.0.min(sw), want.1.min(sh)),
        None => want,
    }
}

/// The remembered source pane `width`, narrowed so a window `window_width` wide keeps
/// `DRAWING_MIN` for the drawing, but no narrower than `SOURCE_MIN`.
pub fn source_width_in(width: f32, window_width: f32) -> f32 {
    width.min(window_width - DRAWING_MIN).max(SOURCE_MIN)
}

/// Which theme windows draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    /// Follow the system's light or dark appearance.
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    fn name(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    fn parse(s: &str) -> Option<Appearance> {
        match s {
            "system" => Some(Appearance::System),
            "light" => Some(Appearance::Light),
            "dark" => Some(Appearance::Dark),
            _ => None,
        }
    }

    /// Whether to draw dark, given whether the system is dark.
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Appearance::System => system_dark,
            Appearance::Light => false,
            Appearance::Dark => true,
        }
    }
}

/// The settings, and the file they are kept in (none keeps them in memory only).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub appearance: Appearance,
    /// Most recent first.
    pub recent: Vec<PathBuf>,
    /// The last window size, in pixels.
    pub window: Option<(f32, f32)>,
    pub show_source: bool,
    pub source_width: f32,
    pub file: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            appearance: Appearance::System,
            recent: Vec::new(),
            window: None,
            show_source: true,
            source_width: SOURCE_WIDTH,
            file: None,
        }
    }
}

impl Global for Settings {}

/// The settings file: `SIRENE_CONFIG_DIR/settings`, else `settings` in the platform's
/// configuration directory for sirene.
pub fn default_file() -> Option<PathBuf> {
    file_from(|k| std::env::var_os(k))
}

/// `default_file`, reading environment variables with `var`.
pub fn file_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let env = |k| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if let Some(dir) = env("SIRENE_CONFIG_DIR") {
        dir
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support/sirene")
    } else if cfg!(windows) {
        env("APPDATA")?.join("sirene")
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))?
            .join("sirene")
    };
    Some(dir.join("settings"))
}

impl Settings {
    /// Parses `key = value` lines; unknown keys and bad values are skipped.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "appearance" => {
                    if let Some(a) = Appearance::parse(value) {
                        s.appearance = a;
                    }
                }
                "recent" if !value.is_empty() && s.recent.len() < RECENT => {
                    s.recent.push(PathBuf::from(value));
                }
                "window" => {
                    let size = value.split_once('x').and_then(|(w, h)| {
                        let (w, h) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
                        (w.is_finite() && h.is_finite() && w >= 100.0 && h >= 100.0)
                            .then_some((w, h))
                    });
                    if size.is_some() {
                        s.window = size;
                    }
                }
                "source" => match value {
                    "shown" => s.show_source = true,
                    "hidden" => s.show_source = false,
                    _ => {}
                },
                "source_width" => {
                    if let Ok(w) = value.parse::<f32>()
                        && w.is_finite()
                        && w >= 100.0
                    {
                        s.source_width = w;
                    }
                }
                _ => {}
            }
        }
        s
    }

    /// The settings as `parse` reads them. Paths that cannot be written on one line are left
    /// out.
    pub fn to_text(&self) -> String {
        let mut out = format!("appearance = {}\n", self.appearance.name());
        if let Some((w, h)) = self.window {
            out.push_str(&format!("window = {w:.0}x{h:.0}\n"));
        }
        let source = if self.show_source { "shown" } else { "hidden" };
        out.push_str(&format!("source = {source}\n"));
        out.push_str(&format!("source_width = {:.0}\n", self.source_width));
        for p in &self.recent {
            if let Some(p) = p.to_str()
                && !p.contains(['\n', '\r'])
                && p.trim() == p
            {
                out.push_str(&format!("recent = {p}\n"));
            }
        }
        out
    }

    /// Reads `file`, or the defaults when it is missing or unreadable. Later saves go to it,
    /// unless it exists but could not be read, so that it is never overwritten with defaults.
    pub fn load(file: PathBuf) -> Settings {
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                log::debug!("read settings from {}", file.display());
                Settings {
                    file: Some(file),
                    ..Settings::parse(&text)
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("no settings at {}", file.display());
                Settings {
                    file: Some(file),
                    ..Settings::default()
                }
            }
            Err(e) => {
                log::warn!("cannot read settings from {}: {e}", file.display());
                Settings::default()
            }
        }
    }

    /// Writes the settings to their file, if they have one.
    pub fn save(&self) {
        let Some(file) = &self.file else {
            return;
        };
        // Written beside, then renamed over, so a crash never leaves the file half written.
        let mut temp = file.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&temp, self.to_text()))
            .and_then(|()| std::fs::rename(&temp, file));
        match written {
            Ok(()) => log::debug!("wrote settings to {}", file.display()),
            Err(e) => log::debug!("cannot write settings to {}: {e}", file.display()),
        }
    }

    /// Puts `path` first in the recent files, once.
    pub fn add_recent(&mut self, path: &Path) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(RECENT);
    }
}

/// The app's settings (the defaults before `init` sets them).
pub fn get(cx: &App) -> &Settings {
    static DEFAULT: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();
    cx.try_global::<Settings>()
        .unwrap_or_else(|| DEFAULT.get_or_init(Settings::default))
}

/// Changes the settings and saves them.
pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) {
    remember(cx, f);
    cx.global::<Settings>().save();
}

/// Changes the settings without saving them yet (they are saved on the next `update`, closing
/// a window, or quitting). Observers are told only when something changed.
pub fn remember(cx: &mut App, f: impl FnOnce(&mut Settings)) {
    if !cx.has_global::<Settings>() {
        cx.set_global(Settings::default());
    }
    let mut next = cx.global::<Settings>().clone();
    f(&mut next);
    if next != *cx.global::<Settings>() {
        cx.set_global(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = Settings {
            appearance: Appearance::Dark,
            recent: vec![PathBuf::from("/a b/c.mmd"), PathBuf::from("/d.mmd")],
            window: Some((900.0, 600.0)),
            show_source: false,
            source_width: 300.0,
            file: None,
        };
        assert_eq!(Settings::parse(&s.to_text()), s);
        assert_eq!(
            Settings::parse(&Settings::default().to_text()),
            Settings::default()
        );
    }

    #[test]
    fn skips_what_it_cannot_read() {
        let s = Settings::parse(
            "appearance = purple\nwindow = 10x10\nwindow = axb\nsource = maybe\n\
             source_width = NaN\nrecent =\nnonsense\nfuture = 1\n",
        );
        assert_eq!(s, Settings::default());
        let many: String = (0..20).map(|i| format!("recent = /f{i}.mmd\n")).collect();
        assert_eq!(Settings::parse(&many).recent.len(), RECENT);
        let odd = Settings {
            recent: vec![PathBuf::from("/a\nb"), PathBuf::from(" /pad")],
            ..Settings::default()
        };
        assert!(Settings::parse(&odd.to_text()).recent.is_empty());
    }

    #[test]
    fn settings_file() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(name, _)| *name == k)
                    .map(|(_, v)| std::ffi::OsString::from(v))
            }
        };
        assert_eq!(
            file_from(env(&[("SIRENE_CONFIG_DIR", "/c"), ("HOME", "/h")])),
            Some(PathBuf::from("/c/settings"))
        );
        let home = file_from(env(&[("SIRENE_CONFIG_DIR", ""), ("HOME", "/h")]));
        let want = if cfg!(target_os = "macos") {
            Some(PathBuf::from(
                "/h/Library/Application Support/sirene/settings",
            ))
        } else if cfg!(windows) {
            None
        } else {
            Some(PathBuf::from("/h/.config/sirene/settings"))
        };
        assert_eq!(home, want);
        if cfg!(all(unix, not(target_os = "macos"))) {
            assert_eq!(
                file_from(env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")])),
                Some(PathBuf::from("/x/sirene/settings"))
            );
        }
        if !cfg!(windows) {
            assert_eq!(file_from(env(&[])), None);
        }
    }

    #[test]
    fn fits_the_screen() {
        assert_eq!(fit_window((900.0, 600.0), None), (900.0, 600.0));
        assert_eq!(
            fit_window((900.0, 600.0), Some((1920.0, 1080.0))),
            (900.0, 600.0)
        );
        assert_eq!(
            fit_window((3000.0, 600.0), Some((1440.0, 875.0))),
            (1440.0, 600.0)
        );
        assert_eq!(source_width_in(300.0, 900.0), 300.0);
        assert_eq!(source_width_in(800.0, 900.0), 660.0);
        assert_eq!(source_width_in(800.0, 300.0), SOURCE_MIN);
    }

    #[test]
    fn recent_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mmd");
        std::fs::write(&a, "").unwrap();
        let mut s = Settings::default();
        for i in 0..12 {
            s.add_recent(&dir.path().join(format!("{i}.mmd")));
        }
        s.add_recent(&a);
        s.add_recent(&dir.path().join(".").join("a.mmd"));
        assert_eq!(s.recent.len(), RECENT);
        assert_eq!(s.recent[0], std::fs::canonicalize(&a).unwrap());
        assert_eq!(s.recent[1], dir.path().join("11.mmd"));
    }

    #[test]
    fn loads_and_saves() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("conf").join("settings");
        let mut s = Settings::load(file.clone());
        assert_eq!(s.file.as_deref(), Some(file.as_path()));
        s.appearance = Appearance::Light;
        s.save();
        assert_eq!(Settings::load(file.clone()).appearance, Appearance::Light);
        // An unwritable file is skipped without failing.
        let blocked = Settings {
            file: Some(file.join("under-a-file")),
            ..Settings::default()
        };
        blocked.save();
        Settings::default().save();
        // Nothing is left beside the file.
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings"]);
        assert!(Appearance::System.is_dark(true) && !Appearance::Light.is_dark(true));
        assert!(Appearance::Dark.is_dark(false));
    }

    #[test]
    fn unreadable_file_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings");
        std::fs::write(&file, b"appearance = light\n\xff\n").unwrap();
        let mut s = Settings::load(file.clone());
        assert_eq!(s.file, None);
        s.appearance = Appearance::Dark;
        s.save();
        assert_eq!(std::fs::read(&file).unwrap(), b"appearance = light\n\xff\n");
    }
}
