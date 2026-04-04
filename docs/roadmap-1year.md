# Draton 12-Month Roadmap

This roadmap describes the next year of development for Draton based on the repository's current state and its locked language philosophy.

It is a planning document, not a syntax proposal. It assumes the canonical syntax rules, anti-drift policies, and language manifesto remain in force.

Ownership-based memory management is now part of the baseline compiler/runtime model.

## Project status summary

Draton has completed its syntax-stabilization phase for the Rust frontend/tooling path:

- canonical syntax is defined and documented
- deprecated inline type syntax is in compatibility mode, with strict enforcement available
- the Rust frontend/tooling path is authoritative
- the historical `src/` self-host mirror has been retired, and the current rewrite now lives under `compiler/`
- `src/` now belongs to the docs site rather than compiler code

This means Draton is no longer primarily in a syntax-definition phase. It is entering a tooling, ecosystem, and performance phase.

## Strategic direction

Over the next year, Draton should focus on becoming a strong tooling-first language and compiler platform.

The intended direction is:

- make the existing language surface easier to use through strong tooling
- improve diagnostics, formatting, editing support, and bootstrap confidence
- build ecosystem foundations that reinforce, rather than dilute, the canonical language design
- invest in compiler speed and stability so future self-host and external tooling work becomes practical
- evolve carefully only where changes preserve readability-first design and the `@type` contract model

The language should grow by making the current design more usable and more reliable, not by reopening settled syntax questions.

## Phase 1: 0-3 months

Focus: developer tooling fundamentals and stabilization.

Priority work:

- stabilize `drat fmt` so canonical syntax has one predictable formatter
- improve parser/typechecker/codegen diagnostics for common user mistakes
- harden `drat lsp` around diagnostics, hover, and definition lookup
- tighten compiler verification and make future self-host bootstrap work easier to reintroduce
- finish contributor-facing docs for install, syntax, self-host status, and workflow expectations

Concrete engineering steps:

- add formatter regression coverage for canonical syntax forms
- improve deprecation diagnostics so compatibility-mode warnings stay actionable and concise
- add more LSP smoke and integration tests around canonical syntax constructs
- keep syntax/tooling CI fast and trustworthy while the in-tree self-host rewrite is still bridged through Rust for major stages
- make docs/examples consistently runnable from the repository root

Expected outcome:

- new contributors can format, build, and inspect Draton code with low friction
- canonical syntax becomes easier to maintain because tooling enforces it by default

## Phase 2: 3-6 months

Focus: ecosystem infrastructure and workflow ergonomics.

Priority work:

- define a practical package/dependency story around `drat`
- improve CLI ergonomics for common project workflows
- add plugin or extension points for tooling integration where this can be done without weakening core language rules
- build small task/build automation utilities in Draton itself

Concrete engineering steps:

- make dependency management commands predictable and well-documented
- improve project layout conventions, templates, and lockfile behavior
- expose stable hooks for tooling-oriented extensions such as format/lint/doc/test workflows
- add example projects showing how Draton can drive structured automation without inventing new syntax

Expected outcome:

- Draton becomes easier to adopt for small real projects
- the ecosystem story becomes concrete enough for external contributors to build on

## Phase 3: 6-9 months

Focus: compiler performance and maturity.

Priority work:

- improve type inference performance on larger programs
- improve code generation performance and backend throughput
- reduce future self-host bootstrap time as the rewrite matures
- introduce incremental or cache-aware build capabilities where technically justified

Concrete engineering steps:

- profile the Rust frontend with representative workloads and define rewrite-ready self-host benchmarks
- remove obvious hot paths in inference, monomorphization, and code generation
- make build outputs and intermediate artifacts easier to reuse safely
- measure strict-canonical checks and compiler workload cost in CI over time

Expected outcome:

- Draton feels materially faster in normal edit-build-run cycles
- the compiler gets faster today without blocking a future self-host rewrite

## Phase 4: 9-12 months

Focus: ecosystem adoption and flagship tooling.

Priority work:

- ship at least one production-quality official tool written in Draton
- grow a small official library set that supports real usage without bloating the language surface
- improve tutorials, reference docs, and onboarding for outside contributors
- strengthen release quality and user-facing install/verification flows

Concrete engineering steps:

- select one flagship tool that demonstrates Draton's tooling-first value
- identify a narrow set of official libraries with clear maintenance value
- add end-to-end tutorials that reflect canonical syntax and real repository workflows
- make contribution guides and issue triage easier for external engineers

Expected outcome:

- Draton demonstrates value through working tools, not just compiler internals
- external contributors can onboard without relearning hidden repository rules

## Non-goals

The roadmap explicitly does not prioritize the following unless a strong repository-specific justification emerges:

- large syntax redesign
- undoing or weakening canonical syntax rules
- treating compatibility syntax as a second supported philosophy
- feature bloat for its own sake
- broad language-surface expansion without strong tooling or ecosystem need
- speculative syntax additions just to match other languages
- reopening settled questions around `let`, explicit `return`, brace imports, `@type`, or class/layer structure

## Success indicators

Signals that the roadmap is working:

- `drat fmt` is stable enough to be trusted in normal development and CI
- `drat lsp` provides reliable diagnostics and basic navigation in real Draton files
- compiler verification is repeatable and future self-host bootstrap work has a clear re-entry path
- at least one official production-quality tool is built in Draton
- package/dependency workflows are usable enough for small real projects
- bootstrap and compile performance are measurably improved over current baselines
- external contributors submit PRs for tooling, docs, or ecosystem work without needing syntax clarification first

## Alignment with repository philosophy

This roadmap preserves the repository's design commitments:

- canonical syntax remains stable rather than being reopened for experimentation
- readability-first code remains the default expectation
- `@type` stays a contract layer, not mandatory inline syntax
- strict mode and CI remain anti-drift mechanisms, not optional polish
- tooling maturity reinforces the language philosophy by making the canonical style easier to use correctly

In short:

- code continues to express behavior
- `@type` continues to express contracts
- tooling, docs, and CI make that split easier to maintain over time
