#!/bin/bash

# Publish every workspace crate to crates.io, in dependency order:
# 1. sirene-core
# 2. sirene-gui
# 3. sirene
#
# A crate whose current version is already on the index is a benign skip: it did
# not change since the last release, and a re-run must stay idempotent. Any
# other failure is fatal and fails the release, so a dependent is never reported
# as released on top of a crate that failed to publish.

set -uo pipefail

# Cargo's pre-flight check against the index produces the first message;
# crates.io rejects the upload itself with the second, which is what a re-run
# hits when an earlier attempt uploaded a crate before the index caught up.
ALREADY_PUBLISHED_RE="already exists on crates.io index|is already uploaded"

published=0
skipped=0
failed=0

publish_crate() {
    local dir=$1
    local out status
    echo "Publishing crate in directory: $dir..."
    # --no-verify: a verification build here would compile every build script
    # and proc macro in the tree while CARGO_REGISTRY_TOKEN is in the
    # environment, so any one of them could read it. The release workflow's
    # build job already ran `cargo package --workspace`, which verifies.
    out=$( (cd "$dir" && cargo publish --locked --no-verify) 2>&1 )
    status=$?

    if [ $status -eq 0 ]; then
        echo "$out"
        echo "Published crate in $dir"
        published=$((published + 1))
        return 0
    fi

    if grep -qE -- "$ALREADY_PUBLISHED_RE" <<<"$out"; then
        echo "Version in $dir is already on the index - nothing to publish."
        skipped=$((skipped + 1))
        return 0
    fi

    echo "$out" >&2
    echo "Failed to publish crate in $dir" >&2
    failed=$((failed + 1))
    return 1
}

# Keep going after a failure so the log shows every problem in one run.
publish_crate "crates/sirene-core" || true
publish_crate "crates/sirene-gui" || true
publish_crate "crates/sirene" || true

echo "Publish summary: $published published, $skipped already current, $failed failed."

if [ $failed -gt 0 ]; then
    echo "Release failed: $failed crate(s) could not be published." >&2
    exit 1
fi

exit 0
