//! `sirene check`, and rendering or printing the diagram blocks of Markdown files (CLI-11,
//! CLI-12).

use std::path::{Path, PathBuf};

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::markdown::{self, Block};

/// Prints `lowered`'s diagnostics as `NAME:LINE:COL: SEVERITY: MESSAGE`, with each position
/// moved by `at`. Returns whether there was an error.
pub fn report(
    name: &str,
    src: &str,
    lowered: &Lowered,
    at: impl Fn(usize, usize) -> (usize, usize),
) -> bool {
    let mut errors = false;
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let (line, col) = at(line, col);
        let severity = match d.severity {
            Severity::Error => {
                errors = true;
                "error"
            }
            Severity::Warning => "warning",
        };
        eprintln!("{name}:{line}:{col}: {severity}: {}", d.message);
    }
    errors
}

/// Checks `path`'s diagrams, printing their diagnostics: a Markdown file's diagram blocks, every
/// diagram under a directory, or one diagram (stdin for `-`). Returns whether all were read
/// and had no errors.
pub fn check(path: &Path) -> bool {
    if path.as_os_str() == "-" {
        return check_source(None);
    }
    if path.is_dir() {
        if let Err(e) = std::fs::read_dir(path) {
            eprintln!("error: cannot read {}: {e}", path.display());
            return false;
        }
        let found = crate::scan(path, path, "mmd");
        if found.is_empty() {
            eprintln!("{}", crate::no_diagrams(path));
            return false;
        }
        let failed = found
            .iter()
            .filter(|(input, _)| !check_source(Some(input)))
            .count();
        return failed == 0;
    }
    if markdown::is_markdown(path) {
        let name = path.display().to_string();
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                eprintln!("error: cannot read {name}: {e}");
                return false;
            }
        };
        let blocks = markdown::mermaid_blocks(&src);
        log::debug!("{name}: {} diagram block(s)", blocks.len());
        let mut ok = true;
        for block in &blocks {
            let lowered = diagram::lower_as(&block.code, crate::lang(Some(block.lang)));
            ok &= !report(&name, &block.code, &lowered, |l, c| block.position(l, c));
        }
        return ok;
    }
    check_source(Some(path))
}

/// Checks one diagram file, or stdin for `None`.
fn check_source(path: Option<&Path>) -> bool {
    match crate::read(path.map(Path::to_path_buf)) {
        Ok((name, src)) => {
            let lowered = crate::lower_file(path.filter(|p| p.as_os_str() != "-"), &src);
            !report(&name, &src, &lowered, |l, c| (l, c))
        }
        Err(message) => {
            eprintln!("error: {message}");
            false
        }
    }
}

/// The image for block `n` (from 1) of Markdown file `md`: `STEM-N.EXT` in `out`, or beside it.
pub fn image_path(md: &Path, out: Option<&Path>, n: usize, ext: &str) -> PathBuf {
    let stem = md.file_stem().map_or_else(
        || "diagram".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let dir = out
        .map(Path::to_path_buf)
        .or_else(|| md.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    dir.join(format!("{stem}-{n}.{ext}"))
}

/// How `md` refers to `image` in a link: relative to `md`'s directory (`../img/a-1.svg` for
/// `docs/a.md` and `img/a-1.svg`), except an absolute `image` outside it, which is kept.
fn link_target(md: &Path, image: &Path) -> String {
    let dir = md.parent().unwrap_or(Path::new(""));
    let rel = relative(dir, image).filter(|rel| !image.is_absolute() || !rel.starts_with(".."));
    rel.as_deref()
        .unwrap_or(image)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The path from directory `from` to `to`, both read against the working directory with `.`
/// and `..` resolved by name. `None` when they share no root, as on two Windows drives.
fn relative(from: &Path, to: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::path::Component;
    /// The root (a drive prefix and root, or just the root) and the names below it.
    fn parts(p: &Path) -> Option<(OsString, Vec<OsString>)> {
        let p = if p.as_os_str().is_empty() {
            Path::new(".")
        } else {
            p
        };
        let abs = std::path::absolute(p).ok()?;
        let mut root = OsString::new();
        let mut names: Vec<OsString> = Vec::new();
        for c in abs.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => root.push(c.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    names.pop();
                }
                Component::Normal(s) => names.push(s.to_owned()),
            }
        }
        Some((root, names))
    }
    let ((from_root, from), (to_root, to)) = (parts(from)?, parts(to)?);
    if from_root != to_root {
        return None;
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut rel = PathBuf::new();
    for _ in common..from.len() {
        rel.push("..");
    }
    for name in &to[common..] {
        rel.push(name);
    }
    Some(rel)
}

/// The line `--link` puts after block `n`.
fn link_line(md: &Path, image: &Path, n: usize) -> String {
    format!("![diagram {n}]({})", link_target(md, image))
}

/// `src` with `lines[k]` (a link line for block `k`) after each block in `blocks` that is
/// closed and not already followed by it. The blocks themselves are never changed.
pub fn with_links(src: &str, blocks: &[(Block, Option<String>)]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (block, line) in blocks {
        let Some(line) = line else {
            continue;
        };
        // An unclosed block runs to the end: nothing can follow it.
        if !block.closed {
            continue;
        }
        let rest = &src[block.end..];
        let next = rest.lines().next().unwrap_or("");
        if next.trim() == line {
            continue;
        }
        out.push_str(&src[at..block.end]);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        at = block.end;
    }
    out.push_str(&src[at..]);
    out
}

/// Prints each diagram block of Markdown file `md` as text, a blank line between them, and
/// their diagnostics. Returns whether the file was read and had no errors.
pub fn print_ascii(md: &Path, charset: Charset) -> bool {
    let name = md.display().to_string();
    let src = match std::fs::read_to_string(md) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("error: cannot read {name}: {e}");
            return false;
        }
    };
    let blocks = markdown::mermaid_blocks(&src);
    if blocks.is_empty() {
        eprintln!("{name}: no diagram blocks");
    }
    let mut ok = true;
    let mut out = String::new();
    for (i, block) in blocks.iter().enumerate() {
        let lowered = diagram::lower_as(&block.code, crate::lang(Some(block.lang)));
        ok &= !report(&name, &block.code, &lowered, |l, c| block.position(l, c));
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&lowered.diagram.ascii(&Options { charset }));
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    use std::io::Write;
    if let Err(e) = std::io::stdout().write_all(out.as_bytes()) {
        eprintln!("error: cannot write the drawing: {e}");
        return false;
    }
    ok
}

/// Renders each diagram block of Markdown file `md` with `render` to an image beside it (or
/// in `out`), named by [`image_path`]; with `link`, adds an image link after each rendered
/// block. Returns whether everything was read, written, and free of errors.
pub fn render(
    md: &Path,
    out: Option<&Path>,
    ext: &str,
    link: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>, String>,
) -> bool {
    let name = md.display().to_string();
    let src = match std::fs::read_to_string(md) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("error: cannot read {name}: {e}");
            return false;
        }
    };
    if let Some(dir) = out {
        if dir.exists() && !dir.is_dir() {
            eprintln!(
                "error: {} is not a directory; a Markdown file renders one image per block into one",
                dir.display()
            );
            return false;
        }
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("error: cannot create {}: {e}", dir.display());
            return false;
        }
    }
    let blocks = markdown::mermaid_blocks(&src);
    if blocks.is_empty() {
        eprintln!("{name}: no diagram blocks");
    }
    let mut ok = true;
    let mut links = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        let n = i + 1;
        let lowered = diagram::lower_as(&block.code, crate::lang(Some(block.lang)));
        ok &= !report(&name, &block.code, &lowered, |l, c| block.position(l, c));
        let image = image_path(md, out, n, ext);
        let written = render(&lowered).and_then(|bytes| {
            std::fs::write(&image, bytes)
                .map_err(|e| format!("cannot write {}: {e}", image.display()))
        });
        match written {
            Ok(()) => {
                eprintln!("wrote {}", image.display());
                links.push((block, Some(link_line(md, &image, n))));
            }
            Err(message) => {
                eprintln!("error: {message}");
                ok = false;
                links.push((block, None));
            }
        }
    }
    if link {
        let linked = with_links(&src, &links);
        if linked != src {
            if let Err(e) = std::fs::write(md, linked) {
                eprintln!("error: cannot write {name}: {e}");
                return false;
            }
            eprintln!("linked the images in {name}");
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_paths_and_links() {
        let md = Path::new("docs/guide.md");
        assert_eq!(
            image_path(md, None, 2, "svg"),
            Path::new("docs/guide-2.svg")
        );
        let out = Path::new("docs/img");
        let image = image_path(md, Some(out), 1, "png");
        assert_eq!(image, Path::new("docs/img/guide-1.png"));
        assert_eq!(link_line(md, &image, 1), "![diagram 1](img/guide-1.png)");
        assert_eq!(
            link_target(md, Path::new("/elsewhere/a.png")),
            "/elsewhere/a.png"
        );
        // An image outside the Markdown file's directory is linked from that directory.
        assert_eq!(
            link_target(md, Path::new("img/guide-1.png")),
            "../img/guide-1.png"
        );
        assert_eq!(
            link_target(Path::new("a/b/c.md"), Path::new("a/x/../d/c-1.svg")),
            "../d/c-1.svg"
        );
        assert_eq!(
            link_target(Path::new("./c.md"), Path::new("c-1.svg")),
            "c-1.svg"
        );
        let abs = std::path::absolute("docs/img/guide-1.png").unwrap();
        assert_eq!(link_target(md, &abs), "img/guide-1.png");
        // From above the working directory, through its name.
        let cwd = std::env::current_dir().unwrap();
        let here = cwd.file_name().unwrap().to_string_lossy();
        assert_eq!(
            link_target(Path::new("../c.md"), Path::new("out/c-1.svg")),
            format!("{here}/out/c-1.svg")
        );
        assert_eq!(
            image_path(Path::new("x.md"), None, 1, "svg"),
            Path::new("x-1.svg")
        );
    }

    #[test]
    fn links_follow_blocks_once() {
        let src = "a\n```mermaid\ngraph TD\n```\nb\n```mermaid\npie\n```\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![
            (blocks[0].clone(), Some("![diagram 1](a-1.svg)".to_owned())),
            (blocks[1].clone(), None),
        ];
        let once = with_links(src, &lines);
        assert_eq!(
            once,
            "a\n```mermaid\ngraph TD\n```\n![diagram 1](a-1.svg)\nb\n```mermaid\npie\n```\n"
        );
        // Again: already linked.
        let blocks = markdown::mermaid_blocks(&once);
        let lines = vec![(blocks[0].clone(), Some("![diagram 1](a-1.svg)".to_owned()))];
        assert_eq!(with_links(&once, &lines), once);
        // A block closed at the very end without a newline, and an unclosed one.
        let src = "```mermaid\ngraph TD\n```";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![(blocks[0].clone(), Some("![d](x.svg)".to_owned()))];
        assert_eq!(
            with_links(src, &lines),
            "```mermaid\ngraph TD\n```\n![d](x.svg)\n"
        );
        let src = "```mermaid\ngraph TD\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![(blocks[0].clone(), Some("![d](x.svg)".to_owned()))];
        assert_eq!(with_links(src, &lines), src);
    }
}
