//! Replays a git graph's statements into commits and branches (MGIT-2, MGIT-3).

use std::collections::HashMap;

use rowan::TextRange;

use super::{Branch, Commit, CommitType, Direction, GitGraph};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (GitGraph, Vec<Diagnostic>) {
    let mut l = Lowerer::new();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(t) = el.as_token()
        {
            l.graph.title = text::frontmatter_title(t.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            HEADER => {
                l.graph.direction = match token(&stmt, DIR).as_ref().map(SyntaxToken::text) {
                    Some("TB") => Direction::TopBottom,
                    Some("BT") => Direction::BottomTop,
                    _ => Direction::LeftRight,
                }
            }
            GIT_STMT => l.statement(&stmt),
            ACC_TITLE_STMT => l.graph.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => l.graph.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    l.finish()
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

/// A name or value token's text, unquoted.
fn value(t: &SyntaxToken) -> String {
    match t.kind() {
        STRING => text::unquote(t.text()).to_owned(),
        _ => t.text().to_owned(),
    }
}

/// The attributes each statement takes.
fn allowed(kw: &str) -> &'static [&'static str] {
    match kw {
        "commit" => &["id", "type", "tag", "msg"],
        "branch" => &["order"],
        "merge" => &["id", "type", "tag"],
        "cherry-pick" => &["id", "parent", "tag"],
        _ => &[],
    }
}

/// A short, stable hash for generated ids (FNV-1a).
fn hash(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

struct State {
    name: String,
    /// `order:`, or `0.k` for the k-th branch declared (MGIT-2).
    order: f64,
    head: Option<usize>,
}

/// A statement's attributes.
#[derive(Default)]
struct Attrs {
    id: Option<(String, TextRange)>,
    kind: CommitType,
    tags: Vec<String>,
    message: Option<String>,
    order: Option<f64>,
    parent: Option<(String, TextRange)>,
}

struct Lowerer {
    graph: GitGraph,
    diagnostics: Vec<Diagnostic>,
    branches: Vec<State>,
    current: usize,
    ids: HashMap<String, usize>,
}

impl Lowerer {
    fn new() -> Self {
        Lowerer {
            graph: GitGraph::default(),
            diagnostics: Vec::new(),
            branches: vec![State {
                name: "main".to_owned(),
                order: 0.0,
                head: None,
            }],
            current: 0,
            ids: HashMap::new(),
        }
    }

    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(range, message));
    }

    fn branch_index(&self, name: &str) -> Option<usize> {
        self.branches.iter().position(|b| b.name == name)
    }

    fn attrs(&mut self, stmt: &SyntaxNode, kw: &str) -> Attrs {
        let mut out = Attrs::default();
        for a in stmt.children().filter(|n| n.kind() == ATTR) {
            let Some(v) = tokens(&a).find(|t| matches!(t.kind(), STRING | IDENT | NUMBER)) else {
                continue;
            };
            let range = a.text_range();
            // A commit's bare `"..."` is its message.
            let key = token(&a, ATTR_KEY);
            let key = key.as_ref().map_or("msg", |k| k.text());
            if !allowed(kw).contains(&key) {
                self.error(range, format!("`{key}` does not apply to `{kw}`"));
                continue;
            }
            let text = value(&v);
            match key {
                "id" => out.id = Some((text, v.text_range())),
                "parent" => out.parent = Some((text, v.text_range())),
                "tag" => out.tags.push(text),
                "msg" => out.message = Some(text),
                "type" => match text.as_str() {
                    "NORMAL" => out.kind = CommitType::Normal,
                    "REVERSE" => out.kind = CommitType::Reverse,
                    "HIGHLIGHT" => out.kind = CommitType::Highlight,
                    _ => self.error(
                        v.text_range(),
                        "expected `NORMAL`, `REVERSE`, or `HIGHLIGHT`",
                    ),
                },
                _ => match text.parse::<f64>() {
                    Ok(n) if v.kind() == NUMBER => out.order = Some(n),
                    _ => self.error(v.text_range(), "`order` takes a whole number"),
                },
            }
        }
        out
    }

    fn statement(&mut self, stmt: &SyntaxNode) {
        let Some(kw) = token(stmt, GIT_KW) else {
            return;
        };
        let kw = kw.text().to_owned();
        // A statement with a syntax error, anywhere in it, is skipped; an attribute without a
        // value is one at the end of a line.
        let incomplete = |n: &SyntaxNode| {
            n.kind() == ATTR && !tokens(n).any(|t| matches!(t.kind(), STRING | IDENT | NUMBER))
        };
        if stmt
            .descendants()
            .any(|n| n.kind() == ERROR || incomplete(&n))
        {
            return;
        }
        let name = tokens(stmt)
            .find(|t| matches!(t.kind(), IDENT | STRING))
            .map(|t| (value(&t), t.text_range()));
        let attrs = self.attrs(stmt, &kw);
        let range = stmt.text_range();
        match (kw.as_str(), name) {
            ("commit", _) => self.commit(attrs, Vec::new(), None, range),
            ("branch", Some((name, at))) => self.branch(name, at, attrs.order),
            ("checkout" | "switch", Some((name, at))) => match self.branch_index(&name) {
                Some(i) => self.current = i,
                None => self.error(at, format!("unknown branch `{name}`")),
            },
            ("merge", Some((name, at))) => self.merge(name, at, attrs, range),
            ("cherry-pick", _) => self.cherry_pick(attrs, range),
            _ => {}
        }
    }

    /// Adds a commit on the current branch after its head, plus `extra` parents.
    fn commit(
        &mut self,
        attrs: Attrs,
        extra: Vec<usize>,
        cherry_pick: Option<String>,
        range: TextRange,
    ) {
        let index = self.graph.commits.len();
        let id = match attrs.id {
            Some((id, at)) => {
                if self.ids.contains_key(&id) {
                    self.error(at, format!("commit id `{id}` is already used"));
                    return;
                }
                id
            }
            None => {
                let branch = &self.branches[self.current].name;
                format!(
                    "{index}-{:07x}",
                    hash(&format!("{index}:{branch}")) & 0x0fff_ffff
                )
            }
        };
        let mut parents: Vec<usize> = self.branches[self.current].head.into_iter().collect();
        parents.extend(extra);
        self.ids.insert(id.clone(), index);
        self.graph.commits.push(Commit {
            id,
            kind: attrs.kind,
            branch: self.current,
            parents,
            tags: attrs.tags,
            message: attrs.message,
            cherry_pick,
            range,
        });
        self.branches[self.current].head = Some(index);
    }

    fn branch(&mut self, name: String, at: TextRange, order: Option<f64>) {
        if self.branch_index(&name).is_some() {
            self.error(at, format!("branch `{name}` already exists"));
            return;
        }
        let k = self.branches.len();
        let order = order.unwrap_or_else(|| format!("0.{k}").parse().unwrap_or(0.0));
        let head = self.branches[self.current].head;
        self.branches.push(State { name, order, head });
        self.current = k;
    }

    fn merge(&mut self, name: String, at: TextRange, attrs: Attrs, range: TextRange) {
        let Some(other) = self.branch_index(&name) else {
            self.error(at, format!("unknown branch `{name}`"));
            return;
        };
        let current = &self.branches[self.current];
        let problem = if other == self.current {
            Some(format!("cannot merge `{name}` into itself"))
        } else if current.head.is_none() {
            Some(format!(
                "cannot merge into `{}`, which has no commits",
                current.name
            ))
        } else if self.branches[other].head.is_none() {
            Some(format!("cannot merge `{name}`, which has no commits"))
        } else if self.branches[other].head == current.head {
            Some(format!(
                "`{name}` and `{}` have the same head",
                current.name
            ))
        } else {
            None
        };
        if let Some(message) = problem {
            self.error(at, message);
            return;
        }
        let head = self.branches[other].head.expect("checked above");
        self.commit(attrs, vec![head], None, range);
    }

    fn cherry_pick(&mut self, mut attrs: Attrs, range: TextRange) {
        let Some((id, at)) = attrs.id.take() else {
            self.error(range, "`cherry-pick` needs `id: \"...\"`");
            return;
        };
        let Some(&source) = self.ids.get(&id) else {
            self.error(at, format!("unknown commit `{id}`"));
            return;
        };
        let commit = &self.graph.commits[source];
        if commit.branch == self.current {
            self.error(at, format!("commit `{id}` is already on this branch"));
            return;
        }
        if self.branches[self.current].head.is_none() {
            let name = self.branches[self.current].name.clone();
            self.error(
                range,
                format!("cannot cherry-pick onto `{name}`, which has no commits"),
            );
            return;
        }
        let mut tag = format!("cherry-pick:{id}");
        if commit.is_merge() {
            let parents: Vec<&str> = commit
                .parents
                .iter()
                .map(|&p| self.graph.commits[p].id.as_str())
                .collect();
            match &attrs.parent {
                None => {
                    self.error(
                        at,
                        format!("`{id}` is a merge: name one of its parents with `parent:`"),
                    );
                    return;
                }
                Some((p, pat)) if !parents.contains(&p.as_str()) => {
                    let message = format!("`{p}` is not a parent of `{id}`");
                    let pat = *pat;
                    self.error(pat, message);
                    return;
                }
                Some((p, _)) => tag = format!("{tag}|parent:{p}"),
            }
        }
        if attrs.tags.is_empty() {
            attrs.tags.push(tag);
        }
        self.commit(attrs, Vec::new(), Some(id), range);
    }

    fn finish(mut self) -> (GitGraph, Vec<Diagnostic>) {
        // Sort by order, ties in declaration order (MGIT-2).
        let mut order: Vec<usize> = (0..self.branches.len()).collect();
        order.sort_by(|&a, &b| self.branches[a].order.total_cmp(&self.branches[b].order));
        let mut position = vec![0; order.len()];
        for (pos, &i) in order.iter().enumerate() {
            position[i] = pos;
        }
        for c in &mut self.graph.commits {
            c.branch = position[c.branch];
        }
        self.graph.branches = order
            .iter()
            .map(|&i| Branch {
                name: self.branches[i].name.clone(),
            })
            .collect();
        (self.graph, self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable() {
        assert_eq!(hash(""), 0x811c_9dc5);
        assert_ne!(hash("0:main"), hash("1:main"));
    }
}
