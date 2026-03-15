#!/usr/bin/env python3
"""GC stress tests — verify no segfault and correct exit code."""

import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def find_selfhost() -> str:
    candidates = [
        os.path.join(REPO, "draton_selfhost"),
        os.path.join(REPO, "draton_selfhost_test"),
        os.path.join(REPO, "target", "debug", "drat"),
    ]
    for path in candidates:
        if os.path.exists(path) and os.access(path, os.X_OK):
            return path
    raise FileNotFoundError("khong tim thay binary self-host hoac host fallback")


SELFHOST = find_selfhost()

CASES = [
    (
        "small alloc — linked list 10k nodes",
        "tests/programs/gc/stress_small_alloc.dt",
        0,
    ),
    (
        "large alloc — 100×1000 Int arrays",
        "tests/programs/gc/stress_large_alloc.dt",
        0,
    ),
]


def run(src_rel, expected_exit):
    src = os.path.join(REPO, src_rel)
    out = src.replace(".dt", "_out")
    r1 = subprocess.run(
        [SELFHOST, "build", src, "-o", out],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if r1.returncode != 0:
        message = r1.stderr.strip()
        if not message:
            message = r1.stdout.strip()
        return False, f"BUILD FAIL: {message[:200]}"
    r2 = subprocess.run([out], capture_output=True, text=True, timeout=30, check=False)
    if r2.returncode != expected_exit:
        return False, f"exit {r2.returncode} != expected {expected_exit}"
    return True, "ok"


def main() -> int:
    passed = failed = 0
    for (desc, src, exp) in CASES:
        try:
            ok, msg = run(src, exp)
        except subprocess.TimeoutExpired:
            print(f"  TIMEOUT  {desc}")
            failed += 1
            continue
        if ok:
            print(f"  PASS     {desc}")
            passed += 1
        else:
            print(f"  FAIL     {desc} — {msg}")
            failed += 1
    print(f"\n{passed}/{passed + failed} pass")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
