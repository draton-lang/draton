---
title: Bindings and assignment
sidebar_position: 16
---

# Bindings and assignment

This page covers local bindings, destructuring, mutation, and assignment-like statements.

## Canonical local binding

The canonical variable form is `let`:

```draton
let x = 1
let name = "Draton"
```

## Mutable bindings

Mutable bindings use `let mut`:

```draton
let mut total = 0
```

## Accepted compatibility type hints

The parser still accepts inline local type hints:

```draton
let total: Int = 0
let items: Array[Int] = []
```

These are accepted for compatibility, not as canonical style. The preferred contract surface is a function-scope `@type` block.

## Local `@type` blocks

Canonical local contracts:

```draton
fn build() {
    @type {
        out: [String]
    }

    let out = []
    return out
}
```

## Tuple destructuring

Draton supports tuple destructuring in `let` statements:

```draton
let (x, y) = (1, 2)
let (a, b, c) = (1, 2, 3)
let (_, y) = (1, 2)
```

Supported pieces:

- named bindings
- `_` wildcard
- `let mut (...)` for mutable destructuring

## Reassignment

Basic assignment:

```draton
x = 10
```

Compound assignment forms:

```draton
x += 1
x -= 1
x *= 2
x /= 2
x %= 2
```

Increment/decrement statement forms:

```draton
x++
x--
```

These are statement forms, not primary expression forms.

## Assignment targets

Assignment-like statements are parsed against expression targets, which means the target can be:

- a local identifier
- a field access
- an index access

Examples:

```draton
user.name = "new"
items[0] = 42
```

## Field bindings in classes

Class fields also start from `let`:

```draton
class User {
    let name
    let mut age
}
```

Accepted compatibility field hints:

```draton
class User {
    let name: String
}
```

Again, accepted does not mean canonical.
