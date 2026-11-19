#!/bin/bash

# Build the @jaemk/sirene npm package from wasm/npm/, the loader, a built
# sirene.wasm, and the licenses: stages it in OUT/package with VERSION set, then
# packs it to OUT/jaemk-sirene-VERSION.tgz.
#
#   bin/npm-package.sh VERSION SIRENE_WASM OUT

set -euo pipefail

version=${1:?version}
wasm=${2:?path to sirene_wasm.wasm}
out=${3:?output directory}
root=$(cd "$(dirname "$0")/.." && pwd)

stage="$out/package"
rm -rf "$stage"
mkdir -p "$stage"
jq --arg v "$version" '.version = $v' "$root/wasm/npm/package.json" > "$stage/package.json"
sed "s/@VERSION/@$version/g" "$root/wasm/npm/README.md" > "$stage/README.md"
cp "$wasm" "$stage/sirene.wasm"
cp "$root/docs/assets/sirene-wasm.js" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" \
    "$root/THIRD-PARTY-NOTICES" "$stage/"
npm pack "$stage" --pack-destination "$out"
