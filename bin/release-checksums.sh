#!/usr/bin/env bash
# Validate the Linux release assets and checksum all published archives.
set -euo pipefail
[ "$#" -eq 3 ] || { echo "usage: $0 VERSION DIST OUTPUT" >&2; exit 2; }
version=$1 dist=$2 output=$3
output="$(cd "$(dirname "$output")" && pwd)/$(basename "$output")"
cd "$dist"
for arch in x86_64 aarch64; do
    test -s "sirene-${version}-1.${arch}.rpm"
    test -s "Sirene-${version}-${arch}.AppImage"
    test -s "Sirene-${version}-${arch}.AppImage-runtime-source.tar.gz"
done
shopt -s nullglob
linux_packages=(*.rpm *.AppImage *.AppImage-runtime-source.tar.gz)
test "${#linux_packages[@]}" -eq 6
tarballs=(*.tar.gz)
zips=(*.zip)
(( ${#tarballs[@]} > 0 && ${#zips[@]} > 0 ))
sha256sum -- "${tarballs[@]}" "${zips[@]}" *.rpm *.AppImage > "$output"
