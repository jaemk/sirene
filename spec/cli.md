# CLI

The `sirene` binary.

## CLI-1 No subcommand

`sirene [FILE...]` with no subcommand opens each FILE in a viewer window, or one empty window
when no FILE is given (`gui-shell.md`). It exits 0 when the last window closes. A FILE named like
a subcommand is given with a path prefix, e.g. `./ascii`.

## CLI-2 `sirene ascii`

`sirene ascii [FILE]` reads a flowchart from FILE, or from stdin when FILE is omitted or `-`,
and prints it to stdout with the ASCII renderer (`ascii-renderer.md`).

`--charset unicode|ascii` picks the character set; the default is `unicode`.

## CLI-3 Diagnostics

Diagnostics are printed to stderr as `NAME:LINE:COL: error: MESSAGE` (or `warning:`), where NAME
is the file path or `<stdin>`, and LINE and COL count from 1 (COL in characters). The diagram is
printed even when there are errors. The exit status is 1 when there is an error and 0 otherwise.

## CLI-4 Read failures

When FILE cannot be read, sirene prints `error: cannot read FILE: REASON` to stderr and exits 1.

## CLI-5 `sirene svg`

`sirene svg [FILE]` reads a flowchart as `sirene ascii` does and prints it to stdout as a
standalone SVG document (`display-list.md`), with text measured by the monospace estimate.
`--theme light|dark` picks the colors; the default is `light`. Diagnostics and exit status
follow CLI-3 and CLI-4.
