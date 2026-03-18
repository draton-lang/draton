---
title: Literals and values
sidebar_position: 15
---

# Literals and values

This page covers literal forms and direct value constructors accepted by the parser.

## Integer literals

Accepted forms:

```draton
42
0
123456
```

The parser also accepts radix-prefixed integer literals:

```draton
0xff
0b101010
```

These are parsed as integer values.

## Floating-point literals

Accepted form:

```draton
3.14
0.5
12.0
```

## Boolean literals

Accepted forms:

```draton
true
false
```

## None literal

Draton accepts the `None` literal:

```draton
let value = None
```

Use it where the surrounding type or contract makes the intended optional/nullish shape clear.

## String literals

Basic string literals:

```draton
"hello"
"draton"
```

## Interpolated string literals

Draton supports f-strings:

```draton
let name = "Minh"
let greeting = f"Hello {name}"
```

Interpolation rules, as currently implemented:

- the outer literal starts with `f"`
- literal fragments and embedded expressions can be mixed
- each `{ ... }` section is parsed as a real expression

## Tuple literals

Accepted forms:

```draton
(1, 2)
(1, 2, 3)
()
```

Parenthesized single expressions stay grouped expressions, not tuples:

```draton
(x + 1)
```

## Array literals

Accepted form:

```draton
[1, 2, 3]
[]
```

## Map literals

Brace literals with `key: value` entries are maps:

```draton
{ "a": 1, "b": 2 }
{}
```

The empty brace literal is currently parsed as an empty map.

## Set literals

Brace literals without `:` are sets:

```draton
{ 1, 2, 3 }
{ "a", "b" }
```

## Result constructors

`Ok(...)` and `Err(...)` are recognized specially when called with exactly one argument:

```draton
Ok(42)
Err("bad input")
```

## Channel type constructor expression

Draton also parses `chan[T]` at expression level as a channel constructor/type-form expression:

```draton
let jobs = chan[Int]
```

See [Concurrency and channels](./concurrency-and-channels.md) for the current syntax boundary.
