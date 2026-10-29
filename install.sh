#!/bin/sh
# Installs the latest Platno release for this computer (Linux or macOS). No Rust needed.
#
#   curl -fsSL https://github.com/WebLJ-si/Platno/releases/latest/download/install.sh | sh
#
# Options (environment variables): PLATNO_DIR (default ~/.local/bin), PLATNO_VERSION (default: latest).
set -eu

REPO="${PLATNO_REPO:-WebLJ-si/Platno}"
BASE="${PLATNO_BASE_URL:-https://github.com/$REPO/releases}"
DIR="${PLATNO_DIR:-$HOME/.local/bin}"
VERSION="${PLATNO_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "$1 is required but was not found."; }

need curl
case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *) fail "This installer supports Linux and macOS. On Windows download the .zip from the releases page." ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) fail "Unsupported processor: $(uname -m)." ;;
esac
if [ "$os" = macos ]; then suffix=macos-universal; else suffix="linux-$arch"; fi

if [ "$VERSION" = latest ]; then
    final=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$BASE/latest") || fail "Could not look up the latest release."
    VERSION=${final##*/}
fi
VERSION=${VERSION#v}
case "$VERSION" in
    ''|*[!0-9A-Za-z.-]*) fail "Unexpected version: $VERSION" ;;
esac
file="platno-$VERSION-$suffix"
url="$BASE/download/v$VERSION"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
say "Downloading Platno $VERSION for $suffix ..."
curl -fsSL -o "$tmp/$file" "$url/$file" || fail "Could not download $url/$file"
curl -fsSL -o "$tmp/SHA256SUMS" "$url/SHA256SUMS" || fail "Could not download the checksum file."

expected=$(awk -v f="$file" '{n=$2; sub(/^\*/,"",n)} n==f {print $1; exit}' "$tmp/SHA256SUMS")
[ -n "$expected" ] || fail "The release does not list a checksum for $file."
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$file" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$file" | awk '{print $1}')
else
    fail "sha256sum or shasum is required to verify the download."
fi
[ "$expected" = "$actual" ] || fail "Checksum mismatch: the download was damaged or altered. Nothing was installed."

chmod +x "$tmp/$file"
"$tmp/$file" --version >/dev/null 2>&1 || fail "The downloaded program does not run on this computer."
mkdir -p "$DIR"
mv "$tmp/$file" "$DIR/platno"
say "Installed $DIR/platno ($("$DIR/platno" --version))"

case ":$PATH:" in
    *":$DIR:"*) ;;
    *) say "Note: $DIR is not on your PATH. Run it with: $DIR/platno" ;;
esac
command -v ffmpeg >/dev/null 2>&1 || say "Note: FFmpeg was not found. Install it so videos that browsers cannot play can be converted (macOS: brew install ffmpeg; Debian/Ubuntu: sudo apt install ffmpeg)."
say ""
say "Start it with:   platno"
say "Then open http://127.0.0.1:8787 and create the owner account."
