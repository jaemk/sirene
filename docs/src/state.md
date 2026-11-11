# State diagrams

sirene draws Mermaid state diagrams (`stateDiagram` and `stateDiagram-v2`) with the same layout,
themes, and outputs as flowcharts:

```
$ printf 'stateDiagram-v2\n  [*] --> Still\n  Still --> Moving : push\n  Moving --> [*]\n' | sirene ascii
     ●
     │
     ▼
 ╭───────╮
 │ Still │
 ╰───┬───╯
     │
   push
     ▼
╭────────╮
│ Moving │
╰────┬───╯
     │
     ▼
     ◉
```

In the editor a state diagram is read-only on the drawing side: select, zoom, export, and copy
work, and the source pane edits as usual. Drawing edits say why they are off.

## What is read

| Syntax | Drawn as |
|---|---|
| `s1`, `state "Description" as s1`, `s1 : Description` | a rounded state; more descriptions add lines |
| `s1 --> s2`, `s1 --> s2 : label` | a transition with an arrow, labeled in the middle |
| `[*]` | a start dot as a source, an end ring as a target, one of each per composite state |
| `state Name { ... }` | a composite state, titled with its name; nested freely |
| `--` inside a composite state | concurrent regions, each in a dashed box |
| `state f <<fork>>`, `<<join>>` | a bar across the direction of flow |
| `state c <<choice>>` | a small diamond |
| `note left of s : text`, `note right of s` ... `end note` | a note beside the state, joined by a dashed line |
| `direction TB\|BT\|LR\|RL` | the diagram's direction, or a composite state's |
| `classDef`, `class s1,s2 name`, `s1:::name`, `style` | colors and strokes, as in flowcharts |
| `%%` comments, frontmatter `title`, `accTitle`, `accDescr` | as in flowcharts |

A note left of a state sits above it in a left-to-right diagram, and one on the right sits below.
A note on a composite state sits beside its box.

## Not drawn

These are warnings on the line that has them, and the rest of the diagram is drawn:

- `click`
- floating notes (`note "text" as N`), which Mermaid parses but does not draw either
- transitions between two concurrent regions of one composite state
- other stereotypes, such as `<<history>>`, which draw as a plain state
