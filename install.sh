#!/usr/bin/env sh
set -eu

REPO="${DRATON_REPO:-draton-lang/draton}"
VERSION="${DRATON_VERSION:-}"
INSTALL_ROOT="${DRATON_INSTALL_ROOT:-$HOME/.local/share/draton}"

usage() {
    cat <<'EOF'
Usage: install.sh [--version <tag>] [--install-root <dir>]

Installs the Draton Early Tooling Preview for Linux or macOS.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        --install-root)
            INSTALL_ROOT="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if command -v curl >/dev/null 2>&1; then
    FETCH='curl -fsSL'
elif command -v wget >/dev/null 2>&1; then
    FETCH='wget -qO-'
else
    echo "need curl or wget to download Draton" >&2
    exit 1
fi

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux) OS_SLUG="linux" ;;
    Darwin) OS_SLUG="macos" ;;
    *)
        echo "unsupported OS: $OS" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH_SLUG="x86_64" ;;
    aarch64|arm64) ARCH_SLUG="aarch64" ;;
    *)
        echo "unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

ARTIFACT="draton-early-${OS_SLUG}-${ARCH_SLUG}.tar.gz"
if [ -n "$VERSION" ]; then
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"
else
    URL="https://github.com/${REPO}/releases/latest/download/${ARTIFACT}"
fi

TMPDIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT INT TERM

ARCHIVE="$TMPDIR/$ARTIFACT"
sh -c "$FETCH \"$URL\" > \"$ARCHIVE\""
tar -xzf "$ARCHIVE" -C "$TMPDIR"

ROOT_DIR="$(find "$TMPDIR" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
if [ -z "$ROOT_DIR" ]; then
    echo "archive did not contain an installable root directory" >&2
    exit 1
fi

mkdir -p "$INSTALL_ROOT"
FINAL_DIR="$INSTALL_ROOT/$(basename "$ROOT_DIR")"
rm -rf "$FINAL_DIR"
mv "$ROOT_DIR" "$FINAL_DIR"

CURRENT_LINK="$INSTALL_ROOT/current"
rm -f "$CURRENT_LINK"
ln -s "$FINAL_DIR" "$CURRENT_LINK"

"$CURRENT_LINK/drat" --version

cat <<EOF

Installed Draton Early Tooling Preview to:
  $FINAL_DIR

Add this directory to PATH:
  $CURRENT_LINK

Example:
  export PATH="$CURRENT_LINK:\$PATH"

Then verify:
  drat --version
  drat fmt --check $CURRENT_LINK/examples/early-preview/hello-app/src
EOF
