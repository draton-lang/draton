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

find_profile_file() {
    shell_name="$(basename "${SHELL:-sh}")"
    case "$shell_name" in
        bash)
            printf '%s\n' "$HOME/.bashrc" "$HOME/.profile"
            ;;
        zsh)
            printf '%s\n' "$HOME/.zshrc" "$HOME/.profile"
            ;;
        *)
            printf '%s\n' "$HOME/.profile"
            ;;
    esac
}

ensure_path_in_profile() {
    target_dir="$1"
    export_line="export PATH=\"$target_dir:\$PATH\""
    for candidate in $(find_profile_file); do
        parent_dir="$(dirname "$candidate")"
        if [ ! -d "$parent_dir" ]; then
            mkdir -p "$parent_dir"
        fi
        if [ ! -e "$candidate" ]; then
            : > "$candidate"
        fi
        if grep -F "$export_line" "$candidate" >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
        if [ -w "$candidate" ]; then
            {
                printf '\n# Added by Draton Early Tooling Preview installer\n'
                printf '%s\n' "$export_line"
            } >> "$candidate"
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
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
CHECKSUM_URL="${URL}.sha256"

TMPDIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT INT TERM

ARCHIVE="$TMPDIR/$ARTIFACT"
CHECKSUM_FILE="$TMPDIR/$ARTIFACT.sha256"
sh -c "$FETCH \"$URL\" > \"$ARCHIVE\""
sh -c "$FETCH \"$CHECKSUM_URL\" > \"$CHECKSUM_FILE\""

EXPECTED_SUM="$(awk 'NF { print $1; exit }' "$CHECKSUM_FILE")"
if [ -z "$EXPECTED_SUM" ]; then
    echo "failed to read SHA256 checksum for $ARTIFACT" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_SUM="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL_SUM="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"
elif command -v openssl >/dev/null 2>&1; then
    ACTUAL_SUM="$(openssl dgst -sha256 "$ARCHIVE" | awk '{print $NF}')"
else
    echo "warning: no SHA256 tool found; skipping checksum verification" >&2
    ACTUAL_SUM="$EXPECTED_SUM"
fi

if [ "$ACTUAL_SUM" != "$EXPECTED_SUM" ]; then
    echo "checksum verification failed for $ARTIFACT" >&2
    exit 1
fi

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

PROFILE_FILE=""
if PROFILE_FILE="$(ensure_path_in_profile "$CURRENT_LINK")"; then
    PATH_NOTE="Updated PATH in: $PROFILE_FILE"
else
    PATH_NOTE="Could not update your shell profile automatically."
fi

cat <<EOF

Installed Draton Early Tooling Preview to:
  $FINAL_DIR

$PATH_NOTE

Use this directory on PATH:
  $CURRENT_LINK

Example:
  export PATH="$CURRENT_LINK:\$PATH"

Then verify:
  drat --version
  drat fmt --check $CURRENT_LINK/examples/early-preview/hello-app/src
EOF
