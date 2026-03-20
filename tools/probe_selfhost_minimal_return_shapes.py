#!/usr/bin/env python3
"""Probe minimal standalone return/call shapes that should parse without crashing."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent


def find_stage1() -> Path:
    candidates = [
        Path("/tmp/draton_s1"),
        REPO / "build" / "debug" / "draton-selfhost-phase1",
        REPO / "draton_selfhost",
    ]
    for path in candidates:
        if path.exists():
            return path
    raise FileNotFoundError("no stage1 self-host binary found")


def run_text(stage1: Path, text: str) -> int:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(text)
        path = Path(handle.name)
    try:
        result = subprocess.run(
            [str(stage1), "ast-dump", str(path)],
            cwd=REPO,
            capture_output=True,
        )
        return result.returncode
    finally:
        path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    variants = {
        "minimal-if-return-call": "@type { main: () -> Int }\nfn main() {\n    if ready() {\n        return foo(2)\n    }\n    0\n}\n",
        "minimal-return-call": "@type { main: () -> Int }\nfn main() {\n    return foo(2)\n}\n",
        "minimal-if-return-ident": "@type { main: () -> Int }\nfn main() {\n    if ready() {\n        return foo\n    }\n    0\n}\n",
        "minimal-if-expr-call": "@type { main: () -> Int }\nfn main() {\n    if ready() {\n        foo(2)\n    }\n    0\n}\n",
        "minimal-if-return-zero": "@type { main: () -> Int }\nfn main() {\n    if ready() {\n        return 0\n    }\n    0\n}\n",
    }

    print(f"stage1: {stage1}")
    all_pass = True
    for label, text in variants.items():
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if code != 0:
            all_pass = False

    if all_pass:
        print("summary: all minimal standalone return/call shapes pass")
        return 0

    print("summary: at least one minimal standalone return/call shape now fails")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
