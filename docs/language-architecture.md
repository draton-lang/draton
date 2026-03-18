# Draton Language Architecture

This document explains what Draton is as a language, how its core pieces fit together, and what architectural boundaries contributors should preserve.

For design rationale, see [language-manifesto.md](language-manifesto.md). For exact syntax shape, see [canonical-syntax-rules.md](canonical-syntax-rules.md). For migration and compatibility rules, see [syntax-migration.md](syntax-migration.md). For a compact architecture summary and diagrams, see [language-class-diagram.md](language-class-diagram.md) and [language-analyst-artifact.md](language-analyst-artifact.md).

## Purpose

Draton is a compiled, statically typed, tooling-first language designed around readability and explicit structure.

Its architecture is intentionally not "everything language design can support". It is optimized for:

- readable source code
- explicit contracts when needed
- strong tooling
- compiler and language-engineering workloads
- structured code organization

## Core architectural model

Draton separates three concerns that many languages blend together:

1. executable behavior
2. type contracts
3. structural organization

The language is easiest to understand if those three layers stay separate.

### Executable behavior

Executable code should say what the program does.

Canonical behavior syntax therefore stays simple:

```draton
let value = 1

fn add(a, b) {
    return a + b
}
```

Important consequences:

- `let` is the canonical binding form
- function bodies are where behavior lives
- `return` stays explicit so control flow is visible

### Type contracts

Types are part of the language, but they are not meant to dominate the executable surface.

Draton therefore puts explicit contracts in `@type` blocks:

```draton
@type {
    value: Int
    add: (Int, Int) -> Int
}
```

Architecturally, this means:

- code stays readable by default
- inference can carry most ordinary code
- stricter modules can add contracts without changing executable syntax
- type intent has one authoritative place instead of many competing inline forms

This is why "code expresses behavior; `@type` expresses contracts" is not just style guidance. It is the language's organizing rule.

### Structural organization

Draton does not treat all methods as one flat bag.

Its structural model is:

- `class` for structure and identity
- `layer` for related capability groups inside a class

Example:

```draton
class User {
    let name

    layer info {
        fn greet() {
            return "hello " + name
        }
    }

    @type {
        name: String
        greet: () -> String
    }
}
```

This is intentional:

- a `class` answers what a thing is
- a `layer` answers what kind of work or capability is grouped together

Contributors should preserve this distinction instead of documenting alternate structural philosophies as co-equal.

## Surface syntax architecture

Draton is designed around one canonical surface syntax, not several equally valid spelling families.

### Variable declarations

Canonical:

```draton
let count = 0
```

Not canonical:

```draton
let count: Int = 0
```

### Functions

Canonical executable form:

```draton
fn parse(input) {
    return input
}
```

Canonical contract form:

```draton
@type {
    parse: (String) -> String
}
```

### Imports

Canonical:

```draton
import { connect } from net.http
```

Architecturally, imports are explicit dependency declarations. Draton does not want multiple primary import dialects competing with each other.

### `@type` blocks

The same contract shape is supported across several scopes:

- file/module scope
- class scope
- layer scope
- interface scope
- function scope

That consistency is part of the architecture. `@type` is one contract mechanism reused across scopes, not several unrelated features.

## Type system position

Draton is statically typed, but it is not architected as a language that requires heavy inline annotation to feel "serious".

The intended balance is:

- inference by default
- explicit contracts when useful
- strict mode and tooling to keep the surface canonical

This lets the language scale across:

- small scripts
- ordinary application code
- stricter codebases that want contracts as documentation and checking guidance

## Module and file model

Draton keeps the module model simple:

- one file acts as one module
- directories act as namespaces
- module paths come from source layout
- canonical imports use `import { ... } from module.path`

The architecture here favors predictable tooling and readable imports over extra declaration syntax.

## Interface and abstraction model

Interfaces are part of the contract layer, not a separate syntax philosophy.

Canonical shape:

```draton
interface Drawable {
    fn draw()

    @type {
        draw: () -> Int
    }
}
```

This keeps executable member syntax minimal while putting method contracts in the same contract system used everywhere else.

## Control-flow model

Draton favors explicit control flow.

That is why:

- `return` is canonical
- compatibility does not imply endorsement of implicit-return-only style
- readability takes priority over terse expression-oriented cleverness

This matters for tooling as well as for human readers. Formatter, linter, diagnostics, and self-host migration all benefit from a stable and explicit control-flow surface.

## Compatibility architecture

Compatibility syntax still exists, but only as a migration boundary.

It is not:

- a second language design direction
- a parallel canonical style
- a basis for new examples or future documentation

Current compatibility support exists so older code can keep building while the repository and ecosystem converge on canonical syntax. Strict mode exists to make syntax drift visible and enforceable.

## Tooling-first implications

Draton is architected to be easy to parse, format, lint, analyze, and migrate consistently.

That means the language surface is chosen with tooling in mind:

- one canonical variable form
- one canonical import form
- one canonical contract form
- one explicit control-flow philosophy

This is a language-engineering choice, not an implementation accident.

## What Draton is not trying to be

Draton is not currently architected as:

- a multi-style language where several surface forms are equally "the right way"
- a syntax playground that keeps reopening settled canonical rules
- a language that makes inline type noise mandatory everywhere
- a kitchen-sink feature language by default

Its design center is narrower and more deliberate:

- readable code
- explicit contracts
- structured organization
- strong tooling

## Architectural invariants

The following statements should remain true unless the repository's actual implementation changes and all linked docs are updated together:

- readability comes first
- code expresses behavior
- `@type` expresses contracts
- `let` remains the canonical binding form
- explicit `return` remains canonical
- brace imports remain canonical
- `class` remains structure
- `layer` remains capability grouping
- compatibility syntax remains migration support, not co-equal design

## Reading order

For a full picture of Draton, read these in order:

1. [language-manifesto.md](language-manifesto.md)
2. [language-architecture.md](language-architecture.md)
3. [canonical-syntax-rules.md](canonical-syntax-rules.md)
4. [syntax-migration.md](syntax-migration.md)
5. [compiler-architecture.md](compiler-architecture.md)
