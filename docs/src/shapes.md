# Shapes

A node's shape comes from the delimiters around its label, or from `@{ shape: NAME }`, which
takes every Mermaid shape name and alias:

```
flowchart LR
    A[Process] --> B{Decision} --> C[(Database)]
    D@{ shape: doc, label: "Report" }
    E[Queue]@{ shape: das }
```

When a node has both, `@{ shape }` wins, and the bracket text stays its label.

| Name | Delimiters | Aliases |
|---|---|---|
| `rect` | `A[text]` | `proc`, `process`, `rectangle` |
| `rounded` | `A(text)` | `event` |
| `stadium` | `A([text])` | `terminal`, `pill` |
| `fr-rect` | `A[[text]]` | `subprocess`, `subproc`, `framed-rectangle`, `subroutine` |
| `cyl` | `A[(text)]` | `db`, `database`, `cylinder` |
| `circle` | `A((text))` | `circ` |
| `dbl-circ` | `A(((text)))` | `double-circle` |
| (ellipse) | `A(-text-)` | |
| `odd` | `A>text]` | |
| `diam` | `A{text}` | `decision`, `diamond`, `question` |
| `hex` | `A{{text}}` | `hexagon`, `prepare` |
| `lean-r` | `A[/text/]` | `lean-right`, `in-out` |
| `lean-l` | `A[\text\]` | `lean-left`, `out-in` |
| `trap-b` | `A[/text\]` | `priority`, `trapezoid-bottom`, `trapezoid` |
| `trap-t` | `A[\text/]` | `manual`, `trapezoid-top`, `inv-trapezoid` |
| `datastore` | | `data-store` |
| `folder` | | `directory` |
| `bucket` | | |
| `console` | | |
| `browser` | | |
| `person` | | |
| `bang` | | |
| `cloud` | | |
| `text` | | |
| `notch-rect` | | `card`, `notched-rectangle` |
| `lin-rect` | | `lined-rectangle`, `lined-process`, `lin-proc`, `shaded-process` |
| `sm-circ` | | `start`, `small-circle` |
| `fr-circ` | | `stop`, `framed-circle` |
| `fork` | | `join` |
| `hourglass` | | `collate` |
| `brace` | | `comment`, `brace-l` |
| `brace-r` | | |
| `braces` | | |
| `bolt` | | `com-link`, `lightning-bolt` |
| `doc` | | `document` |
| `delay` | | `half-rounded-rectangle` |
| `h-cyl` | | `das`, `horizontal-cylinder` |
| `lin-cyl` | | `disk`, `lined-cylinder` |
| `curv-trap` | | `curved-trapezoid`, `display` |
| `div-rect` | | `div-proc`, `divided-rectangle`, `divided-process` |
| `tri` | | `extract`, `triangle` |
| `win-pane` | | `internal-storage`, `window-pane` |
| `f-circ` | | `junction`, `filled-circle` |
| `notch-pent` | | `loop-limit`, `notched-pentagon` |
| `flip-tri` | | `manual-file`, `flipped-triangle` |
| `sl-rect` | | `manual-input`, `sloped-rectangle` |
| `docs` | | `documents`, `st-doc`, `stacked-document` |
| `st-rect` | | `procs`, `processes`, `stacked-rectangle` |
| `bow-rect` | | `stored-data`, `bow-tie-rectangle` |
| `cross-circ` | | `summary`, `crossed-circle` |
| `tag-doc` | | `tagged-document` |
| `tag-rect` | | `tagged-rectangle`, `tag-proc`, `tagged-process` |
| `flag` | | `paper-tape` |
| `lin-doc` | | `lined-document` |

The classic shapes, circles, the ellipse, triangles, the fork, and the notched and sloped
rectangles have their own outlines; the other shapes are drawn as rectangles for now. Small,
filled, and framed circles and forks show no label. An unknown name, or one with uppercase
letters or `_`, is an error and leaves the shape unchanged.

The [shapes example](examples.md#shapes) draws the shapes that have delimiters.
