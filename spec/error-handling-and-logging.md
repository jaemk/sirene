# Error Handling and Logging

Error propagation and configurable stderr logging across the core library, CLI, editor, and
release tooling.

## ERRORH-1 Production panic policy

Production code does not use `panic!`, `unwrap`, `expect`, `unreachable!`, `todo!`,
`unimplemented!`, assertions, or debug assertions to handle operations or validate input.
Tests may use these. Production crate roots enforce the corresponding Clippy restrictions.
Internal invariants are encoded in control flow; invalid public graph endpoints return an
error. Selecting a node in a non-flowchart diagram cannot index the empty flowchart model.

## ERRORH-2 Causal chains

Operational failures retain the original error object as a `std::error::Error::source`, with
context identifying the operation and resource. This applies to file/stream reads and writes,
UTF-8 decoding, SVG/PNG processing, updates, editor saves/exports, and release signing. A
source-less validation error may contain only a message. Intermediate functions do not
replace an underlying error with its formatted string.

The shared `sirene_core::Error` is cloneable without losing causes. Its `Display` shows its
own message, `source()` exposes the underlying error, and `report()` formats the complete
chain. CLI and GUI reporting boundaries use that report; successful diagram output stays on
stdout. Parser syntax problems continue to use diagnostics and recovery.

## ERRORH-3 Library limits and rendering

Public parsers reject source that cannot be represented by rowan's byte offsets with a limit
diagnostic before constructing its tree. Graph construction and the generic ASCII/scene
rendering entry points return results; malformed caller-constructed graphs return errors.
PNG allocation, parsing, and encoding failures return errors with any available cause.

## ERRORH-4 Streams and directory traversal

CLI and release-tool output uses fallible writes. A failed output stream terminates cleanly
with a failure status, without panicking while reporting that failure. Batch renders and
checks continue processing accessible files, but directory-listing and entry failures and
metadata failures for candidate diagram files are reported and make the command unsuccessful.
Unrelated symlinks need not resolve. Watches report failures and continue polling for recovery.

## ERRORH-5 Cancellation and persistence

A negative confirmation or dismissed dialog is cancellation. Failed prompt output, input,
platform dialogs, and dialog receivers are errors, retain their causes, and are reported
separately. Failed editor saves and exports remain visible in the editor. Settings persistence
returns or records an observable error; missing settings on initial load remain an expected
default case. Persistent application or settings errors do not hide current document status.
A settings save failure during exit is presented in the application even when logging is
disabled, with a way to leave without saving settings.

## ERRORH-6 Logging configuration

Without verbosity flags or a nonempty logging environment variable, no additional log records
are printed. Global `-v`/`--verbose` enables debug logging; repeated verbosity enables trace.
`SIRENE_LOG` takes precedence over `RUST_LOG`. Configuration accepts comma-separated levels
(`off`, `error`, `warn`, `info`, `debug`, `trace`) and `TARGET=LEVEL` directives. Target
directives use module prefixes and the most specific matching prefix wins. Verbosity flags
override the general environment level; explicit target directives remain effective. Invalid
configuration produces an actionable startup error and exit status 2.

Verbose defaults cover sirene's own crates; dependency targets can be explicitly enabled in
the environment. Log records include elapsed time, severity, target, and operation context,
and go exclusively to stderr. Paths, counts, stages, outcomes, and timings can be logged;
diagram contents, signing-key material, credentials, and environment dumps are not logged.

## ERRORH-7 Verification

Regression tests cover causal-chain traversal/downcasting, invalid graph references, parser
size guards, non-flowchart mouse selection, failing streams, partial traversal failures,
confirmation/dialog failures, settings persistence, and logging precedence/filtering. Existing
rendering snapshots and GUI behavior remain covered by the repository test suite. The full
check includes formatting, Clippy, workspace/headless tests, and the coverage gate (WORKSP-5).

## ERRORH-8 Phase timing

Lowering, scene building, text drawing, PNG writing, and the layout stages (build, order,
ports, position, and each gap's track assignment) log their duration at trace level under the
`sirene_core::profile` target as `phase <name> <ns> ns`, e.g.
`SIRENE_LOG=off,sirene_core::profile=trace`. A phase that runs more than once logs once per
run. Nothing is measured unless the target is enabled. `sirene_core::profile::parse` reads a
record back. The `stress` bench's `--phases` option prints the summed timings of one run at
the larger size per shape. Tests: `crates/sirene-core/tests/profile.rs`,
`phase_timings_follow_their_target` in `crates/sirene/tests/cli.rs`.
