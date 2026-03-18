---
title: Syntax overview
sidebar_position: 10
---

# Syntax overview

This page explains the canonical surface of Draton as it exists in the repository today. It is the fastest way to understand how Draton code is meant to look.

For design intent, see [language-manifesto](../language-manifesto.md). For hard style rules, see [canonical syntax rules](../canonical-syntax-rules.md).

## The organizing rule

Draton is easiest to read when each surface layer stays in its own place:

- executable code expresses behavior
- `@type` expresses contracts
- class and layer structure express organization

That is why the language rejects the idea that every declaration should carry inline type noise.

## Variables

Canonical variable bindings use `let`:

```draton
let count = 0
let name = "Draton"
let items = [1, 2, 3]
```

Mutable bindings still start from the same surface:

```draton
let mut total = 0
```

Not canonical:

```draton
let total: Int = 0
```

If a binding needs an explicit contract, the canonical place is a type block:

```draton
@type {
    total: Int
}
```

## Functions

Executable functions keep parameter lists visually light:

```draton
@type {
    add: (Int, Int) -> Int
}

fn add(a, b) {
    return a + b
}
```

Canonical function rules:

- parameter names are not typed inline in executable definitions
- explicit `return` remains the canonical control-flow style
- contracts belong in `@type`

## Imports

Draton uses brace imports as the one canonical import form:

```draton
import { read_line } from std.io
import { http as nethttp } from std.net
```

Imports are meant to be readable and explicit at the top of a file.

## Contracts with `@type`

`@type` blocks are the contract surface of the language:

```draton
@type {
    greet: (String) -> String
    retries: Int
}
```

They are supported at multiple scopes:

- file scope
- class scope
- layer scope
- interface scope
- function scope

The goal is precision without turning ordinary executable code into annotation-heavy markup.

## Classes and layers

The structural model is not “all methods in one flat bag”. Draton splits structure from capability grouping:

```draton
class User {
    let name

    layer display {
        fn greeting() {
            return f"hello {name}"
        }
    }

    @type {
        name: String
        greeting: () -> String
    }
}
```

Read it like this:

- `class` answers what the thing is
- `layer` answers what related capability this part of the class provides

## Control flow

Draton prefers visible control flow:

```draton
fn choose(flag) {
    if flag {
        return "yes"
    }
    return "no"
}
```

The language deliberately avoids making an implicit-return-only style canonical.

## System builtins

Core interactive system builtins remain explicit and minimal:

```draton
print("working...")
println("done")
let name = input("Name: ")
```

Current input semantics:

- `input("Prompt: ")` is a builtin, not a library import
- it prints the prompt without a newline
- it reads one line from stdin
- it trims trailing line endings
- it returns a `String`

## Compatibility syntax

Compatibility syntax still exists for migration support, but it is not a co-equal design direction.

Strict mode exists to stop syntax drift:

```sh
drat build --strict-syntax
drat run --strict-syntax
```

When writing new examples, docs, tests, or migrated self-host code, stay on the canonical surface.
