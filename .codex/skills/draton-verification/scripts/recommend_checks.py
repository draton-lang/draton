#!/usr/bin/env python3
from __future__ import annotations

import sys

CHECKS = {
    "parser": ["cargo test -p draton-parser --test items"],
    "typeck": ["cargo test -p draton-typeck --test errors"],
    "workspace": ["cargo test --workspace"],
    "fmt": ["cargo fmt --all --check"],
    "clippy": ["cargo clippy --workspace -- -D warnings"],
    "docs": ["consistency pass between changed docs and referenced implementation"],
    "release": ["python3 scripts/package_release.py ...", "python3 scripts/smoke_release.py ..."],
    "llvm": ["python3 scripts/vendor_llvm.py fetch --target host", "python3 scripts/vendor_llvm.py print-env --target host"],
}


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: recommend_checks.py <area> [<area> ...]")
        print("known areas:", ", ".join(sorted(CHECKS)))
        return 1
    for area in argv[1:]:
        commands = CHECKS.get(area)
        if commands is None:
            print(f"[unknown] {area}")
            continue
        print(f"[{area}]")
        for command in commands:
            print(command)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
