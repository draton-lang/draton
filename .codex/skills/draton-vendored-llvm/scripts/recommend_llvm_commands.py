#!/usr/bin/env python3
from __future__ import annotations

import sys

COMMANDS = {
    "fetch": [
        "python3 scripts/vendor_llvm.py fetch --target host",
        "python3 scripts/vendor_llvm.py print-env --target host",
    ],
    "build": [
        "python3 scripts/vendor_llvm.py fetch --target host",
        "eval \"$(python3 scripts/vendor_llvm.py print-env --target host)\"",
        "cargo build --release",
    ],
    "package": [
        "python3 scripts/package_release.py ...",
        "python3 scripts/smoke_release.py --archive <artifact>",
    ],
    "smoke": [
        "python3 scripts/smoke_release.py --archive <artifact>",
    ],
}


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in COMMANDS:
        print("usage: recommend_llvm_commands.py <fetch|build|package|smoke>")
        return 1
    for command in COMMANDS[argv[1]]:
        print(command)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
