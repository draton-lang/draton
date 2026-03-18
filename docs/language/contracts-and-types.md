---
title: Contracts and types
sidebar_position: 11
---

# Contracts and types

Draton is statically typed, but the language does not treat inline annotations as the main surface. The architectural rule is:

> Code expresses behavior. `@type` expresses contracts.

This page explains what that means in practice.

## Why contracts live in `@type`

Many languages let types leak into every line of executable code. Draton chooses a different shape:

- executable syntax stays readable first
- inference carries ordinary code
- type intent can still be stated precisely
- one canonical contract surface exists across files, classes, layers, interfaces, and functions

That choice is not just style. It keeps code review, formatting, linting, migration, and self-host parity simpler.

## File-level contracts

Use file-level `@type` when a module needs an explicit contract surface:

```draton
@type {
    parse_user: (String) -> User
    default_retries: Int
}
```

This is the canonical way to make a module’s type intent explicit without rewriting executable definitions into typed syntax.

## Function contracts

Function signatures belong in `@type`:

```draton
@type {
    parse_user: (String) -> User
}

fn parse_user(raw) {
    return User(raw)
}
```

What Draton deliberately avoids as canonical style:

```draton
fn parse_user(raw: String) -> User {
    return User(raw)
}
```

## Local contracts

Function-scope `@type` is the place for local explicit hints when inference needs help:

```draton
fn collect() {
    @type {
        out: [String]
    }

    let out = []
    return out
}
```

The point is to keep local intent explicit without turning the whole function into mixed executable and annotation syntax.

## Interface and structural contracts

`@type` is also how classes, layers, and interfaces make their boundaries clear:

```draton
interface Loader {
    @type {
        load: (String) -> Result[String, Error]
    }
}
```

This makes contracts compositional: the same contract surface exists whether the subject is a module, a class, a layer, or a local binding.

## What types are for in Draton

Types are used for:

- contracts
- interfaces
- explicit API boundaries
- local hints when inference needs help
- internal compiler/runtime reasoning

Types are not used to make every binding visually heavy by default.

## The role of inference

Inference is the normal path for day-to-day code. That keeps the language readable:

```draton
let user = parse_user(raw)
let greeting = f"hello {user.name}"
```

When inference is enough, extra contract syntax is unnecessary.

When contract clarity matters, use `@type`.

## Hard repository rules

Draton’s repo policy explicitly forbids treating these forms as canonical:

- inline variable types
- typed function parameters
- inline return types

Those forms may appear only as temporary migration compatibility, never as a second official philosophy.

## Related references

- [Language manifesto](../language-manifesto.md)
- [Canonical syntax rules](../canonical-syntax-rules.md)
- [Syntax migration](../syntax-migration.md)
- [Contributor language rules](../contributor-language-rules.md)
