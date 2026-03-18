---
title: Control flow and builtins
sidebar_position: 13
---

# Control flow and builtins

Draton keeps control flow explicit and keeps core system builtins small. This combination is part of the language’s readability-first design.

## Explicit return

Draton keeps `return` visible:

```draton
fn label(flag) {
    if flag {
        return "enabled"
    }
    return "disabled"
}
```

This is not accidental. The repository rules explicitly reject moving toward an implicit-return-only philosophy.

## Visible branching

Conditional logic is meant to read directly from top to bottom. The language favors code that makes branch behavior obvious instead of relying on hidden return conventions.

## Pattern matching, results, and errors

Draton supports structured results and matching. In practical code, the same rule still applies: keep behavior visible, keep contracts where they belong, and keep structural grouping clear.

This is especially important for compiler and tooling code, where the language is often used in this repository.

## Core interactive builtins

The core builtins for interactive I/O are intentionally small:

```draton
print("working")
println("done")
let name = input("Name: ")
```

### `print(...)`

- writes output without forcing a trailing newline
- keeps same-line prompts and progress messages possible

### `println(...)`

- writes output and then ends the line
- the normal choice for line-oriented user output

### `input("Prompt: ")`

- builtin, not stdlib import
- takes exactly one prompt argument
- prints the prompt without a newline
- reads one line from stdin
- trims trailing line endings
- returns a `String`

Example:

```draton
fn main() {
    let name = input("Name: ")
    println(f"Hello {name}")
}
```

## Why builtins stay small

Draton avoids making core I/O feel like a formatting mini-language. The goal is to keep the code readable and keep the compiler/runtime surface straightforward.

That is why the canonical interactive path is:

- `print`
- `println`
- `input`

rather than a large family of formatting syntaxes.

## Builtins versus libraries

Some functionality belongs in libraries. Some belongs in the language/runtime surface because it is part of the minimal interactive model.

`input("...")` belongs to the second category:

- it is a system capability
- it is expected to work without import ceremony
- its semantics are intentionally narrow and predictable

## Related references

- [Language syntax overview](./syntax-overview.md)
- [Language manifesto](../language-manifesto.md)
- [CLI overview](../tooling/cli-overview.md)
