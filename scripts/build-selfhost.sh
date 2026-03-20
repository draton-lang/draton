#!/usr/bin/env bash
# Build the Draton selfhost binary (phase 1).
#
# This script works around two limitations of the current pre-built drat.exe:
#   1. The @llvm.global_ctors.N "unknown special variable" LLVM crash.
#      Fixed in Rust source (draton-codegen/src/gc.rs) but not yet released.
#      Workaround: post-process the emitted .ll to merge all ctor globals.
#   2. The "unsupported GC: shadow-stack" backend crash.
#      Workaround: disable GCRoot during selfhost emission (DRATON_DISABLE_GCROOT=1).
#
# Usage: bash scripts/build-selfhost.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DRAT_INSTALL="${DRAT_INSTALL:-$HOME/AppData/Local/Draton/current}"
CLANG="${CLANG:-$(command -v clang 2>/dev/null || echo '/c/Program Files/LLVM/bin/clang.exe')}"
GNU_CC="${GNU_CC:-$DRAT_INSTALL/windows-gnu/bin/cc.exe}"
RUNTIME_LIB="${DRATON_RUNTIME_LIB:-$DRAT_INSTALL/libdraton_runtime.a}"

PROFILE=debug
BUILD_FLAGS=""
if [[ "${1:-}" == "--release" ]]; then
    PROFILE=release
    BUILD_FLAGS="--release"
fi

BUILD_DIR="$REPO_ROOT/build/$PROFILE"
mkdir -p "$BUILD_DIR"

PROJECT_NAME="draton-selfhost-phase1"
LL_RAW="$BUILD_DIR/${PROJECT_NAME}.ll"
LL_FIXED="$BUILD_DIR/${PROJECT_NAME}-fixed.ll"
OBJ="$BUILD_DIR/${PROJECT_NAME}.o"
BIN="$BUILD_DIR/${PROJECT_NAME}.exe"

echo "==> Emitting LLVM IR (DRATON_DISABLE_GCROOT=1 to skip shadow-stack GC)..."
DRATON_DISABLE_GCROOT=1 drat build $BUILD_FLAGS 2>&1 | grep -v "^type warning" || true

# drat crashes before writing the .o file but writes the .ll file first.
if [[ ! -f "$LL_RAW" ]]; then
    echo "ERROR: $LL_RAW was not written by drat build" >&2
    exit 1
fi

echo "==> Merging @llvm.global_ctors entries in $LL_RAW..."
node - "$LL_RAW" "$LL_FIXED" << 'JSEOF'
const fs = require('fs');
const [,, src, dst] = process.argv;
const content = fs.readFileSync(src, 'utf8');
const lines = content.split('\n');
const ctorsRe = /^@llvm\.global_ctors(?:\.\d+)?\s*=\s*appending\s+global\s+\[1\s+x\s+\{[^}]+\}\]\s*\[(\{[^\]]+\})\]/;
const entries = [];
const keep = [];
for (const line of lines) {
    const m = ctorsRe.exec(line.trim());
    if (m) { entries.push(m[1]); } else { keep.push(line); }
}
console.log(`Merged ${entries.length} @llvm.global_ctors entries.`);
const n = entries.length;
const ty = '{ i32, void ()*, i8* }';
if (n > 0) keep.push(`@llvm.global_ctors = appending global [${n} x ${ty}] [${entries.join(', ')}]\n`);
fs.writeFileSync(dst, keep.join('\n'));
JSEOF

echo "==> Compiling $LL_FIXED with clang (target: x86_64-pc-windows-gnu)..."
"$CLANG" -target x86_64-pc-windows-gnu -c "$LL_FIXED" -o "$OBJ"

echo "==> Linking $BIN..."
"$GNU_CC" -o "$BIN" "$OBJ" "$RUNTIME_LIB" \
    -static-libgcc -static-libstdc++ -lbcrypt -luserenv -lntdll

echo ""
echo "SUCCESS: $BIN"
"$BIN" 2>&1 | head -3 || true
