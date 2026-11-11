//! Tree views as text (MTREEV-5): the tree as `tree` prints it, with descriptions lined up to
//! the right.

use super::{Charset, Options, width};
use crate::treeview::{Entry, TreeView};

/// Renders `view` as text (MTREEV-5).
pub fn render(view: &TreeView, options: &Options) -> String {
    let (tee, last, bar, gap) = match options.charset {
        Charset::Unicode => ("├── ", "└── ", "│   ", "    "),
        Charset::Ascii => ("|-- ", "`-- ", "|   ", "    "),
    };
    let mut rows: Vec<(String, Option<&str>)> = vec![("/".to_owned(), None)];
    fn walk<'a>(
        entries: &'a [Entry],
        prefix: &str,
        marks: (&str, &str, &str, &str),
        rows: &mut Vec<(String, Option<&'a str>)>,
    ) {
        let (tee, last, bar, gap) = marks;
        for (i, e) in entries.iter().enumerate() {
            let end = i + 1 == entries.len();
            let slash = if e.directory { "/" } else { "" };
            rows.push((
                format!("{prefix}{}{}{slash}", if end { last } else { tee }, e.name),
                e.description.as_deref(),
            ));
            let deeper = format!("{prefix}{}", if end { gap } else { bar });
            walk(&e.children, &deeper, marks, rows);
        }
    }
    walk(&view.entries, "", (tee, last, bar, gap), &mut rows);
    let col = rows.iter().map(|(r, _)| width(r)).max().unwrap_or(0) + 2;
    let mut out = String::new();
    for (row, desc) in rows {
        out.push_str(&row);
        if let Some(d) = desc {
            out.push_str(&" ".repeat((col - width(&row)).max(0) as usize));
            out.push_str(d);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree() {
        let (view, d) = crate::treeview::parse(
            "treeView-beta\n  src/\n    a.ts ## entry\n    b/\n      c\n  README.md\n",
        )
        .lower();
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            render(&view, &Options::default()),
            "/\n├── src/\n│   ├── a.ts   entry\n│   └── b/\n│       └── c\n└── README.md\n"
        );
        let ascii = Options {
            charset: Charset::Ascii,
        };
        assert!(render(&view, &ascii).ends_with("|       `-- c\n`-- README.md\n"));
    }
}
