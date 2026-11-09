//! Exporting and copying the drawing in gpui's headless test platform (GUISHE-9).

mod common;

use common::{SECONDARY, open, write};
use gpui::{ClipboardEntry, ImageFormat, TestAppContext};
use sirene_core::scene::png::Quality;
use sirene_gui::document::Document;
use sirene_gui::export::Format;
use sirene_gui::viewer::POLL;
use sirene_gui::{CopyImage, ExportPng, ExportSvg};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

#[gpui::test]
fn export_png_and_svg(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let src = write(dir.path(), "a.mmd", "flowchart LR\nA[Hello] --> B[World]\n");
    let (viewer, vcx) = open(cx, Document::open(&src));
    viewer.read_with(vcx, |v, _| {
        assert_eq!(v.export_name(Format::Png), "a.png");
        assert_eq!(v.export_name(Format::Svg), "a.svg");
    });

    vcx.dispatch_action(ExportPng);
    vcx.run_until_parked();
    assert!(cx.did_prompt_for_new_path());
    let png = dir.path().join("out.png");
    let chosen = png.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(PNG));
    let doc = viewer.read_with(vcx, |v, _| v.doc.clone());
    let fine = Format::Png
        .render(&doc, &sirene_core::scene::Theme::LIGHT, Quality::Fine)
        .unwrap();
    assert_eq!(bytes, fine);
    let status = viewer.read_with(vcx, |v, _| v.status());
    assert_eq!(status, format!("exported {}", png.display()));

    // SVG in the window's theme.
    viewer.update(vcx, |v, cx| v.set_dark(true, cx));
    vcx.simulate_keystrokes(&format!("{SECONDARY}-shift-e"));
    vcx.run_until_parked();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    vcx.dispatch_action(ExportSvg);
    vcx.run_until_parked();
    let svg = dir.path().join("out.svg");
    let chosen = svg.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(
        text.starts_with("<svg") && text.contains("fill=\"#1e1e1e\""),
        "{text}"
    );

    // The notice lasts until the document changes.
    write(dir.path(), "a.mmd", "flowchart LR\nA --> C\n");
    vcx.executor().advance_clock(POLL);
    vcx.run_until_parked();
    assert_eq!(viewer.read_with(vcx, |v, _| v.status()), "a.mmd");
}

#[gpui::test]
fn failed_exports_show_the_error(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(cx, Document::untitled());
    viewer.read_with(vcx, |v, _| {
        assert_eq!(v.export_name(Format::Png), "diagram.png")
    });
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("missing").join("x.svg");
    let ok = viewer.update(vcx, |v, cx| v.export_to(Format::Svg, &bad, cx));
    assert!(!ok);
    let status = viewer.read_with(vcx, |v, _| v.status());
    assert!(
        status.starts_with(&format!("cannot write {}: ", bad.display())),
        "{status}"
    );
}

#[gpui::test]
fn copy_image(cx: &mut TestAppContext) {
    let (viewer, vcx) = open(
        cx,
        Document::from_source(None, "flowchart\nA --> B\n".into()),
    );
    vcx.dispatch_action(CopyImage);
    let item = vcx
        .read_from_clipboard()
        .expect("the clipboard has the drawing");
    let [ClipboardEntry::Image(image)] = item.entries() else {
        panic!("not one image: {:?}", item.entries());
    };
    assert_eq!(image.format(), ImageFormat::Png);
    assert!(image.bytes().starts_with(PNG));
    let doc = viewer.read_with(vcx, |v, _| v.doc.clone());
    let normal = Format::Png
        .render(&doc, &sirene_core::scene::Theme::LIGHT, Quality::Normal)
        .unwrap();
    assert_eq!(image.bytes(), normal.as_slice());
    let status = viewer.read_with(vcx, |v, _| v.status());
    assert_eq!(status, "copied the drawing as an image");

    vcx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new()));
    vcx.simulate_keystrokes(&format!("{SECONDARY}-shift-c"));
    assert!(matches!(
        vcx.read_from_clipboard().unwrap().entries(),
        [ClipboardEntry::Image(_)]
    ));
}
