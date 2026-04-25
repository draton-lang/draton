# Stage0 Driver Contract

`compiler/driver/` contains the current self-host stage0 pipeline entrypoints.

Current boundary:

- the planned machine-readable interface is `lex --json`, `parse --json`, `typeck --json`, and `build`
- the current executable stage0 implementation lives in the hidden Rust command `drat selfhost-stage0`
- `compiler/main.dt` dispatches the stage0 subcommands into this directory
- `pipeline.dt` currently implements `lex_json`, `parse_json`, and `typeck_json` in Draton
- `pipeline.dt` owns the current bridge-free stage0 parse payload; `parse_stage.dt` remains the planned full self-host parser payload path and must keep parser JSON aligned with the Rust oracle contract before promotion
- `typeck_stage.dt` owns the planned self-host typechecker stage0 payload path and keeps typed-program JSON aligned with the Rust oracle contract
- `pipeline.dt` still routes `build_json` through `host_build_json`
- the hidden Rust stage0 wrapper now dispatches `parse` to bridge-free Draton code in `pipeline.dt`; `typeck` still calls `host_type_json` because the full typechecker stage0 binary does not yet fit the local verification envelope
- the Rust stage0 command remains the bootstrap and parity wrapper around these files

Current file ownership:

- `pipeline.dt`: full pipeline orchestration plus the current bridge-free stage0 parse JSON payload
- `parse_stage.dt`: planned full self-host parser stage0 JSON payload generation
- `typeck_stage.dt`: self-host typechecker stage0 JSON payload generation
- `diagnostics.dt`: human-readable diagnostic formatting
- `options.dt`: compile option parsing and normalization
