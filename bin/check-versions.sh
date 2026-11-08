#!/bin/bash

# Assert the workspace crate versions agree, before anything is published.
#
# The release workflow decides whether to publish from one signal: the root
# crate's version is not on the crates.io index yet. That signal says nothing
# about the other crates, so a half-bumped workspace would pass it:
#
#   1. A crate is bumped but a dependent's pin on it is not. `cargo publish`
#      resolves the stale pin against the index, where the old version exists,
#      and ships the dependent against the old crate.
#   2. A pin is bumped but the crate's own version is not. The crate's publish is
#      skipped as already published and the dependent then requires a version
#      that does not exist.
#
# Checks every dependency between publishable workspace members: the requirement
# must be exactly `^VERSION` of that member, and a stable crate must not depend
# on a pre-release one.
#
# Reads `cargo metadata` from the current directory. A metadata JSON file may be
# passed as $1 instead, which is how the tests drive the failure cases.

set -euo pipefail

ROOT_CRATE="sirene"

if [ $# -gt 0 ]; then
    metadata=$(cat "$1")
else
    metadata=$(cargo metadata --no-deps --format-version 1)
fi

root_version=$(jq -r --arg root "$ROOT_CRATE" \
    '.packages[] | select(.name == $root) | .version' <<<"$metadata")
if [ -z "$root_version" ] || [ "$root_version" = "null" ]; then
    echo "Could not determine the $ROOT_CRATE version from cargo metadata." >&2
    exit 1
fi

# One "crate version dependency requirement dependency-version" line per
# non-dev dependency between publishable members. `publish = false` serializes
# as `"publish": []`.
edges=$(jq -r '
    [.packages[] | select(.publish != [])] as $members
    | ($members | map({(.name): .version}) | add) as $versions
    | $members[]
    | . as $p
    | .dependencies[]
    | select(.kind != "dev" and $versions[.name] != null)
    | "\($p.name) \($p.version) \(.name) \(.req) \($versions[.name])"
' <<<"$metadata")

status=0
while read -r name version dep req dep_version; do
    [ -z "$name" ] && continue
    # `version = "0.1.0"` reaches metadata as the requirement "^0.1.0".
    pin=${req#^}
    if [ "$pin" != "$dep_version" ]; then
        echo "Version mismatch: $name depends on $dep $req, but the workspace builds $dep $dep_version." >&2
        echo "  Bump the $dep version and every pin on it together." >&2
        status=1
        continue
    fi
    if [[ "$version" != *-* && "$dep_version" == *-* ]]; then
        echo "Pre-release dependency: $name $version is a stable release but depends on $dep $dep_version." >&2
        status=1
    fi
done <<<"$edges"

if [ $status -ne 0 ]; then
    echo "Workspace versions do not agree - refusing to publish." >&2
    exit 1
fi

echo "Workspace versions agree ($ROOT_CRATE $root_version)."
exit 0
