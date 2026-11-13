# Tree views

sirene draws Mermaid's tree views (`treeView-beta`) as images and as text. Entries nest by
indentation, or by box-drawing characters as `tree` prints them:

```mermaid
treeView-beta
    my-project/
        src/
            App.tsx :::highlight icon(logos:react) ## main component
            index.js ## entry point
        .env ## environment variables
        Dockerfile
        package.json
```

As an image, the rows are laid out as Mermaid lays them out, under a root `/`. See
`crates/sirene-core/tests/fixtures/treeview/valid/` for more. In the editor a tree view is
read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `name`, `"name with spaces"`, `'name'` | a row, nested under the nearest line indented less |
| `name/` | a directory, in bold |
| `├── name`, `└── name`, `│`, and the heavy `┣━━`, `┗━━`, `┃` | the same tree, nested by the branch's column |
| `:::highlight` | a shaded row |
| `icon(file)`, `icon(folder)`, `icon()` | that icon, or none |
| `## text` | a description in italics, lined up right of the labels |
| frontmatter `treeView`: `rowIndent`, `paddingX`, `paddingY`, `lineThickness`, `showIcons` | read |
| `title`, `%% comment` | read; Mermaid does not draw the title |

## Not drawn

Icons from icon packs (`icon(logos:react)`) are drawn as `?`, as Mermaid draws an icon that is not
registered. `defaultIconPack`, `filenameIcons`, `extensionIcons`, and the theme variables are not
read. In text, icons and highlights are left out.
