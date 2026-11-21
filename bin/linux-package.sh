#!/usr/bin/env bash
# Package the GNU editor binary using the same commands in CI and releases (LINUXP-6).
set -euo pipefail

if [ "$#" -ne 4 ]; then
    echo "usage: $0 ARCH VERSION BINARY OUT" >&2
    exit 2
fi

arch="$1"
version="$2"
binary="$3"
out="$4"

case "$arch" in
    x86_64|aarch64) ;;
    *)
        echo "unsupported Linux package architecture: $arch" >&2
        exit 1
        ;;
esac
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-dev)?$ ]]; then
    echo "expected a release or -dev version: $version" >&2
    exit 1
fi
if [ ! -x "$binary" ]; then
    echo "expected an executable GNU editor binary: $binary" >&2
    exit 1
fi

rpm_version="${version/-dev/\~dev}"
mkdir -p "$out"
cargo run --locked --package xtask -- linux --binary "$binary" --out target/linux
cargo generate-rpm -p crates/sirene --arch "$arch" --auto-req find-requires \
    --set-metadata "version = \"$rpm_version\"" -o "$out/"
bash bin/appimage.sh "$arch" target/linux/AppDir "$out/Sirene-$version-$arch.AppImage"

# Fail before attesting/uploading if the generator changed its output names or metadata.
rpm_file="$out/sirene-$rpm_version-1.$arch.rpm"
test -s "$rpm_file"
test -x "$out/Sirene-$version-$arch.AppImage"
test -s "$out/Sirene-$version-$arch.AppImage-runtime-source.tar.gz"
test "$(rpm -qp --queryformat '%{NAME} %{VERSION} %{RELEASE} %{ARCH}' "$rpm_file")" = \
    "sirene $rpm_version 1 $arch"
