---
title: Expressions and operators
sidebar_position: 18
---

# Expressions and operators

This page covers expression syntax and operator precedence families exposed by the parser.

## Binary operators

The parser currently supports these binary operator families:

- nullish: `??`
- logical OR: `||`
- logical AND: `&&`
- equality: `==`, `!=`
- comparison: `<`, `<=`, `>`, `>=`
- range: `..`
- bitwise OR / XOR: `|`, `^`
- bitwise AND: `&`
- shifts: `<<`, `>>`
- additive: `+`, `-`
- multiplicative: `*`, `/`, `%`

Examples:

```draton
a + b * c
left ?? right
flags & mask
1 .. 10
```

## Unary operators

The parser supports:

- unary minus: `-x`
- logical not: `!flag`
- bitwise not: `~bits`
- address-of: `&value`
- dereference: `*ptr`

Example:

```draton
let neg = -x
let inverted = !ready
```

## Cast expressions

Cast syntax:

```draton
value as Int
floatValue as Int
```

## Grouping

Parentheses group expressions:

```draton
(a + b) * c
```

## Field access

Field syntax:

```draton
self.name
user.id
```

## Match expressions

`match` is an expression form:

```draton
match value {
    0 => "zero",
    1 => "one",
    _ => "other",
}
```

Arm bodies can be either:

- a single expression
- a block

Example with a block arm:

```draton
match value {
    0 => {
        return "zero"
    },
    _ => "other",
}
```

## Patterns currently seen in syntax

The parser reuses expression syntax for arm patterns. In practice, repo tests and examples currently exercise:

- literal patterns
- wildcard `_`
- `Ok(...)` / `Err(...)`-style constructor shapes

## Class literals

The parser recognizes class-literal style construction when an identifier is immediately followed by a brace body with `field: value` entries:

```draton
User { name: "Minh", age: 20 }
Stack[Int] { items: [] }
```

This is distinct from map/set brace literals because it is attached to an identifier expression.
