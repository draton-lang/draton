# Draton Language Manifesto

This document explains the design intent behind Draton's canonical syntax. It is not a roadmap and it is not a place to propose new syntax. Its job is to preserve consistency.

If a future syntax change conflicts with the principles here, the default answer should be "no" unless the repository's existing behavior has already changed and the docs are being updated to match it.

## Core philosophy

### Readability first

Draton code should read clearly from top to bottom. Syntax should emphasize the behavior of the program, not force the reader to decode type noise or competing forms.

The language prefers one obvious surface form over several equivalent spellings.

### Code expresses behavior

Executable code should primarily answer:

- what does this value do
- what does this function return
- what capability does this part of the program provide

That is why Draton keeps `let`, explicit `return`, brace imports, and class/layer structure as the canonical surface.

### Types are contracts, not inline noise

Draton is statically typed, but the canonical style does not require types to be repeated inline everywhere.

Types belong in `@type` blocks when they are useful as contracts, specifications, optimization hints, or explicit interfaces. The goal is to let normal code stay readable while still giving projects a precise place to state type intent.

`@type` is therefore optional in ordinary code, but authoritative when used.

## Canonical syntax

### Variables

```draton
let x = 10
```

`let` is the canonical declaration form. Draton does not want multiple primary variable syntaxes competing with each other.

### Functions

```draton
fn add(a, b) {
    return a + b
}
```

Explicit `return` is canonical. Draton prefers visible control flow over style rules that depend on implicit trailing expressions.

### Imports

```draton
import { connect } from net
```

Brace imports are the canonical module import form. Imports should make dependencies explicit and readable at the top of a file.

Module names come from file layout:

- one file = one module
- directories = namespaces
- module paths are inferred from source roots

Draton does not require a top-of-file `module ...` declaration.

### Type blocks

```draton
@type {
    add: (Int, Int) -> Int
}
```

`@type` blocks define contracts in one place instead of scattering type declarations through executable code.

This is the preferred home for:

- function contracts
- named bindings that benefit from explicit type specification
- class and layer contracts

### Class and layer model

```draton
class User {
    layer info {
        fn greet() {
            return "hello"
        }
    }
}
```

`class` is the structural container.

`layer` is the capability container inside a class.

This is intentional. Draton wants contributors and users to group code by responsibility, not by an arbitrary pile of methods. A class describes what a thing is. Layers describe what kinds of behavior that thing exposes.

## Why `@type` exists

`@type` blocks are not meant to make Draton "more abstract". They exist to separate implementation from contract.

That separation supports several goals:

- readable executable code
- explicit contracts when a project wants them
- a stable place for advanced type information
- a canonical syntax that scales from small scripts to stricter codebases

In practice:

- normal code can often rely on inference
- stricter codebases can add `@type` blocks
- tooling and CI can enforce canonical style with strict mode

## Deprecated syntax and compatibility

The following forms are no longer canonical:

- inline variable types: `let x: T = ...`
- typed parameters: `fn f(a: T)`
- inline return types: `fn f() -> T`

They are still accepted in compatibility mode to avoid unnecessary breakage in existing code. The Rust frontend/tooling path emits deprecation warnings for them.

Under strict mode:

```sh
drat build --strict-syntax
drat run --strict-syntax
```

those deprecated forms become hard errors.

Strict mode exists so modern codebases, CI pipelines, and canonical examples can enforce the documented surface without immediately removing compatibility for older code.

## Design constraints for contributors

Contributors should treat the following as design constraints, not casual style suggestions.

- Do not introduce new inline type syntax.
- Do not add multiple competing import styles as equal citizens.
- Do not introduce syntax that weakens the class/layer model.
- Do not prefer implicit behavior when explicit syntax is clearer.
- Do not let docs drift away from parser behavior.
- Do not present compatibility syntax as if it were canonical syntax.

When in doubt, prefer syntax that keeps code visually simple and pushes contracts into `@type` blocks.

## Current scope and limits

The Rust frontend/tooling path is the authoritative implementation of the canonical syntax rules.

No in-tree self-host compiler source is currently shipped while the compiler rewrite is being prepared. Contributors should not imply that a retired `src/` mirror still defines or verifies canonical syntax.

## Authority of this document

This manifesto exists to stop syntax drift.

If you are updating parser behavior, formatter behavior, tests, examples, or contributor-facing docs, this file should be the reference for why the canonical syntax looks the way it does.
