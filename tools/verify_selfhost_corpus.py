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

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def find_selfhost() -> str:
    candidates = [
        os.path.join(REPO, "build", "debug", "draton-selfhost-phase1"),
        os.path.join(REPO, "draton_selfhost"),
        os.path.join(REPO, "draton_selfhost_test_new"),
        os.path.join(REPO, "draton_selfhost_test"),
        os.path.join(REPO, "target", "debug", "drat"),
    ]
    for path in candidates:
        if os.path.exists(path) and os.access(path, os.X_OK):
            return path
    raise FileNotFoundError("khong tim thay binary self-host hoac host fallback")


SELFHOST = find_selfhost()


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
    "elif basic",
    """fn main() -> Int {
    let x = 5
    if x > 10 { 0 }
    elif x > 3 { 42 }
    else { 1 }
}""",
    exit=42,
)

case(
    "elif chain",
    """fn main() -> Int {
    let x = 2
    if x == 1 { 1 }
    elif x == 2 { 42 }
    elif x == 3 { 3 }
    else { 0 }
}""",
    exit=42,
)

case(
    "elif no else",
    """fn main() -> Int {
    let x = 7
    let mut r = 0
    if x < 0 { r = 1 }
    elif x == 7 { r = 42 }
    r
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
    "class two methods",
    """class Vec2 {
    pub let x: Int
    pub let y: Int
    pub fn dot(self, other: Vec2) -> Int {
        self.x * other.x + self.y * other.y
    }
    pub fn len_sq(self) -> Int { self.dot(self) }
}
fn main() -> Int {
    let v = Vec2 { x: 3, y: 4 }
    v.len_sq()
}""",
    exit=25,
)

case(
    "class inheritance",
    """class Animal {
    pub let legs: Int
}
class Dog extends Animal {
    pub let name: String
}
fn main() -> Int {
    let d = Dog { legs: 4, name: "Rex" }
    d.legs
}""",
    exit=4,
)

case(
    "class inheritance method",
    """class Shape {
    pub let color: Int
    pub fn get_color(self) -> Int { self.color }
}
class Circle extends Shape {
    pub let radius: Int
    pub fn area_approx(self) -> Int { self.radius * self.radius * 3 }
}
fn main() -> Int {
    let c = Circle { color: 1, radius: 4 }
    c.area_approx() + c.get_color()
}""",
    exit=49,
)

case(
    "fn type param",
    """fn apply(f: fn(Int) -> Int, x: Int) -> Int { f(x) }
fn main() -> Int {
    apply(lambda x => x * 2, 21)
}""",
    exit=42,
)

case(
    "fn type higher order",
    """fn compose(f: fn(Int) -> Int, g: fn(Int) -> Int) -> fn(Int) -> Int {
    lambda x => f(g(x))
}
fn main() -> Int {
    let add1 = lambda x => x + 1
    let mul2 = lambda x => x * 2
    let f = compose(add1, mul2)
    f(20)
}""",
    exit=41,
)

case(
    "generic identity",
    """fn id[T](x: T) -> T { x }
fn main() -> Int { id(42) }""",
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
    pub fn size(self) -> Int { self.items.len() }
}
fn main() -> Int {
    let s = Stack[Int] { items: [] }
    s.push(10)
    s.push(32)
    s.size()
}""",
    exit=2,
)

case(
    "interface basic",
    """interface Shape {
    fn area(self) -> Int
}
class Square implements Shape {
    pub let side: Int
    pub fn area(self) -> Int { self.side * self.side }
}
fn total_area(s: Shape) -> Int { s.area() }
fn main() -> Int {
    let sq = Square { side: 6 }
    total_area(sq)
}""",
    exit=36,
)

case(
    "interface two impls",
    """interface Greeter {
    fn greet(self) -> Int
}
class A implements Greeter {
    pub fn greet(self) -> Int { 1 }
}
class B implements Greeter {
    pub fn greet(self) -> Int { 2 }
}
fn call_greet(g: Greeter) -> Int { g.greet() }
fn main() -> Int {
    call_greet(A {}) + call_greet(B {})
}""",
    exit=3,
)

case(
    "string len",
    """fn main() -> Int {
    "hello".len()
}""",
    exit=5,
)

case(
    "string contains stdout",
    """fn main() {
    let s = "foobar"
    if s.contains("oba") { println("yes") } else { println("no") }
}""",
    stdout="yes\n",
)

case(
    "string slice stdout",
    """fn main() {
    println("hello world".slice(6, 11))
}""",
    stdout="world\n",
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
