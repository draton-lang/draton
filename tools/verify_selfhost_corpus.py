#!/usr/bin/env python3
"""
Verify draton_selfhost against an expanded corpus.
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
            build_output = r1.stderr.strip()
            if not build_output:
                build_output = r1.stdout.strip()
            return (-1, f"BUILD FAIL: {build_output}")
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


# Exit-code cases

case(
    "constant 42",
    """fn main() -> Int { 42 }""",
    exit=42,
)

case(
    "arithmetic",
    """fn main() -> Int { 2 + 3 * 4 - 1 }""",
    exit=13,
)

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
    """fn main() -> Int {
    if true { 1 } else { 0 }
}""",
    exit=1,
)

case(
    "if-else false branch",
    """fn main() -> Int {
    if false { 1 } else { 2 }
}""",
    exit=2,
)

case(
    "nested if",
    """fn main() -> Int {
    let x = 5
    if x > 10 { 0 }
    elif x > 3 { 42 }
    else { 1 }
}""",
    exit=42,
)

case(
    "while loop",
    """fn main() -> Int {
    let mut i = 0
    let mut s = 0
    while i < 10 {
        s += i
        i++
    }
    s
}""",
    exit=45,
)

case(
    "for loop over range",
    """fn main() -> Int {
    let mut s = 0
    for i in 0..10 { s += i }
    s
}""",
    exit=45,
)

case(
    "recursion fibonacci",
    """fn fib(n: Int) -> Int {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() -> Int { fib(10) }""",
    exit=55,
)

# Class + field access

case(
    "class field read",
    """class Point {
    pub let x: Int
    pub let y: Int
}
fn main() -> Int {
    let p = Point { x: 3, y: 4 }
    p.x + p.y
}""",
    exit=7,
)

case(
    "class field mutation",
    """class Counter {
    pub let mut n: Int
}
fn main() -> Int {
    let c = Counter { n: 0 }
    c.n += 10
    c.n += 32
    c.n
}""",
    exit=42,
)

case(
    "class method",
    """class Rect {
    pub let w: Int
    pub let h: Int
    pub fn area(self) -> Int { self.w * self.h }
}
fn main() -> Int {
    let r = Rect { w: 6, h: 7 }
    r.area()
}""",
    exit=42,
)

case(
    "class inheritance field",
    """class Animal {
    pub let name: String
}
class Dog extends Animal {
    pub let breed: String
}
fn main() -> Int {
    let d = Dog { name: "Rex", breed: "Lab" }
    d.name.len()
}""",
    exit=3,
)

# Match exhaustive

case(
    "match int literal",
    """fn main() -> Int {
    let x = 2
    match x {
        1 => 10
        2 => 42
        _ => 0
    }
}""",
    exit=42,
)

case(
    "match bool",
    """fn main() -> Int {
    match true {
        true  => 1
        false => 0
    }
}""",
    exit=1,
)

case(
    "match enum variant",
    """enum Color { Red Green Blue }
fn color_code(c: Color) -> Int {
    match c {
        Color.Red   => 1
        Color.Green => 2
        Color.Blue  => 3
    }
}
fn main() -> Int { color_code(Color.Green) }""",
    exit=2,
)

case(
    "match nested",
    """fn classify(n: Int) -> Int {
    match n {
        0 => 0
        1 => 1
        _ => match n > 0 {
            true  => 2
            false => 3
        }
    }
}
fn main() -> Int { classify(5) }""",
    exit=2,
)

# Closure + capture

case(
    "closure basic",
    """fn apply(f: fn(Int) -> Int, x: Int) -> Int { f(x) }
fn main() -> Int {
    let double = lambda x => x * 2
    apply(double, 21)
}""",
    exit=42,
)

case(
    "closure captures local",
    """fn make_adder(n: Int) -> fn(Int) -> Int {
    lambda x => x + n
}
fn main() -> Int {
    let add10 = make_adder(10)
    add10(32)
}""",
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

# Generic fn/class

case(
    "generic identity fn",
    """fn id[T](x: T) -> T { x }
fn main() -> Int { id(42) }""",
    exit=42,
)

case(
    "generic swap",
    """fn swap[T](a: T, b: T) -> (T, T) { (b, a) }
fn main() -> Int {
    let (x, y) = swap(1, 42)
    y
}""",
    exit=42,
)

case(
    "generic max",
    """fn max_val[T](a: T, b: T) -> T {
    if a > b { a } else { b }
}
fn main() -> Int { max_val(17, 42) }""",
    exit=42,
)

case(
    "generic class Stack",
    """class Stack[T] {
    pub let mut items: Array[T]
    pub fn push(self, v: T) { self.items.push(v) }
    pub fn pop(self) -> T   { self.items.pop() }
    pub fn size(self) -> Int { self.items.len() }
}
fn main() -> Int {
    let s = Stack[Int] { items: [] }
    s.push(10)
    s.push(32)
    s.pop() + s.pop()
}""",
    exit=42,
)

# Interface dispatch

case(
    "interface basic",
    """interface Shape {
    fn area(self) -> Int
}
class Square implements Shape {
    pub let side: Int
    pub fn area(self) -> Int { self.side * self.side }
}
fn print_area(s: Shape) -> Int { s.area() }
fn main() -> Int {
    let sq = Square { side: 6 }
    print_area(sq)
}""",
    exit=36,
)

case(
    "interface two impls",
    """interface Greet {
    fn hello(self) -> Int
}
class En implements Greet {
    pub fn hello(self) -> Int { 1 }
}
class Vi implements Greet {
    pub fn hello(self) -> Int { 2 }
}
fn call(g: Greet) -> Int { g.hello() }
fn main() -> Int {
    call(En {}) + call(Vi {})
}""",
    exit=3,
)

# String + fstring

case(
    "string len",
    """fn main() -> Int {
    let s = "hello"
    s.len()
}""",
    exit=5,
)

case(
    "string concat stdout",
    """fn main() {
    let a = "foo"
    let b = "bar"
    println(a + b)
}""",
    stdout="foobar\n",
)

case(
    "fstring stdout",
    """fn main() {
    let name = "Draton"
    let ver  = 1
    println(f"hello {name} v{ver}")
}""",
    stdout="hello Draton v1\n",
)

case(
    "fstring with expr",
    """fn main() {
    let x = 6
    let y = 7
    println(f"{x} * {y} = {x * y}")
}""",
    stdout="6 * 7 = 42\n",
)

case(
    "string comparison stdout",
    """fn main() {
    let s = "ok"
    if s == "ok" { println("yes") } else { println("no") }
}""",
    stdout="yes\n",
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
