#!/bin/sh
# Runs in the Alpine container with the corresponding source tree at /work.
set -eu

src=/work/sources
mode=${1:-build}
case "$mode" in
    build|relink) ;;
    *) echo "usage: $0 [build|relink]" >&2; exit 2 ;;
esac
# Leave build outputs readable and removable by the owner of the host workspace.
if [ -n "${SIRENE_RUNTIME_OWNER:-}" ]; then
    trap 'chown -R "$SIRENE_RUNTIME_OWNER" /work' EXIT
fi
apk add --no-cache alpine-sdk autoconf automake binutils clang libtool linux-headers meson xz eudev-dev gettext-dev
if [ "$mode" = build ]; then
    (cd "$src" && sha256sum -c SHA256SUMS)
fi
apk info -v | sort > /work/build-packages.txt

build_dir=$(mktemp -d)
cd "$build_dir"
tar -xf "$src/type2-runtime.tar.gz"
tar -xf "$src/fuse-3.15.0.tar.xz"
runtime_src=$(printf '%s\n' "$build_dir"/AppImage-*/src/runtime)
fuse_src="$build_dir/fuse-3.15.0"
patch -d "$fuse_src" -p1 < "$runtime_src/../../patches/libfuse/mount.c.diff"
meson setup --default-library=static --prefix=/usr --libdir=lib -Dexamples=false "$fuse_src/build" "$fuse_src"
meson compile -C "$fuse_src/build"
meson install -C "$fuse_src/build"

relink=/work/relink
if [ "$mode" = build ]; then
    tar -xf "$src/zstd-1.5.7.tar.gz"
    tar -xf "$src/squashfuse-0.5.2.tar.gz"
    tar -xf "$src/musl-1.2.5.tar.gz"
    make -C zstd-1.5.7/lib -j"$(nproc)" install-static install-includes PREFIX=/usr
    cd squashfuse-0.5.2
    ./autogen.sh
    LDFLAGS=-static ./configure --disable-shared --without-zlib --without-xz --without-lzo --without-lz4
    make -j"$(nproc)"
    make install
    install -m644 ./*.h /usr/local/include/squashfuse/
    mkdir -p "$relink" /work/licenses
    # Use captured application archives and object code so a modified libfuse
    # can be linked without rebuilding the other application objects.
    clang -std=gnu99 -Os -D_FILE_OFFSET_BITS=64 \
        '-DGIT_COMMIT="https://github.com/AppImage/type2-runtime/commit/dd6cebedcbddde9c82f89b011e8e1d40b6e43868"' \
        -ffunction-sections -fdata-sections -I /usr/local/include/squashfuse -I /usr/include/fuse3 \
        -c "$runtime_src/runtime.c" -o "$relink/runtime.o"
    cp /usr/local/lib/libsquashfuse.a /usr/local/lib/libsquashfuse_ll.a /usr/lib/libzstd.a \
        "$runtime_src/data_sections.ld" "$relink/"
    install -m644 "$runtime_src/../../LICENSE" /work/licenses/type2-runtime.txt
    install -m644 "$fuse_src/LGPL2.txt" /work/licenses/libfuse-LGPL-2.1.txt
    install -m644 LICENSE /work/licenses/squashfuse.txt
    install -m644 ../zstd-1.5.7/LICENSE /work/licenses/zstd.txt
    install -m644 ../musl-1.2.5/COPYRIGHT /work/licenses/musl.txt
    cp /usr/lib/libfuse3.a "$relink/libfuse3.a"
    fuse_archive="$relink/libfuse3.a"
else
    fuse_archive=/usr/lib/libfuse3.a
fi

cd "$relink"
clang -static -static-pie -Wl,--gc-sections -T data_sections.ld -o /work/runtime \
    runtime.o libsquashfuse.a libsquashfuse_ll.a libzstd.a "$fuse_archive"
strip --strip-debug --strip-unneeded /work/runtime
printf 'AI\002' | dd of=/work/runtime bs=1 count=3 seek=8 conv=notrunc
