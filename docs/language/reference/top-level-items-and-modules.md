---
title: Top-level items and modules
sidebar_position: 20
---

# Top-level items and modules

This page covers the syntax that can appear at top level in a source file.

## Imports

Canonical form:

```draton
import { connect, listen as serve } from net.http
```

Supported details:

- brace import list
- aliases with `as`
- dotted module path after `from`
- multiline import item list

Example:

```draton
import {
    fs as f
    net as n
} from std.io
```

## Functions

Top-level functions:

```draton
fn main() {
    return 0
}
```

Public top-level functions:

```draton
pub fn build() {
    return 0
}
```

## Classes

Classes may include:

- fields
- methods
- layers
- class-level `@type` blocks
- inheritance with `extends`
- interface implementation with `implements`
- generic parameters

Example:

```draton
class User extends Entity implements Named {
    let label
}
```

## Interfaces

Interfaces contain method declarations and interface-level `@type` blocks:

```draton
interface Drawable {
    fn draw()

    @type {
        draw: () -> Int
    }
}
```

## Enums

Enum syntax:

```draton
enum Direction { North, South, East, West }
```

## Errors

Error syntax:

```draton
error NotFound(msg)
error NotFound(msg: String)
```

The parser accepts parameter syntax similar to function parameter declarations.

## Constants

Constant syntax:

```draton
const ANSWER = 42
```

## Type blocks

Top-level file contracts:

```draton
@type {
    add: (Int, Int) -> Int
}
```

## Extern blocks

Extern block syntax:

```draton
@extern "C" {
    fn puts(msg)
}
```

Accepted details:

- ABI string after `@extern`
- function declarations inside the block
- parameter hints and return types are accepted by the parser here as well

## Panic and OOM handlers

Top-level special handlers:

```draton
@panic_handler
fn on_panic(msg) { }

@oom_handler
fn on_oom() { }
```

These are parser-level items, not ordinary attributes attached to arbitrary declarations.
