# Syntax Migration Notes

For the design rationale behind these syntax rules, see [language-manifesto.md](language-manifesto.md). For exact style expectations, see [canonical-syntax-rules.md](canonical-syntax-rules.md). For contributor guardrails, see [contributor-language-rules.md](contributor-language-rules.md).

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

In the Rust compiler/tooling path:

- default mode keeps legacy inline type syntax working and emits deprecation warnings
- `drat build --strict-syntax` and `drat run --strict-syntax` turn the same deprecated forms into hard errors

No in-tree self-host compiler source currently ships in this repository. Canonical syntax enforcement therefore applies to the Rust frontend/tooling path today, and any future self-host rewrite should start from the same canonical surface instead of reviving compatibility-form syntax.

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
- interface scope
- function scope

Each scope uses the same declaration shape:

```draton
@type {
    name: Type
}
```

Canonical examples for the newly supported scopes:

```draton
fn parse_head() {
    @type {
        head: Node??
    }
    let head = None
    return head
}

interface Drawable {
    fn draw()

    @type {
        draw: () -> Int
    }
}
```

Repository status tracking for future self-host work:

- [Self-Host Canonical Migration Status](selfhost-canonical-migration-status.md)

Current CI status:

- strict syntax is enforced through Rust frontend/tooling checks such as `drat build --strict-syntax`
- there is no active self-host syntax subset or bootstrap gate until a new self-host implementation is introduced
