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

## Conventions

- Each normative statement carries a stable ID (e.g. `FEAT-1`, `API-3`). IDs are
  append-only: retire an ID by marking it removed, never reuse the number.
- Specs are document-first: a feature is documented (status `pending`, or
  `research` if it needs design work) before implementation begins. Flip to
  `done` only once implemented and verified.
- Spec files are named `<slug>.md` and linked from the table above.
