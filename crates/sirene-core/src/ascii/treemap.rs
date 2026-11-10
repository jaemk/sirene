//! Treemaps as text (MTREE-4): an outline of the sections and leaves, largest first, each with
//! its value and a bar for its share of the whole.

use super::sankey::bar;
use super::{Charset, Options, width};
use crate::treemap::Treemap;

/// One outline line: the guides and name, and the item's value.
struct Line {
    name: String,
    value: f64,
}

struct Outline<'a> {
    treemap: &'a Treemap,
    values: Vec<f64>,
    glyphs: [&'static str; 3],
    lines: Vec<Option<Line>>,
}

impl Outline<'_> {
    /// Item `i` after `guides` and its `branch`, then its items under it.
    fn walk(&mut self, i: usize, guides: &str, branch: &str) {
        let t = self.treemap;
        self.lines.push(Some(Line {
            name: format!("{guides}{branch}{}", t.items[i].name),
            value: self.values[i],
        }));
        let [tee, elbow, pipe] = self.glyphs;
        let below = match branch {
            "" => guides.to_owned(),
            b if b == elbow => format!("{guides}   "),
            _ => format!("{guides}{pipe}"),
        };
        let children = t.by_value(t.items[i].children.iter().copied(), &self.values);
        for (k, &c) in children.iter().enumerate() {
            self.walk(c, &below, if k + 1 == children.len() { elbow } else { tee });
        }
    }
}

/// Renders `treemap` as text (MTREE-4).
pub fn render(treemap: &Treemap, options: &Options) -> String {
    let values = treemap.values();
    let total: f64 = treemap.roots().map(|i| values[i]).sum();
    let roots = treemap.by_value(treemap.roots(), &values);
    let mut o = Outline {
        treemap,
        values,
        glyphs: match options.charset {
            Charset::Unicode => ["├─ ", "└─ ", "│  "],
            Charset::Ascii => ["+- ", "`- ", "|  "],
        },
        lines: Vec::new(),
    };
    for (k, &i) in roots.iter().enumerate() {
        if k > 0 {
            o.lines.push(None);
        }
        o.walk(i, "", "");
    }

    let shown: Vec<Option<(&str, String, f64)>> = o
        .lines
        .iter()
        .map(|l| {
            l.as_ref()
                .map(|l| (l.name.as_str(), treemap.format_value(l.value), l.value))
        })
        .collect();
    let name_w = shown
        .iter()
        .flatten()
        .map(|(n, ..)| width(n))
        .max()
        .unwrap_or(0);
    let value_w = shown
        .iter()
        .flatten()
        .map(|(_, v, _)| width(v))
        .max()
        .unwrap_or(0);
    let mut rows = Vec::new();
    for line in &shown {
        let Some((name, value, v)) = line else {
            rows.push(String::new());
            continue;
        };
        let share = if total > 0.0 { v / total } else { 0.0 };
        let row = format!(
            "{name}{}  {}{value}  {}",
            " ".repeat((name_w - width(name)).max(0) as usize),
            " ".repeat((value_w - width(value)).max(0) as usize),
            bar(share, options.charset)
        );
        rows.push(row.trim_end().to_owned());
    }

    let mut out = String::new();
    if let Some(t) = treemap
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
        out.push_str(&" ".repeat(((w - width(t)).max(0) / 2) as usize));
        out.push_str(t);
        out.push_str("\n\n");
    }
    for r in rows {
        out.push_str(&r);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treemap::Item;

    #[test]
    fn outline() {
        let item =
            |name: &str, value: Option<f64>, parent: Option<usize>, children: Vec<usize>| Item {
                name: name.into(),
                value,
                parent,
                children,
                ..Item::default()
            };
        let t = Treemap {
            items: vec![
                item("s", None, None, vec![1, 2]),
                item("a", Some(1.0), Some(0), vec![]),
                item("b", Some(3.0), Some(0), vec![]),
                item("c", Some(4.0), None, vec![]),
            ],
            ..Treemap::default()
        };
        let out = render(
            &t,
            &Options {
                charset: Charset::Ascii,
            },
        );
        assert_eq!(
            out,
            "s     4  ############\n+- b  3  #########\n`- a  1  ###\n\nc     4  ############\n"
        );
    }
}
