#!/usr/bin/env python3
"""
Verify draton_selfhost against an executable corpus that matches the
currently supported self-host surface.
Each test case specifies: source code, expected exit code, expected stdout.
"""

import os
import subprocess
import sys
import tempfile

SELFHOST = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "draton_selfhost",
)


def run_case(source: str) -> tuple[int, str]:
    with tempfile.TemporaryDirectory() as tmp:
        src = os.path.join(tmp, "test.dt")
        out = os.path.join(tmp, "test_out")
        with open(src, "w", encoding="utf-8") as handle:
            handle.write(source)
        r1 = subprocess.run(
            [SELFHOST, "build", src, "-o", out],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        if r1.returncode != 0:
            message = r1.stderr.strip()
            if not message:
                message = r1.stdout.strip()
            return (-1, f"BUILD FAIL: {message}")
        r2 = subprocess.run(
            [out],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        return (r2.returncode, r2.stdout)


CORPUS: list[tuple[str, str, int, str]] = []


def case(desc, src, exit=0, stdout=""):
    CORPUS.append((desc, src, exit, stdout))


case("constant 42", """fn main() -> Int { 42 }""", exit=42)

case("arithmetic", """fn main() -> Int { 2 + 3 * 4 - 1 }""", exit=13)

case(
    "let binding",
    """fn main() -> Int {
    let x = 10
    let y = 32
    x + y
}""",
    exit=42,
)

case(
    "if-else true branch",
    """fn main() {
    let mut out = 0
    if (true) { out = 1 } else { out = 2 }
    out
}""",
    exit=1,
)

case(
    "if-else false branch",
    """fn main() {
    let mut out = 0
    if (false) { out = 1 } else { out = 2 }
    out
}""",
    exit=2,
)

case(
    "nested if",
    """fn main() {
    let x = 5
    let mut out = 0
    if (x > 10) { out = 0 } else if (x > 3) { out = 42 } else { out = 1 }
    out
}""",
    exit=42,
)

case(
    "while loop",
    """fn main() {
    let mut i = 0
    let mut s = 0
    while (i < 10) {
        s += i
        i++
    }
    s
}""",
    exit=45,
)

case(
    "for loop over range",
    """fn main() {
    let mut s = 0
    for i in 0..10 { s += i }
    s
}""",
    exit=45,
)

case(
    "recursion factorial",
    """fn fact(n: Int) -> Int {
    let mut out = 1
    if (n <= 1) { out = 1 } else { out = n * fact(n - 1) }
    out
}
fn main() -> Int { fact(5) }""",
    exit=120,
)

case(
    "bool logic",
    """fn main() -> Int {
    let a = true
    let b = false
    let mut out = 0
    if (a && !b) { out = 1 } else { out = 0 }
    out
}""",
    exit=1,
)

case(
    "comparisons",
    """fn main() -> Int {
    let mut out = 0
    if (3 < 4 && 5 >= 5) { out = 1 } else { out = 0 }
    out
}""",
    exit=1,
)

case(
    "match int literal",
    """fn main() {
    match 2 {
        1 => 10
        2 => 42
        _ => 0
    }
}""",
    exit=42,
)

case(
    "match bool",
    """fn main() {
    match true {
        true  => 1
        false => 0
    }
}""",
    exit=1,
)

case(
    "match nested",
    """fn classify(n: Int) {
    match n {
        0 => 0
        1 => 1
        _ => match (n > 0) {
            true  => 2
            false => 3
        }
    }
}
fn main() { classify(5) }""",
    exit=2,
)

case(
    "closure basic",
    """fn main() {
    let double = lambda x => x * 2
    double(21)
}""",
    exit=42,
)

case(
    "closure captures local",
    """fn main() -> Int {
    let y = 10
    let addY = lambda x => x + y
    addY(5)
}""",
    exit=15,
)

case(
    "closure apply",
    """fn apply(f, x) { f(x) }
fn main() { apply(lambda x => x + 1, 41) }""",
    exit=42,
)

case(
    "closure in loop",
    """fn main() -> Int {
    let mut total = 0
    let add = lambda x => x + 1
    for i in 0..5 { total += add(i) }
    total
}""",
    exit=15,
)

case(
    "nested calls",
    """fn add(a: Int, b: Int) -> Int { a + b }
fn mul(a: Int, b: Int) -> Int { a * b }
fn main() -> Int { add(mul(2, 3), 7) }""",
    exit=13,
)

case(
    "float cast to int",
    """fn main() {
    let x = 3.75 as Int
    x
}""",
    exit=3,
)

case(
    "loop multiply",
    """fn main() {
    let mut x = 1
    let mut i = 0
    while (i < 4) {
        x = x * 2
        i++
    }
    x
}""",
    exit=16,
)

case(
    "array literal runtime",
    """fn main() {
    let xs = [1, 2, 3]
    3
}""",
    exit=3,
)

case(
    "print literal stdout",
    """fn main() {
    print("hello")
    0
}""",
    stdout="hello\n",
)

case(
    "fstring int stdout",
    """fn main() {
    let x = 6
    let y = 7
    print(f"{x} * {y} = {x * y}")
    0
}""",
    stdout="<Int> * <Int> = <Int>\n",
)

case(
    "fstring string stdout",
    """fn main() {
    let name = "Draton"
    print(f"hi {name}")
    0
}""",
    stdout="hi <String>\n",
)


def main() -> int:
    passed = failed = 0
    for (desc, src, exp_exit, exp_stdout) in CORPUS:
        try:
            got_exit, got_stdout = run_case(src)
        except subprocess.TimeoutExpired:
            print(f"  TIMEOUT  {desc}")
            failed += 1
            continue

        ok = True
        if got_exit != exp_exit:
            print(f"  FAIL     {desc}")
            print(f"    exit: expected {exp_exit}, got {got_exit}")
            if got_exit == -1:
                print(f"    {got_stdout}")
            ok = False
        if exp_stdout and got_stdout != exp_stdout:
            if ok:
                print(f"  FAIL     {desc}")
            print(f"    stdout: expected {repr(exp_stdout)}, got {repr(got_stdout)}")
            ok = False
        if ok:
            print(f"  PASS     {desc}")
            passed += 1
        else:
            failed += 1

    print(f"\n{passed}/{passed + failed} pass")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
