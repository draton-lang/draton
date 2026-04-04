# Stage0 Driver Contract

`compiler/driver/` contains the current self-host stage0 pipeline entrypoints.

Current boundary:

- the planned machine-readable interface is `lex --json`, `parse --json`, `typeck --json`, and `build`
- the current executable stage0 implementation lives in the hidden Rust command `drat selfhost-stage0`
- `compiler/main.dt` dispatches the stage0 subcommands into this directory
- `pipeline.dt` currently implements `lex_json` and `parse_json` in Draton
- `parse_stage.dt` owns the self-host parser stage0 payload path and keeps parser JSON aligned with the Rust oracle contract
- `pipeline.dt` still routes `typeck_json` and `build_json` through `host_type_json` and `host_build_json`
- the Rust stage0 command remains the bootstrap and parity wrapper around these files

Current file ownership:

- `pipeline.dt`: full pipeline orchestration
- `parse_stage.dt`: self-host parser stage0 JSON payload generation
- `diagnostics.dt`: human-readable diagnostic formatting
- `options.dt`: compile option parsing and normalization
