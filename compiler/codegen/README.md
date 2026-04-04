# Self-Host Code Generation

This directory contains the in-tree self-host code generation rewrite.

Current boundary:

- code generation truth still comes from `crates/draton-codegen`
- stage0 `build` currently bridges through `host_build_json` in `compiler/driver/pipeline.dt`
- `compiler/codegen/llvm/` still contains placeholder/stub LLVM-surface files and must not be described as production-ready
- Rust stage0 remains the executable reference while the Draton implementation here is introduced incrementally
