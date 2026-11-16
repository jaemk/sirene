//! Exporting the drawing as PNG or SVG, and copying it as an image (GUISHE-9).

use std::path::{Path, PathBuf};

use sirene_core::{Error, Result};

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
    pub fn render(self, doc: &Document, theme: &Theme, quality: Quality) -> Result<Vec<u8>> {
        match self {
            Format::Png => {
                let scene = doc.scene(&FontMetrics, theme)?;
                let scale = png::fit(&scene, quality.scale());
                png::write(&scene, png::Options { scale, quality })
            }
            Format::Svg => {
                let scene = doc.scene(&Approx, theme)?;
                Ok(scene::svg::write(&scene).into_bytes())
            }
        }
    }
}

/// Writes `doc`'s diagram to `path` in `format`, at fine quality for PNG. A path that is the
/// document's own file is refused, so an export never replaces the diagram's source.
fn export_file(doc: &Document, format: Format, theme: &Theme, path: &Path) -> Result<()> {
    if doc
        .path
        .as_deref()
        .is_some_and(|own| crate::document::same_path(own, path))
    {
        return Err(Error::new(format!(
            "cannot export to {}: it is the diagram's own file",
            path.display()
        )));
    }
    let bytes = format.render(doc, theme, Quality::Fine)?;
    crate::document::write_atomic(path, &bytes)
        .map_err(|e| Error::with_source(format!("cannot write {}", path.display()), e))
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
        let written = export_file(&self.doc, format, &self.theme(), path);
        let ok = written.is_ok();
        self.notice = Some(match written {
            Ok(()) => {
                log::debug!("exported {}", path.display());
                format!("exported {}", path.display())
            }
            Err(e) => {
                log::error!("export failed: {}", e.report());
                e.report()
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
            let path = match crate::dialog_result(chosen.await, "export dialog failed") {
                Ok(Some(path)) => path,
                Ok(None) => {
                    log::debug!("export cancelled");
                    return None;
                }
                Err(error) => {
                    let _ = this.update(cx, |v, cx| v.report_error(error, cx));
                    return None;
                }
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
                Err(e) => {
                    let error = e.context("cannot copy the drawing");
                    log::error!("{}", error.report());
                    error.report()
                }
            },
        );
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An export to the document's own file is refused and leaves it as it was; another path
    /// gets the drawing.
    #[test]
    fn export_refuses_the_documents_own_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mmd");
        std::fs::write(&path, "flowchart LR\nA --> B\n").unwrap();
        let doc = Document::open(&path);
        let err = export_file(&doc, Format::Svg, &Theme::LIGHT, &path).unwrap_err();
        assert_eq!(
            err.report(),
            format!(
                "cannot export to {}: it is the diagram's own file",
                path.display()
            )
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "flowchart LR\nA --> B\n"
        );
        let svg = dir.path().join("a.svg");
        export_file(&doc, Format::Svg, &Theme::LIGHT, &svg).unwrap();
        assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));
    }
}
