#!/usr/bin/env bash
# Create a binary release archive or extract its executable for Linux packaging.
set -euo pipefail
case "${1:-}" in
  pack)
    [ "$#" -eq 5 ] || { echo "usage: $0 pack VERSION TARGET BINARY OUTDIR" >&2; exit 2; }
    version=$2 target=$3 binary=$4 out=$5
    mkdir -p "$out"
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    cp "$binary" "$work/sirene"
    cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES "$work/"
    tar -czf "$out/sirene-${version}-${target}.tar.gz" -C "$work" sirene LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES
    ;;
  extract)
    [ "$#" -eq 3 ] || { echo "usage: $0 extract ARCHIVE OUTDIR" >&2; exit 2; }
    mkdir -p "$3"
    tar -xzf "$2" -C "$3" sirene
    ;;
  *) echo "usage: $0 {pack|extract} ..." >&2; exit 2 ;;
esac
