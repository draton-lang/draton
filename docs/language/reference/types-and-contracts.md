---
title: Types and contract syntax
sidebar_position: 21
---

# Types and contract syntax

This page covers the syntax surface for type expressions and `@type` blocks.

## Named types

Examples:

```draton
Int
String
User
```

## Generic types

Examples:

```draton
Array[Int]
Result[String, Error]
Stack[Array[Int]]
```

## Function types

Function contract form:

```draton
(Int, Int) -> Int
() -> String
```

In parser terms, function types are first-class type expressions.

## Pointer type marker

The type grammar includes the raw pointer marker:

```draton
@pointer
```

This appears primarily in low-level and extern-facing contexts.

## Inferred type marker

Type expressions also support `_` as an inferred type marker:

```draton
@type {
    value: _
}
```

## `@type` block member forms

Inside a type block, the parser accepts two member shapes:

### Binding-style contract

```draton
@type {
    count: Int
}
```

### Function-style contract entry

```draton
@type {
    add: (Int, Int) -> Int
}
```

The canonical repository guidance favors binding-style `name: Type` entries.

## Supported scopes

`@type` blocks are accepted at:

- file scope
- class scope
- layer scope
- interface scope
- function scope

## Compatibility inline type syntax

The parser still accepts:

- `let x: Int = 1`
- `fn add(a: Int) -> Int`
- class fields with inline type hints

These are accepted to support compatibility and migration. They are not the canonical contract surface.

## Related references

- [Contracts and types guide](../contracts-and-types.md)
- [Canonical syntax rules](../../canonical-syntax-rules.md)
- [Syntax migration](../../syntax-migration.md)
