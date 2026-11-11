# Flowchart Model

Semantic flowchart model lowered from the syntax tree (`flowchart-parser.md`), with
diagnostics. Lowering never fails: statements the parser marked as errors are skipped, and
semantic problems are reported as diagnostics.

Reference semantics: mermaid `packages/mermaid/src/diagrams/flowchart/flowDb.ts` and the shape
table in `rendering-util/rendering-elements/shapes.ts`.

## FLOWMO-1 Statement order

Statements are applied in source order, as mermaid does. Where a statement refers to an entity
by id, it sees only entities created by earlier statements (FLOWMO-11, FLOWMO-12, FLOWMO-13).

## FLOWMO-2 Diagnostics

Every diagnostic has a byte range, a message, and a severity: error or warning. Parser
diagnostics are errors. Lowering reports an error where mermaid fails to render and a warning
where mermaid silently ignores or guesses.

## FLOWMO-3 Direction and title

- The header direction sets the diagram direction. `TD` and `v` mean `TB`, `>` means `LR`, `<`
  means `RL`, `^` means `BT`. With no direction the diagram is `TB`.
- `direction` outside a subgraph has no effect and is a warning.
- A `title:` key at the top level of the frontmatter sets the diagram title. Other frontmatter
  keys and `%%{init}%%` directives are ignored for now.
- `accTitle` and `accDescr` set the accessible title and description, trimmed. A later
  statement replaces an earlier one.

## FLOWMO-4 Nodes

A node is created by its first mention in a vertex statement (or by `style`, FLOWMO-12). Nodes
keep first-mention order, and each node records the range of every mention.

- A shape on any mention sets the node's shape and label; the last one wins.
- A node never given a label is labeled with its id.
- `:::name` appends a class.
- A node named `end` is skipped (the parser reports it).

## FLOWMO-5 Shapes

Each delimiter pair maps to a shape:

| Delimiters | Shape |
|---|---|
| `[ ]` | `rect` |
| `( )` | `rounded` |
| `([ ])` | `stadium` |
| `[[ ]]` | `fr-rect` |
| `[( )]` | `cyl` |
| `(( ))` | `circle` |
| `((( )))` | `dbl-circ` |
| `(- -)` | ellipse (no `@{ shape }` name) |
| `> ]` | `odd` |
| `{ }` | `diam` |
| `{{ }}` | `hex` |
| `[/ /]` | `lean-r` |
| `[\ \]` | `lean-l` |
| `[/ \]` | `trap-b` |
| `[\ /]` | `trap-t` |

`@{ shape: name }` accepts every documented mermaid shape by short name or alias (e.g. `rect`,
`proc`, `process`, `rectangle`). An unknown name, or a name with uppercase letters or `_`, is an
error and leaves the shape unchanged.

## FLOWMO-6 Labels

A label is text plus a markdown flag.

- Unquoted and quoted text is trimmed, and each line is trimmed.
- `` "`...`" `` strings are markdown; other labels are plain.
- `<br>`, `<br/>`, and `<br />` (any case) become line breaks.
- Entity codes are decoded: `#quot;`, `#amp;`, `#lt;`, `#gt;`, `#apos;`, `#nbsp;`, and decimal
  `#NNN;`, and the same as HTML entities (`&nbsp;`, `&#9829;`). Unknown codes are kept verbatim.
- `Label::spans` gives each line as styled spans. In markdown, `**bold**` and `__bold__` are bold
  and `*italic*` and `_italic_` italic; a marker without a partner on its line stays as text, and
  `_` marks only at a word edge (`snake_case` is text). `Label::lines` is the spans' text.

## FLOWMO-7 Metadata

An `@{ ... }` body is a flat YAML mapping: `key: value` entries separated by commas or newlines.
Values may be plain, double-quoted (with `\"` and `\\` escapes), or single-quoted (with `''`
escape). Every entry is kept in order as a string pair; later entries replace earlier ones with
the same key. An entry without `:` is an error.

Interpreted node keys: `shape` (FLOWMO-5) and `label` (a markdown label unless `labelType` is
`text` or `string`). Interpreted edge keys: `animate` (`true`/`false`), `animation`, and `curve`.

`<id>@{ ... }` targets, in order of precedence:

1. an edge with that user-defined id (any mention of an edge id refers to the edge and creates no
   node);
2. a subgraph already declared with that id;
3. a node.

## FLOWMO-8 Edges

A vertex statement `G1 e1 G2 e2 G3 ...` adds, for each edge `ek`, one edge from every node of
`Gk` to every node of `Gk+1`, in that order. Edges keep source order; `linkStyle` indexes
(FLOWMO-13) count in that order.

- Stroke: normal (`-`), thick (`=`), dotted (`.`), or invisible (`~`).
- Arrowheads: the start marker (`<`, `x`, `o`) and end marker (`>`, `x`, `o`) are read
  independently as point, cross, or circle. Mermaid drops a start marker that does not match the
  end marker; sirene keeps it. Invisible edges have no arrowheads.
- Length: the number of dots for dotted strokes, otherwise the stroke length minus 2 after
  removing the start marker (so `-->`, `---`, and `~~~` have length 1). For an inline label the
  closing stroke gives the length. Lengths are capped at 10.
- Label: the pipe or inline label (FLOWMO-6), absent when empty.
- An edge id `e1@` is given only to the first edge the stroke creates (from the last node of
  the source group to the first node of the target group). A repeated edge id is a warning and
  the edge gets no id.

## FLOWMO-9 Subgraphs

A subgraph's id and title come from its header:

| Header | Id | Title |
|---|---|---|
| `subgraph` | auto | empty |
| `subgraph one` | `one` | `one` |
| `subgraph id[title]` | `id` | `title` |
| `subgraph "title"` or `subgraph title text` | the title if it has no whitespace, else auto | the title |

Auto ids are `subGraph<n>`, where `n` counts every subgraph in the order their `end`s appear.
`direction` inside a subgraph sets its direction (the last one wins); otherwise it has none.
Subgraphs are listed in declaration order.

## FLOWMO-10 Subgraph membership

A subgraph's members are the nodes mentioned in its body's vertex statements plus the subgraphs
declared directly in its body, in source order, without duplicates. Subgraphs are resolved in the
order their `end`s appear:

- a member already belonging to an earlier-resolved subgraph stays there and is dropped from this
  one; when that earlier subgraph is not nested inside this one, the drop is a warning;
- a subgraph is never its own member, and a subgraph that already contains this one (at any
  depth) is dropped with a warning;
- a second subgraph with an existing id adds its members to the first and keeps the first's
  title and direction.

A node whose id equals a subgraph id is the subgraph: it is removed from the node list, and its
classes and styles move to the subgraph. Edges may connect to subgraphs.

## FLOWMO-11 Classes

- `classDef a,b decls` appends the declarations to classes `a` and `b`, creating them in order.
- `class ids name` appends `name` to each node, edge (by id), or subgraph with that id. An id
  with no such entity is a warning.
- Classes are kept by name; the `default` class is not applied by the model.

## FLOWMO-12 Styles

Style text is split into declarations at commas (`\,` is a literal comma). Each declaration is a
property and a value split at the first `:`, both trimmed. Empty declarations are skipped; one
without `:` is a warning.

`style id decls` appends to the node, subgraph, or edge (by id, a sirene extension) with that
id. An unknown id creates a node, with a warning.

## FLOWMO-13 Link styles

`linkStyle` applies to the edges listed by index, or with `default` to the diagram's default
edge style. Its declarations replace the target's styles. `interpolate <curve>` sets the curve
name. An index with no edge yet is an error.

## FLOWMO-14 Click

`click id ...` attaches to an existing node either a callback (name and arguments) or a link
(URL and optional target), plus an optional tooltip. Callback arguments are split at commas
outside double quotes, trimmed, and unquoted. A `click` on an unknown node is a warning.

## FLOWMO-15 Collapsed subgraphs

A subgraph whose metadata has `view: collapsed` (e.g. `one@{ view: collapsed }` after its `end`)
is drawn as one node in place of its contents, as Mermaid 12.3 does. The model keeps every node
and subgraph; `Flowchart::collapsed` returns the folded chart that the scene and text renderers
draw, with each element mapped back to the original:

- every member of a collapsed subgraph, at any depth, is hidden and stands for the outermost
  collapsed subgraph that contains it;
- each visible collapsed subgraph becomes a `Shape::Collapsed` node with its title (its id when
  untitled), classes, and styles, placed where its first hidden node was in node order;
- an edge to or from a hidden element goes to the collapsed node instead, and an edge that
  becomes a loop on a collapsed node only by folding is dropped;
- a collapsed node is drawn with the cluster colors: a rounded box, its title, a dashed rule,
  and three dots (in text: a rounded box with a dotted rule and `···`).
