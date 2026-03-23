---
title: Inferred Ownership Spec
sidebar_position: 31
---

# Inferred Ownership Spec

This document defines Draton's compile-time memory management model for safe code. It replaces tracing GC ownership with an inferred ownership pass that runs after Hindley-Milner type inference and before LLVM lowering.

The model is intentionally conservative:

- users write no ownership syntax
- the compiler decides `copy`, `borrow`, `exclusive borrow`, or `move` at each use site
- the compiler inserts generated `free()` calls after the last owned use
- any case that could create multiple long-lived owners, an unresolved escape, or an ownership cycle is a compile error
- `@pointer`, `@unsafe`, and `@asm` remain the explicit escape hatches

In this document, `free(x)` means generated destruction of `x`. For aggregate owners, that destruction first frees every owned child and then frees the storage for `x` itself. There is no user-written `free` in safe Draton code.

The analysis tracks every non-`Copy` binding through these ownership states:

- `owned`: the current scope has the only owning path to the value
- `borrowed(shared)`: one or more read-only uses are active
- `borrowed(exclusive)`: one mutating use is active
- `moved`: ownership left the binding and the binding cannot be read again until it is reassigned
- `escaped`: ownership left the current function or crossed into an unsafe/raw boundary

## 1. Value categories

Draton divides values into `Copy` values and move-by-default values.

### 1.1 `Copy` values

A type is `Copy` when duplicating its bits cannot create aliasing or double-free.

`Copy` types are:

- `Bool`
- signed and unsigned integers
- floating-point numbers
- `Char`
- `Byte`
- `Unit`
- `@pointer`
- named function items with no closure environment
- enums with no payload
- tuples, rows, `Option[T]`, and `Result[T, E]` when every contained type is `Copy` and the total ABI size is at most two target machine words

User `class` values are never `Copy`, even when every field is `Copy`. Classes carry identity and are always heap owners in safe Draton.

Valid:

```draton
fn main() {
    let a = 1
    let b = a
    print(a)
    print(b)
}
```

Compiler does:

- classify `Int` as `Copy`
- duplicate `a` into `b`
- keep both bindings usable
- insert no `free()`

Valid:

```draton
fn main() {
    let p = external_ptr()
    let q = p
    use_ptr(p)
    use_ptr(q)
}
```

Compiler does:

- classify `@pointer` as `Copy`
- duplicate the raw pointer bits
- perform no lifetime checking for the pointee

### 1.2 Move-by-default values

A type is move-by-default when duplicating it would duplicate ownership.

Move-by-default types are:

- `String`
- `Array[T]`
- `Map[K, V]`
- `Set[T]`
- `Chan[T]`
- all user `class` values
- all closure values
- enums with payloads when any payload field is move-by-default
- tuples, rows, `Option[T]`, and `Result[T, E]` when any contained type is move-by-default or the total ABI size is more than two target machine words

Valid:

```draton
fn main() {
    let name = input("name: ")
    print(name.len())
}
```

Compiler does:

- classify `String` as move-by-default
- keep `name` as a single owner
- infer a shared borrow for `name.len()`
- insert `free(name)` after the `print` call because that is the last use

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let other = name
    print(name)
}
```

Compiler does:

- move ownership of `name` into `other`
- mark `name` as moved
- reject `print(name)` because `name` no longer owns a value

## 2. Move semantics

A move transfers ownership from one owner to another. Moves happen only for move-by-default values.

### 2.1 Bindings

`let b = a` copies when `a` is `Copy` and moves when `a` is move-by-default.

Valid:

```draton
fn main() {
    let name = input("name: ")
    let other = name
    print(other.len())
}
```

Compiler does:

- move ownership from `name` to `other`
- mark `name` as moved
- infer a shared borrow for `other.len()`
- insert `free(other)` after the last use

If a mutable binding is moved, it becomes uninitialized until it is reassigned.

Valid:

```draton
fn persist(text) {
    save_text(text)
    return ()
}

fn main() {
    let mut name = input("name: ")
    persist(name)
    name = input("name again: ")
    print(name.len())
}
```

Compiler does:

- infer that `persist` moves `text`
- move `name` into `persist`
- allow `name = ...` because `name` is `mut`
- insert `free(name)` after the final `print`

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let other = name
    print(name.len())
}
```

Compiler does:

- move `name` into `other`
- reject `name.len()` as a use after move

### 2.2 Function and method calls

Every parameter of every safe Draton function is inferred into one of four effects:

- `copy`
- `borrow(shared)`
- `borrow(exclusive)`
- `move`

The compiler infers the strongest effect needed by any reachable path through the function body. The effect order is:

`copy < borrow(shared) < borrow(exclusive) < move`

That means:

- a parameter that is only read becomes `borrow(shared)`
- a parameter that is mutated becomes `borrow(exclusive)`
- a parameter that is returned, stored, captured by an escaping closure, reassigned into another owner, or passed to a `move` parameter becomes `move`
- if different paths require different effects, the strongest one wins

### 2.2.1 Recursive summary inference

Recursive calls use the same summary inference, but the compiler solves all functions in a recursive cycle together.

The rule is:

- each parameter in a recursive strongly connected component starts at `copy`
- the compiler repeatedly analyzes every function in that component
- every recursive call edge uses the current summary of the callee parameter
- summaries keep rising through `copy < borrow(shared) < borrow(exclusive) < move` until they stop changing

For a function that recursively calls itself with a move-by-default argument:

- the recursive call counts as `move` if the converged parameter summary is `move`
- the recursive call counts as `borrow(shared)` or `borrow(exclusive)` if the converged summary is a borrow
- once the summary converges to `move`, every recursive edge in that function consumes the argument in the current frame

For mutual recursion between two functions:

- the compiler infers both summaries in one step over the same recursive component
- ownership requirements flow around the whole cycle, not one function at a time
- if either function forces a parameter to `move`, every call edge in that cycle to the matching parameter is treated as `move` after convergence

Valid:

```draton
fn pass_down(text, n) {
    if n == 0 {
        return text
    }
    return pass_down(text, n - 1)
}

fn main() {
    let name = input("name: ")
    let out = pass_down(name, 3)
    print(out.len())
}
```

Compiler does:

- infer `pass_down(text)` as `move` because the base case returns `text`
- treat the recursive call `pass_down(text, n - 1)` as a move of `text` into the next frame
- allow the function because the current frame does not use `text` after that recursive move
- insert `free(out)` after the last use in `main`

Invalid:

```draton
fn first(text, n) {
    if n == 0 {
        return text
    }
    return second(text, n - 1)
}

fn second(text, n) {
    let out = first(text, n)
    print(text.len())
    return out
}
```

Compiler does:

- infer `first(text)` and `second(text)` together because they are mutually recursive
- raise both summaries to `move` because `first` returns `text`
- treat `first(text, n)` inside `second` as a move
- reject `print(text.len())` because `text` was moved into the recursive call

Valid:

```draton
fn show(text) {
    print(text.len())
    return ()
}

fn main() {
    let name = input("name: ")
    show(name)
    show(name)
}
```

Compiler does:

- infer `show(text)` as `borrow(shared)`
- pass `name` by borrow on both calls
- keep `name` owned by `main`
- insert `free(name)` after the second call

Valid:

```draton
fn forward(text) {
    return text
}

fn main() {
    let name = input("name: ")
    let out = forward(name)
    print(out.len())
}
```

Compiler does:

- infer `forward(text)` as `move`
- move `name` into `forward`
- receive the returned owner into `out`
- insert `free(out)` after the last use

Invalid:

```draton
fn forward(text) {
    return text
}

fn main() {
    let name = input("name: ")
    let out = forward(name)
    print(name.len())
}
```

Compiler does:

- infer `forward(text)` as `move`
- move `name` into `forward`
- reject the later use of `name`

### 2.3 Assignment and overwrite

Assignment `a = b` uses the same rule as `let a = b`:

- copy when `b` is `Copy`
- move when `b` is move-by-default

Before overwriting `a`, the compiler frees the old value in `a` if `a` still owns one and that value has not escaped.

Valid:

```draton
fn main() {
    let mut name = input("name: ")
    name = input("new name: ")
    print(name.len())
}
```

Compiler does:

- insert `free(name)` for the old first string immediately before the overwrite
- store the new owner into `name`
- insert `free(name)` again after the final use

### 2.4 No partial moves in safe code

Safe Draton never leaves an aggregate partially moved.

- reading a `Copy` field copies that field
- reading a move-by-default field borrows through the parent unless the entire parent is moved in the same operation
- moving one field out of a live class, tuple, array slot, or map entry is rejected

Valid:

```draton
class Point {
    let x
    let y

    @type {
        x: Int
        y: Int
    }
}

fn main() {
    let p = Point { x: 1, y: 2 }
    let x = p.x
    print(x)
    print(p.y)
}
```

Compiler does:

- borrow `p` to read `x`
- copy `x` because `Int` is `Copy`
- keep `p` owned and usable
- insert `free(p)` after its last use

Invalid:

```draton
class User {
    let name

    @type {
        name: String
    }
}

fn main() {
    let user = User { name: input("name: ") }
    let name = user.name
    print(user)
}
```

Compiler does:

- see that `user.name` is move-by-default
- reject the field extraction because it would leave `user` partially moved

## 3. Borrow semantics

Borrowing gives temporary access without transferring ownership.

Draton infers two borrow kinds:

- `borrow(shared)` for read-only access
- `borrow(exclusive)` for mutating access

There is no user-written borrow syntax. Borrowing is inferred from use.

### 3.1 When the compiler infers a borrow

The compiler infers `borrow(shared)` when a use site only reads:

- passing an argument to a parameter inferred as `borrow(shared)`
- calling a method whose receiver is only read
- reading a field or computing `len()`
- matching on a value when every arm only inspects it

Valid:

```draton
fn show(text) {
    print(text.len())
    return ()
}

fn main() {
    let name = input("name: ")
    show(name)
    print(name.len())
}
```

Compiler does:

- borrow `name` for `show`
- end that borrow when `show` returns
- borrow `name` again for `name.len()`
- insert `free(name)` after the final use

The compiler infers `borrow(exclusive)` when a use site may mutate:

- passing an argument to a parameter inferred as `borrow(exclusive)`
- calling a mutating method on a move-by-default receiver
- assigning through a field or index into an owned aggregate

Valid:

```draton
fn append(items, value) {
    items.push(value)
    return ()
}

fn main() {
    @type {
        items: Array[String]
    }
    let mut items = []
    append(items, "a")
    append(items, "b")
    print(items.len())
}
```

Compiler does:

- infer `append(items, value)` as `borrow(exclusive)` for `items` and `move` for `value`
- borrow `items` exclusively for each call
- end each exclusive borrow when the call returns
- move each string into the array
- insert `free(items)` after the final `print`, which recursively frees both stored strings

### 3.2 Borrow lifetime

A borrow begins at the borrow site and ends at the earliest program point where the borrowed access is no longer needed.

This is flow-sensitive, not purely lexical.

- a borrow for a call argument ends when the call returns
- a borrow for `x.field` or `x.len()` ends when that expression finishes
- a borrow used in a branch ends at the end of the last reachable use inside that branch
- a borrow captured by a non-escaping closure ends after the closure's last call

Valid:

```draton
fn main() {
    let name = input("name: ")
    print(name.len())
    save_text(name)
}
```

Compiler does:

- infer a shared borrow for `name.len()`
- end the borrow after `print(...)`
- allow `save_text(name)` to move `name` after the borrow ends

### 3.3 Coexisting borrows

Multiple shared borrows may overlap. An exclusive borrow may not overlap with any other borrow or move.

Valid:

```draton
fn compare(a, b) {
    return a.len() == b.len()
}

fn main() {
    let name = input("name: ")
    let same = compare(name, name)
    print(same)
    print(name.len())
}
```

Compiler does:

- infer both arguments of `compare` as shared borrows
- allow the same owner to be borrowed twice for one call
- keep `name` usable after the call

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let reader = lambda => name.len()
    save_text(name)
    print(reader())
}
```

Compiler does:

- infer that `reader` holds a shared borrow of `name` until `reader`'s last call
- reject `save_text(name)` because a move cannot overlap that borrow

Invalid:

```draton
fn append(items, value) {
    items.push(value)
    return ()
}

fn main() {
    @type {
        items: Array[String]
    }
    let mut items = []
    let count = lambda => items.len()
    append(items, "x")
    print(count())
}
```

Compiler does:

- infer that `count` holds a shared borrow of `items` until `count()` is no longer reachable
- infer `append(items, value)` as an exclusive borrow of `items`
- reject the `append` call because shared and exclusive borrows would overlap

## 4. Last-use analysis

The compiler inserts `free()` by flow-sensitive last-use analysis on the typed control-flow graph of each function.

### 4.1 Analysis rule

For every move-by-default local binding:

1. build a control-flow graph with explicit edges for `if`, `elif`, `else`, `match`, loops, `return`, `break`, and `continue`
2. mark every use as `borrow(shared)`, `borrow(exclusive)`, `move`, or `escape`
3. compute whether the owner is live on each outgoing edge
4. insert `free()` on every edge where the owner stops being live and has not already moved or escaped

The free point is edge-based, not statement-based. That is what makes branch-local frees correct.

### 4.2 Straight-line code

Valid:

```draton
fn main() {
    let name = input("name: ")
    print(name.len())
}
```

Compiler does:

- mark `print(name.len())` as the last use of `name`
- insert `free(name)` immediately after the `print`

### 4.3 Branching

Valid:

```draton
fn main(flag) {
    let name = input("name: ")
    if flag {
        print(name.len())
    } else {
        save_text(name)
    }
    return ()
}
```

Compiler does:

- in the `then` branch, borrow `name` and insert `free(name)` at the end of that branch
- in the `else` branch, move `name` into `save_text`
- insert no merge-point `free()` because each branch already resolves ownership

Valid:

```draton
fn main(flag) {
    let name = input("name: ")
    if flag {
        print("yes")
    } else {
        print("no")
    }
    print(name.len())
}
```

Compiler does:

- keep `name` live across both branches
- insert no branch-local `free()`
- insert `free(name)` after the final `print`

### 4.4 Match

`match` works the same way. Each arm is analyzed separately and then joined.

Valid:

```draton
fn main(flag) {
    let name = input("name: ")
    match flag {
        true => {
            print(name.len())
        }
        false => {
            save_text(name)
        }
    }
    return ()
}
```

Compiler does:

- free `name` at the end of the `true` arm
- move `name` in the `false` arm
- insert no later `free()`

If one arm moves a value and another arm only borrows it, the whole `match` still consumes ownership on the move arm only. The join point sees the binding as dead after the `match`.

### 4.5 Early return

Before every `return`, the compiler frees all still-owned locals that do not escape in the returned value.

Valid:

```draton
fn main(flag) {
    let name = input("name: ")
    if flag {
        return 0
    }
    print(name.len())
    return 1
}
```

Compiler does:

- insert `free(name)` on the `return 0` path
- insert `free(name)` after the final `print` on the fallthrough path

Valid:

```draton
fn forward() {
    let name = input("name: ")
    return name
}
```

Compiler does:

- treat `return name` as an escape to the caller
- insert no local `free(name)`

### 4.6 Loops

Loop back-edges are part of liveness. A value cannot be moved inside a loop body if control may reach the next iteration and the value has not been reassigned.

Invalid:

```draton
fn main(items) {
    let mut name = input("name: ")
    while items.len() > 0 {
        save_text(name)
    }
}
```

Compiler does:

- see that `name` is moved in the first iteration
- see that the loop may execute again without reinitializing `name`
- reject the program

## 5. Escape analysis

Escape analysis decides whether ownership remains local, leaves the current owner, or leaves the current function entirely.

### 5.1 A value escapes the current function when

- it is returned
- it is stored into a value that later escapes
- it is captured by a closure that later escapes
- it is converted to raw form and passed beyond the safe boundary

### 5.2 A value escapes the current owner when

- it is moved to another local binding
- it is passed to a `move` parameter
- it is stored into a field, array, map, set, or closure environment

### 5.3 Return from function

Return is always an escape from the current function.

Valid:

```draton
fn make_name() {
    let name = input("name: ")
    return name
}
```

Compiler does:

- move `name` to the caller
- mark `name` as escaped
- insert no local `free()`

### 5.4 Pass to another function

Passing a value to another function is:

- a borrow when the callee summary is `borrow(shared)` or `borrow(exclusive)`
- a move when the callee summary is `move`
- a copy when the value category is `Copy`

For safe Draton functions, summaries are inferred to a fixed point over each strongly connected component of the call graph.

For calls through function values:

- if the callee set is closed and known, the compiler joins the candidate summaries with the same effect order used for direct calls
- if the callee set is open or unknown, a binding-level `@type` effect contract may declare how that function value treats its non-`Copy` argument
- if the callee set is open or unknown and no effect contract is present, non-`Copy` arguments are rejected in safe code

For an open higher-order callee, the supported `@type` effect contracts are:

- `name: (T) -> borrow`
- `name: (T) -> move`

These are ownership-effect summaries for function-value bindings, not ordinary return-type contracts.

- `borrow` means the call site applies `borrow(shared)` to the argument
- `move` means the call site moves the argument into the callee
- the compiler trusts the declared effect at the call site instead of trying to infer it from an unknown implementation
- if the effect is omitted, the current rejection rule remains in force
- open-callee exclusive mutation is still rejected unless the callee set is closed and known

Valid:

```draton
fn show(text) {
    print(text.len())
    return ()
}

fn save(text) {
    store(text)
    return ()
}

fn main() {
    let a = input("a: ")
    let b = input("b: ")
    show(a)
    save(b)
}
```

Compiler does:

- borrow `a`
- move `b`
- insert `free(a)` after its last use
- insert no local `free(b)` because ownership moved into `save`

Valid:

```draton
fn show(text) {
    print(text.len())
    return ()
}

fn run(op, text) {
    @type {
        op: (String) -> borrow
    }
    op(text)
    print(text.len())
}

fn main() {
    let name = input("name: ")
    run(show, name)
}
```

Compiler does:

- treat `op` as an open higher-order callee
- read `@type { op: (String) -> borrow }` as a trusted call-site effect summary
- borrow `text` for `op(text)` instead of rejecting the call
- allow the later `print(text.len())`
- keep `name` owned by `main` and insert `free(name)` after its last use

Invalid:

```draton
fn save(text) {
    store(text)
    return ()
}

fn show(text) {
    print(text.len())
    return ()
}

fn run(op, text) {
    op(text)
    print(text.len())
}
```

Compiler does:

- see that `op` may be a borrower or a mover
- reject the call for non-`Copy` `text` because the call cannot be classified deterministically

### 5.5 Capture in closure

Closure capture is:

- a borrow when the closure is proven non-escaping
- a move when the closure escapes its defining region

The full closure rules are defined in section 7.

### 5.6 Store in collection or object

Storing a move-by-default value into a collection or class field always moves ownership into that container slot.

If the container stays local, the value does not escape the function. It is freed when the container is freed.

If the container escapes, the stored value escapes with it.

Valid:

```draton
fn main() {
    @type {
        items: Array[String]
    }
    let mut items = []
    let name = input("name: ")
    items.push(name)
    print(items.len())
}
```

Compiler does:

- move `name` into `items[0]`
- mark `name` as moved
- free the stored string when `items` is freed

Invalid:

```draton
fn main() {
    @type {
        items: Array[String]
    }
    let mut items = []
    let name = input("name: ")
    items.push(name)
    print(name.len())
}
```

Compiler does:

- move `name` into `items`
- reject the later use of `name`

## 6. Aliasing rules

Safe Draton allows temporary borrow aliasing, not long-lived owner aliasing.

### 6.1 What counts as an alias

Two paths are aliases when they can both reach the same move-by-default object after the same sequence point and at least one path could outlive a single expression.

Examples of aliasing:

- two live local bindings that both own the same object
- one live local owner and one escaping closure capture of the same object
- the same object stored in two containers
- a safe owner that remains live while raw code also keeps a usable alias

Non-aliases:

- separate copies of `Copy` values
- multiple shared borrows inside one borrow region
- a temporary borrow that ends before the next owner action

### 6.2 Detection rule

Every move-by-default allocation gets one ownership origin.

The compiler tracks:

- which binding currently owns that origin
- which borrow regions are active for that origin
- whether the origin is stored in exactly one parent container

The following are rejected:

- more than one live owner for the same origin
- a move while a borrow is live
- a borrow that starts after ownership already moved
- a second parent for the same child
- any case where the compiler cannot prove that only one owner remains

Valid:

```draton
fn main() {
    let value = 1
    let left = [value]
    let right = [value]
    print(left.len() + right.len())
}
```

Compiler does:

- copy `value` twice because `Int` is `Copy`
- allow both arrays to contain independent integers

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let left = [name]
    let right = [name]
    print(left.len() + right.len())
}
```

Compiler does:

- move `name` into `left`
- reject the second store because it would require a second owner for the same string

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let a = lambda => name.len()
    let b = lambda => name.len()
    return [a, b]
}
```

Compiler does:

- see that both closures escape
- require `name` to move into both closure environments
- reject the second capture because that would create two owners

## 7. Closure capture rules

Closure capture is inferred from how long the closure value lives.

### 7.1 Non-escaping closures

A closure is non-escaping when the compiler can prove that it is:

- created and called within the same function
- never returned
- never stored into a collection or object
- never passed to a `move` parameter
- never captured by another escaping closure

Capture mode for a non-escaping closure:

- shared borrow if the closure only reads the captured value
- exclusive borrow if the closure mutates the captured value

Valid:

```draton
fn main() {
    let name = input("name: ")
    let show = lambda => name.len()
    print(show())
    print(name.len())
}
```

Compiler does:

- prove that `show` is non-escaping
- capture `name` by shared borrow
- end the borrow after the last reachable call to `show`
- keep `name` owned by `main`
- insert `free(name)` after the final `print`

Valid:

```draton
fn main() {
    @type {
        items: Array[String]
    }
    let mut items = []
    let push_one = lambda => items.push("x")
    push_one()
    push_one()
    print(items.len())
}
```

Compiler does:

- prove that `push_one` is non-escaping
- capture `items` by exclusive borrow
- keep the borrow live across both calls to `push_one`
- insert `free(items)` after the final `print`

### 7.2 Escaping closures

A closure is escaping when it is:

- returned
- stored into another owned value
- passed to a `move` parameter
- assigned to a binding that itself escapes

Every move-by-default value captured by an escaping closure is moved into the closure environment at closure creation time.

Valid:

```draton
fn make_reader() {
    let name = input("name: ")
    return lambda => name.len()
}
```

Compiler does:

- classify the closure as escaping
- move `name` into the closure environment when the lambda is created
- insert no local `free(name)`
- free the closure environment, and then `name`, when the returned closure reaches its own last use

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let reader = lambda => name.len()
    save_text(name)
    print(reader())
}
```

Compiler does:

- keep `reader`'s capture borrow live until `reader()` is done
- reject `save_text(name)` because the move overlaps an active capture borrow

Invalid:

```draton
fn main() {
    let name = input("name: ")
    let a = lambda => name.len()
    let b = lambda => name.len()
    return [a, b]
}
```

Compiler does:

- classify both closures as escaping
- move `name` into the first closure environment
- reject the second capture because `name` is already moved

### 7.3 Captured value lifetime

- a borrowed capture lives until the closure's last reachable call
- a moved capture lives until the closure value itself reaches last use
- when a closure environment is freed, every moved capture inside it is freed first

## 8. Cycle detection

Safe Draton forbids ownership cycles.

### 8.1 Rule

The safe heap graph must remain a forest of ownership trees:

- each move-by-default object has at most one owning parent
- temporary borrows do not count as parents
- adding an owning edge is allowed only when the compiler can prove that the child is not already an ancestor of the parent

If that proof fails, the program is rejected.

This applies to:

- class fields
- array, map, and set contents
- closure environments
- nested combinations of the above

### 8.2 Direct and indirect cycles

Valid:

```draton
class Node {
    let next

    @type {
        next: Option[Node]
    }
}

fn main() {
    let mut root = Node { next: None }
    let child = Node { next: None }
    root.next = Some(child)
    print(root)
}
```

Compiler does:

- move `child` into `root.next`
- record `root` as the only parent of `child`
- free `child` when `root` is freed

Invalid:

```draton
class Node {
    let next

    @type {
        next: Option[Node]
    }
}

fn link_back(root) {
    match root.next {
        Some(child) => {
            child.next = Some(root)
        }
        None => { }
    }
}
```

Compiler does:

- see that `child` is already owned under `root`
- reject `child.next = Some(root)` because it would make `root` a descendant of itself

### 8.3 Conservative rejection

If the compiler cannot prove that a new parent edge is acyclic, it rejects the store.

That includes:

- shape information lost behind an unknown call
- values re-entering safe code from raw pointer APIs without a unique-owner proof
- container updates where parent-child ancestry is no longer statically known

### 8.4 `@acyclic`

`@acyclic` is a compile-time class annotation:

```draton
@acyclic
class Package {
    ...
}
```

It means:

- the programmer asserts that no instance of that class will ever participate in an ownership cycle
- the compiler trusts that assertion for values of that class and skips per-store ancestry walks for edges whose full parent-child path stays inside statically known `@acyclic` types

The compiler still performs one definition-time check before accepting the annotation:

- reject `@acyclic` if the class has an obvious direct self-owning field
- a direct self-owning field includes the class itself or a built-in owning wrapper that immediately contains the same class, such as `Option[Self]`, `Result[Self, E]`, `Tuple[..., Self]`, `Array[Self]`, `Map[K, Self]`, or `Set[Self]`

What the compiler trusts:

- stores through fields of this class do not need dynamic cycle checks when every owning type on that path is known and marked `@acyclic`

What the compiler still checks:

- indirect cycles through non-`@acyclic` named classes
- indirect cycles through generic or otherwise unknown field types
- any edge that passes through raw pointers or re-enters from raw code

Valid:

```draton
@acyclic
class Artifact {
    let path

    @type {
        path: String
    }
}

@acyclic
class Package {
    let name
    let artifacts

    @type {
        name: String
        artifacts: Array[Artifact]
    }
}

fn main() {
    @type {
        artifacts: Array[Artifact]
    }
    let mut artifacts = []
    artifacts.push(Artifact { path: "main.o" })
    let pkg = Package { name: "app", artifacts: artifacts }
    print(pkg)
}
```

Compiler does:

- accept `@acyclic` on `Artifact` and `Package` because neither class directly owns itself
- trust that moving `artifacts` into `pkg` cannot form an ownership cycle through the declared `@acyclic` field path
- skip the usual ancestry walk for that store
- still keep the normal conservative checks for any later store through a non-`@acyclic` or unknown field type

There is no fallback to silent runtime cycle handling in safe code.

Can two heap objects reference each other?

- in safe Draton: no
- in `@pointer` or raw unsafe code: yes, but the compiler inserts no `free()` for that raw aliasing graph

## 9. Error messages

The inferred ownership pass must use these user-facing diagnostics. The wording is fixed. Spans and notes are added with the usual line and column information.

### 9.1 Use after move

```text
'{name}' was moved here and cannot be used again
hint: use '{name}' before the move or assign a new value to it first
```

### 9.2 Move while borrowed

```text
cannot move '{name}' while it is still borrowed
hint: finish the earlier read first, or move '{name}' after the borrow ends
```

### 9.3 Read during exclusive borrow

```text
cannot read '{name}' here because it is still being modified
hint: move this read after the modification finishes
```

### 9.4 Exclusive borrow during read borrow

```text
cannot modify '{name}' here because it is still being read
hint: move the modification later, or shorten the earlier read
```

### 9.5 Partial move

```text
cannot move field '{field}' out of '{base}' without moving the whole value
hint: move '{base}' as a whole, duplicate '{field}' explicitly, or use @pointer
```

### 9.6 Ambiguous call ownership

```text
cannot decide whether this call should borrow or move '{name}'
hint: call a more specific function, split the control flow, or use @pointer
```

### 9.7 Borrowed value escapes

```text
'{name}' does not live long enough for this use
hint: return or store an owned value instead, or move '{name}' into the closure
```

### 9.8 Multiple owners

```text
'{name}' would end up with more than one owner
hint: keep exactly one owner, duplicate the value explicitly, or use @pointer for shared access
```

### 9.9 Ownership cycle

```text
this assignment would create an ownership cycle
hint: keep the ownership graph acyclic, or use @pointer for cyclic structures
```

### 9.10 Loop move without reinitialization

```text
'{name}' is moved in one loop iteration but the loop may use it again
hint: reassign '{name}' before the next iteration, or move the value outside the loop
```

### 9.11 External boundary rejection

```text
cannot pass owned value '{name}' to external code
hint: convert it to @pointer inside @unsafe, or keep the call in safe Draton
```

### 9.12 Safe-to-raw alias rejection

```text
owned value '{name}' cannot cross into @pointer while a safe owner still exists
hint: move '{name}' completely, or create the raw value entirely inside @pointer
```

## 10. Interaction with escape hatches

`@pointer`, `@unsafe`, and `@asm` keep their existing syntax. Ownership rules change only at the safety boundary.

### 10.1 `@pointer`

Inside `@pointer`:

- `@pointer` values are always `Copy`
- the compiler does not infer ownership for the pointee graph
- the compiler inserts no `free()` for raw pointer aliasing
- the programmer is responsible for allocation, deallocation, aliasing, and cycles

Valid:

```draton
@extern "C" {
    fn malloc(size: UInt64) -> @pointer
    fn free(ptr: @pointer)
}

fn main() {
    @pointer {
        let p = malloc(64)
        free(p)
    }
}
```

Compiler does:

- treat `p` as a `Copy` raw pointer
- perform no safe ownership analysis for the pointee

Crossing from safe code into `@pointer` follows these rules:

- `Copy` values may cross by copy
- move-by-default safe values may cross only by full move
- borrowing a safe owner into `@pointer` is rejected
- returning a safe owned value from `@pointer` is allowed only when the compiler can treat it as a fresh owner with no remaining raw aliases; otherwise it is rejected

Invalid:

```draton
fn main() {
    let name = input("name: ")
    @pointer {
        raw_keep(name)
    }
    print(name.len())
}
```

Compiler does:

- reject the boundary crossing
- refuse to create a raw alias while `name` still exists as a safe owner

### 10.2 `@unsafe`

`@unsafe` does not disable inferred ownership.

Inside `@unsafe`:

- type checking continues
- move, borrow, last-use, alias, and cycle rules still apply
- unsafe-only operations are allowed, but they must still respect ownership at the safe boundary

### 10.2.1 `@unsafe` versus `@pointer`

`@unsafe` is for operations whose safety depends on caller-established preconditions, while `@pointer` is for raw pointer-oriented values and graphs.

`@unsafe` allows operations that safe code does not:

- calling extern or runtime entry points whose correctness depends on ABI, layout, alignment, bounds, or non-null guarantees that the compiler cannot prove
- performing unchecked casts or reinterpretations between raw handles and typed values when the programmer establishes validity
- doing one-shot boundary operations where the dangerous step is local but the surrounding values remain safe before and after the block

`@unsafe` does not disable:

- ownership inference
- borrow lifetime checking
- move-after-use errors
- alias rejection
- cycle detection
- the boundary rules for values that cross into raw code

Use `@unsafe` when:

- the operation is locally dangerous
- the value should still remain a normal safe Draton value before and after that operation
- you do not need a long-lived raw alias or manual ownership graph

Use `@pointer` when:

- the value itself must stay raw across multiple operations
- you need manual allocation or manual `free`
- you need shared raw aliases or cyclic raw graphs
- the compiler cannot or should not reason about the pointee ownership graph

| Topic | Safe code | `@unsafe` | `@pointer` |
| --- | --- | --- | --- |
| Caller-proved ABI or layout preconditions | rejected | allowed | allowed |
| Automatic move and borrow checks on safe values | enforced | enforced | enforced only for the raw pointer value itself, not the pointee graph |
| Long-lived raw aliases | rejected | rejected for safe values | allowed |
| Manual allocation and manual `free` | rejected | only through raw operations that still obey the boundary rules | allowed and expected |
| Cyclic or shared manual memory graphs | rejected | rejected for safe values | allowed |

`@unsafe` is the right tool when the dangerous operation is local and ownership remains safe:

```draton
@extern "C" {
    fn getpid() -> Int
}

fn main() {
    let mut pid = 0
    @unsafe {
        pid = getpid()
    }
    print(pid)
}
```

Compiler does:

- allow the unchecked extern call inside `@unsafe`
- keep normal `Copy` and assignment rules for `pid`
- insert no ownership-specific escape behavior because no owned safe value crosses into raw code

`@pointer` is the right tool when the value itself must stay raw and manually managed:

```draton
@extern "C" {
    fn malloc(size: UInt64) -> @pointer
    fn free(ptr: @pointer)
}

fn main() {
    @pointer {
        let p = malloc(64)
        free(p)
    }
}
```

Compiler does:

- treat `p` as a raw pointer value
- skip pointee ownership analysis
- leave allocation and deallocation to the programmer

Valid:

```draton
fn main() {
    let name = input("name: ")
    @unsafe {
        print(name.len())
    }
    save_text(name)
}
```

Compiler does:

- infer the same shared borrow for `name.len()` that it would infer outside `@unsafe`
- end the borrow when the `print` call returns
- allow `save_text(name)` afterward

Invalid:

```draton
@extern "C" {
    fn retain_name(name: String)
}

fn main() {
    let name = input("name: ")
    @unsafe {
        retain_name(name)
    }
}
```

Compiler does:

- reject the call
- require an explicit raw-pointer conversion if external code will keep the value

### 10.3 `@asm`

`@asm` is opaque raw code.

- only `Copy` scalars and explicit `@pointer` values may cross into `@asm`
- move-by-default safe values cannot be referenced directly from `@asm`
- the compiler assumes `@asm` may keep any raw alias it is given

Invalid:

```draton
fn main() {
    let name = input("name: ")
    @asm { ;; use name directly }
}
```

Compiler does:

- reject the direct use of `name`
- require the program to cross into raw code through `@pointer`

### 10.4 Values crossing back into safe code

When raw code produces a value that safe Draton will own:

- the handoff must produce exactly one new safe owner
- no raw alias may remain usable after the handoff unless the value stays in `@pointer`
- if uniqueness cannot be proven, the value stays raw and safe code cannot treat it as an owned Draton value

That rule is why `@pointer` is the explicit escape hatch: the compiler never guesses when raw code might still share a value.
