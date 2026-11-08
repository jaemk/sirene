//! PNG output with the embedded font (DISPLA-11).

use resvg::tiny_skia::Pixmap;
use sirene_core::flowchart;
use sirene_core::scene::png::{self, FontMetrics};
use sirene_core::scene::{self, Approx, Color, Scene, TextMetrics, Theme};

fn scene_of(src: &str, theme: &Theme) -> Scene {
    let chart = flowchart::parse(src).lower().flowchart;
    scene::flowchart(&chart, &FontMetrics, theme)
}

fn dark_pixels(p: &Pixmap) -> usize {
    p.pixels()
        .iter()
        .filter(|c| c.red() < 80 && c.green() < 80 && c.blue() < 80)
        .count()
}

#[test]
fn font_metrics() {
    let m = FontMetrics;
    assert_eq!(m.width("", 16.0), 0.0);
    let w = m.width("Hello", 16.0);
    assert!(w > 0.0 && w != Approx.width("Hello", 16.0), "{w}");
    assert!((m.width("Hello", 32.0) - 2.0 * w).abs() < 1e-3);
    // Proportional: "i" is narrower than "M".
    assert!(m.width("i", 16.0) < m.width("M", 16.0));
    // Characters the font lacks fall back to the estimate.
    assert_eq!(m.width("中", 16.0), Approx.width("中", 16.0));
    assert_eq!(
        m.width("a中", 16.0),
        m.width("a", 16.0) + Approx.width("中", 16.0)
    );
}

#[test]
fn image_size_and_background() {
    let s = scene_of("flowchart LR\nA --> B\n", &Theme::LIGHT);
    for scale in [1.0, 2.0, 0.5] {
        let bytes = png::write(&s, scale).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let p = Pixmap::decode_png(&bytes).unwrap();
        assert_eq!(p.width(), (s.width * scale).ceil() as u32);
        assert_eq!(p.height(), (s.height * scale).ceil() as u32);
        let c = p.pixel(0, 0).unwrap();
        let bg = Theme::LIGHT.background;
        assert_eq!(
            (c.red(), c.green(), c.blue(), c.alpha()),
            (bg.r, bg.g, bg.b, 255)
        );
    }
    let dark =
        Pixmap::decode_png(&png::write(&scene_of("flowchart\nA\n", &Theme::DARK), 1.0).unwrap())
            .unwrap();
    let c = dark.pixel(0, 0).unwrap();
    let bg = Theme::DARK.background;
    assert_eq!((c.red(), c.green(), c.blue()), (bg.r, bg.g, bg.b));
}

#[test]
fn text_is_drawn() {
    // One node and no edges: the only dark pixels are its label.
    let blank = scene_of("flowchart\nA[\" \"]\n", &Theme::LIGHT);
    let label = scene_of("flowchart\nA[Hello world]\n", &Theme::LIGHT);
    let blank = Pixmap::decode_png(&png::write(&blank, 2.0).unwrap()).unwrap();
    let label = Pixmap::decode_png(&png::write(&label, 2.0).unwrap()).unwrap();
    assert_eq!(dark_pixels(&blank), 0);
    assert!(dark_pixels(&label) > 100, "{}", dark_pixels(&label));
}

#[test]
fn deterministic() {
    let s = scene_of(
        "flowchart TD\nA[One] --> B{Two}\nB -->|x| C((Three))\n",
        &Theme::LIGHT,
    );
    assert_eq!(png::write(&s, 2.0).unwrap(), png::write(&s, 2.0).unwrap());
}

#[test]
fn empty_scene_is_an_error() {
    let s = Scene {
        width: 0.0,
        height: 0.0,
        background: Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        },
        elements: Vec::new(),
    };
    let e = png::write(&s, 1.0).unwrap_err();
    assert!(e.starts_with("cannot draw the diagram: "), "{e}");
}

#[test]
fn errors() {
    let s = scene_of("flowchart\nA\n", &Theme::LIGHT);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let e = png::write(&s, bad).unwrap_err();
        assert!(e.starts_with("scale must be a positive number"), "{e}");
    }
    let e = png::write(&s, 1000.0).unwrap_err();
    assert!(e.contains("too large (at most 16384 pixels a side)"), "{e}");
}
