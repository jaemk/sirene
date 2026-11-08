# Settings

The editor keeps its settings in a `settings` file of `key = value` lines:

| Platform | Path |
|---|---|
| Linux and others | `$XDG_CONFIG_HOME/sirene/settings`, else `~/.config/sirene/settings` |
| macOS | `~/Library/Application Support/sirene/settings` |
| Windows | `%APPDATA%\sirene\settings` |

`SIRENE_CONFIG_DIR` overrides the directory.

| Key | Values | Set by |
|---|---|---|
| `appearance` | `system`, `light`, `dark` | View > Appearance |
| `window` | `WxH`, e.g. `1200x760` | the size of the last window closed |
| `source` | `shown`, `hidden` | showing or hiding the source pane |
| `source_width` | pixels | dragging the divider |
| `recent` | a path, up to 10 lines, most recent first | opening and saving files |

The file is written by the editor; editing it by hand works, but unknown keys and bad values are
skipped, and a missing or unreadable file gives the defaults. New windows open at the saved size
(1200x760 at first), cascading from the active window, with the source pane as it last was.
