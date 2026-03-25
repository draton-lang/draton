# Stage0 Driver Contract

`compiler/driver/` will hold the self-host pipeline entrypoints.

Phase 1 boundary:

- the planned machine-readable interface is `lex --json`, `parse --json`, `typeck --json`, and `build`
- the current executable stage0 implementation lives in the hidden Rust command `drat selfhost-stage0`
- the Rust stage0 command is the parity surface until the Draton driver files in this directory become runnable

Planned file ownership:

- `pipeline.dt`: full pipeline orchestration
- `diagnostics.dt`: human-readable diagnostic formatting
- `options.dt`: compile option parsing and normalization
