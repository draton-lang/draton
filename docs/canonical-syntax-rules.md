# Canonical Syntax Rules

This document is the repository's exact syntax-style reference. It describes what contributors should write in code, tests, examples, and docs.

For design rationale, see [language-manifesto.md](language-manifesto.md). For old-to-new mapping, see [syntax-migration.md](syntax-migration.md).

## Core rule

Prefer one canonical form. Do not present compatibility syntax as an equal alternative.

## Variables

Canonical:

```draton
let x = 10
let mut count = 0
```

Not canonical:

```draton
let x: Int = 10
```

If a binding needs an explicit contract, place it in `@type`:

```draton
@type {
    count: Int
}
```

## Functions

Canonical:

```draton
@type {
    add: (Int, Int) -> Int
}

fn add(a, b) {
    return a + b
}
```

Not canonical:

```draton
fn add(a: Int, b: Int) -> Int {
    return a + b
}
```

Rules:

- keep parameter names untyped in executable definitions
- keep return flow explicit with `return`
- put contracts in `@type`

## Imports

Canonical:

```draton
import { login } from services.auth
import { http as nethttp } from std.net
```

Rules:

- brace imports are the canonical form
- module paths come from file layout
- do not introduce a second canonical import style

## `@type` blocks

Canonical shape everywhere:

```draton
@type {
    name: Type
}
```

Supported scopes:

- file/module scope
- class scope
- layer scope
- interface scope
- function scope

Purpose:

- contracts
- interfaces
- explicit local hints where inference needs help

Non-purpose:

- mandatory inline noise on every binding

## Class and layer model

Canonical structure:

```draton
class User {
    let name

    layer info {
        fn greet() {
            return "hello " + name
        }
    }

    @type {
        name: String
        greet: () -> String
    }
}
```

Rules:

- `class` groups structure
- `layer` groups related processing or capability functions
- do not document or implement alternate structural philosophies as co-equal

## Compatibility and strict mode

Compatibility mode still accepts deprecated inline type syntax for migration support.

Strict mode rejects it:

```sh
drat build --strict-syntax
drat run --strict-syntax
```

Strict mode exists to prevent syntax drift. New examples, docs, tests, and migrated self-host code should follow canonical syntax by default.

