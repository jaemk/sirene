#!/usr/bin/env bash
# Move the workspace from a released version to the next patch's `-dev`
# version, so a build from main never reports the released version.
#
# Usage: bump-dev.sh <version> [<manifest>]
#   <version>   the version just released (e.g. 0.1.1)
#   <manifest>  the workspace manifest; defaults to Cargo.toml
#
# Rewrites `[workspace.package] version` and the `version` of every
# `[workspace.dependencies]` entry with a `crates/` path, prints the new
# version, and changes nothing when the manifest is not on <version> (main has
# already moved on).
set -euo pipefail

version="${1:?usage: bump-dev.sh VERSION [MANIFEST]}"
manifest="${2:-Cargo.toml}"

if ! grep -qx "version = \"${version}\"" "$manifest"; then
  echo "$manifest is not on ${version}; leaving it alone" >&2
  exit 0
fi

IFS=. read -r major minor patch <<<"$version"
next="${major}.${minor}.$((patch + 1))-dev"
escaped="${version//./\\.}"
# Through a temporary file, since `sed -i` differs between GNU and BSD.
tmp="${manifest}.bump-dev"
sed \
  -e "s/^version = \"${escaped}\"$/version = \"${next}\"/" \
  -e "s/\(path = \"crates\/[^\"]*\", version = \)\"${escaped}\"/\1\"${next}\"/" \
  "$manifest" > "$tmp"
mv "$tmp" "$manifest"
echo "$next"
