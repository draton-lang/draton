---
title: Concurrency and channels
sidebar_position: 23
---

# Concurrency and channels

This page covers the surface syntax for the concurrency features that are already visible in the parser and examples.

## Spawn statement

Draton supports `spawn` as a statement:

### Spawn an expression

```draton
spawn work()
```

### Spawn a block

```draton
spawn {
    let x = 1
}
```

This is not a function-call convention. `spawn` is parsed as its own statement form.

## Channel constructor/type expression

The parser recognizes:

```draton
chan[Int]
```

This appears as a dedicated expression node rather than a normal identifier-plus-index pattern.

## Current documentation boundary

The repo README already treats channels and `spawn` as part of the language feature set. This reference documents the syntax that the parser currently exposes:

- `spawn <expr>`
- `spawn { ... }`
- `chan[T]`

Higher-level runtime semantics, scheduling, and library patterns should stay documented where the implementation truth exists instead of being guessed here.
