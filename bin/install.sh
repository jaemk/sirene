#!/usr/bin/env sh
# sirene installer: download the release binary for this platform and drop it on PATH.
#
#   curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | sh
#
# Honors:
#   SIRENE_VERSION       version to install (e.g. 0.2.0); default: latest release
#   SIRENE_INSTALL_DIR   install directory; default: ~/.local/bin
set -eu

REPO="jaemk/sirene"
INSTALL_DIR="${SIRENE_INSTALL_DIR:-$HOME/.local/bin}"

err() {
	echo "sirene-install: $*" >&2
	exit 1
}

CONNECT_TIMEOUT=15
MAX_TIME=600

# A downloader: curl or wget, whichever exists.
fetch() {
	# fetch <url> -> stdout
	if command -v curl >/dev/null 2>&1; then
		curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL \
			--connect-timeout "$CONNECT_TIMEOUT" --max-time "$MAX_TIME" "$1"
	elif command -v wget >/dev/null 2>&1; then
		wget --https-only --tries=1 --timeout="$CONNECT_TIMEOUT" -qO- "$1"
	else
		err "need curl or wget on PATH"
	fi
}

fetch_to() {
	# fetch_to <url> <dest-file>
	if command -v curl >/dev/null 2>&1; then
		curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL \
			--connect-timeout "$CONNECT_TIMEOUT" --max-time "$MAX_TIME" "$1" -o "$2"
	elif command -v wget >/dev/null 2>&1; then
		wget --https-only --tries=1 --timeout="$CONNECT_TIMEOUT" -qO "$2" "$1"
	else
		err "need curl or wget on PATH"
	fi
}

# Map uname to a release target triple. Linux prefers the musl leg: it is
# statically linked, so it runs on any glibc version, but it is built without
# the GUI. The gnu leg includes the GUI and needs the system's X11/Wayland and
# Vulkan libraries; set SIRENE_TARGET to request it (or any other leg).
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
Linux) os_part="unknown-linux-musl" ;;
Darwin) os_part="apple-darwin" ;;
*) err "unsupported OS: $os (must build from source)" ;;
esac
case "$arch" in
x86_64 | amd64) arch_part="x86_64" ;;
aarch64 | arm64) arch_part="aarch64" ;;
*) err "unsupported architecture: $arch" ;;
esac
target="${SIRENE_TARGET:-${arch_part}-${os_part}}"

# Resolve the version: explicit SIRENE_VERSION, else the latest release tag.
version="${SIRENE_VERSION:-}"
if [ -z "$version" ]; then
	tag="$(fetch "https://api.github.com/repos/${REPO}/releases/latest" \
		| sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
		| head -n 1)"
	[ -n "$tag" ] || err "could not determine the latest release; set SIRENE_VERSION"
	version="${tag#v}"
fi

asset="sirene-${version}-${target}.tar.gz"
url="https://github.com/${REPO}/releases/download/v${version}/${asset}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# This script is served from `main` but installs the latest RELEASE, whose
# asset set may predate what main builds. If the musl leg is missing, fall back
# to the gnu leg once, on Linux only.
echo "sirene-install: downloading ${asset}"
if ! fetch_to "$url" "$tmp/$asset"; then
	if [ "$os" = "Linux" ] && [ "$target" = "${arch_part}-unknown-linux-musl" ]; then
		echo "sirene-install: musl asset unavailable for ${version}, retrying with gnu"
		target="${arch_part}-unknown-linux-gnu"
		asset="sirene-${version}-${target}.tar.gz"
		url="https://github.com/${REPO}/releases/download/v${version}/${asset}"
		fetch_to "$url" "$tmp/$asset" || err "download failed: $url"
	else
		err "download failed: $url"
	fi
fi

# Verify the tarball against the published checksums.
sums_url="https://github.com/${REPO}/releases/download/v${version}/SHA256SUMS"
echo "sirene-install: downloading SHA256SUMS"
fetch_to "$sums_url" "$tmp/SHA256SUMS" || err "could not download SHA256SUMS"
# Exact match on the filename field (awk field splitting, not a grep regex),
# so a `.` in the asset name is never a metacharacter and a same-prefix asset
# name can never match.
expected="$(awk -v f="$asset" '$2 == f { print $1 }' "$tmp/SHA256SUMS")"
[ -n "$expected" ] || err "SHA256SUMS has no entry for ${asset}"
if command -v sha256sum >/dev/null 2>&1; then
	actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
	actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
else
	err "need sha256sum or shasum to verify the download; install one and retry"
fi
[ "$expected" = "$actual" ] || err "checksum mismatch for ${asset}: expected ${expected}, got ${actual}"
echo "sirene-install: checksum OK"

# Soft-verify GitHub build provenance attestation when `gh` is available. It
# never fails the install when `gh` is absent or the check itself errors.
if command -v gh >/dev/null 2>&1; then
	if gh attestation verify "$tmp/$asset" --repo "$REPO" >/dev/null 2>&1; then
		echo "sirene-install: build provenance verified"
	else
		echo "sirene-install: build provenance could not be verified (continuing)" >&2
	fi
fi

tar -xzf "$tmp/$asset" -C "$tmp" || err "could not extract $asset"
[ -f "$tmp/sirene" ] || err "archive did not contain a 'sirene' binary"

mkdir -p "$INSTALL_DIR" || err "could not create install directory: ${INSTALL_DIR}"
if command -v install >/dev/null 2>&1 && install -m 0755 "$tmp/sirene" "$INSTALL_DIR/sirene" 2>/dev/null; then
	:
elif cp "$tmp/sirene" "$INSTALL_DIR/sirene" 2>/dev/null && chmod 0755 "$INSTALL_DIR/sirene" 2>/dev/null; then
	:
else
	err "could not write to ${INSTALL_DIR}; check permissions or set SIRENE_INSTALL_DIR to a writable directory"
fi

echo "sirene-install: installed sirene ${version} to ${INSTALL_DIR}/sirene"
case ":${PATH}:" in
*":${INSTALL_DIR}:"*) ;;
*) echo "sirene-install: add ${INSTALL_DIR} to your PATH, e.g. export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac
