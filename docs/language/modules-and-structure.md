---
title: Modules, classes, and layers
sidebar_position: 12
---

# Modules, classes, and layers

This page explains how Draton organizes code structurally. The language does not treat modules, objects, and methods as one undifferentiated space.

## Modules

Modules come from file layout. The canonical import form is:

```draton
import { login } from services.auth
```

The design goal is simple:

- dependency edges are explicit
- file layout is reflected directly in imports
- there is one canonical import form

Draton does not want several co-equal module syntaxes competing with each other.

## Classes

`class` is the structural unit of identity and stored state.

Use a class when the code needs:

- named fields
- object identity
- grouped state
- explicit structural boundaries

Example:

```draton
class Session {
    let token
    let user_id
}
```

## Layers

`layer` groups related capabilities within a class.

Example:

```draton
class Session {
    let token

    layer display {
        fn describe() {
            return f"session {token}"
        }
    }
}
```

This separation matters:

- `class` says what data/identity exists
- `layer` says which related operations belong together

That gives large classes a more explicit internal organization than a flat method list.

## Why Draton keeps both

The class/layer split is a deliberate architectural decision. It helps:

- readers scan capabilities by group
- docs and code review talk about structure and capability separately
- tooling reason about related behavior in a stable way

The repo policy explicitly rejects alternate structural philosophies being documented as co-equal canonical design.

## Interfaces and implementation boundaries

Interfaces describe capability contracts. Classes and layers provide the implementation shape.

That separation keeps the language easy to reason about:

- interface: what must be provided
- class: what exists structurally
- layer: how behavior is grouped

## Practical guidance

Use modules for namespace boundaries.

Use classes for:

- stateful domain objects
- compiler/runtime structures
- things with clear identity

Use layers for:

- display behavior
- parsing-related behavior
- validation behavior
- lifecycle-oriented grouped behavior

Avoid turning layers into arbitrary style decoration. They exist to group coherent capability.

## Related references

- [Language architecture](../language-architecture.md)
- [Language class diagram](../language-class-diagram.md)
- [Compiler architecture](../compiler-architecture.md)
