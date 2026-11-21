#!/usr/bin/env bash
# Exercise packages from linux-package.sh on their native architecture (LINUXP-7).
set -euo pipefail

if [ "$#" -ne 4 ]; then
    echo "usage: $0 ARCH VERSION BINARY DIST" >&2
    exit 2
fi

arch="$1"
version="$2"
binary="$(realpath -- "$3")"
dist="$(realpath -- "$4")"

case "$arch" in
    x86_64) machine="Advanced Micro Devices X86-64" ;;
    aarch64) machine="AArch64" ;;
    *)
        echo "unsupported Linux package architecture: $arch" >&2
        exit 1
        ;;
esac
test "$(uname -m)" = "$arch"

rpm_version="${version/-dev/\~dev}"
rpm_file="sirene-$rpm_version-1.$arch.rpm"
upgrade_file="sirene-$rpm_version-2.$arch.rpm"
appimage="$dist/Sirene-$version-$arch.AppImage"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

test -s "$dist/$rpm_file"
test -x "$appimage"
mkdir "$work/runtime-source"
tar -xzf "$dist/Sirene-$version-$arch.AppImage-runtime-source.tar.gz" -C "$work/runtime-source"
(
    cd "$work/runtime-source"
    test -s README.md
    test -s build-runtime.sh
    test -s build-packages.txt
    test -s licenses/type2-runtime.txt
    test -s licenses/libfuse-LGPL-2.1.txt
    test -s relink/runtime.o
    test -s relink/libfuse3.a
    (cd sources && sha256sum -c SHA256SUMS)
    docker run --rm --volume "$PWD:/work" --env "SIRENE_RUNTIME_OWNER=$(id -u):$(id -g)" \
        "$(cat builder-image)" sh /work/build-runtime.sh relink
)
LC_ALL=C readelf -h "$appimage" | grep -E "Machine:[[:space:]]+$machine$"
LC_ALL=C readelf -h "$binary" | grep -E "Machine:[[:space:]]+$machine$"
test "$("$appimage" --appimage-extract-and-run --version)" = "sirene $version"
offset="$("$appimage" --appimage-offset)"
[[ "$offset" =~ ^[0-9]+$ ]]
cp "$work/runtime-source/runtime" "$work/relinked.AppImage"
tail -c "+$((offset + 1))" "$appimage" >> "$work/relinked.AppImage"
chmod +x "$work/relinked.AppImage"
test "$("$work/relinked.AppImage" --appimage-extract-and-run --version)" = "sirene $version"
printf 'flowchart LR\n  Start --> Done\n' |
    "$appimage" --appimage-extract-and-run ascii > "$work/drawing"
grep -F 'Done' "$work/drawing"
"$appimage" --appimage-extract-and-run update --check --to "$version" > "$work/update-check"
grep -F 'runs from an AppImage' "$work/update-check"

(
    cd "$work"
    "$appimage" --appimage-extract >/dev/null
    cmp "$binary" squashfs-root/usr/bin/sirene
    desktop-file-validate squashfs-root/sirene.desktop
    test -f squashfs-root/.sirene-appimage
    cmp "$work/runtime-source/licenses/type2-runtime.txt" \
        squashfs-root/usr/share/licenses/appimage-runtime/type2-runtime.txt
    cmp "$work/runtime-source/licenses/libfuse-LGPL-2.1.txt" \
        squashfs-root/usr/share/licenses/appimage-runtime/libfuse-LGPL-2.1.txt
    mv squashfs-root renamed-appdir
    if ./renamed-appdir/AppRun update --yes --to "$version" > update-refusal 2>&1; then
        echo "the extracted AppImage accepted self-update" >&2
        exit 1
    fi
    grep -F 'runs from an AppImage' update-refusal
)

# Change the RPM release number to exercise a real dnf upgrade using the same build.
mkdir "$work/upgrade"
cargo generate-rpm -p crates/sirene --arch "$arch" --auto-req find-requires \
    --set-metadata "$(printf 'version = "%s"\nrelease = "2"' "$rpm_version")" \
    -o "$work/upgrade/"
test -s "$work/upgrade/$upgrade_file"

docker run --rm -i \
    --volume "$binary:/inputs/sirene:ro" \
    --volume "$dist/$rpm_file:/inputs/$rpm_file:ro" \
    --volume "$work/upgrade/$upgrade_file:/inputs/$upgrade_file:ro" \
    fedora:43 bash -s -- "$rpm_file" "$upgrade_file" "$version" <<'FEDORA'
set -euo pipefail
rpm_file="$1"
upgrade_file="$2"
version="$3"

dnf -y install desktop-file-utils shared-mime-info glib2 diffutils
dnf -y --setopt=tsflags=nodocs install "/inputs/$rpm_file"
test "$(rpm -q --queryformat '%{RELEASE}' sirene)" = 1
cmp /inputs/sirene /usr/bin/sirene
test "$(sirene --version)" = "sirene $version"
printf 'flowchart LR\n  Start --> Done\n' | sirene ascii > /tmp/drawing
grep -F 'Done' /tmp/drawing
if sirene update --yes --to "$version" > /tmp/update-refusal 2>&1; then
    echo "the RPM accepted self-update" >&2
    exit 1
fi
grep -F 'system' /tmp/update-refusal
for notice in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES; do
    test -s "/usr/share/licenses/sirene/$notice"
done
desktop-file-validate /usr/share/applications/sirene.desktop
grep -Fx 'Exec=/usr/bin/sirene %F' /usr/share/applications/sirene.desktop
test -s /usr/share/icons/hicolor/scalable/apps/sirene.svg
printf 'flowchart LR\n  Start --> Done\n' > /tmp/example.mmd
gio info -a standard::content-type /tmp/example.mmd | grep -F 'text/x-mermaid'
printf 'digraph { Start -> Done }\n' > /tmp/example.gv
gio info -a standard::content-type /tmp/example.gv | grep -F 'text/vnd.graphviz'
printf '@startuml\nAlice -> Bob\n@enduml\n' > /tmp/example.puml
gio info -a standard::content-type /tmp/example.puml | grep -F 'text/x-plantuml'
for mime in text/x-mermaid text/vnd.graphviz text/x-plantuml text/markdown; do
    gio mime "$mime" | grep -F 'sirene.desktop'
done

dnf -y --setopt=tsflags=nodocs upgrade "/inputs/$upgrade_file"
test "$(rpm -q --queryformat '%{RELEASE}' sirene)" = 2
cmp /inputs/sirene /usr/bin/sirene
dnf -y remove sirene
test ! -e /usr/bin/sirene
test ! -e /usr/share/applications/sirene.desktop
FEDORA
