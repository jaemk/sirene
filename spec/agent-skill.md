# Agent Skill

A skill that teaches agents to render and check diagrams with the sirene CLI, offered through
[mind](https://github.com/jaemk/mind). Tests: `crates/sirene/tests/skill.rs`.

## AGENTS-1 The repo is a mind source

`mind.toml` at the root declares the repo a source with namespace `sirene`, so
`mind meld jaemk/sirene` offers each skill under `skills/` as `sirene:<name>`. It declares no
`release-tags`: the source follows the default branch. `mind review .` reports no issues.

## AGENTS-2 `sirene:render`

`skills/render/SKILL.md` describes, for an agent: checking that `sirene` is installed and asking
before installing it; `check`, `ascii`, `png`, and `svg` on files, directories, Markdown files,
and stdin (CLI-2, CLI-5, CLI-6, CLI-9, CLI-11, CLI-12); never running `sirene FILE` with no
command, which opens the editor; reading diagnostics (CLI-3); and rendering a PNG or text to look
at a diagram it wrote before handing it back. Each command in its `sh` block runs and succeeds
as written, and none changes a Markdown file.
