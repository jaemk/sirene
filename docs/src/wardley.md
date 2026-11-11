# Wardley maps

sirene draws Mermaid's Wardley maps (`wardley-beta`) as images and as text. Components are
placed by visibility (up) and evolution (across):

```
$ sirene ascii pipeline.mmd
                         Storage Options

Visibility
│                ┆               ┆              ┆
│                ┆               ┆           ◆ App
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆     ■ Database
│                ● File System   ● SQL DB    ● NoSQL   ● Cloud DB
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
│                ┆               ┆              ┆
└─────────────────────────────────────────────────────────────────
     Genesis       Custom Built      Product       Commodity
                            Evolution

Links:
  App -- Database

Pipelines:
  Database: File System, SQL DB, NoSQL, Cloud DB
```

```
wardley-beta
title Storage Options
anchor App [0.95, 0.70]
component Database [0.40, 0.60]
App -> Database

pipeline Database {
  component "File System" [0.25]
  component "SQL DB" [0.50]
  component "NoSQL" [0.70]
  component "Cloud DB" [0.85]
}
```

See `crates/sirene-core/tests/fixtures/wardley/valid/` for more. In the editor a Wardley map is
read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `anchor Name [visibility, evolution]` | bold text |
| `component Name [visibility, evolution]` | a circle with its name |
| `label [x, y]` after a component | its name moved by that offset |
| `(build)`, `(buy)`, `(outsource)`, `(market)` | a shaded ring, or three dots for market |
| `(inertia)` | a bar right of the component |
| `A -> B`, `A -.-> B`, `A +> B`, `A +<> B`, `A +'text'> B`, `; label` | links, dashed, with arrowheads, labeled |
| `evolve Name 0.8` | a red arrow to the new evolution |
| `pipeline Name { component X [evolution] }` | a box of components under its parent |
| `evolution A -> B@0.4 -> C / D` | stage names, at positions when given |
| `note`, `annotation`, `annotations`, `accelerator`, `deaccelerator` | text, numbered circles with a list, arrows |
| `title`, `size [w, h]`, `%% comment` | read; the title is drawn above |

Coordinates from 0 to 1 are fractions, and above 1 percentages. A link or `evolve` naming an
unknown component is left out with a warning.

## Not drawn

Labels on links and the `Visibility` axis label stay level instead of turning. The hand-drawn
look and the `wardley` config are not read.
