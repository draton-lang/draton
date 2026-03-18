---
title: Syntax reference
sidebar_position: 14
---

# Syntax reference

This section is the detailed syntax reference for the Draton language as it exists in the repository today. It is organized by syntax family instead of by feature marketing.

The goal is straightforward:

- document what the parser accepts
- distinguish canonical syntax from compatibility syntax
- keep examples aligned with the real Rust frontend

## What this reference covers

The pages below cover the current surface area of the language:

- literals and values
- variable bindings and assignment
- functions, calls, lambdas, and generics
- expressions and operators
- control flow and pattern matching
- top-level items and modules
- types and `@type` contracts
- classes, interfaces, enums, errors, and layers
- concurrency and channels
- low-level, compile-time, and runtime-special syntax

## Canonical versus accepted

This reference uses two terms deliberately:

- **canonical** means the preferred language surface
- **accepted** means the parser still accepts it, often for migration compatibility

If a form is accepted but not canonical, the docs say so explicitly.

## Recommended reading order

If you want the whole syntax with context:

1. [Literals and values](./literals-and-values.md)
2. [Bindings and assignment](./bindings-and-assignment.md)
3. [Functions, calls, and lambdas](./functions-calls-and-lambdas.md)
4. [Expressions and operators](./expressions-and-operators.md)
5. [Control flow and pattern matching](./control-flow-and-patterns.md)
6. [Top-level items and modules](./top-level-items-and-modules.md)
7. [Types and contracts](./types-and-contracts.md)
8. [Classes, interfaces, enums, and errors](./classes-interfaces-enums-and-errors.md)
9. [Concurrency and channels](./concurrency-and-channels.md)
10. [Low-level and compile-time syntax](./low-level-and-compile-time.md)
