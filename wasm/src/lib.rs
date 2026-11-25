//! `sirene-core` for JavaScript on `wasm32-unknown-unknown` (WORKSP-11). The docs site draws
//! diagrams in the browser with it, through `docs/assets/sirene-wasm.js`, and CI renders every
//! file in `examples/` with it under Node (`scripts/wasm-test.mjs`).
//!
//! The module imports nothing. A call goes through its linear memory:
//!
//! 1. `input(len)` returns where to write `len` bytes of UTF-8 source.
//! 2. `render(format, lang, theme)` reads them and returns 0 when it drew the source, or 1 when it
//!    could not, with the codes below.
//! 3. `output_ptr`/`output_len` locate the drawing (SVG or ASCII text, or PNG bytes), or the error
//!    message after a 1. `diagnostics_ptr`/`diagnostics_len` locate a JSON array of the problems
//!    found reading the source: `{"severity", "line", "column", "message"}`, line and column
//!    1-based, column in characters.
//!
//! A panic traps; the caller drops the instance and starts a new one.

use std::cell::{Cell, RefCell};

use sirene_core::diagnostic::{LineIndex, Severity};
use sirene_core::diagram::{self, Lang, Lowered};
use sirene_core::scene::Theme;

/// `render`'s `format`: an SVG document.
pub const FORMAT_SVG: u32 = 0;
/// `render`'s `format`: the `sirene ascii` text.
pub const FORMAT_ASCII: u32 = 1;
/// `render`'s `format`: a PNG image (feature `png`).
pub const FORMAT_PNG: u32 = 2;

/// `render`'s `lang`: detect the language from the source, as `sirene` does for stdin.
pub const LANG_DETECT: u32 = 0;
pub const LANG_MERMAID: u32 = 1;
pub const LANG_DOT: u32 = 2;
pub const LANG_PLANTUML: u32 = 3;
/// One TeX formula (LEMB-3).
pub const LANG_LATEX: u32 = 4;
/// A Visio `.vsdx` package, as bytes (VISIO-12); drawn as its first page. Detected too when the
/// source starts with `PK\x03\x04` and the language is `LANG_DETECT`.
pub const LANG_VISIO: u32 = 5;

pub const THEME_LIGHT: u32 = 0;
pub const THEME_DARK: u32 = 1;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static DIAGNOSTICS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The page `render` draws of a Visio drawing, from 0 (`set_page`).
    static PAGE: Cell<u32> = const { Cell::new(0) };
    /// The pages the last `render`ed source has (`page_count`).
    static PAGES: Cell<u32> = const { Cell::new(0) };
}

/// Chooses the page `render` draws of a Visio drawing, from 0, until changed. Other languages
/// have one page (VISIO-11).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn set_page(page: u32) {
    PAGE.set(page);
}

/// The pages of the source the last `render` read: the drawing's pages for a Visio package, else 1
/// (0 before any render).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn page_count() -> u32 {
    PAGES.get()
}

/// Makes room for `len` bytes of source, zeroed, and returns where they start.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn input(len: usize) -> *mut u8 {
    INPUT.with_borrow_mut(|input| {
        input.clear();
        input.resize(len, 0);
        input.as_mut_ptr()
    })
}

/// Draws the source last written after [`input`]. Returns 0 with the drawing as the output,
/// or 1 with an error message as the output.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn render(format: u32, lang: u32, theme: u32) -> u32 {
    let (result, diagnostics, pages) =
        INPUT.with_borrow(|src| draw_page(src, format, lang, theme, PAGE.get() as usize));
    PAGES.set(pages as u32);
    DIAGNOSTICS.set(diagnostics.into_bytes());
    match result {
        Ok(bytes) => {
            OUTPUT.set(bytes);
            0
        }
        Err(message) => {
            OUTPUT.set(message.into_bytes());
            1
        }
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    OUTPUT.with_borrow(|o| o.as_ptr())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn output_len() -> usize {
    OUTPUT.with_borrow(Vec::len)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn diagnostics_ptr() -> *const u8 {
    DIAGNOSTICS.with_borrow(|d| d.as_ptr())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn diagnostics_len() -> usize {
    DIAGNOSTICS.with_borrow(Vec::len)
}

/// Reads and draws `src`: the drawing or an error message, and the diagnostics as JSON.
pub fn draw(bytes: &[u8], format: u32, lang: u32, theme: u32) -> (Result<Vec<u8>, String>, String) {
    let (drawn, diagnostics, _) = draw_page(bytes, format, lang, theme, 0);
    (drawn, diagnostics)
}

/// [`draw`] showing page `page` (from 0) of a Visio drawing, and the number of pages the source
/// has (0 when it could not be read at all).
pub fn draw_page(
    bytes: &[u8],
    format: u32,
    lang: u32,
    theme: u32,
    page: usize,
) -> (Result<Vec<u8>, String>, String, usize) {
    let none = |e: String| (Err(e), "[]".to_owned(), 0);
    let package = lang == LANG_VISIO || (lang == LANG_DETECT && bytes.starts_with(b"PK\x03\x04"));
    let text = std::str::from_utf8(bytes);
    if !package && text.is_err() {
        return none("the source is not UTF-8".to_owned());
    }
    let src = if package { "" } else { text.unwrap_or("") };
    let lang = match lang {
        LANG_DETECT | LANG_VISIO => None,
        LANG_MERMAID => Some(Lang::Mermaid),
        LANG_DOT => Some(Lang::Dot),
        LANG_PLANTUML => Some(Lang::PlantUml),
        LANG_LATEX => Some(Lang::Latex),
        _ => return none(format!("unknown language code {lang}")),
    };
    let theme = match theme {
        THEME_LIGHT => Theme::LIGHT,
        THEME_DARK => Theme::DARK,
        _ => return none(format!("unknown theme code {theme}")),
    };
    let lowered = if package {
        diagram::lower_bytes(bytes, Some(Lang::Visio))
    } else {
        diagram::lower_as(src, lang)
    };
    let diagnostics = diagnostics_json(src, &lowered);
    let pages = lowered.page_count();
    let Some(lowered) = lowered.with_page(page) else {
        let message = format!("page {page} is out of range; the drawing has {pages} pages");
        return (Err(message), diagnostics, pages);
    };
    let drawn = match format {
        FORMAT_SVG => lowered.svg(&theme).map(String::into_bytes),
        FORMAT_ASCII => lowered.ascii(&Default::default()).map(String::into_bytes),
        #[cfg(feature = "png")]
        FORMAT_PNG => lowered.png(&theme, sirene_core::scene::png::Quality::Draft, None),
        _ => {
            return (
                Err(format!("unknown format code {format}")),
                diagnostics,
                pages,
            );
        }
    };
    (drawn.map_err(|e| e.report()), diagnostics, pages)
}

fn diagnostics_json(src: &str, lowered: &Lowered) -> String {
    let index = LineIndex::new(src);
    let items: Vec<String> = (lowered.diagnostics.iter())
        .map(|d| {
            let (line, column) = index.line_col(d);
            let severity = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            format!(
                r#"{{"severity":"{severity}","line":{line},"column":{column},"message":{}}}"#,
                json_string(&d.message)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The test-only `.vsdx` package writer, shared with `sirene-core`'s tests.
#[cfg(test)]
#[path = "../../crates/sirene-core/src/visio/testpkg.rs"]
mod testpkg;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testpkg::{Builder, rect_geometry, shape, xform};

    fn text(r: Result<Vec<u8>, String>) -> String {
        String::from_utf8(r.unwrap()).unwrap()
    }

    #[test]
    fn draws_svg_and_ascii() {
        let src = b"flowchart LR\n  a --> b\n";
        let (svg, diagnostics) = draw(src, FORMAT_SVG, LANG_DETECT, THEME_DARK);
        assert!(text(svg).starts_with("<svg"));
        assert_eq!(diagnostics, "[]");
        let (ascii, _) = draw(src, FORMAT_ASCII, LANG_MERMAID, THEME_LIGHT);
        let ascii = text(ascii);
        assert!(ascii.contains('a') && ascii.contains('b'), "{ascii}");
    }

    #[test]
    fn reads_the_given_language() {
        let (svg, diagnostics) = draw(b"digraph { a -> b }", FORMAT_SVG, LANG_DOT, THEME_LIGHT);
        assert!(text(svg).starts_with("<svg"));
        assert_eq!(diagnostics, "[]");
        let puml = b"@startuml\nAlice -> Bob: hi\n@enduml\n";
        let (svg, _) = draw(puml, FORMAT_SVG, LANG_PLANTUML, THEME_LIGHT);
        assert!(text(svg).starts_with("<svg"));
        let (svg, diagnostics) = draw(b"$$ x^2 $$", FORMAT_SVG, LANG_LATEX, THEME_LIGHT);
        assert!(text(svg).starts_with("<svg"));
        assert_eq!(diagnostics, "[]");
    }

    #[test]
    fn draws_a_visio_package_from_bytes() {
        let sh = shape(
            1,
            "",
            &format!("{}{}", xform(1.0, 1.0, 2.0, 1.0), rect_geometry(2.0, 1.0)),
        );
        let pkg = Builder::new().page(6.0, 4.0, &sh).build();
        for lang in [LANG_DETECT, LANG_VISIO] {
            let (svg, diagnostics) = draw(&pkg, FORMAT_SVG, lang, THEME_LIGHT);
            let svg = text(svg);
            assert!(svg.starts_with("<svg") && svg.contains("<path"), "{svg}");
            assert_eq!(diagnostics, "[]");
        }
        let (ascii, _) = draw(&pkg, FORMAT_ASCII, LANG_DETECT, THEME_LIGHT);
        assert_eq!(ascii.unwrap_err(), "a Visio drawing has no text form");
        let (svg, diagnostics) = draw(b"PK\x03\x04 junk", FORMAT_SVG, LANG_DETECT, THEME_LIGHT);
        assert!(svg.is_ok());
        assert!(
            diagnostics.contains(r#""severity":"error""#),
            "{diagnostics}"
        );
    }

    #[test]
    fn chooses_a_page_of_a_visio_package() {
        let sh = shape(
            1,
            "",
            &format!("{}{}", xform(1.0, 1.0, 2.0, 1.0), rect_geometry(2.0, 1.0)),
        );
        let pkg = Builder::new()
            .page(6.0, 4.0, &sh)
            .page(3.0, 2.0, &sh)
            .build();
        let (svg, _, pages) = draw_page(&pkg, FORMAT_SVG, LANG_VISIO, THEME_LIGHT, 1);
        assert!(text(svg).contains("width=\"288\""));
        assert_eq!(pages, 2);
        let (err, _, pages) = draw_page(&pkg, FORMAT_SVG, LANG_VISIO, THEME_LIGHT, 2);
        assert!(err.unwrap_err().contains("out of range"));
        assert_eq!(pages, 2);
        // Through the exports: the page and its count.
        INPUT.with_borrow_mut(|i| *i = pkg.clone());
        set_page(1);
        assert_eq!(render(FORMAT_SVG, LANG_VISIO, THEME_LIGHT), 0);
        assert_eq!(page_count(), 2);
        set_page(0);
        // Text sources have one page.
        INPUT.with_borrow_mut(|i| *i = b"flowchart LR\nA-->B\n".to_vec());
        assert_eq!(render(FORMAT_SVG, LANG_DETECT, THEME_LIGHT), 0);
        assert_eq!(page_count(), 1);
    }

    #[test]
    fn reports_diagnostics_with_line_and_column() {
        let (_, diagnostics) = draw(
            b"flowchart LR\n  a --> \n",
            FORMAT_SVG,
            LANG_DETECT,
            THEME_LIGHT,
        );
        assert!(
            diagnostics.starts_with(r#"[{"severity":"error","line":2,"#),
            "{diagnostics}"
        );
    }

    #[test]
    fn refuses_bad_input_and_codes() {
        let (r, d) = draw(b"\xff", FORMAT_SVG, LANG_DETECT, THEME_LIGHT);
        assert_eq!(r.unwrap_err(), "the source is not UTF-8");
        assert_eq!(d, "[]");
        let src = b"flowchart LR\n  a\n";
        assert_eq!(
            draw(src, FORMAT_SVG, 9, THEME_LIGHT).0.unwrap_err(),
            "unknown language code 9"
        );
        assert_eq!(
            draw(src, FORMAT_SVG, LANG_DETECT, 9).0.unwrap_err(),
            "unknown theme code 9"
        );
        assert_eq!(
            draw(src, 9, LANG_DETECT, THEME_LIGHT).0.unwrap_err(),
            "unknown format code 9"
        );
    }

    #[cfg(feature = "png")]
    #[test]
    fn draws_png() {
        let (png, _) = draw(b"flowchart LR\n  a\n", FORMAT_PNG, LANG_DETECT, THEME_DARK);
        assert!(png.unwrap().starts_with(b"\x89PNG"));
    }

    #[test]
    fn escapes_json_strings() {
        assert_eq!(json_string("a\"b\\c\nd\te"), r#""a\"b\\c\nd\u0009e""#);
    }

    /// The exports, as JavaScript calls them, with the input written through a safe slice.
    #[test]
    fn exports_round_trip() {
        let src = b"flowchart LR\n  a --> b\n";
        input(src.len());
        INPUT.with_borrow_mut(|i| i.copy_from_slice(src));
        assert_eq!(render(FORMAT_SVG, LANG_DETECT, THEME_LIGHT), 0);
        assert!(OUTPUT.with_borrow(|o| o.starts_with(b"<svg")));
        assert_eq!(output_ptr(), OUTPUT.with_borrow(|o| o.as_ptr()));
        assert!(output_len() > 0);
        assert_eq!(diagnostics_len(), 2);
        assert_eq!(diagnostics_ptr(), DIAGNOSTICS.with_borrow(|d| d.as_ptr()));
        assert_eq!(render(9, LANG_DETECT, THEME_LIGHT), 1);
        assert_eq!(OUTPUT.with_borrow(|o| o.clone()), b"unknown format code 9");
    }
}
