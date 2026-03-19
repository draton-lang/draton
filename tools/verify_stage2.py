#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys
import tempfile
import time


CASES = [
    (
        "hello world print",
        'fn main() {\n    print("hello")\n    0\n}',
        0,
        "hello",
    ),
    (
        "arithmetic exit code",
        "fn main() -> Int { 2 + 3 * 4 - 1 }",
        13,
        "",
    ),
    (
        "let binding exit",
        "fn main() -> Int {\n    let x = 10\n    let y = 32\n    x + y\n}",
        42,
        "",
    ),
    (
        "string concat",
        'fn main() {\n    let a = "foo"\n    let b = "bar"\n    print(str_concat(a, b))\n    0\n}',
        0,
        "foobar",
    ),
    (
        "if else branch",
        "fn main() -> Int {\n    let x = 5\n    if x > 3 { 42 } else { 0 }\n}",
        42,
        "",
    ),
    (
        "while loop sum",
        "fn main() -> Int {\n    let mut i = 0\n    let mut s = 0\n    while i < 5 {\n        s = s + i\n        i = i + 1\n    }\n    s\n}",
        10,
        "",
    ),
    (
        "function call",
        "@type { add: (Int, Int) -> Int }\nfn add(a, b) { a + b }\nfn main() -> Int { add(20, 22) }",
        42,
        "",
    ),
    (
        "fstring print",
        'fn main() {\n    let name = "draton"\n    print(f"hi {name}")\n    0\n}',
        0,
        "hi draton",
    ),
]


def build_stage(label: str, cmd: list[str], output_path: str) -> bool:
    """Build một stage, print progress, return True nếu thành công."""
    print(f"Building {label}...")
    print(f"  {' '.join(cmd)}")
    start = time.time()
    result = subprocess.run(cmd, capture_output=True, text=True)
    elapsed = time.time() - start
    if result.returncode != 0:
        print(f"  FAIL (elapsed: {elapsed:.1f}s)")
        print(result.stderr[:500] if result.stderr else result.stdout[:500])
        return False
    print(f"  OK (elapsed: {elapsed:.1f}s)")
    return True


def run_case(binary: str, src: str, tmp_dir: str, suffix: str) -> tuple[int, str, str]:
    """
    Compile src với binary, chạy output.
    Return (exit_code, stdout, error_message).
    error_message là empty string nếu thành công.
    """
    src_path = os.path.join(tmp_dir, f"test_{suffix}.dt")
    out_path = os.path.join(tmp_dir, f"test_{suffix}_out")
    with open(src_path, "w", encoding="utf-8") as handle:
        handle.write(src)

    build = subprocess.run(
        [binary, "build", src_path, "-o", out_path],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if build.returncode != 0:
        msg = (build.stderr or build.stdout or "").strip()
        return (-999, "", f"BUILD FAIL: {msg[:200]}")

    run = subprocess.run([out_path], capture_output=True, text=True, timeout=10)
    return (run.returncode, run.stdout, "")


def compare_case(desc, src, expected_exit, expected_stdout_contains, s1, s2, verbose):
    with tempfile.TemporaryDirectory() as tmp:
        ec1, out1, err1 = run_case(s1, src, tmp, "s1")
        ec2, out2, err2 = run_case(s2, src, tmp, "s2")

    if err1 and err2:
        if err1[:50] == err2[:50]:
            return True, "both compile failed consistently"
        return False, f"both failed but differently:\n  s1: {err1}\n  s2: {err2}"

    if err1 and not err2:
        return False, f"s1 compile failed, s2 succeeded:\n  s1: {err1}"
    if err2 and not err1:
        return False, f"s2 compile failed, s1 succeeded:\n  s2: {err2}"

    mismatches = []
    if ec1 != ec2:
        mismatches.append(f"exit code: s1={ec1}, s2={ec2}")
    if ec1 != expected_exit:
        mismatches.append(f"s1 exit mismatch: expected {expected_exit}, got {ec1}")
    if ec2 != expected_exit:
        mismatches.append(f"s2 exit mismatch: expected {expected_exit}, got {ec2}")
    if expected_stdout_contains and expected_stdout_contains not in out1:
        mismatches.append(
            f"s1 stdout missing '{expected_stdout_contains}': got '{out1.strip()}'"
        )
    if expected_stdout_contains and expected_stdout_contains not in out2:
        mismatches.append(
            f"s2 stdout missing '{expected_stdout_contains}': got '{out2.strip()}'"
        )
    if out1 != out2:
        mismatches.append(f"stdout mismatch:\n  s1: {repr(out1)}\n  s2: {repr(out2)}")

    if mismatches:
        return False, "\n  ".join(mismatches)
    return True, ""


def compare_help(s1, s2):
    r1 = subprocess.run([s1, "--help"], capture_output=True, text=True, timeout=10)
    r2 = subprocess.run([s2, "--help"], capture_output=True, text=True, timeout=10)
    out1 = (r1.stdout + r1.stderr).strip()
    out2 = (r2.stdout + r2.stderr).strip()
    if out1 != out2:
        return False, f"--help mismatch:\n  s1: {repr(out1[:100])}\n  s2: {repr(out2[:100])}"
    return True, ""


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--s1", default=None)
    parser.add_argument("--s2", default=None)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    s1 = args.s1 or "/tmp/draton_s1"
    s2 = args.s2 or "/tmp/draton_s2"

    if not args.skip_build:
        ok = build_stage(
            "stage 1 (drat Rust compiler)",
            [
                "cargo",
                "run",
                "-p",
                "drat",
                "--",
                "build",
                os.path.join(repo, "src/main.dt"),
                "-o",
                s1,
            ],
            s1,
        )
        if not ok:
            sys.exit(1)
        ok = build_stage(
            "stage 2 (stage 1 self-compiling)",
            [s1, "build", os.path.join(repo, "src/main.dt"), "-o", s2],
            s2,
        )
        if not ok:
            sys.exit(1)

    print("\nStage 2 verification")
    print(f"  s1: {s1}")
    print(f"  s2: {s2}\n")

    passed = 0
    failed = 0

    ok, msg = compare_help(s1, s2)
    if ok:
        print("[PASS] --help output")
        passed += 1
    else:
        print("[FAIL] --help output")
        if args.verbose or True:
            print(f"  {msg}")
        failed += 1

    for desc, src, expected_exit, expected_stdout in CASES:
        ok, msg = compare_case(desc, src, expected_exit, expected_stdout, s1, s2, args.verbose)
        if ok:
            print(f"[PASS] {desc}")
            passed += 1
        else:
            print(f"[FAIL] {desc}")
            print(f"  {msg}")
            failed += 1

    total = passed + failed
    print(f"\n{passed}/{total} cases passed")
    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
