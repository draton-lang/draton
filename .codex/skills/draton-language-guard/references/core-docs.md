# Core docs

Read these before approving syntax-facing changes:

- [`AGENTS.md`](../../../../AGENTS.md)
- [`docs/language-manifesto.md`](../../../../docs/language-manifesto.md)
- [`docs/language-architecture.md`](../../../../docs/language-architecture.md)
- [`docs/canonical-syntax-rules.md`](../../../../docs/canonical-syntax-rules.md)
- [`docs/contributor-language-rules.md`](../../../../docs/contributor-language-rules.md)
- [`docs/syntax-migration.md`](../../../../docs/syntax-migration.md)

## Reject list

Reject or challenge changes that:

- make inline variable types canonical
- make typed parameters canonical
- make inline return types canonical
- introduce a second canonical import style
- weaken explicit `return` into an implicit-return-only philosophy
- contradict the class/layer model
- let docs or examples drift away from parser or typechecker behavior

## Sync checklist

- Update examples in the same task when syntax-facing behavior changes.
- Update `docs/syntax-migration.md` for user-visible syntax changes.
- Coordinate verification with `cargo test -p draton-parser --test items` or `cargo test -p draton-typeck --test errors` when relevant.
