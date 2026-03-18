---
title: Classes, interfaces, enums, and errors
sidebar_position: 22
---

# Classes, interfaces, enums, and errors

This page gathers the structural and type-oriented declaration forms used across the language.

## Classes

Base form:

```draton
class User {
    let name
}
```

Supported class syntax:

- fields with `let`
- `let mut` fields
- methods
- `layer` blocks
- class-level `@type` blocks
- generic parameters: `class Stack[T]`
- inheritance: `extends Base`
- interface implementation: `implements Named, Drawable`

## Layers

Layer syntax:

```draton
layer Display {
    fn title() {
        return self.name
    }
}
```

Layer-specific rules visible in the parser/tests:

- layers are allowed only inside classes
- nested layers are rejected
- layers can contain methods
- layers can contain layer-scope `@type` blocks
- layer methods can be `pub`

## Interfaces

Interface syntax:

```draton
interface Drawable {
    fn draw()

    @type {
        draw: () -> Int
    }
}
```

Interfaces can contain:

- method declarations
- interface-scope `@type` blocks

## Enums

Enum syntax:

```draton
enum Color { Red, Green, Blue }
```

The currently documented parser shape is a named enum with a brace-delimited variant list.

## Errors

Error declarations:

```draton
error NotFound(msg)
error NotFound(msg: String)
```

Use error declarations when the code needs a named structured error constructor.

## Structural contracts

Classes, layers, and interfaces all participate in the `@type` contract model:

```draton
class User {
    let name

    layer Access {
        fn getName() {
            return self.name
        }
    }

    @type {
        name: String
        getName: () -> String
    }
}
```

That keeps structural declarations and contract declarations aligned with the language’s architecture.
