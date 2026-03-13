#!/usr/bin/env python3
"""Verify self-host parser phase 2 against Rust reference."""

import os
import subprocess
import sys
import tempfile

CORPUS = [
    ("empty fn", "fn main() {}"),
    ("fn with return", "fn add(a: Int, b: Int) -> Int { return a + b }"),
    ("let stmt", "fn main() { let x = 42 }"),
    ("if stmt", "fn main() { if x { return 1 } else { return 0 } }"),
    ("for loop", "fn main() { for i in 0..10 { print(i) } }"),
    ("while loop", "fn main() { while true { break } }"),
    ("class def", "class Foo { pub let x: Int }"),
    ("match expr", "fn main() { match x { 1 => true, _ => false } }"),
    ("lambda", "fn main() { let f = lambda x => x + 1 }"),
    ("fstring", 'fn main() { let s = f"hello {name}" }'),
    ("import", "import { foo, bar as baz }"),
    ("enum def", "enum Color { Red Green Blue }"),
    ("interface def", "interface Drawable { fn draw(self) }"),
    ("binary ops", "fn main() { let x = 1 + 2 * 3 - 4 / 2 }"),
    ("array literal", "fn main() { let a = [1, 2, 3] }"),
    ("map literal", 'fn main() { let m = {"a": 1, "b": 2} }'),
    ("method call", "fn main() { x.foo(1, 2) }"),
    ("nested if", "fn main() { if a { 1 } else if b { 2 } else { 3 } }"),
]

DRATON_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BUILD_DIR = os.path.join(DRATON_DIR, "build/debug")


def get_rust_ast_dump(source: str) -> str:
    with tempfile.NamedTemporaryFile(suffix=".dt", mode="w", delete=False) as handle:
        handle.write(source)
        path = handle.name
    try:
        result = subprocess.run(
            ["cargo", "run", "-p", "drat", "--", "ast-dump", path],
            capture_output=True,
            text=True,
            cwd=DRATON_DIR,
            env={**os.environ, "DRATON_DISABLE_GCROOT": "1"},
            check=False,
        )
        return result.stdout.strip()
    finally:
        os.unlink(path)


def get_selfhost_ast_dump(source: str) -> str:
    binary = os.path.join(BUILD_DIR, "draton-selfhost-phase1")
    with tempfile.NamedTemporaryFile(suffix=".dt", mode="w", delete=False) as handle:
        handle.write(source)
        path = handle.name
    try:
        result = subprocess.run(
            [binary, "ast-dump", path],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        return result.stdout.strip()
    except subprocess.TimeoutExpired:
        return "TIMEOUT"
    finally:
        os.unlink(path)


def main() -> int:
    passed = 0
    failed = 0
    for desc, source in CORPUS:
        rust_out = get_rust_ast_dump(source)
        self_out = get_selfhost_ast_dump(source)
        if rust_out == self_out:
            print(f"  PASS  {desc}")
            passed += 1
        else:
            print(f"  FAIL  {desc}")
            print(f"    expected: {rust_out[:80]}")
            print(f"    got:      {self_out[:80]}")
            failed += 1
    print(f"\n{passed}/{passed + failed} pass")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
