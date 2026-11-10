# Quadrant charts

sirene draws Mermaid quadrant charts (`quadrantChart`) as images and as text:

```
$ sirene ascii campaigns.mmd
                     Reach and engagement of campaigns

                ┌────────────────────┬────────────────────┐
                │  Need to promote   │  We should expand  │
                │                    │                    │
High Engagement │   Campaign F ●     │                    │
                │                    │  ● Campaign C      │
                │ Campaign A ●       │                    │
                ├────────────────────┼────────────────────┤
                │    Re-evaluate     │  May be improved   │
                │     Campaign E ●   │          1         │
 Low Engagement │       Campaign B ● │                    │
                │                    │                    │
                │                    │                    │
                └────────────────────┴────────────────────┘
                       Low Reach           High Reach

                1 Campaign D
```

```
quadrantChart
    title Reach and engagement of campaigns
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
    Campaign C: [0.57, 0.69]
    Campaign D: [0.78, 0.34]
    Campaign E: [0.40, 0.34]
    Campaign F: [0.35, 0.78]
```

In the editor a quadrant chart is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `title text` | the title above; frontmatter `title` too |
| `x-axis Low --> High` | labels under the left and right halves; `x-axis Low` alone is centered |
| `y-axis Low --> High` | labels beside the bottom and top halves; `y-axis Low` alone is centered |
| `quadrant-1 text` to `quadrant-4 text` | labels at the top of the top right, top left, bottom left, and bottom right quadrants |
| `Name: [x, y]` | a point, with `x` and `y` from 0 to 1 |
| `"Name": [x, y]` | a quoted name, which may hold `:` |
| `Name: [x, y] radius: 10, color: #f00` | a point's own style |
| `Name:::cls: [x, y]` and `classDef cls color: #f00` | a style from a class; the point's own styles win |
| `accTitle`, `accDescr` | kept in the model |

A point's styles are `radius`, `color`, `stroke-color`, and `stroke-width`. A stroke is drawn only
with `stroke-width`. With no points, quadrant labels sit in the middle of their quadrants.

In text, a point is `●` with its name beside it. A name that would cross another name, a point,
or a middle line is replaced by a key (`1`, `2`, ...) listed under the chart.

## Errors

A point without a complete `[x, y]`, or with a coordinate that is not a number, is an error, and
the point is skipped. A coordinate outside 0 to 1 is a warning and is drawn at the nearest edge.
An unknown style, a bad style value, and a class with no `classDef` are warnings and ignored.

## Not drawn

Frontmatter `config` for `quadrantChart` (sizes, paddings, font sizes, axis positions) and theme
variables are ignored. The x-axis labels are always below the chart and the y-axis labels always
left of it.
