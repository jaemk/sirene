# Themes

A diagram can pick one of Mermaid's themes, and override its colors, in its frontmatter or in an
`%%{init}%%` directive. The diagram's theme replaces `--theme` and the editor's light or dark
setting when drawing it as an image.

```mermaid
%%{init: {'theme': 'forest'}}%%
flowchart LR
  A[Start] --> B{Ready?}
  B -->|yes| C[Ship]
```

The same in frontmatter, with `themeVariables`:

```mermaid
---
config:
  theme: base
  themeVariables:
    primaryColor: '#BB2528'
    lineColor: '#F8B229'
    fontSize: 18px
---
flowchart LR
  A --> B
```

Themes: `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`,
`redux-dark`, `redux-color`, and `redux-dark-color`. An unknown name is a warning.

`themeVariables` work as in Mermaid: variables such as `primaryColor`, `primaryBorderColor`,
`primaryTextColor`, `lineColor`, `background`, `mainBkg`, `nodeBorder`, `clusterBkg`, and
`edgeLabelBackground` feed the colors the theme derives. With no `theme`, the variables apply to
the application's theme.

A theme also colors pie slices (`pie1` to `pie12`) and the sections of mindmaps, timelines,
radar charts, and treemaps (`cScale0` to `cScale11`); set one of those variables to change a
single color.

Note that text output is not themed, and other charts (git graphs, gantt charts, journeys, sankey
diagrams, XY charts, Venn diagrams) keep sirene's own colors.
