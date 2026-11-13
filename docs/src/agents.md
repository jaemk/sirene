# Agents

The sirene repo is a [mind](https://github.com/jaemk/mind) source. It offers one skill,
`sirene:render`, that teaches coding agents to check and render Mermaid, PlantUML, and DOT
diagrams with the [command line](cli.md): checking a diagram for errors, printing it as text,
writing PNG or SVG, and rendering the diagram blocks of a Markdown file.

```
mind meld jaemk/sirene
```

The skill needs `sirene` on the PATH (see [Install](install.md)); the agent asks before
installing it. It never opens the editor, and never changes a Markdown file unless asked to add
image links. Its text is at
[skills/render/SKILL.md](https://github.com/jaemk/sirene/blob/main/skills/render/SKILL.md).
