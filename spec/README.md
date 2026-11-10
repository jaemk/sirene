# Spec

Every feature is documented here before or as it lands, with its status.

## Feature status

Status values: `done` (implemented and covered by tests), `pending` (documented,
not yet built; the default), `research` (needs investigation or design before it
can be built). Keep each row's status current with `spec.py set`.

| Feature | Status | Spec |
|---------|--------|------|
| Workspace | done | [workspace.md](workspace.md) |
| Flowchart Parser | done | [flowchart-parser.md](flowchart-parser.md) |
| Layout | done | [layout.md](layout.md) |
| ASCII Renderer | done | [ascii-renderer.md](ascii-renderer.md) |
| CLI | done | [cli.md](cli.md) |
| Display List | done | [display-list.md](display-list.md) |
| GUI Shell | done | [gui-shell.md](gui-shell.md) |
| WYSIWYG Editing | done | [wysiwyg-editing.md](wysiwyg-editing.md) |
| macOS Packaging | research | [macos-packaging.md](macos-packaging.md) |
| Flowchart Model | done | [flowchart-model.md](flowchart-model.md) |
| Markdown | done | [markdown.md](markdown.md) |
| Agent Skill | done | [agent-skill.md](agent-skill.md) |
| Self Update | done | [self-update.md](self-update.md) |
| Diagram Languages | pending | [diagram-languages.md](diagram-languages.md) |
| Mermaid State Diagrams | done | [mermaid-state-diagrams.md](mermaid-state-diagrams.md) |
| Mermaid Class Diagrams | done | [mermaid-class-diagrams.md](mermaid-class-diagrams.md) |
| Mermaid ER Diagrams | done | [mermaid-er-diagrams.md](mermaid-er-diagrams.md) |
| Mermaid Sequence Diagrams | done | [mermaid-sequence-diagrams.md](mermaid-sequence-diagrams.md) |
| Mermaid Charts | research | [mermaid-charts.md](mermaid-charts.md) |
| Graphviz DOT | pending | [graphviz-dot.md](graphviz-dot.md) |
| D2 | research | [d2.md](d2.md) |
| PlantUML | research | [plantuml.md](plantuml.md) |
| Mermaid Git Graphs | done | [mermaid-git-graphs.md](mermaid-git-graphs.md) |
| Mermaid Timelines | done | [mermaid-timelines.md](mermaid-timelines.md) |
| Mermaid Pie Charts | done | [mermaid-pie-charts.md](mermaid-pie-charts.md) |
| Mermaid Gantt Charts | done | [mermaid-gantt-charts.md](mermaid-gantt-charts.md) |

## Conventions

- Each normative statement carries a stable ID (e.g. `FEAT-1`, `API-3`). IDs are
  append-only: retire an ID by marking it removed, never reuse the number.
- Specs are document-first: a feature is documented (status `pending`, or
  `research` if it needs design work) before implementation begins. Flip to
  `done` only once implemented and verified.
- Spec files are named `<slug>.md` and linked from the table above.
