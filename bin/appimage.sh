#!/usr/bin/env bash
# Packs an AppDir (`cargo xtask linux`) into an AppImage (LINUXP-5):
# the pinned AppImage type 2 runtime followed by a zstd squashfs of the AppDir.
#
#   bin/appimage.sh ARCH APPDIR OUT
#
# ARCH is x86_64 or aarch64. Needs curl, sha256sum, and mksquashfs (squashfs-tools).
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 ARCH APPDIR OUT" >&2
    exit 2
fi
arch="$1"
appdir="$2"
out="$3"

# https://github.com/AppImage/type2-runtime/releases/tag/20251108
runtime_tag="20251108"
case "$arch" in
    x86_64) runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d" ;;
    aarch64) runtime_sha256="00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444" ;;
    *)
        echo "no AppImage runtime for $arch" >&2
        exit 1
        ;;
esac
if [ ! -x "$appdir/AppRun" ]; then
    echo "$appdir has no AppRun; write it with cargo xtask linux" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL -o "$work/runtime" \
    "https://github.com/AppImage/type2-runtime/releases/download/${runtime_tag}/runtime-${arch}"
echo "${runtime_sha256}  $work/runtime" | sha256sum -c --quiet -
mksquashfs "$appdir" "$work/squashfs" -root-owned -noappend -comp zstd -quiet
cat "$work/runtime" "$work/squashfs" > "$out"
chmod 0755 "$out"
echo "$out"
