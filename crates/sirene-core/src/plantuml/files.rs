//! PlantUML file trees (PFILES-*): `@startfiles` paths, one per line, drawn as a tree view
//! (MTREEV-2), with each `<note>` as its entry's description.

use rowan::TextRange;

use super::lines::{self, Common, Cursor};
use crate::diagnostic::Diagnostic;
use crate::lines::MAX_DEPTH;
use crate::syntax::SyntaxNode;
use crate::treeview::{Entry, TreeView};

/// The entry at `path` (indices into each level's children) in `entries`.
fn entry<'a>(entries: &'a mut [Entry], path: &[usize]) -> &'a mut Entry {
    let (first, rest) = path.split_first().expect("a path has an entry");
    let mut e = &mut entries[*first];
    for &i in rest {
        e = &mut e.children[i];
    }
    e
}

pub(crate) fn lower(root: &SyntaxNode) -> (TreeView, Vec<Diagnostic>) {
    let mut d = Vec::new();
    let mut view = TreeView {
        config: crate::treeview::Config {
            show_icons: true,
            ..Default::default()
        },
        ..TreeView::default()
    };
    let mut cursor = Cursor::new(lines::read(root));
    // The last path's entry, which a note describes.
    let mut last: Option<Vec<usize>> = None;
    while let Some(line) = cursor.next() {
        match lines::common(&line, &mut cursor, &mut d) {
            Common::No => {}
            Common::Title(t) => {
                view.title = Some(t);
                continue;
            }
            Common::Done | Common::Direction(_) => continue,
        }
        let t = line.text.as_str();
        if t.eq_ignore_ascii_case("<note>") {
            let (body, closer) = cursor.until(&["</note>"]);
            if closer.is_none() {
                d.push(Diagnostic::new(line.range, "`<note>` has no `</note>`"));
            }
            let text: Vec<&str> = body.iter().map(|l| l.text.as_str()).collect();
            match &last {
                Some(path) => {
                    let e = entry(&mut view.entries, path);
                    let note = text.join(" ");
                    e.description = Some(match e.description.take() {
                        Some(before) => format!("{before} {note}"),
                        None => note,
                    });
                }
                None => d.push(Diagnostic::new(
                    line.range,
                    "a note follows the path it describes",
                )),
            }
            continue;
        }
        let Some(path) = t.strip_prefix('/').or_else(|| t.strip_prefix("~/")) else {
            d.push(Diagnostic::new(
                line.range,
                "expected a path such as `/docs/readme.md`",
            ));
            continue;
        };
        last = add(&mut view.entries, path, line.range, &mut d);
    }
    (view, d)
}

/// Adds `path`'s directories and its entry, returning the entry's place.
fn add(
    entries: &mut Vec<Entry>,
    path: &str,
    range: TextRange,
    d: &mut Vec<Diagnostic>,
) -> Option<Vec<usize>> {
    let directory = path.ends_with('/');
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    if parts.len() > MAX_DEPTH {
        d.push(Diagnostic::new(
            range,
            format!("paths nest more than {MAX_DEPTH} levels deep"),
        ));
        return None;
    }
    let mut place = Vec::new();
    let mut level = entries;
    for (k, part) in parts.iter().enumerate() {
        let leaf = k + 1 == parts.len();
        let i = match level.iter().position(|e| e.name == *part) {
            Some(i) => i,
            None => {
                level.push(Entry {
                    name: (*part).to_owned(),
                    directory: !leaf || directory,
                    ..Entry::default()
                });
                level.len() - 1
            }
        };
        if !leaf || directory {
            level[i].directory = true;
        }
        place.push(i);
        level = &mut level[i].children;
    }
    Some(place)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| {
                let name = if e.directory {
                    format!("{}/", e.name)
                } else {
                    e.name.clone()
                };
                if e.children.is_empty() {
                    name
                } else {
                    format!("{name}({})", names(&e.children).join(" "))
                }
            })
            .collect()
    }

    #[test]
    fn paths() {
        let mut entries = Vec::new();
        let mut d = Vec::new();
        let r = TextRange::default();
        assert_eq!(add(&mut entries, "a/b.txt", r, &mut d), Some(vec![0, 0]));
        assert_eq!(add(&mut entries, "a/c/", r, &mut d), Some(vec![0, 1]));
        assert_eq!(add(&mut entries, "a/c/d", r, &mut d), Some(vec![0, 1, 0]));
        assert_eq!(add(&mut entries, "e", r, &mut d), Some(vec![1]));
        // A file named again as a directory becomes one.
        assert_eq!(add(&mut entries, "e/f", r, &mut d), Some(vec![1, 0]));
        assert_eq!(add(&mut entries, "/", r, &mut d), None);
        assert_eq!(names(&entries), ["a/(b.txt c/(d))", "e/(f)"]);
        let deep = "x/".repeat(MAX_DEPTH + 1);
        assert_eq!(add(&mut entries, &deep, r, &mut d), None);
        assert_eq!(d.len(), 1);
    }
}
