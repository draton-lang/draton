#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "vendor_llvm.py"


def main() -> int:
    if len(sys.argv) < 2:
        raise SystemExit("usage: cargo_with_vendor_llvm.py <cargo args...>")

    env = os.environ.copy()
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), "print-env", "--target", "host"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    for line in completed.stdout.splitlines():
        if not line.startswith("export "):
            continue
        name, value = line[len("export ") :].split("=", 1)
        env[name] = value.strip().strip("'")

    cmd = ["cargo", *sys.argv[1:]]
    return subprocess.call(cmd, cwd=ROOT, env=env)


if __name__ == "__main__":
    raise SystemExit(main())
