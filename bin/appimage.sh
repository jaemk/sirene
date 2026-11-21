#!/usr/bin/env bash
# Build the AppImage runtime and its corresponding source bundle, then append
# a zstd squashfs of the AppDir (LINUXP-5). Needs Docker and squashfs-tools.
#
#   bin/appimage.sh ARCH APPDIR OUT
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 ARCH APPDIR OUT" >&2
    exit 2
fi
arch="$1"
appdir="$2"
out="$3"
case "$arch" in
    x86_64) platform=linux/amd64 ;;
    aarch64) platform=linux/arm64 ;;
    *)
        echo "no AppImage runtime for $arch" >&2
        exit 1
        ;;
esac
if [ ! -x "$appdir/AppRun" ]; then
    echo "$appdir has no AppRun; write it with cargo xtask linux" >&2
    exit 1
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir "$work/sources"
download_source() {
    local file="$1" sha="$2" url="$3"
    curl --proto '=https' --tlsv1.2 --retry 3 -fsSL -o "$work/sources/$file" "$url"
    printf '%s  %s\n' "$sha" "$file" >> "$work/sources/SHA256SUMS"
}
download_source type2-runtime.tar.gz 70129e551d44571a438b06640cd12ea3b6db67f31b142b607dcccf66d5dbc320 \
    https://codeload.github.com/AppImage/type2-runtime/legacy.tar.gz/dd6cebedcbddde9c82f89b011e8e1d40b6e43868
download_source fuse-3.15.0.tar.xz 70589cfd5e1cff7ccd6ac91c86c01be340b227285c5e200baa284e401eea2ca0 \
    https://github.com/libfuse/libfuse/releases/download/fuse-3.15.0/fuse-3.15.0.tar.xz
download_source squashfuse-0.5.2.tar.gz db0238c5981dabbd80ee09ae15387f390091668ca060a7bc38047912491443d3 \
    https://github.com/vasi/squashfuse/archive/0.5.2.tar.gz
download_source zstd-1.5.7.tar.gz eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3 \
    https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-1.5.7.tar.gz
download_source musl-1.2.5.tar.gz a9a118bbe84d8764da0ea0d28b3ab3fae8477fc7e4085d90102b8596fc7c75e4 \
    https://musl.libc.org/releases/musl-1.2.5.tar.gz

cp "$script_dir/appimage-runtime.sh" "$work/build-runtime.sh"
# The compiler and musl system toolchain run inside Alpine. Corresponding
# application inputs and non-system objects from this build go in the bundle.
builder=alpine:3.21@sha256:bc21487101ae47172509130ee1092cc273a831968d64b617776d1f69c81e42ab
docker run --rm --platform "$platform" \
    --volume "$work:/work" --env "SIRENE_RUNTIME_OWNER=$(id -u):$(id -g)" \
    "$builder" sh /work/build-runtime.sh

cp -a "$appdir" "$work/AppDir"
install -dm755 "$work/AppDir/usr/share/licenses/appimage-runtime"
cp "$work/licenses/"* "$work/AppDir/usr/share/licenses/appimage-runtime/"
mksquashfs "$work/AppDir" "$work/squashfs" -root-owned -noappend -comp zstd -quiet
cat "$work/runtime" "$work/squashfs" > "$work/image"
chmod 0755 "$work/image"
printf '%s\n' "$builder" > "$work/builder-image"
cp "$script_dir/appimage-runtime.md" "$work/README.md"
tar -czf "$work/source.tar.gz" -C "$work" \
    README.md builder-image build-runtime.sh sources licenses relink build-packages.txt
source_out="${out%.AppImage}.AppImage-runtime-source.tar.gz"
mv -fT -- "$work/source.tar.gz" "$source_out"
mv -fT -- "$work/image" "$out"
printf '%s\n' "$out" "$source_out"
