#!/usr/bin/env python3
import subprocess, sys, os, tempfile

DRATON_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BUILD_DIR  = os.path.join(DRATON_DIR, "build/debug")

CORPUS = [
    ("identity fn",   "fn id(x: Int) -> Int { return x }"),
    ("binary add",    "fn add(a: Int, b: Int) -> Int { return a + b }"),
    ("let binding",   "fn main() { let x: Int = 42 }"),
    ("bool expr",     "fn main() { let b = true && false }"),
    ("array literal", "fn main() { let a: Array[Int] = [1, 2, 3] }"),
    ("for loop",      "fn main() { for i in [1, 2, 3] { println(int_to_string(i)) } }"),
    ("option type",   "fn main() { let x: Option[Int] = None }"),
    ("lambda",        "fn main() { let f = lambda x => x + 1 }"),
    ("match expr",    "fn main() { match 1 { 1 => true, _ => false } }"),
    ("class",         "class Foo { pub let x: Int }"),
]

def get_rust(source):
    with tempfile.NamedTemporaryFile(suffix=".dt", mode="w", delete=False) as f:
        f.write(source); fname = f.name
    try:
        r = subprocess.run(["cargo", "run", "-p", "drat", "--", "type-dump", fname],
            capture_output=True, text=True, cwd=DRATON_DIR,
            env={**os.environ, "DRATON_DISABLE_GCROOT": "1"})
        return r.stdout.strip()
    finally:
        os.unlink(fname)

def get_selfhost(source):
    binary = os.path.join(BUILD_DIR, "draton-selfhost-phase1")
    with tempfile.NamedTemporaryFile(suffix=".dt", mode="w", delete=False) as f:
        f.write(source); fname = f.name
    try:
        r = subprocess.run([binary, "type-dump", fname], capture_output=True, text=True, timeout=10)
        return r.stdout.strip()
    except subprocess.TimeoutExpired:
        return "TIMEOUT"
    finally:
        os.unlink(fname)

def main():
    passed = failed = 0
    for desc, source in CORPUS:
        rust_out = get_rust(source)
        self_out = get_selfhost(source)
        if rust_out == self_out:
            print(f"  PASS  {desc}")
            passed += 1
        else:
            print(f"  FAIL  {desc}")
            print(f"    expected: {rust_out[:100]}")
            print(f"    got:      {self_out[:100]}")
            failed += 1
    print(f"\n{passed}/{passed+failed} pass")
    sys.exit(0 if failed == 0 else 1)

if __name__ == "__main__":
    main()
