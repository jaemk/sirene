//! The Linux package tree (`spec/linux-packaging.md`): an AppDir that is both the AppImage's root
//! and, under `usr/`, the files of the RPM.

use std::fs;
use std::path::{Path, PathBuf};

use sirene_core::{Error, Result};

use crate::{APP, DOCUMENT_TYPES, ICON_SVG, LICENSES, context, icon_png, write};

/// The desktop entry's, icon's, and MIME package's base name.
pub const NAME: &str = "sirene";
/// The MIME types of `DOCUMENT_TYPES`, in the same order (LINUXP-3).
pub const MIME_TYPES: [&str; 4] = [
    "text/x-mermaid",
    "text/vnd.graphviz",
    "text/x-plantuml",
    "application/vnd.ms-visio.drawing",
];
/// Markdown, which the app also opens (MARKDO-1); shared-mime-info declares it.
pub const MARKDOWN_MIME: &str = "text/markdown";
/// The PNG icon sizes under `usr/share/icons/hicolor`, beside the scalable SVG.
pub const ICON_SIZES: [u32; 5] = [32, 48, 128, 256, 512];

/// The desktop entry (LINUXP-2).
fn desktop_entry(executable: &str) -> String {
    let mimes: String = MIME_TYPES
        .iter()
        .chain([&MARKDOWN_MIME])
        .map(|m| format!("{m};"))
        .collect();
    format!(
        "[Desktop Entry]
Type=Application
Name={APP}
GenericName=Diagram Editor
Comment=Mermaid, PlantUML, and Graphviz DOT diagram editor and viewer
Exec={executable} %F
Icon={NAME}
Terminal=false
Categories=Development;
Keywords=mermaid;plantuml;graphviz;dot;diagram;flowchart;
MimeType={mimes}
"
    )
}

/// The shared-mime-info package declaring the diagram file types (LINUXP-3).
pub fn mime_package() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">
",
    );
    for ((id, comment, extensions), mime) in DOCUMENT_TYPES.iter().zip(MIME_TYPES) {
        // Visio drawings are ZIP packages, not text.
        let parent = if *id == crate::VISIO_UTI {
            "application/zip"
        } else {
            "text/plain"
        };
        xml.push_str(&format!(
            "  <mime-type type=\"{mime}\">\n    <comment>{comment}</comment>\n    <sub-class-of type=\"{parent}\"/>\n"
        ));
        for e in *extensions {
            xml.push_str(&format!("    <glob pattern=\"*.{e}\"/>\n"));
        }
        xml.push_str("  </mime-type>\n");
    }
    xml.push_str("</mime-info>\n");
    xml
}

/// Writes `dir/AppDir` around `binary` (LINUXP-1) and returns its path.
pub fn appdir(binary: &Path, dir: &Path) -> Result<PathBuf> {
    // Read the binary before removing the old tree, which may hold it.
    let exe_bytes = fs::read(binary).map_err(|e| context(binary, e))?;
    let root = dir.join("AppDir");
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| context(&root, e))?;
    }
    let share = root.join("usr/share");
    let exe = root.join("usr/bin").join(NAME);
    let mut files: Vec<(PathBuf, Vec<u8>)> = vec![
        (exe.clone(), exe_bytes),
        (
            share.join(format!("applications/{NAME}.desktop")),
            desktop_entry(&format!("/usr/bin/{NAME}")).into_bytes(),
        ),
        (
            share.join(format!("mime/packages/{NAME}.xml")),
            mime_package().into_bytes(),
        ),
        (
            share.join(format!("icons/hicolor/scalable/apps/{NAME}.svg")),
            ICON_SVG.as_bytes().to_vec(),
        ),
        (
            root.join(format!("{NAME}.desktop")),
            desktop_entry(NAME).into_bytes(),
        ),
        // UPDATE-4 identifies the mounted or extracted AppImage by this file.
        (root.join(".sirene-appimage"), Vec::new()),
        (root.join(format!("{NAME}.png")), icon_png(256)?),
    ];
    for size in ICON_SIZES {
        files.push((
            share.join(format!("icons/hicolor/{size}x{size}/apps/{NAME}.png")),
            icon_png(size)?,
        ));
    }
    for (name, text) in LICENSES {
        files.push((
            share.join(format!("licenses/{NAME}/{name}")),
            text.as_bytes().to_vec(),
        ));
    }
    for (path, bytes) in &files {
        let parent = path
            .parent()
            .ok_or_else(|| Error::new(format!("{}: no parent", path.display())))?;
        fs::create_dir_all(parent).map_err(|e| context(parent, e))?;
        write(path, bytes)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))
            .map_err(|e| context(&exe, e))?;
        for (link, target) in [
            ("AppRun", format!("usr/bin/{NAME}")),
            (".DirIcon", format!("{NAME}.png")),
        ] {
            let link = root.join(link);
            symlink(target, &link).map_err(|e| context(&link, e))?;
        }
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_types_follow_document_types() {
        let xml = mime_package();
        for ((_, comment, extensions), mime) in DOCUMENT_TYPES.iter().zip(MIME_TYPES) {
            let block = xml
                .split(&format!("<mime-type type=\"{mime}\">"))
                .nth(1)
                .and_then(|rest| rest.split("</mime-type>").next())
                .unwrap();
            assert!(
                block.contains(&format!("<comment>{comment}</comment>")),
                "{xml}"
            );
            for e in *extensions {
                assert!(block.contains(&format!("pattern=\"*.{e}\"")), "{xml}");
            }
        }
        // The zip pairs each MIME type with its own document type.
        assert!(xml.contains("type=\"text/x-mermaid\">\n    <comment>Mermaid"));
        assert!(xml.contains("type=\"text/vnd.graphviz\">\n    <comment>Graphviz"));
        assert!(xml.contains("type=\"text/x-plantuml\">\n    <comment>PlantUML"));
        assert!(xml.contains(
            "type=\"application/vnd.ms-visio.drawing\">\n    <comment>Visio drawing</comment>\n    <sub-class-of type=\"application/zip\"/>\n    <glob pattern=\"*.vsdx\"/>"
        ));
        // Text types stay text.
        assert!(xml.contains("<sub-class-of type=\"text/plain\"/>"));
    }

    #[test]
    fn desktop_entry_lists_every_mime_type() {
        for executable in ["/usr/bin/sirene", "sirene"] {
            let d = desktop_entry(executable);
            assert!(
                d.contains(
                    "MimeType=text/x-mermaid;text/vnd.graphviz;text/x-plantuml;application/vnd.ms-visio.drawing;text/markdown;\n"
                ),
                "{d}"
            );
            assert!(d.contains(&format!("Exec={executable} %F\n")), "{d}");
        }
    }
}
