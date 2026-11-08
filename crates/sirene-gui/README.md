# sirene-gui

The [sirene](https://github.com/jaemk/sirene) editor window, built on gpui (GPUI Kit): a Mermaid
flowchart's source beside a live drawing, where edits on the drawing are written back to the
source.

```rust,no_run
sirene_gui::run(vec!["diagram.mmd".into()]);
```

`run` opens one window per path (or a new document), and returns when the last window closes.
Most users want the `sirene` binary instead (`cargo install sirene`). The window's behavior is
specified in the repository's `spec/gui-shell.md` and `spec/wysiwyg-editing.md`.
