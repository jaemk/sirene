# Mermaid Git Graphs

`gitGraph`, lowered to its own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/gitgraph.html>. Code: `crates/sirene-core/src/git/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/git.rs` and the
corpus under `crates/sirene-core/tests/fixtures/git/` (LANGS-6).

## MGIT-1 Syntax

- Header: `gitGraph`, optionally followed by `LR`, `TB`, or `BT` and a colon (`gitGraph TB:`);
  `gitGraph:` alone is `LR`.
- `commit` with optional attributes in any order: `id: "ID"`, `type: NORMAL|REVERSE|HIGHLIGHT`,
  `tag: "TAG"`, and `msg: "TEXT"` (read, not drawn).
- `branch NAME` with an optional `order: N`; a name that is a keyword is quoted
  (`branch "cherry-pick"`).
- `checkout NAME` and `switch NAME`.
- `merge NAME` with optional `id`, `tag`, and `type`.
- `cherry-pick id: "ID"` with optional `parent: "ID"` and `tag`.
- Comments, directives, frontmatter `title`, `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests:
`snapshots`, `syntax_errors`.

## MGIT-2 Model

`git::GitGraph`: the direction, the branches in drawing order, and the commits in source order.
A graph starts on branch `main` with no commits. `commit` adds a commit whose parent is the current
branch's head; `branch` starts a branch at the current head and makes it current; `checkout`
makes a branch current; `merge` adds a merge commit with the current head and the merged branch's
head as parents; `cherry-pick` adds a commit on the current branch tagged `cherry-pick:ID` (with
`|parent:ID` when one is given).

A commit without an `id` gets `N-XXXXXXX`: its index in source order and seven hex digits derived
from that index and its branch, so the same source always draws the same ids.

Branches are ordered as Mermaid orders them: `main` has order 0, a branch with `order: N` has N,
and the k-th branch declared without one has `0.k` read as a decimal (so the tenth comes before
the second); ties keep declaration order. Tests: `commits_and_branches`, `merges_and_cherry_picks`,
`branch_order`.

## MGIT-3 Errors

Each of these is an error on its statement, which is then skipped; the rest is drawn:

- `branch` of a name that exists;
- `checkout` or `merge` of an unknown branch;
- `merge` of the current branch, of a branch with no commits, into a branch with no commits, or
  of a branch whose head is already the current head;
- `cherry-pick` of an unknown id, of a commit on the current branch, onto a branch with no
  commits, of a merge commit without `parent`, or with a `parent` that is not one of its parents;
- a commit `id` used twice.

Tests: `errors`.

## MGIT-4 Drawing

Each branch is a lane, and commits are placed along it in source order: left to right for `LR`,
top to bottom for `TB`, bottom to top for `BT`. A lane is a thin dashed line in its branch's color
with the branch name in a filled label at its start. Branch colors cycle through eight. A commit
is a circle in its branch's color; `REVERSE` adds a cross, `HIGHLIGHT` is a filled square, a merge
is a ringed circle, and a cherry-pick has a small mark inside. Each commit's id is drawn beside it
(below it for `LR`, to its left for `TB` and `BT`) and tags in labels on the other side. A
parent on another lane is joined by a line that leaves the parent along its cross axis and turns
toward the child at the child's position, with a rounded corner; it takes the child's branch color
for a branch's first commit and the parent's for a merge. Spacing along a lane grows to keep ids
from overlapping. Every element is owned by the diagram (`scene::Owner::Diagram`). Tests:
`drawing`.

## MGIT-5 ASCII

Text draws every direction as rows, one commit per row (`BT` from the bottom up), with a column
two cells apart per lane, in the style of `git log --graph`. The branch names head their lanes,
one per row above the first commit, each preceded by `│` for the lanes to its left. A commit is
`●` (`*` in ASCII), `⊗` (`x`) for `REVERSE`, `■` (`#`) for `HIGHLIGHT`, and `◉` (`@`) for a merge.
A parent on another lane is joined by its lane's line down to the child's row, then across to the
child. The id and any tags (`[tag]`) follow the lanes on the commit's row. Tests: `snapshots`,
`ascii_charset_snapshots`.

## MGIT-6 Not drawn

Frontmatter `config` for `gitGraph` (`showBranches`, `showCommitLabel`, `mainBranchName`,
`mainBranchOrder`, `parallelCommits`, `rotateCommitLabel`) is ignored; the defaults are drawn,
except that ids are never rotated.
