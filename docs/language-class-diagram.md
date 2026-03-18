# Draton Class Diagrams

This document provides visual architecture summaries for the Draton language model and the Rust toolchain implementation.

For the full narrative, see [language-architecture.md](language-architecture.md) and [compiler-architecture.md](compiler-architecture.md).

## Language Architecture Diagram

```mermaid
classDiagram
    class Program {
        +modules
        +imports
        +items
    }

    class Module {
        +file-backed unit
        +namespace from directories
    }

    class Import {
        +canonical brace import
        +module path source
    }

    class Class {
        +structure and identity
        +fields
        +layers
        +contracts
    }

    class Layer {
        +capability grouping
        +methods
        +contracts
    }

    class Interface {
        +behavioral contract
        +methods
        +contracts
    }

    class Function {
        +executable behavior
        +explicit return
    }

    class TypeBlock {
        +contract declarations
        +file scope
        +class scope
        +layer scope
        +interface scope
        +function scope
    }

    class Binding {
        +let declaration
        +mutable or immutable
    }

    Program --> Module : organized as
    Module --> Import : declares
    Module --> Class : contains
    Module --> Interface : contains
    Module --> Function : contains
    Module --> TypeBlock : may declare
    Class --> Layer : groups
    Class --> TypeBlock : may declare
    Layer --> Function : groups
    Layer --> TypeBlock : may declare
    Interface --> Function : declares
    Interface --> TypeBlock : canonical contracts
    Function --> Binding : uses
    Function --> TypeBlock : may use local contracts
```

## Language Responsibility Diagram

```mermaid
classDiagram
    class Behavior {
        +function bodies
        +expressions
        +explicit control flow
    }

    class Contract {
        +@type blocks
        +function signatures
        +interface member contracts
    }

    class Structure {
        +class
        +layer
        +module layout
    }

    class Tooling {
        +formatter
        +linter
        +LSP
        +strict syntax checks
    }

    Behavior : code expresses behavior
    Contract : @type expresses contracts
    Structure : class/layer organize capability
    Tooling : enforces canonical surface

    Structure --> Behavior : organizes
    Contract --> Behavior : constrains
    Tooling --> Behavior : formats and diagnoses
    Tooling --> Contract : validates and surfaces
    Tooling --> Structure : keeps consistent
```

## Compiler And Toolchain Diagram

```mermaid
classDiagram
    class DratCLI {
        +build
        +run
        +fmt
        +lint
        +task
        +doc
        +lsp
    }

    class LexerCrate {
        +tokenize source
    }

    class AstCrate {
        +shared syntax structures
    }

    class ParserCrate {
        +parse tokens into AST
    }

    class TypeCheckerCrate {
        +infer and check types
        +apply contracts
    }

    class CodegenCrate {
        +lower typed program to LLVM
    }

    class RuntimeCrate {
        +GC
        +safepoints
        +scheduler
        +runtime ABI
    }

    class StdlibCrate {
        +host-side standard library
    }

    class LspCrate {
        +diagnostics
        +hover
        +definition
        +symbols
        +completion
    }

    class SelfHostMirror {
        +src/lexer
        +src/ast
        +src/parser
        +src/typeck
        +src/codegen
        +src/mono
    }

    DratCLI --> LexerCrate : invokes frontend
    DratCLI --> ParserCrate : invokes frontend
    DratCLI --> TypeCheckerCrate : invokes semantic checks
    DratCLI --> CodegenCrate : invokes backend
    DratCLI --> RuntimeCrate : links against
    DratCLI --> LspCrate : launches
    ParserCrate --> AstCrate : produces
    TypeCheckerCrate --> AstCrate : consumes
    CodegenCrate --> TypeCheckerCrate : consumes typed program
    CodegenCrate --> RuntimeCrate : emits ABI calls for
    RuntimeCrate --> StdlibCrate : integrates with
    SelfHostMirror ..> LexerCrate : mirrors semantics of
    SelfHostMirror ..> ParserCrate : mirrors semantics of
    SelfHostMirror ..> TypeCheckerCrate : mirrors semantics of
    SelfHostMirror ..> CodegenCrate : mirrors semantics of
```

## Interpretation Rules

Use these diagrams with the following constraints in mind:

- Rust frontend/tooling remains authoritative.
- The self-host mirror reflects that behavior; it does not define a competing behavior.
- `@type` is a contract layer, not a second executable syntax family.
- `class` and `layer` are structural architecture, not optional style sugar.
- Compatibility syntax should not be read as a second architecture.

## Reading Order

1. [language-manifesto.md](language-manifesto.md)
2. [language-architecture.md](language-architecture.md)
3. [language-class-diagram.md](language-class-diagram.md)
4. [compiler-architecture.md](compiler-architecture.md)
5. [language-analyst-artifact.md](language-analyst-artifact.md)
