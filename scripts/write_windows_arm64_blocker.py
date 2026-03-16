#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
import urllib.request
from pathlib import Path

ASSETS_URL = "https://raw.githubusercontent.com/KyleMayes/install-llvm-action/master/assets.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Write the Windows aarch64 early-release blocker note."
    )
    parser.add_argument("--llvm-version", default="14.0.6")
    parser.add_argument("--out", required=True)
    return parser.parse_args()


def load_assets() -> dict[str, object]:
    with urllib.request.urlopen(ASSETS_URL, timeout=30) as response:
        return json.load(response)


def main() -> int:
    args = parse_args()
    data = load_assets()
    win_arm64 = data.get("win32", {}).get("arm64", {})
    if args.llvm_version in win_arm64:
        raise SystemExit(
            "LLVM "
            + args.llvm_version
            + " now has a Windows arm64 prebuilt asset in install-llvm-action; "
            + "remove the explicit blocker and enable a real release build for "
            + "aarch64-pc-windows-msvc."
        )

    note = (
        "Windows aarch64 Early Tooling Preview is blocked because LLVM "
        + args.llvm_version
        + " does not have a published win32/arm64 prebuilt asset in the "
        + "install-llvm-action distribution matrix. Draton currently targets "
        + "inkwell/llvm-sys 14, so there is no verified way to build and smoke-test "
        + "an aarch64-pc-windows-msvc release artifact without producing and "
        + "maintaining a separate LLVM 14 arm64 Windows toolchain."
    )
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(note + "\n", encoding="utf-8")
    sys.stdout.write(note + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
