//! Mermaid tree views (MTREEV-*): a directory-like hierarchy of files and folders, nested by
//! indentation or drawn with box-drawing characters.

mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::lines::{MAX_DEPTH, too_deep};
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// A file or directory.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    /// The name, without a directory's trailing `/`.
    pub name: String,
    /// Whether the name ended in `/`.
    pub directory: bool,
    /// The class from `:::class`; `highlight` is drawn.
    pub class: Option<String>,
    /// The icon from `icon(name)`; `none` for `icon()`.
    pub icon: Option<String>,
    /// The text after `##`.
    pub description: Option<String>,
    pub children: Vec<Entry>,
}

impl Entry {
    /// Whether `:::highlight` marks this entry.
    pub fn highlighted(&self) -> bool {
        self.class
            .as_deref()
            .is_some_and(|c| c.split_whitespace().any(|c| c == "highlight"))
    }

    /// The icon to draw, if any: an explicit `icon()`, else `folder` or `file` when
    /// `show_icons` is on (MTREEV-4).
    pub fn shown_icon(&self, show_icons: bool) -> Option<&str> {
        match self.icon.as_deref() {
            Some("none") => None,
            Some(i) => Some(i),
            None if show_icons => Some(if self.directory { "folder" } else { "file" }),
            None => None,
        }
    }
}

/// The `treeView` config: Mermaid's defaults, or frontmatter values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    pub row_indent: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub line_thickness: f32,
    pub show_icons: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            row_indent: 10.0,
            padding_x: 5.0,
            padding_y: 5.0,
            line_thickness: 1.0,
            show_icons: false,
        }
    }
}

/// A tree view (MTREEV-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeView {
    /// The `title` statement, or the frontmatter title. Mermaid does not draw it.
    pub title: Option<String>,
    pub config: Config,
    /// The top-level entries, under the root `/`.
    pub entries: Vec<Entry>,
}

/// The result of parsing tree view source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`TreeView`], with the parser's diagnostics.
    pub fn lower(&self) -> (TreeView, Vec<Diagnostic>) {
        let root = self.syntax();
        let src = root.to_string();
        let mut diagnostics = self.diagnostics.clone();
        let mut view = TreeView::default();
        let mut stmt_title = None;
        for el in root.children_with_tokens() {
            if el.kind() == FRONTMATTER
                && let Some(tok) = el.as_token()
            {
                view.title = text::frontmatter_title(tok.text());
                config(&mut view.config, tok.text());
            }
            if el.kind() == TITLE_STMT
                && let Some(n) = el.as_node()
                && let Some(t) = tokens(n).find(|t| t.kind() == TEXT)
            {
                stmt_title = Some(t.text().trim().to_owned()).filter(|t| !t.is_empty());
            }
        }
        if stmt_title.is_some() {
            view.title = stmt_title;
        }
        let nodes: Vec<SyntaxNode> = root.children().filter(|n| n.kind() == NODE).collect();
        let boxed = nodes
            .iter()
            .any(|n| tokens(n).any(|t| t.kind() == TREE_BRANCH))
            || parser::boxed(&src[header_end(&root)..]);
        // The column of a node's branch character, with tabs as four columns.
        let column = |n: &SyntaxNode| -> Option<usize> {
            let branch = tokens(n).find(|t| t.kind() == TREE_BRANCH)?;
            let start = usize::from(n.text_range().start());
            let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
            let prefix = format!("{}{}", &src[line_start..start], branch.text());
            let at = prefix.find(['└', '┗', '├', '┣'])?;
            Some(
                prefix[..at]
                    .chars()
                    .map(|c| if c == '\t' { 4 } else { 1 })
                    .sum(),
            )
        };
        let segment = nodes
            .iter()
            .filter_map(column)
            .find(|&c| c > 0)
            .unwrap_or(4);
        let mut flat: Vec<(isize, Entry)> = Vec::new();
        let mut ranges = Vec::new();
        for n in &nodes {
            let Some(entry) = entry(n) else { continue };
            let start = usize::from(n.text_range().start());
            let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
            let lead = &src[line_start..start];
            let level = if boxed {
                match column(n) {
                    Some(c) => ((c as f32 / segment as f32).round() as isize + 1) * 4,
                    None => {
                        if !lead.is_empty() {
                            diagnostics.push(Diagnostic::new(
                                n.text_range(),
                                "expected `├──` or `└──` before an indented line in a tree \
                                 drawn with box-drawing characters",
                            ));
                        }
                        0
                    }
                }
            } else {
                lead.chars().count() as isize
            };
            flat.push((level, entry));
            ranges.push(n.text_range());
        }
        let (entries, left_out) = build(flat);
        view.entries = entries;
        if let Some(i) = left_out {
            diagnostics.push(Diagnostic::new(ranges[i], too_deep("entries", MAX_DEPTH)));
        }
        log::debug!("lowered a tree view");
        (view, diagnostics)
    }
}

/// Where the header line ends in `root`'s text.
fn header_end(root: &SyntaxNode) -> usize {
    root.children()
        .find(|n| n.kind() == HEADER)
        .map_or(0, |h| usize::from(h.text_range().end()))
}

fn tokens(n: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + use<> {
    n.children_with_tokens().filter_map(|e| e.into_token())
}

/// The entry a `NODE` names, or `None` when it has no name.
fn entry(n: &SyntaxNode) -> Option<Entry> {
    let mut e = Entry::default();
    let mut named = false;
    for t in tokens(n) {
        let s = t.text();
        match t.kind() {
            STRING => {
                e.name = s[1..s.len() - 1].to_owned();
                named = true;
            }
            TEXT => {
                e.name = s.to_owned();
                named = true;
            }
            CLASS_SHORTHAND => e.class = Some(s[3..].trim().to_owned()),
            TREE_ICON => {
                let name = &s[5..s.len() - 1];
                e.icon = Some(if name.is_empty() { "none" } else { name }.to_owned());
            }
            TREE_DESC => {
                e.description = Some(s[2..].trim().to_owned()).filter(|d| !d.is_empty());
            }
            _ => {}
        }
    }
    if let Some(name) = e.name.strip_suffix('/') {
        e.name = name.to_owned();
        e.directory = true;
    }
    named.then_some(e)
}

/// The largest `treeView` size setting, in pixels.
const MAX_CONFIG: f32 = 1000.0;

/// Reads the `treeView` frontmatter config into `c`.
fn config(c: &mut Config, block: &str) {
    for (k, v) in text::frontmatter_config(block, "treeView") {
        // Capped so sizes summed over a deep tree stay finite.
        let num = v
            .parse::<f32>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(|n| n.min(MAX_CONFIG));
        match (k.as_str(), num) {
            ("rowIndent", Some(n)) => c.row_indent = n,
            ("paddingX", Some(n)) => c.padding_x = n,
            ("paddingY", Some(n)) => c.padding_y = n,
            ("lineThickness", Some(n)) => c.line_thickness = n,
            ("showIcons", _) => c.show_icons = v == "true",
            _ => {}
        }
    }
}

/// The tree of `(level, entry)` lines as Mermaid builds it: each line is a child of the nearest
/// line above it with a lower level, under a root at level -1 (MTREEV-2). An entry more than
/// `MAX_DEPTH` deep is left out, and the index of the first such line is returned (LANGS-8).
fn build(lines: Vec<(isize, Entry)>) -> (Vec<Entry>, Option<usize>) {
    // An arena of entries with their children's indexes; index 0 is the root.
    let mut nodes: Vec<(Entry, Vec<usize>)> = vec![(Entry::default(), Vec::new())];
    let mut stack: Vec<(isize, usize)> = vec![(-1, 0)];
    let mut left_out = None;
    for (n, (level, e)) in lines.into_iter().enumerate() {
        while stack.last().is_some_and(|&(l, _)| level <= l) {
            stack.pop();
        }
        if stack.len() > MAX_DEPTH {
            left_out.get_or_insert(n);
            continue;
        }
        let parent = stack.last().map_or(0, |&(_, i)| i);
        let i = nodes.len();
        nodes.push((e, Vec::new()));
        nodes[parent].1.push(i);
        stack.push((level, i));
    }
    fn take(nodes: &mut [(Entry, Vec<usize>)], i: usize) -> Entry {
        let kids = std::mem::take(&mut nodes[i].1);
        let mut e = std::mem::take(&mut nodes[i].0);
        e.children = kids.into_iter().map(|k| take(nodes, k)).collect();
        e
    }
    (take(&mut nodes, 0).children, left_out)
}

/// Parses tree view source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(es: &[Entry]) -> String {
        let parts: Vec<String> = es
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
                    format!("{name}({})", shape(&e.children))
                }
            })
            .collect();
        parts.join(",")
    }

    fn lowered(src: &str) -> TreeView {
        let (v, d) = parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        v
    }

    #[test]
    fn trees() {
        let e = |name: &str| Entry {
            name: name.to_owned(),
            ..Entry::default()
        };
        let (t, _) = build(vec![
            (4, e("a")),
            (8, e("b")),
            (2, e("c")),
            (6, e("d")),
            (0, e("f")),
        ]);
        // A line left of the first is a sibling of what it does not nest under.
        assert_eq!(shape(&t), "a(b),c(d),f");
    }

    #[test]
    fn indentation() {
        let v = lowered(
            "treeView-beta\n  my-project/\n    src/\n      \"index.js\" :::highlight ## entry\n    \
             'b c' icon()\n    d icon(logos:react)\n",
        );
        assert_eq!(shape(&v.entries), "my-project/(src/(index.js),b c,d)");
        let src = &v.entries[0].children[0].children[0];
        assert!(src.highlighted());
        assert_eq!(src.description.as_deref(), Some("entry"));
        let kids = &v.entries[0].children;
        assert_eq!(kids[1].icon.as_deref(), Some("none"));
        assert_eq!(kids[1].shown_icon(true), None);
        assert_eq!(kids[2].shown_icon(false), Some("logos:react"));
        assert_eq!(kids[0].shown_icon(true), Some("folder"));
    }

    #[test]
    fn box_drawing() {
        let v = lowered(
            "treeView-beta\nroot/\n├── src/\n│   ├── a.ts\n│\n│   └── b.ts\n├── c\n└── d/\n    └── e\n",
        );
        assert_eq!(shape(&v.entries), "root/(src/(a.ts,b.ts),c,d/(e))");
        // Branches at column 0 alone give each depth four columns.
        let v = lowered("treeView-beta\n┣━━ a\n┃   ┗━━ b\n┗━━ c\n");
        assert_eq!(shape(&v.entries), "a(b),c");
        let (_, d) = parse("treeView-beta\n├── a\n  b\n").lower();
        assert_eq!(d.len(), 1, "{d:?}");
    }

    #[test]
    fn title_and_config() {
        let v = lowered(
            "---\ntitle: F\nconfig:\n  treeView:\n    rowIndent: 20\n    showIcons: true\n---\n\
             treeView-beta\n  a\n",
        );
        assert_eq!(v.title.as_deref(), Some("F"));
        assert_eq!(v.config.row_indent, 20.0);
        assert!(v.config.show_icons);
        let v = lowered("treeView-beta\ntitle Files\n  a\n");
        assert_eq!(v.title.as_deref(), Some("Files"));
        assert_eq!(shape(&v.entries), "a");
    }
}
