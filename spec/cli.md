# CLI

The `sirene` binary.

## CLI-1 No subcommand

`sirene` with no subcommand exits 0. (It will open the GUI.)

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
