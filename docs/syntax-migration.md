# Syntax Migration Notes

This repository now documents and tests the following syntax as canonical:

## Removed from canonical syntax

- Inline variable annotations such as `let value: Int = 1`
- Inline function signatures as the primary style such as `fn add(a: Int, b: Int) -> Int`
- `@layer` blocks
- Import examples without an explicit module source

## Deprecated but still accepted for compatibility

- Inline field and function annotations in older source files
- Legacy function type syntax `fn(Int) -> Int`
- Older import forms that omit `from module.path`

These forms remain temporarily to avoid breaking existing projects, but they are no longer the documented style.

## Canonical replacements

Variable declaration:

```draton
let value = 1
```

Type declarations via `@type`:

```draton
@type {
    value: Int
    add: (Int, Int) -> Int
}
```

Explicit returns:

```draton
fn add(a, b) {
    return a + b
}
```

Class and layer organization:

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

Brace imports:

```draton
import { login } from services.auth
import { http as nethttp } from std.net
```

## Scope rules for `@type`

`@type` blocks are supported at:

- file/module scope
- class scope
- layer scope

Each scope uses the same declaration shape:

```draton
@type {
    name: Type
}
```
