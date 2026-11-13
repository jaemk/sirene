//! Exporting the drawing as PNG or SVG, and copying it as an image (GUISHE-9).

use std::path::{Path, PathBuf};

use gpui::{ClipboardItem, Context, Image, ImageFormat, Task, Window};
use sirene_core::scene::png::{self, FontMetrics, Quality};
use sirene_core::scene::{self, Approx, Theme};

use crate::document::Document;
use crate::viewer::Viewer;
use crate::{CopyImage, ExportPng, ExportSvg};

/// A file format the drawing can be exported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Svg,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Svg => "svg",
        }
    }

    /// `doc`'s diagram in this format, as `sirene png` and `sirene svg` write it, at `quality`
    /// for PNG.
    pub fn render(
        self,
        doc: &Document,
        theme: &Theme,
        quality: Quality,
    ) -> Result<Vec<u8>, String> {
        match self {
            Format::Png => {
                let scene = doc.scene(&FontMetrics, theme);
                let scale = png::fit(&scene, quality.scale());
                png::write(&scene, png::Options { scale, quality })
            }
            Format::Svg => {
                let scene = doc.scene(&Approx, theme);
                Ok(scene::svg::write(&scene).into_bytes())
            }
        }
    }
}

impl Viewer {
    /// The file name an export suggests: the document's, with the format's extension; for a
    /// Markdown file's block, `STEM-N` as `sirene svg FILE.md` names it.
    pub fn export_name(&self, format: Format) -> String {
        let mut stem = self
            .doc
            .path
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(
                || "diagram".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        if let Some((n, _, _)) = self.doc.block_info() {
            stem = format!("{stem}-{n}");
        }
        format!("{stem}.{}", format.extension())
    }

    /// Writes the drawing to `path`, reporting the outcome in the status bar.
    pub fn export_to(&mut self, format: Format, path: &Path, cx: &mut Context<Self>) -> bool {
        let written = format
            .render(&self.doc, &self.theme(), Quality::Fine)
            .and_then(|bytes| {
                crate::document::write_atomic(path, &bytes)
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))
            });
        let ok = written.is_ok();
        self.notice = Some(match written {
            Ok(()) => {
                log::debug!("exported {}", path.display());
                format!("exported {}", path.display())
            }
            Err(e) => {
                log::debug!("export failed: {e}");
                e
            }
        });
        cx.notify();
        ok
    }

    /// Asks for a path, beside the document by default, and exports there. Resolves to the path
    /// written, if any.
    pub fn export(
        &mut self,
        format: Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Option<PathBuf>> {
        let dir = crate::dialog_dir(self.doc.path.as_deref());
        let chosen = cx.prompt_for_new_path(&dir, Some(&self.export_name(format)));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                log::debug!("export cancelled");
                return None;
            };
            this.update(cx, |v, cx| v.export_to(format, &path, cx).then_some(path))
                .ok()
                .flatten()
        })
    }

    pub(crate) fn export_png(
        &mut self,
        _: &ExportPng,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export(Format::Png, window, cx).detach();
    }

    pub(crate) fn export_svg(
        &mut self,
        _: &ExportSvg,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export(Format::Svg, window, cx).detach();
    }

    /// Puts the drawing on the clipboard as a PNG, at normal quality.
    pub(crate) fn copy_image(&mut self, _: &CopyImage, _: &mut Window, cx: &mut Context<Self>) {
        self.notice = Some(
            match Format::Png.render(&self.doc, &self.theme(), Quality::Normal) {
                Ok(bytes) => {
                    log::debug!("copied a {} byte PNG", bytes.len());
                    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
                        ImageFormat::Png,
                        bytes,
                    )));
                    "copied the drawing as an image".to_owned()
                }
                Err(e) => format!("cannot copy the drawing: {e}"),
            },
        );
        cx.notify();
    }
}
