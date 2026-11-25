//! `sirene check`, and rendering or printing the diagram blocks of Markdown files (CLI-11,
//! CLI-12).

use std::path::{Path, PathBuf};

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Lowered};
use sirene_core::markdown::{self, Block, Part};
use sirene_core::{Error, Result, Severity};

/// Where `sirene check` reports: stderr as text, or collected for one JSON array on stdout
/// (CLI-16).
pub struct Out {
    json: Option<Vec<String>>,
}

impl Out {
    pub fn text() -> Self {
        Out { json: None }
    }

    pub fn json() -> Self {
        Out {
            json: Some(Vec::new()),
        }
    }

    /// One diagnostic: `FILE:LINE:COL: SEVERITY: MESSAGE` with a position, `SEVERITY: MESSAGE`
    /// without one.
    fn diagnostic(
        &mut self,
        file: Option<&str>,
        at: Option<(usize, usize)>,
        severity: &str,
        message: &str,
    ) {
        let Some(records) = &mut self.json else {
            match (file, at) {
                (Some(file), Some((line, col))) => {
                    diagnostic!("{file}:{line}:{col}: {severity}: {message}")
                }
                (Some(file), None) => diagnostic!("{file}: {message}"),
                _ => diagnostic!("{severity}: {message}"),
            }
            return;
        };
        let string = |s: Option<&str>| s.map_or_else(|| "null".to_owned(), json_string);
        let number = |n: Option<usize>| n.map_or_else(|| "null".to_owned(), |n| n.to_string());
        records.push(format!(
            "{{\"file\":{},\"line\":{},\"column\":{},\"severity\":{},\"message\":{}}}",
            string(file),
            number(at.map(|a| a.0)),
            number(at.map(|a| a.1)),
            json_string(severity),
            json_string(message),
        ));
    }

    fn error(&mut self, e: &Error) {
        self.diagnostic(None, None, "error", &e.report());
    }

    /// Writes the JSON array to stdout, when collecting one.
    pub fn finish(self) -> std::io::Result<()> {
        let Some(records) = self.json else {
            return Ok(());
        };
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        if records.is_empty() {
            out.write_all(b"[]\n")?;
        } else {
            out.write_all(format!("[\n  {}\n]\n", records.join(",\n  ")).as_bytes())?;
        }
        out.flush()
    }
}

/// `s` as a JSON string literal.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Prints `lowered`'s diagnostics as `NAME:LINE:COL: SEVERITY: MESSAGE`, with each position
/// moved by `at`. Returns whether there was an error.
pub fn report(
    name: &str,
    src: &str,
    lowered: &Lowered,
    at: impl Fn(usize, usize) -> (usize, usize),
) -> bool {
    report_to(&mut Out::text(), name, src, lowered, at)
}

/// `report`, to `out`.
fn report_to(
    out: &mut Out,
    name: &str,
    src: &str,
    lowered: &Lowered,
    at: impl Fn(usize, usize) -> (usize, usize),
) -> bool {
    let mut errors = false;
    let lines = sirene_core::diagnostic::LineIndex::new(src);
    for d in &lowered.diagnostics {
        let (line, col) = lines.line_col(d);
        let severity = match d.severity {
            Severity::Error => {
                errors = true;
                "error"
            }
            Severity::Warning => "warning",
        };
        out.diagnostic(Some(name), Some(at(line, col)), severity, &d.message);
    }
    errors
}

/// The parts of a Markdown file to draw (MARKDO-5): each diagram block and formula with its number
/// from 1, which counts every part found, drawn or not, so a build without math numbers them as
/// one with it.
struct Plan {
    /// How many parts the file has.
    total: usize,
    parts: Vec<(usize, Part)>,
    /// How many formulas were left out for want of math support.
    skipped: usize,
}

impl Plan {
    /// The parts of `src`, without the formulas when `math` is off.
    fn new(src: &str, math: bool) -> Plan {
        let all = markdown::parts(src);
        let total = all.len();
        let mut skipped = 0;
        let mut parts = Vec::with_capacity(total);
        for (i, part) in all.into_iter().enumerate() {
            if !math && matches!(part, Part::Math(_)) {
                skipped += 1;
            } else {
                parts.push((i + 1, part));
            }
        }
        Plan {
            total,
            parts,
            skipped,
        }
    }

    /// The one warning for the formulas left out, if any.
    fn warning(&self) -> Option<String> {
        let n = self.skipped;
        (n > 0).then(|| {
            let s = if n == 1 { "" } else { "s" };
            format!("math support is off in this build; {n} formula{s} not drawn")
        })
    }
}

/// `part` lowered, in its language.
fn lower(part: &Part) -> Lowered {
    match part {
        Part::Diagram(block) => diagram::lower_as(&block.code, crate::lang(Some(block.lang))),
        Part::Math(math) => math.lower(),
    }
}

/// The text `part` is lowered from.
fn code(part: &Part) -> &str {
    match part {
        Part::Diagram(block) => &block.code,
        Part::Math(math) => &math.source,
    }
}

/// Where a line and column of `code(part)` is in the Markdown.
fn position(part: &Part, line: usize, col: usize) -> (usize, usize) {
    match part {
        Part::Diagram(block) => block.position(line, col),
        Part::Math(math) => math.position(line, col),
    }
}

/// Checks `path`'s diagrams, reporting their diagnostics to `out`: a Markdown file's diagram
/// blocks, every diagram under a directory, or one diagram (stdin for `-`). Returns whether all
/// were read and had no errors. With `as_markdown`, a file or stdin is read as Markdown whatever
/// its name (CLI-15).
pub fn check(out: &mut Out, path: &Path, as_markdown: bool) -> bool {
    let stdin = path.as_os_str() == "-";
    if as_markdown && !path.is_dir() {
        return check_markdown(out, path);
    }
    if stdin {
        return check_source(out, None);
    }
    if path.is_dir() {
        if let Err(e) = std::fs::read_dir(path) {
            out.error(&Error::with_source(
                format!("cannot read {}", path.display()),
                e,
            ));
            return false;
        }
        let scanned = crate::scan(path, path, "mmd");
        for error in &scanned.errors {
            out.error(error);
        }
        if scanned.found.is_empty() {
            if scanned.errors.is_empty() {
                let message = crate::no_diagrams(path);
                let message = message.strip_prefix("error: ").unwrap_or(&message);
                out.diagnostic(None, None, "error", message);
            }
            return false;
        }
        let failed = scanned
            .found
            .iter()
            .filter(|(input, _)| !check_source(out, Some(input)))
            .count();
        return failed == 0 && scanned.errors.is_empty();
    }
    if markdown::is_markdown(path) {
        return check_markdown(out, path);
    }
    check_source(out, Some(path))
}

/// Checks a Markdown file's diagram blocks, or stdin's for `-`.
fn check_markdown(out: &mut Out, path: &Path) -> bool {
    let (name, src) = match crate::read(Some(path.to_path_buf())) {
        Ok(read) => read,
        Err(e) => {
            out.error(&e);
            return false;
        }
    };
    let plan = Plan::new(&src, sirene_core::equation::MATH);
    log::debug!("{name}: {} diagram block(s) and formula(s)", plan.total);
    if plan.total == 0 {
        out.diagnostic(Some(&name), None, "warning", "no diagram blocks");
    }
    if let Some(warning) = plan.warning() {
        out.diagnostic(Some(&name), None, "warning", &warning);
    }
    let mut ok = true;
    for (_, part) in &plan.parts {
        let lowered = lower(part);
        ok &= !report_to(out, &name, code(part), &lowered, |l, c| {
            position(part, l, c)
        });
    }
    ok
}

/// Checks one diagram file, or stdin for `None`.
fn check_source(out: &mut Out, path: Option<&Path>) -> bool {
    match crate::read(path.map(Path::to_path_buf)) {
        Ok((name, src)) => {
            let lowered = crate::lower_file(path.filter(|p| p.as_os_str() != "-"), &src);
            !report_to(out, &name, &src, &lowered, |l, c| (l, c))
        }
        Err(e) => {
            out.error(&e);
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
    let path = rel.as_deref().unwrap_or(image).to_string_lossy();
    let mut target = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            // On Windows backslashes are separators; on Unix they are filename characters.
            '\\' if cfg!(windows) => target.push('/'),
            '\\' => target.push_str("%5C"),
            '%' => target.push_str("%25"),
            '#' => target.push_str("%23"),
            '?' => target.push_str("%3F"),
            ':' => target.push_str("%3A"),
            '&' => target.push_str("%26"),
            c if c.is_ascii_control() => {
                use std::fmt::Write as _;
                let _ = write!(target, "%{:02X}", c as u32);
            }
            c => target.push(c),
        }
    }
    // A CommonMark link destination with spaces or parentheses goes in `<...>`.
    if target.contains(|c: char| c.is_whitespace() || "()<>".contains(c)) {
        format!("<{}>", target.replace('<', "\\<").replace('>', "\\>"))
    } else {
        target
    }
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

/// Recognize a complete link in the form generated by `link_line`.
fn generated_link(line: &str) -> bool {
    let Some((number, dest)) = line
        .strip_prefix("![diagram ")
        .and_then(|line| line.split_once("]("))
    else {
        return false;
    };
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Some(dest) = dest.strip_suffix(')') else {
        return false;
    };
    if let Some(dest) = dest.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        let mut chars = dest.chars();
        while let Some(c) = chars.next() {
            match c {
                '<' | '>' => return false,
                '\\' if !matches!(chars.next(), Some('<' | '>')) => return false,
                _ => {}
            }
        }
        true
    } else {
        !dest.is_empty() && !dest.contains(|c: char| c.is_whitespace() || "()<>\\".contains(c))
    }
}

/// `src` with `lines[k]` (a link line for block `k`, at the fence's indentation) after each block
/// in `blocks` that is closed and not already followed by it. A `![diagram ...](...)` line from
/// an earlier run, numbered for another block, is replaced. The blocks themselves are never
/// changed.
pub fn with_links(src: &str, blocks: &[(Block, Option<String>)]) -> String {
    let newline = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (block, line) in blocks {
        let Some(line) = line else {
            continue;
        };
        // An unclosed block has no closing fence for the link to follow.
        if !block.closed {
            continue;
        }
        let rest = &src[block.end..];
        let next = rest.lines().next().unwrap_or("");
        // Only consider links in the same container and at the same indentation.
        let prefix = format!("{}{}", block.quote, " ".repeat(block.indent));
        let old = next.strip_prefix(&prefix).map(str::trim_end);
        if old == Some(line.as_str()) {
            continue;
        }
        out.push_str(&src[at..block.end]);
        if !out.ends_with('\n') {
            out.push_str(newline);
        }
        out.push_str(&block.quote);
        out.push_str(&" ".repeat(block.indent));
        out.push_str(line);
        out.push_str(newline);
        at = block.end;
        if old.is_some_and(generated_link) {
            let ending = &rest[next.len()..];
            let ending = if ending.starts_with("\r\n") {
                2
            } else {
                usize::from(ending.starts_with('\n'))
            };
            at += next.len() + ending;
        }
    }
    out.push_str(&src[at..]);
    out
}

/// Prints each diagram block of Markdown file `md` (stdin for `-`) as text, a blank line between
/// them, and their diagnostics. Reports output completion separately from input diagnostics.
pub fn print_ascii(md: &Path, charset: Charset, width: Option<usize>) -> crate::Done {
    let (name, src) = match crate::read(Some(md.to_path_buf())) {
        Ok(read) => read,
        Err(e) => {
            diagnostic!("error: {}", e.report());
            return crate::Done {
                written: false,
                clean: false,
            };
        }
    };
    let plan = Plan::new(&src, sirene_core::equation::MATH);
    if plan.total == 0 {
        diagnostic!("{name}: no diagram blocks");
    }
    if let Some(warning) = plan.warning() {
        diagnostic!("{name}: {warning}");
    }
    let mut ok = true;
    let mut written = true;
    let mut out = String::new();
    for (i, (n, part)) in plan.parts.iter().enumerate() {
        let lowered = lower(part);
        ok &= !report(&name, code(part), &lowered, |l, c| position(part, l, c));
        if i > 0 {
            out.push('\n');
        }
        match lowered.ascii(&Options { charset }) {
            Ok(drawing) => {
                let block_name = format!("{name} block {n}");
                ok &= !crate::too_wide(&block_name, &drawing, width);
                out.push_str(&drawing);
            }
            Err(e) => {
                diagnostic!(
                    "error: {}",
                    e.context(format!("cannot render {name} block {n}"))
                        .report()
                );
                ok = false;
                written = false;
            }
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(out.as_bytes())
        .and_then(|()| stdout.flush())
    {
        diagnostic!(
            "error: {}",
            Error::with_source("cannot write the drawing", e).report()
        );
        return crate::Done {
            written: false,
            clean: false,
        };
    }
    crate::Done { written, clean: ok }
}

/// Renders each diagram block of Markdown file `md` with `render` to an image beside it (or
/// in `out`), named by [`image_path`]; with `link`, adds an image link after each rendered
/// block. Reports output completion separately from input diagnostics.
pub fn render(
    md: &Path,
    out: Option<&Path>,
    ext: &str,
    link: bool,
    render: &impl Fn(&Lowered) -> Result<Vec<u8>>,
) -> crate::Done {
    let name = md.display().to_string();
    let src = match crate::read_text(md) {
        Ok(src) => src,
        Err(e) => {
            diagnostic!("error: {}", e.report());
            return crate::Done {
                written: false,
                clean: false,
            };
        }
    };
    if let Some(dir) = out {
        if dir.exists() && !dir.is_dir() {
            diagnostic!(
                "error: {} is not a directory; a Markdown file renders one image per block into one",
                dir.display()
            );
            return crate::Done {
                written: false,
                clean: false,
            };
        }
        if let Err(e) = std::fs::create_dir_all(dir) {
            diagnostic!("error: cannot create {}: {e}", dir.display());
            return crate::Done {
                written: false,
                clean: false,
            };
        }
    }
    let plan = Plan::new(&src, sirene_core::equation::MATH);
    if plan.total == 0 {
        diagnostic!("{name}: no diagram blocks");
    }
    if let Some(warning) = plan.warning() {
        diagnostic!("{name}: {warning}");
    }
    let mut ok = true;
    let mut completed = true;
    let mut links = Vec::new();
    for (n, part) in plan.parts {
        let lowered = lower(&part);
        ok &= !report(&name, code(&part), &lowered, |l, c| position(&part, l, c));
        let image = image_path(md, out, n, ext);
        let written = render(&lowered).and_then(|bytes| {
            crate::write_file(&image, &bytes, false)
                .map_err(|e| Error::with_source(format!("cannot write {}", image.display()), e))
        });
        match written {
            Ok(()) => {
                diagnostic!("wrote {}", image.display());
                if let Part::Diagram(block) = part {
                    links.push((block, Some(link_line(md, &image, n))));
                }
            }
            Err(message) => {
                diagnostic!("error: {}", message.report());
                ok = false;
                completed = false;
                if let Part::Diagram(block) = part {
                    links.push((block, None));
                }
            }
        }
    }
    if link {
        let linked = with_links(&src, &links);
        if linked != src {
            if let Err(e) = crate::write_file(md, linked.as_bytes(), true) {
                diagnostic!("error: cannot write {name}: {e}");
                return crate::Done {
                    written: false,
                    clean: false,
                };
            }
            diagnostic!("linked the images in {name}");
        }
    }
    crate::Done {
        written: completed,
        clean: ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without math support the formulas are left out with one warning, and the numbers of the
    /// parts are those of a build with it.
    #[test]
    fn formulas_are_skipped_once_without_math() {
        let src = "$a$ $b$\n\n```mermaid\npie\n```\n\n$$c$$\n\n```math\nd\n```\n";
        let on = Plan::new(src, true);
        assert_eq!((on.total, on.skipped, on.warning()), (5, 0, None));
        assert_eq!(
            on.parts.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        let off = Plan::new(src, false);
        assert_eq!((off.total, off.skipped), (5, 4));
        assert_eq!(off.parts.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [3]);
        assert_eq!(
            off.warning().as_deref(),
            Some("math support is off in this build; 4 formulas not drawn")
        );
        let one = Plan::new("$a$\n", false);
        assert_eq!(
            one.warning().as_deref(),
            Some("math support is off in this build; 1 formula not drawn")
        );
        // A file with no formulas has no warning, however it is built.
        assert_eq!(Plan::new("```mermaid\npie\n```\n", false).warning(), None);
        assert_eq!(Plan::new("", false).total, 0);
    }

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
        // Spaces and parentheses need `<...>` to stay one link destination.
        assert_eq!(
            link_target(md, Path::new("docs/my img/a (1).svg")),
            "<my img/a (1).svg>"
        );
        assert_eq!(
            link_target(md, Path::new("docs/a<b>.svg")),
            "<a\\<b\\>.svg>"
        );
        for (image, target) in [
            ("docs/guide#v2.svg", "guide%23v2.svg"),
            ("docs/guide?x.svg", "guide%3Fx.svg"),
            ("docs/guide%20.svg", "guide%2520.svg"),
            ("docs/guide:v2.svg", "guide%3Av2.svg"),
            ("docs/guide&amp;.svg", "guide%26amp;.svg"),
            ("docs/a\nb\rc\td\u{7f}.svg", "a%0Ab%0Dc%09d%7F.svg"),
            ("docs/img#1/guide (1)%.svg", "<img%231/guide (1)%25.svg>"),
        ] {
            assert_eq!(link_target(md, Path::new(image)), target);
        }
        #[cfg(unix)]
        assert_eq!(link_target(md, Path::new("docs/a\\b.svg")), "a%5Cb.svg");
    }

    #[test]
    fn links_preserve_unrelated_adjacent_images() {
        for (src, expected) in [
            (
                "> ```mermaid\n> pie\n> ```\n![diagram 9](personal.svg)\n",
                "> ```mermaid\n> pie\n> ```\n> ![diagram 1](a-1.svg)\n![diagram 9](personal.svg)\n",
            ),
            (
                "```mermaid\npie\n```\n![diagram architecture](personal.svg)\n",
                "```mermaid\npie\n```\n![diagram 1](a-1.svg)\n![diagram architecture](personal.svg)\n",
            ),
            (
                "```mermaid\npie\n```\n![diagram 9](old.svg) and ![x](keep.svg)\n",
                "```mermaid\npie\n```\n![diagram 1](a-1.svg)\n![diagram 9](old.svg) and ![x](keep.svg)\n",
            ),
            (
                "```mermaid\npie\n```\n![diagram 9](<old (1).svg>) and ![x](keep.svg)\n",
                "```mermaid\npie\n```\n![diagram 1](a-1.svg)\n![diagram 9](<old (1).svg>) and ![x](keep.svg)\n",
            ),
            (
                "> ```mermaid\n> pie\n> ```\n> ![diagram 9](old.svg)\n",
                "> ```mermaid\n> pie\n> ```\n> ![diagram 1](a-1.svg)\n",
            ),
            (
                "```mermaid\npie\n```\n![diagram 9](<old (1)\\<x\\>.svg>)\n",
                "```mermaid\npie\n```\n![diagram 1](a-1.svg)\n",
            ),
        ] {
            let link = Some("![diagram 1](a-1.svg)".to_owned());
            let blocks = markdown::mermaid_blocks(src);
            let once = with_links(src, &[(blocks[0].clone(), link.clone())]);
            assert_eq!(once, expected);
            let blocks = markdown::mermaid_blocks(&once);
            assert_eq!(with_links(&once, &[(blocks[0].clone(), link)]), once);
        }
    }

    #[test]
    fn links_renumber_and_indent() {
        // A block that moved from 1 to 2 gets its link replaced, not a second one.
        let src = "```mermaid\npie\n```\n```mermaid\ngraph TD\n```\n![diagram 1](a-1.svg)\nb\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![
            (blocks[0].clone(), Some("![diagram 1](a-1.svg)".to_owned())),
            (blocks[1].clone(), Some("![diagram 2](a-2.svg)".to_owned())),
        ];
        assert_eq!(
            with_links(src, &lines),
            "```mermaid\npie\n```\n![diagram 1](a-1.svg)\n```mermaid\ngraph TD\n```\n![diagram 2](a-2.svg)\nb\n"
        );
        // A fence in a list item keeps the link inside it.
        let src = "- item\n\n  ```mermaid\n  pie\n  ```\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![(blocks[0].clone(), Some("![diagram 1](a-1.svg)".to_owned()))];
        let once = with_links(src, &lines);
        assert_eq!(once, format!("{src}  ![diagram 1](a-1.svg)\n"));
        assert_eq!(with_links(&once, &lines), once);
        // A fence in a blockquote's list item keeps the link in both.
        let src = "> 1. x\n>    ```mermaid\n>    pie\n>    ```\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![(blocks[0].clone(), Some("![diagram 1](a-1.svg)".to_owned()))];
        let once = with_links(src, &lines);
        assert_eq!(once, format!("{src}>    ![diagram 1](a-1.svg)\n"));
        assert_eq!(with_links(&once, &lines), once);
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
        // CRLF lines stay CRLF.
        let src = "```mermaid\r\ngraph TD\r\n```\r\nb\r\n";
        let blocks = markdown::mermaid_blocks(src);
        let lines = vec![(blocks[0].clone(), Some("![d](x.svg)".to_owned()))];
        assert_eq!(
            with_links(src, &lines),
            "```mermaid\r\ngraph TD\r\n```\r\n![d](x.svg)\r\nb\r\n"
        );
    }
}
