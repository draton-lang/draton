# Draton Self-Host Checklist

This checklist tracks the transition from the current Rust-authoritative repository to a Draton-first toolchain and the long-term goal of a repository that contains Draton plus assembly/bootstrap glue only.

The checklist is intentionally operational:

- every item must be either verifiable now or blocked by a named technical reason
- every phase has explicit exit gates
- every blocker has a reproduction command
- every completed tranche should update this file and land as its own commit

## Status legend

- `[x]` done
- `[-]` in progress
- `[ ]` not started
- `[!]` blocked by a known issue

## How To Use This Checklist

Use this file as the single execution board for self-host work.

- update `Current snapshot` after every meaningful tranche
- do not mark an item done unless its verification command has been rerun
- if an item regresses, change it back from `[x]` to `[!]` or `[-]`
- when a blocker is narrowed, replace vague text with the smallest confirmed failing scope
- when a phase gate passes, record the commit that first made it pass

## Progress Ledger

These are the committed tranches already landed during the current self-host push.

- `[x]` `ffa5374` `fix: improve self-host bootstrap diagnostics and linux codegen path`
  - `tools/verify_stage2.py` now reports signal-based failures
  - self-host Linux link path no longer hardcodes Windows-only libraries
  - self-host backend emits a native `main(argc, argv)` wrapper around `draton_user_main`
  - self-host textual LLVM uses real newlines and `double` / `float`
- `[x]` `a0a4775` `docs: add self-host progress checklist`
  - initial self-host roadmap and gate structure added
- `[x]` `e1cb534` `docs: track current self-host blockers`
  - focused blocker harness added at `tools/repro_selfhost_blockers.py`
  - parser crash narrowed to `header + main()` extracted from `src/main.dt`
- `[x]` `e36ee91` `fix: unblock self-host hello world codegen`
  - self-host string literal LLVM escaping and terminator emission fixed
  - self-host `print` / `println` now lower to runtime symbols with fallback definitions
  - stage1 now builds and runs `examples/hello.dt` on Linux
- `[x]` `a0c297f` `test: check in self-host parser repro fixture`
  - parser repro fixture checked in at `tests/programs/selfhost/parser_header_plus_main.dt`
  - blocker harness now uses the checked-in fixture instead of a temp file
- `[x]` `d76c576` `tools: add self-host parser backtrace helper`
  - gdb-based parser backtrace helper added at `tools/capture_selfhost_parser_bt.py`
  - checklist now tracks a stable stack for the checked-in parser repro
- `[x]` `263f5ce` `test: narrow self-host parser repro to main prefix-4`
  - automated prefix probe added at `tools/probe_selfhost_main_prefixes.py`
  - first crashing `main()` prefix identified as `prefix-4`
  - blocker harness now points at `tests/programs/selfhost/parser_main_prefix4.dt`
- `[x]` `843c27e` `tools: probe self-host parser statement interactions`
  - automated subset probe added at `tools/probe_selfhost_main_subsets.py`
  - all probed strict subsets of the first 4 statements pass
  - only the full 4-statement interaction crashes in the subset probe
- `[x]` `a124b01` `tools: contrast self-host parser call-return contexts`
  - return-call variants now have a dedicated probe inside `parser_main_prefix4`
  - minimal standalone call-return shapes now have a contrast probe
  - current evidence points to a context-sensitive parser interaction, not a generic call-expression crash
- `[x]` `1c099a6` `tools: probe self-host parser header dependencies`
  - automated header dependency probe added at `tools/probe_selfhost_header_dependencies.py`
  - all proper header subsets pass for `parser_main_prefix4`
  - only the full `imports + class + @type + main` combination crashes
- `[x]` `a5df47c` `fix: harden self-host postfix lookahead rooting`
  - rooted `expr`, `tokens`, and lookahead token kinds in the self-host postfix/type-arg/class-literal path
  - baseline strict subset and focused Rust parser/typecheck tests still pass
  - parser crash signature remained unchanged on all current self-host probes
- `[x]` `79c4c60` `tools: probe self-host parser header payloads`
  - automated header payload probe added at `tools/probe_selfhost_header_payloads.py`
  - shrinking either the `class ParsedArgs` payload or the `@type` payload removes the crash
  - this further supports a context/state accumulation bug over a local grammar bug
- `[x]` `9616f54` `tools: probe self-host parser header thresholds`
  - automated threshold probe added at `tools/probe_selfhost_header_thresholds.py`
  - the crash starts at 2 class fields and at 16 top-level `@type` entries
  - the bug now has measurable payload thresholds, not just binary full/mini conditions
- `[x]` `b2b6e1c` `tools: probe self-host parser stmt1 variants`
  - automated first-statement variant probe added at `tools/probe_selfhost_stmt1_variants.py`
  - simple statement-1 variants pass, while binary-expression statement-1 variants preserve the crash
  - this points to expression-shape interaction in statement 1 rather than mere statement count
- `[x]` `113b6ef` `tools: probe self-host parser stmt1 operator families`
  - automated operator-family probe added at `tools/probe_selfhost_stmt1_operator_families.py`
  - all probed binary-operator families preserve the crash in statement 1
  - this suggests operator family is not the distinguishing variable once statement 1 has the bad shape
- `[x]` `8ff9bb7` `tools: probe self-host parser stmt1 body variants`
  - automated body-shape probe added at `tools/probe_selfhost_stmt1_body_variants.py`
  - once statement 1 has the bad condition shape, all probed non-empty bodies preserve the crash
  - body emptiness is now a concrete distinguishing variable for statement 1
- `[x]` `178d5f6` `tools: probe self-host parser stmt3/stmt4 variants`
  - automated statement-3/4 probe added at `tools/probe_selfhost_stmt34_variants.py`
  - both command branches now show the same split between safe simplifications and crashing high-pressure condition/return pairs
  - the parser bug is no longer framed as something uniquely tied to literal `build` / `run` branch names
- `[x]` `7e95637` `tools: probe self-host parser stmt3/stmt4 return shapes`
  - automated statement-3/4 return-shape probe added at `tools/probe_selfhost_stmt34_return_shapes.py`
  - under the original command-branch condition, only ident returns and ungrouped zero-arg calls pass
  - adding grouping or moving to one-arg, wrapper, or nested calls is enough to bring the crash back in both branches
- `[x]` `8856244` `tools: probe self-host parser stmt3/stmt4 grouped returns`
  - automated grouped-return probe added at `tools/probe_selfhost_stmt34_grouped_returns.py`
  - under the original command-branch condition, `return (cmd)` and `return (0)` already crash
  - this narrows the parser suspicion from “grouped calls” down to grouped return expressions more generally
- `[x]` `0e03497` `tools: probe self-host parser stmt3/stmt4 grouped contexts`
  - automated grouped-context probe added at `tools/probe_selfhost_stmt34_grouped_contexts.py`
  - under the original command-branch condition, grouped ident/literal/zero-arg-call expressions fail in `return`, `let`, and bare-expression positions
  - the same grouped expressions pass again once the condition is simplified away from `str_eq_main(cmd, ...)`
- `[x]` `fbdd368` `tools: probe self-host parser stmt3/stmt4 condition shapes`
  - automated condition-shape probe added at `tools/probe_selfhost_stmt34_condition_shapes.py`
  - grouped stmt3/stmt4 bodies fail under `str_eq_main(...)`-family conditions even when the arguments are changed to `cmd`, literals, or `cli_arg(1)`
  - the same grouped bodies still pass under simpler call and binary conditions such as `ready()`, `collect_cli_args(2)`, `cli_arg(1)`, and `1 < 2`
- `[x]` `451dc4a` `tools: probe self-host parser stmt3/stmt4 condition arities`
  - automated condition-arity probe added at `tools/probe_selfhost_stmt34_condition_arities.py`
  - grouped stmt3/stmt4 bodies fail for every multi-argument call-like condition shape probed so far, including generic `foo(...)`, qualified calls, and nested two-arg calls
  - zero-arg and one-arg call conditions still pass, so the condition side is now narrowed from `str_eq_main(...)` to multi-argument call parsing more generally
- `[x]` `0143386` `tools: probe self-host parser stmt3/stmt4 condition commas`
  - automated comma-bearing condition probe added at `tools/probe_selfhost_stmt34_condition_commas.py`
  - grouped stmt3/stmt4 bodies also fail for tuple, array, map-like, multi-index, and comma-bearing nested expressions in the condition
  - single-index conditions still pass, so the current tightest condition-side narrowing is now “comma-bearing condition expression”, not just multi-argument calls
- `[x]` `b55e291` `tools: probe self-host parser stmt3/stmt4 branch dependency`
  - automated branch-dependency probe added at `tools/probe_selfhost_stmt34_branch_dependency.py`
  - one bad stmt3/stmt4 branch still crashes if the sibling branch remains in its original crashing form
  - replacing both branches with the same bad grouped-body shape, or deleting the sibling branch entirely, clears the crash
- `[x]` `416120a` `tools: probe self-host parser stmt3/stmt4 order spacing`
  - automated order/spacing probe added at `tools/probe_selfhost_stmt34_order_spacing.py`
  - mixed stmt3/stmt4 pairs still crash when their order is swapped
  - inserting a neutral `let gap = cmd` between the mixed branches does not clear the crash
- `[x]` `1b7fdee` `docs: record parser stmt3/stmt4 order spacing`
  - checklist updated with the order/spacing probe command, finding, and follow-up task
- `[x]` `88f9535` `tools: probe self-host parser stmt3/stmt4 adjacency`
  - automated adjacency probe added at `tools/probe_selfhost_stmt34_adjacency.py`
  - adjacent `both-bad` stmt3/stmt4 pairs still pass in either order
  - inserting one neutral statement makes even the `both-bad` pair crash, which first exposed the importance of barriers between the two branches
- `[x]` `5390a3c` `tools: probe self-host parser stmt3/stmt4 layout barriers`
  - automated layout-only barrier probe added at `tools/probe_selfhost_stmt34_layout_only_barriers.py`
  - adjacent `both-bad` stmt3/stmt4 pairs still pass across blank lines and line/doc/block comments
  - layout-only barriers do not change the outcome, so source spacing is not the key variable
- `[x]` `d7c9483` `tools: probe self-host parser stmt3/stmt4 intervening statements`
  - automated intervening-statement probe added at `tools/probe_selfhost_stmt34_intervening_statements.py`
  - every probed intervening statement shape makes the `both-bad` stmt3/stmt4 pair crash
  - this sharpens the adjacency result into a statement-boundary result, not a special neutral-statement-shape result
- `[x]` `531412f` `tools: probe self-host parser stmt3/stmt4 empty boundaries`
  - automated empty-boundary probe added at `tools/probe_selfhost_stmt34_empty_boundaries.py`
  - empty blocks leave the adjacent `both-bad` stmt3/stmt4 pair passing, but semicolons and empty control-flow statements still make it crash
  - this supersedes the earlier “any intervening statement” phrasing with a more precise distinction between harmless empty blocks and harmful parsed statement paths
- `[x]` `cedf0d9` `tools: probe self-host parser stmt3/stmt4 block separators`
  - automated block-separator probe added at `tools/probe_selfhost_stmt34_block_separators.py`
  - only a plain empty block leaves the `both-bad` stmt3/stmt4 pair passing
  - non-empty blocks and annotated empty blocks still make it crash, which points more specifically at parse paths around block statements rather than braces alone
- `[x]` `635b3d0` `tools: probe self-host parser stmt3/stmt4 plain block only`
  - automated plain-block-only probe added at `tools/probe_selfhost_stmt34_plain_block_only.py`
  - only a bare `{}` block is harmless between the `both-bad` stmt3/stmt4 pair
  - adding a semicolon or any statement wrapper makes it crash again, which points directly at the `parse_stmt` `LBrace -> parse_block` fast path as the current unique harmless separator
- `[x]` `4515021` `tools: probe self-host parser stmt3/stmt4 spawn block fast path`
  - automated spawn-block fast-path probe added at `tools/probe_selfhost_stmt34_spawn_block_fast_path.py`
  - among probed variants, `spawn {}` is the only wrapper that shares the harmless empty-block behavior with bare `{}`
  - adding a semicolon, content, an expression body, or a different wrapper makes it crash again, so the harmless path is now narrowed to two empty-block fast paths instead of one
- `[x]` `e346d22` `tools: probe self-host parser stmt3/stmt4 doc comment blocks`
  - automated doc-comment-block probe added at `tools/probe_selfhost_stmt34_doc_comment_blocks.py`
  - line-comment-only blocks preserve the harmless empty-block behavior, but doc-comment-only blocks still crash
  - this is the strongest current clue that `parser_skip_doc_comments` and doc-comment token handling matter more than raw source text layout inside the harmless block paths
- `[x]` `da7abbd` `tools: probe self-host parser stmt3/stmt4 doc comment positions`
  - automated doc-comment-position probe added at `tools/probe_selfhost_stmt34_doc_comment_positions.py`
  - the same doc-comment tokens are harmless before, after, or between the harmless plain/spawn empty-block separators
  - doc comments only become harmful when they are the sole contents of those otherwise harmless block fast paths, which sharply narrows the suspect path around `parse_block` plus `parser_skip_doc_comments`
- `[x]` `ad23799` `tools: probe standalone doc comment only blocks`
  - automated standalone doc-comment-only block probe added at `tools/probe_selfhost_doc_comment_only_blocks.py`
  - minimal standalone plain/spawn doc-comment-only blocks fail with `invalid expression at line 4, col 5`
  - this elevates the finding from a self-host interaction clue to a general parser bug around doc-comment-only blocks
- `[x]` `78234a8` `fix: skip doc-comment-only blocks before parsing statements`
  - Rust and self-host `parse_block` now re-check `RBrace` / `Eof` immediately after `parser_skip_doc_comments`
  - Rust parser regression test now covers doc-comment-only plain/spawn blocks
  - standalone plain/spawn doc-comment-only blocks now parse, while the main self-host `prefix-4` crash still remains
- `[x]` `4b48119` `tools: probe self-host parser stmt3/stmt4 spawn doc contexts`
  - automated spawn-doc-context probe added at `tools/probe_selfhost_stmt34_spawn_doc_contexts.py`
  - after the `parse_block` fix, `spawn { /// gap }` only changes outcomes inside the `both-bad` stmt3/stmt4 pair
  - outside that narrow context it behaves like an ordinary harmless separator or standalone statement, so the remaining doc-comment issue is much smaller than before
- `[x]` `5753ded` `tools: probe self-host parser stmt3/stmt4 spawn doc presence`
  - automated spawn-doc-presence probe added at `tools/probe_selfhost_stmt34_spawn_doc_presence.py`
  - inside the `both-bad` stmt3/stmt4 context, any probed doc-comment presence inside `spawn { ... }` restores the crash
  - doc-comment-free and line-comment-only spawn blocks still pass there, so the residual spawn issue is now narrowed to doc-comment token presence rather than block contents in general
- `[x]` `d770374` `fix: harden self-host spawn parser rooting`
  - added extra self-host roots around `parse_spawn_stmt`, `stmt_from_spawn`, `spawn_body_expr`, `spawn_body_block`, and `new_spawn_stmt`
  - rebuilt stage1 and reran `tools/probe_selfhost_stmt34_spawn_doc_contexts.py` plus `tools/repro_selfhost_blockers.py`
  - no observable behavior change: the residual `spawn { /// gap }` mixed-context crash and the main `prefix-4` crash class both remain
- `[x]` `d968701` `tools: compare self-host spawn-doc backtraces`
  - added `tools/probe_selfhost_spawn_doc_backtrace.py` to compare the main `parser_main_prefix4` crash stack against the residual `both-bad + spawn { /// gap }` stack
  - the main `prefix-4` crash still runs through `parse_return_stmt -> parse_arg_list -> parse_postfix`
  - the residual spawn-doc crash is much shallower, stopping at `parser_current -> parser_current_kind -> parser_is_eof -> parser_parse`, so these should be treated as distinct crash paths until proven otherwise
- `[x]` `beae2dc` `tools: probe self-host spawn-doc tail functions`
  - added `tools/probe_selfhost_spawn_doc_tail_functions.py` to measure how the residual `both-bad + spawn { /// gap }` corruption reacts to later top-level items
  - later `class` / `@type` items and later empty or comment-only tail functions keep the residual case at `SIGSEGV`
  - later tail functions with non-empty bodies flip the residual case to `SIGABRT` with huge allocation failures, pointing at a second corruption path that only appears after re-entering `parse_block` / `parse_stmt` in a subsequent function body
- `[x]` `1b6d91f` `tools: probe self-host spawn-doc tail stmt shapes`
  - added `tools/probe_selfhost_spawn_doc_tail_stmt_shapes.py` to narrow the later-tail-function split by statement shape
  - the later tail-function path does **not** flip to OOM for every non-empty body: `let`, empty control-flow, `spawn expr`, and call/grouped return forms stay on `SIGSEGV`
  - the later tail-function path does flip to `SIGABRT` / OOM for bare call or grouped expression statements, literal returns, and `spawn {}`
- `[x]` `facec30` `tools: compare self-host spawn-doc tail backtraces`
  - added `tools/probe_selfhost_spawn_doc_tail_backtraces.py` to compare residual base, later-tail `SIGSEGV`, and later-tail `SIGABRT/OOM` parser stacks
  - the bare residual case still dies at `parser_current -> parser_current_kind -> parser_is_eof -> parser_parse`
  - later non-OOM tail variants die one level deeper at `parser_current -> parser_current_kind -> parser_is_eof -> parse_block -> parse_fn_def -> parser_parse`, while later OOM variants converge on `parser_record_error -> parser_error_unexpected -> parse_primary -> parse_postfix -> parse_expr_stmt_or_assignment`
- `[x]` `333ad26` `tools: probe self-host spawn-doc tail dispatch`
  - added `tools/probe_selfhost_spawn_doc_tail_dispatch.py` to compare statement-dispatch paths for later-tail `SIGSEGV` vs later-tail `SIGABRT/OOM` variants
  - later-tail OOM variants unexpectedly funnel through `parse_expr_stmt_or_assignment` regardless of source spelling (`return 0`, bare `ready()`, `spawn {}`)
  - later-tail non-OOM variants like `return ready()` and `let x = 0` die earlier in `parse_block -> parse_fn_def -> parse_item -> parser_parse` without ever reaching `parse_stmt`, which strongly suggests parser-state drift before statement dispatch
- `[x]` `386e956` `tools: probe self-host spawn-doc tail keyword dispatch`
  - added `tools/probe_selfhost_spawn_doc_tail_keyword_dispatch.py` to classify keyword-led tail statements by the parser path they now reach
  - none of the probed `return` / `spawn` / `let` tail statements still reach their dedicated parsers in the residual spawn-doc context
  - `return 0` and `spawn {}` fall through `parse_expr_stmt_or_assignment`, `let x = ready()` / `let x = (0)` are misrouted into `parse_if_stmt_tail`, and the remaining probed keyword-led variants die earlier before `parse_stmt` dispatch
- `[x]` `05cc3bd` `tools: map self-host spawn-doc tail dispatch matrix`
  - added `tools/probe_selfhost_spawn_doc_tail_dispatch_matrix.py` to map `return/spawn/let` tail variants across `ident/literal/call/grouped` shapes
  - keyword alone does not predict the parser path anymore in the residual spawn-doc family
  - only literal `return` and empty-block `spawn` fall into expr-dispatch; only `let` with call/grouped values misroutes into `parse_if_stmt_tail`; all remaining ident/call/grouped `return/spawn` plus ident/literal `let` forms die earlier before statement dispatch
- `[x]` `8aaa055` `tools: probe self-host spawn-doc tail body prefixes`
  - added `tools/probe_selfhost_spawn_doc_tail_body_prefixes.py` to measure how leading layout-only vs real statements in the tail body change dispatch buckets
  - layout-only prefixes (blank line, line comment) do not change buckets, but leading `{}`, `spawn {}`, and `let warm = 0` do
  - the same target statement can jump across all three bad buckets depending on the first real statement: for example `return ready()` is `pre-dispatch` by default, becomes `if-tail-dispatch` after a leading `{}`, and becomes `expr-dispatch` after a leading `spawn {}` or `let warm = 0`
- `[x]` `79a0a5a` `tools: probe self-host spawn-doc tail prefix interactions`
  - added `tools/probe_selfhost_spawn_doc_tail_prefix_interactions.py` to compare prefix statements alone vs the same prefixes when followed by a second tail statement
  - the tested prefixes are not uniformly harmful on their own: `{}` and `let warm = 0` alone stay `pre-dispatch`, while `spawn {}` alone already hits `expr-dispatch`
  - the bucket of the following statement depends on the exact prefix+target pair, which makes the residual tail-body path a real two-statement state machine rather than a one-statement rule
- `[x]` `b59da86` `tools: contrast self-host spawn-doc contexts`
  - added `tools/probe_selfhost_spawn_doc_context_contrast.py` to compare representative tail-body prefix pairs across `orig`, `both-bad`, and `residual` contexts
  - the same tail-body pair does not have one stable parser sink across contexts
  - this rules out the idea that the tail-body state machine is just a generic parser quirk; it depends on context poisoning that only fully appears after the residual spawn-doc step
- `[x]` `250a119` `tools: probe self-host spawn-doc header thresholds`
  - added `tools/probe_selfhost_spawn_doc_header_thresholds.py` to connect a representative residual tail-body pair back to class/@type header pressure
  - for `spawn {} + return ready()`, the full class payload is required before the pair reaches the OOM branch
  - the `@type` payload now has three measured phases at class5: low counts pass, mid counts `SIGSEGV`, and only the full 22-entry block reaches OOM

## Current Snapshot

Last refreshed: `2026-03-22`

### What is currently true

- `[x]` Self-host compiler mirror exists under `src/`
- `[x]` Strict canonical self-host syntax check exists
- `[x]` Rust stage0 can build `src/main.dt` into a stage1 self-host binary
- `[x]` Stage2 verification now reports crash signals explicitly instead of failing silently
- `[x]` A focused blocker harness exists at `tools/repro_selfhost_blockers.py`
- `[x]` Self-host Linux link path no longer hardcodes Windows-only libraries
- `[x]` Self-host backend now emits a `main(argc, argv)` wrapper around `draton_user_main`
- `[x]` Self-host textual LLVM backend now emits real newlines and uses `double` / `float` instead of `f64` / `f32`
- `[x]` Self-host stage1 now builds `examples/hello.dt` successfully on Linux
- `[x]` Self-host-built `hello` binary now runs and prints `hello, draton!`
- `[x]` A checked-in parser repro fixture exists at `tests/programs/selfhost/parser_header_plus_main.dt`
- `[x]` A smaller checked-in parser repro fixture exists at `tests/programs/selfhost/parser_main_prefix4.dt`
- `[x]` A dedicated parser backtrace helper exists at `tools/capture_selfhost_parser_bt.py`
- `[x]` Prefix probing shows the first crashing `main()` prefix is `prefix-4`
- `[x]` Subset probing shows the crash needs all first 4 statements together; all probed strict subsets pass
- `[x]` Return-call variant probing inside `prefix-4` shows only `return 0` passes; all probed call-bearing return variants crash
- `[x]` Minimal standalone return/call shapes all pass, so the crash is not a generic call-expression parse failure
- `[x]` Header dependency probing shows only the full `imports + class + @type + main` fixture crashes; all proper header subsets pass
- `[x]` Header payload probing shows shrinking either the `class ParsedArgs` payload or the `@type` payload removes the crash
- `[x]` Header threshold probing shows the crash starts at 2 class fields and at 16 top-level `@type` entries
- `[x]` Statement-1 probing shows only `if` conditions with binary expressions and non-empty bodies preserve the crash
- `[x]` Statement-1 operator-family probing shows all probed binary-operator families preserve the crash
- `[x]` Statement-1 body probing shows that once the bad condition shape is present, any probed non-empty body preserves the crash
- `[x]` Statement-3/4 probing shows both branches only preserve the crash under high-pressure condition+call-return pairs; simpler conditions or non-call returns clear it
- `[x]` Statement-3/4 return-shape probing shows the original command-branch condition only tolerates ident returns and ungrouped zero-arg calls; grouping or any more complex call shape restores the crash
- `[x]` Statement-3/4 grouped-return probing shows grouping alone is already sufficient to restore the crash for ident, literal, and zero-arg-call returns
- `[x]` Statement-3/4 grouped-context probing shows grouped expressions fail across `return`, `let`, and bare-expression positions under the original `str_eq_main(cmd, ...)` condition; later probes narrow that further to multi-argument call-like conditions more generally
- `[x]` Statement-3/4 condition-shape probing shows grouped bodies fail under `str_eq_main(...)`-family conditions, not under every call condition
- `[x]` Statement-3/4 condition-arity probing shows grouped bodies fail for multi-argument call-like conditions, while zero-arg and one-arg call conditions still pass
- `[x]` Statement-3/4 condition-comma probing shows grouped bodies fail for comma-bearing condition expressions more generally, while single-index conditions still pass
- `[x]` Statement-3/4 branch-dependency probing shows the crash is strongest in a mixed pair: one bad grouped-body branch plus one original sibling branch
- `[x]` Statement-3/4 order/spacing probing shows the mixed-pair crash survives branch reordering and an inserted neutral statement
- `[x]` Statement-3/4 adjacency probing first showed that an intervening neutral statement is enough to make even the `both-bad` pair crash
- `[x]` Statement-3/4 layout-only barrier probing shows `both-bad` pairs still pass across blank lines and comments, so only intervening statements change the result
- `[x]` Statement-3/4 intervening-statement probing shows every probed non-block intervening statement shape makes the `both-bad` pair crash
- `[x]` Statement-3/4 empty-boundary probing shows empty blocks do not disturb the passing `both-bad` pair, while semicolons and empty control-flow statements do
- `[x]` Statement-3/4 block-separator probing shows only a plain empty block is harmless; non-empty blocks and annotated empty blocks still crash
- `[x]` Statement-3/4 plain-block-only probing shows a bare `{}` is a harmless separator only when it has no semicolon and no wrapper
- `[x]` Statement-3/4 spawn-block fast-path probing shows that, among probed variants, only bare `{}` and `spawn {}` share the harmless empty-block behavior
- `[x]` Statement-3/4 doc-comment-block probing shows line-comment-only blocks stay harmless but doc-comment-only blocks still crash
- `[x]` `parse_block` in both Rust and self-host now re-checks `RBrace` / `Eof` after skipping doc comments
- `[x]` Minimal standalone plain/spawn doc-comment-only blocks now parse successfully after the `parse_block` fix
- `[x]` In the mixed self-host stmt3/stmt4 interaction, `doc-only-plain-block` now passes but `doc-only-spawn-block` still crashes
- `[x]` After the `parse_block` fix, `spawn { /// gap }` only changes outcomes inside the `both-bad` stmt3/stmt4 pair and is otherwise harmless
- `[x]` Inside that residual `both-bad` case, any probed doc-comment presence inside `spawn { ... }` is enough to restore the crash
- `[x]` The residual `both-bad + spawn { /// gap }` crash does not share the main `prefix-4` backtrace; it currently dies much earlier in `parser_current -> parser_current_kind -> parser_is_eof -> parser_parse`
- `[x]` The residual `both-bad + spawn { /// gap }` case stays at `SIGSEGV` across later `class` / `@type` items and later empty/comment-only tail functions, but flips to `SIGABRT` / OOM once a later top-level function has a non-empty body
- `[x]` That later-tail-function `SIGABRT` / OOM path is statement-shape-sensitive: bare call/grouped expression statements, literal returns, and `spawn {}` trigger it, while `let`, empty control-flow, `spawn expr`, and call/grouped returns do not
- `[x]` The residual spawn-doc family now has three measured stack depths: bare residual (`parser_parse`), later-tail non-OOM (`parse_block -> parse_fn_def`), and later-tail OOM (`parser_record_error -> parser_error_unexpected -> parse_primary -> parse_postfix -> parse_expr_stmt_or_assignment`)
- `[x]` Later-tail OOM variants are not following their source-level statement parsers anymore: even `return 0` and `spawn {}` currently fall through the `parse_expr_stmt_or_assignment` sink, while later-tail non-OOM variants never make it as far as `parse_stmt`
- `[x]` Keyword-led tail statements now split across three bad paths in the residual spawn-doc family: expr-dispatch drift (`return 0`, `spawn {}`), `parse_if_stmt_tail` drift (`let x = ready()`, `let x = (0)`), and pre-dispatch death (`return ready()`, `return (0)`, `spawn ready()`, `spawn (0)`, `let x = 0`)
- `[x]` The updated dispatch matrix shows keyword is no longer the primary predictor; the `keyword + operand-shape` combination now decides between expr-dispatch drift, `parse_if_stmt_tail` drift, and pre-dispatch death
- `[x]` Tail-body prefixes add another dimension: layout-only prefixes leave buckets unchanged, but real leading statements can move the same target statement between `pre-dispatch`, `expr-dispatch`, and `parse_if_stmt_tail`
- `[x]` Prefix statements do not have a fixed global severity: `{}` and `let warm = 0` are `pre-dispatch` alone, `spawn {}` is `expr-dispatch` alone, and each prefix changes the following statement differently depending on the pair
- `[x]` The residual tail-body state machine is context-dependent, not generic: representative `prefix + target` pairs land in different first-parse sinks across `orig`, `both-bad`, and `residual` contexts
- `[x]` The residual tail-body family also depends on header pressure: for `spawn {} + return ready()`, full class payload is needed before OOM appears, and the `@type` payload now splits into pass / `SIGSEGV` / OOM regions instead of a single threshold
- `[-]` Extra self-host spawn-path rooting hardening was tried and did not change the residual `spawn { /// gap }` mixed-context crash or the main `prefix-4` crash
- `[-]` Extra self-host `parser_skip_doc_comments` rooting hardening was tried locally and also did not change the residual `spawn { /// gap }` crash or the main `prefix-4` crash, so that experiment was backed out to avoid adding noise on top of the already-dirty parser file
- `[-]` Targeted rooting hardening in self-host postfix/lookahead parsing was tried and did not change the crash signature
- `[x]` Temporarily disabling `parser_looks_like_type_args_before_class_literal` did not change any current parser probe result
- `[!]` Stage1 `check src/main.dt` still crashes with `SIGSEGV`
- `[!]` Stage1 `ast-dump src/main.dt` still crashes with `SIGSEGV`
- `[!]` Stage1 `ast-dump` on `tests/programs/selfhost/parser_main_prefix4.dt` still crashes with `SIGSEGV`

### Current blocker matrix

| Workstream | Repro command | Current result | Notes |
| --- | --- | --- | --- |
| Parser self-check | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `check-src-main -> -11` | Current crash class is `SIGSEGV` |
| Parser AST dump | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `ast-dump-src-main -> -11` | Same failure class as self-check |
| Reduced parser repro | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `ast-dump-main-prefix4 -> -11` | Harness now points at `tests/programs/selfhost/parser_main_prefix4.dt` |
| Prefix probe | `python3 tools/probe_selfhost_main_prefixes.py --stage1 /tmp/draton_s1` | `first failing prefix: 4` | Prefixes 1-3 pass; prefix 4 is the first crash |
| Subset probe | `python3 tools/probe_selfhost_main_subsets.py --stage1 /tmp/draton_s1` | `first failing subset: stmt1_if_argc+stmt2_let_cmd+stmt3_if_build+stmt4_if_run` | All probed strict subsets of the first 4 statements pass |
| Return-call variants | `python3 tools/probe_selfhost_return_call_variants.py --stage1 /tmp/draton_s1` | `only the non-call return variant passes inside parser_main_prefix4` | Replacing the crashing return with other call-bearing forms still crashes |
| Minimal return shapes | `python3 tools/probe_selfhost_minimal_return_shapes.py --stage1 /tmp/draton_s1` | `all minimal standalone return/call shapes pass` | The bug needs accumulated parser context, not just `return foo(2)` alone |
| Header dependencies | `python3 tools/probe_selfhost_header_dependencies.py --stage1 /tmp/draton_s1` | `only the full imports+class+type+main fixture fails` | This points toward a context/state pressure bug, not a single syntax form in isolation |
| Header payloads | `python3 tools/probe_selfhost_header_payloads.py --stage1 /tmp/draton_s1` | `shrinking either the class payload or the @type payload removes the crash` | The bug depends on accumulated header payload, not only section presence |
| Header thresholds | `python3 tools/probe_selfhost_header_thresholds.py --stage1 /tmp/draton_s1` | `first failing class field count: 2; first failing type entry count: 16` | The crash has measurable payload thresholds rather than only binary full/mini behavior |
| Statement-1 variants | `python3 tools/probe_selfhost_stmt1_variants.py --stage1 /tmp/draton_s1` | `stmt1 only preserves the crash when a binary-expression condition appears inside an if with a non-empty body` | Bare/let binary expressions and empty-body `if` variants pass |
| Statement-1 operator families | `python3 tools/probe_selfhost_stmt1_operator_families.py --stage1 /tmp/draton_s1` | `all probed binary-operator families preserve the crash in stmt1` | Operator family is not the distinguishing variable inside statement 1 |
| Statement-1 body variants | `python3 tools/probe_selfhost_stmt1_body_variants.py --stage1 /tmp/draton_s1` | `once stmt1 has the bad binary-condition shape, any probed non-empty body preserves the crash` | Body emptiness is the key variable after the condition shape is fixed |
| Statement-3/4 variants | `python3 tools/probe_selfhost_stmt34_variants.py --stage1 /tmp/draton_s1` | `stmt3/stmt4 only preserve the crash under high-pressure condition+call-return pairs; the original two-argument condition keeps failing with call returns, and binary conditions can still fail with grouped or nested call returns` | Simpler conditions or non-call returns clear the crash in both branches |
| Statement-3/4 return shapes | `python3 tools/probe_selfhost_stmt34_return_shapes.py --stage1 /tmp/draton_s1` | `under the original stmt3/stmt4 conditions, only ident returns and ungrouped zero-arg calls pass; grouped zero-arg calls, one-arg calls, wrapper calls, and nested calls all preserve the crash` | This is the cleanest current evidence that return-expression shape, not only `if` presence, is part of the failing parser state |
| Statement-3/4 grouped returns | `python3 tools/probe_selfhost_stmt34_grouped_returns.py --stage1 /tmp/draton_s1` | `under the original stmt3/stmt4 conditions, grouping alone is sufficient to restore the crash; parenthesized ident, literal, and zero-arg call returns all fail even though their ungrouped forms pass` | This points directly at grouped-expression parsing in return position, not just call complexity |
| Statement-3/4 grouped contexts | `python3 tools/probe_selfhost_stmt34_grouped_contexts.py --stage1 /tmp/draton_s1` | `under the original stmt3/stmt4 condition, grouped expressions fail in return, let, and bare-expression positions; the same grouped expressions pass again once that condition is simplified` | This established that the bug is a grouped-expression + condition-interaction problem, not a return-only problem |
| Statement-3/4 condition shapes | `python3 tools/probe_selfhost_stmt34_condition_shapes.py --stage1 /tmp/draton_s1` | `grouped stmt3/stmt4 bodies only crash under str_eq_main-style conditions; simpler call and binary conditions still pass even when the body stays grouped` | This narrows the condition side of the interaction from “original branch condition” to the `str_eq_main(...)` family itself |
| Statement-3/4 condition arities | `python3 tools/probe_selfhost_stmt34_condition_arities.py --stage1 /tmp/draton_s1` | `grouped stmt3/stmt4 bodies fail for multi-argument call-like conditions, while zero-arg and one-arg call conditions still pass` | This is the current tightest condition-side narrowing: the bug now looks like grouped-body parsing interacting with comma-bearing call-condition parsing |
| Statement-3/4 condition commas | `python3 tools/probe_selfhost_stmt34_condition_commas.py --stage1 /tmp/draton_s1` | `grouped stmt3/stmt4 bodies fail for comma-bearing condition expressions more generally, not just for multi-argument call syntax; single-index conditions still pass` | This is the strongest current condition-side narrowing: the interaction now points at comma-bearing condition parsing plus grouped body parsing |
| Statement-3/4 branch dependency | `python3 tools/probe_selfhost_stmt34_branch_dependency.py --stage1 /tmp/draton_s1` | `one bad stmt3/stmt4 branch is enough only while the sibling branch remains in its original crashing form; replacing both branches or deleting the sibling clears the crash` | This suggests parser state is being poisoned by a mixed branch pair, not by a single isolated bad branch shape |
| Statement-3/4 order spacing | `python3 tools/probe_selfhost_stmt34_order_spacing.py --stage1 /tmp/draton_s1` | `mixed stmt3/stmt4 branch pairs keep crashing even when their order is swapped or a neutral statement separates them` | This suggests the poisoned parser state survives branch reordering and carries across at least one intervening statement |
| Statement-3/4 adjacency | `python3 tools/probe_selfhost_stmt34_adjacency.py --stage1 /tmp/draton_s1` | `adjacent both-bad pairs pass, but inserting one neutral statement makes even the both-bad pair crash` | This was the first sign that a barrier between the two branches matters, even though later probes narrowed that barrier story further |
| Statement-3/4 layout-only barriers | `python3 tools/probe_selfhost_stmt34_layout_only_barriers.py --stage1 /tmp/draton_s1` | `layout-only spacing does not matter; both-bad pairs still pass across blank lines and comments, so the crash is keyed to intervening statements rather than source layout` | This tightens the adjacency finding: the parser only cares about statement boundaries here, not extra whitespace or comment tokens |
| Statement-3/4 intervening statements | `python3 tools/probe_selfhost_stmt34_intervening_statements.py --stage1 /tmp/draton_s1` | `every probed non-block intervening statement is enough to make the both-bad stmt3/stmt4 pair crash` | This shows the parser state flips on many ordinary intervening statement paths, even though later probes found a block-shaped exception |
| Statement-3/4 empty boundaries | `python3 tools/probe_selfhost_stmt34_empty_boundaries.py --stage1 /tmp/draton_s1` | `empty blocks do not disturb the passing both-bad pair, but semicolons and empty control-flow statements do` | This narrows the previous row: the parser is not reacting to every syntactic separator, but to specific parsed statement paths between the two branches |
| Statement-3/4 block separators | `python3 tools/probe_selfhost_stmt34_block_separators.py --stage1 /tmp/draton_s1` | `only a plain empty block leaves the both-bad stmt3/stmt4 pair passing; non-empty blocks and annotated empty blocks still make it crash` | This points more specifically at parse paths around block statements: braces alone are not enough to trigger the bug, but any added content or block annotation is |
| Statement-3/4 plain block only | `python3 tools/probe_selfhost_stmt34_plain_block_only.py --stage1 /tmp/draton_s1` | `a plain bare {} block is harmless between the both-bad stmt3/stmt4 pair, but adding a semicolon or most wrappers makes it crash again` | This narrowed the separator-side story to an empty-block fast path before the later `spawn {}` exception was checked |
| Statement-3/4 spawn block fast path | `python3 tools/probe_selfhost_stmt34_spawn_block_fast_path.py --stage1 /tmp/draton_s1` | `among probed variants, only bare {} and spawn {} share the harmless empty-block fast path` | This is the current tightest separator-side narrowing: the harmless path seems tied to two empty-block fast paths, while semicolons, non-empty blocks, expression-bodied spawn, and other wrappers all still crash |
| Statement-3/4 doc-comment blocks | `python3 tools/probe_selfhost_stmt34_doc_comment_blocks.py --stage1 /tmp/draton_s1` | `plain doc-comment-only blocks now pass, but spawn doc-comment-only and semicolon-only blocks still crash` | The `parse_block` fix removed the plain-block half of this issue, but the spawn-specific mixed-interaction path is still unresolved |
| Statement-3/4 doc-comment positions | `python3 tools/probe_selfhost_stmt34_doc_comment_positions.py --stage1 /tmp/draton_s1` | `the same doc-comment tokens are harmless before, after, or between the harmless block separators; only the doc-only spawn block still crashes in the mixed stmt3/stmt4 context` | This narrows the remaining doc-comment interaction further: the general `parse_block` bug is fixed, but the spawn-wrapped mixed case still carries poisoned parser state |
| Standalone doc-comment-only blocks | `python3 tools/probe_selfhost_doc_comment_only_blocks.py --stage1 /tmp/draton_s1` | `pattern changed: standalone plain/spawn doc-comment-only blocks now parse successfully` | Fixed by `78234a8`; this issue is no longer a general parser blocker, only a clue that helped isolate the remaining self-host crash |
| Statement-3/4 spawn doc contexts | `python3 tools/probe_selfhost_stmt34_spawn_doc_contexts.py --stage1 /tmp/draton_s1` | ``spawn { /// gap }` only changes outcomes inside the both-bad stmt3/stmt4 pair; outside that context it behaves like an ordinary harmless separator or standalone statement` | This is the current tightest doc-comment narrowing: the remaining doc-comment-sensitive crash is confined to the both-bad mixed self-host interaction, not the general parser or general spawn path |
| Statement-3/4 spawn doc presence | `python3 tools/probe_selfhost_stmt34_spawn_doc_presence.py --stage1 /tmp/draton_s1` | `inside the both-bad stmt3/stmt4 context, any probed doc-comment presence inside spawn { ... } is enough to restore the crash` | This is the current tightest residual narrowing: the remaining spawn-specific failure is keyed to doc-comment token presence inside the otherwise harmless `spawn { ... }` separator |
| Spawn-doc backtrace comparison | `python3 tools/probe_selfhost_spawn_doc_backtrace.py --stage1 /tmp/draton_s1` | `main prefix-4 and residual both-bad + spawn-doc crashes follow different stacks` | The main repro still runs through `parse_return_stmt -> parse_arg_list -> parse_postfix`, while the residual spawn-doc crash currently dies much earlier at `parser_current -> parser_current_kind -> parser_is_eof -> parser_parse` |
| Spawn-doc tail functions | `python3 tools/probe_selfhost_spawn_doc_tail_functions.py --stage1 /tmp/draton_s1` | `later class/type items and later empty/comment-only tail functions keep the residual case at SIGSEGV, but later tail functions with non-empty bodies flip it to SIGABRT/OOM` | This points at a second residual corruption path that only appears after the parser re-enters `parse_block` / `parse_stmt` for a subsequent top-level function body |
| Spawn-doc tail stmt shapes | `python3 tools/probe_selfhost_spawn_doc_tail_stmt_shapes.py --stage1 /tmp/draton_s1` | `the later tail-function path is shape-sensitive: bare call/grouped expr statements, literal returns, and spawn {} flip to SIGABRT/OOM, while let/control-flow/spawn expr/call-return paths stay on SIGSEGV` | This rules out the simplistic hypothesis that any non-empty later function body causes the OOM branch; the later corruption depends on specific statement families |
| Spawn-doc tail backtraces | `python3 tools/probe_selfhost_spawn_doc_tail_backtraces.py --stage1 /tmp/draton_s1` | `later-tail SIGSEGV and later-tail SIGABRT/OOM variants do not share the same parser sink` | Later non-OOM tail variants still die at `parser_current -> parser_current_kind -> parser_is_eof -> parse_block -> parse_fn_def -> parser_parse`, while later OOM variants converge on `parser_record_error -> parser_error_unexpected -> parse_primary -> parse_postfix -> parse_expr_stmt_or_assignment` |
| Spawn-doc tail dispatch | `python3 tools/probe_selfhost_spawn_doc_tail_dispatch.py --stage1 /tmp/draton_s1` | `later-tail OOM variants fall through parse_expr_stmt_or_assignment, while later-tail SIGSEGV variants never reach parse_stmt` | This is the strongest current hint that statement dispatch itself is already corrupted by the residual spawn-doc state: source-level `return` and `spawn` are no longer reaching their dedicated parsers in the OOM branch |
| Spawn-doc tail keyword dispatch | `python3 tools/probe_selfhost_spawn_doc_tail_keyword_dispatch.py --stage1 /tmp/draton_s1` | `none of the probed keyword-led tail statements reach their dedicated parsers; return/spawn/let variants now split across expr-dispatch, if-tail-dispatch, and pre-dispatch failure modes` | This is the clearest current evidence that statement-kind dispatch has drifted badly in the residual tail-function path: even keyword-led statements no longer select parser branches by their source spelling |
| Spawn-doc tail dispatch matrix | `python3 tools/probe_selfhost_spawn_doc_tail_dispatch_matrix.py --stage1 /tmp/draton_s1` | `keyword no longer predicts parser path; only literal return and empty-block spawn hit expr-dispatch, only let+call/grouped hits parse_if_stmt_tail, and the rest die pre-dispatch` | This is the tightest current matrix for the residual tail-function family: parser drift depends on the combination of statement keyword and operand shape, not on keyword alone |
| Spawn-doc tail body prefixes | `python3 tools/probe_selfhost_spawn_doc_tail_body_prefixes.py --stage1 /tmp/draton_s1` | `layout-only prefixes do not change buckets, but real leading statements can move the same tail statement between pre-dispatch, expr-dispatch, and if-tail-dispatch` | This is the strongest current sign that the first real statement inside the tail body is mutating parser state directly: the same target statement changes bucket depending on whether it is preceded by `{}`, `spawn {}`, or `let warm = 0` |
| Spawn-doc tail prefix interactions | `python3 tools/probe_selfhost_spawn_doc_tail_prefix_interactions.py --stage1 /tmp/draton_s1` | `prefix statements are not uniformly harmful on their own; the bucket of the following statement depends on the exact prefix+target pair` | This is the clearest current evidence that the residual tail-body path is a two-statement state machine: neither the prefix nor the target alone is enough to predict the resulting parser sink |
| Spawn-doc context contrast | `python3 tools/probe_selfhost_spawn_doc_context_contrast.py --stage1 /tmp/draton_s1` | `the same representative tail-body prefix pairs land in different first-parse sinks across orig, both-bad, and residual contexts` | This confirms the tail-body state machine is not a generic parser property; it depends on the broader poisoned context built up before the tail function |
| Spawn-doc header thresholds | `python3 tools/probe_selfhost_spawn_doc_header_thresholds.py --stage1 /tmp/draton_s1` | `for residual spawn {} + return ready(), full class payload is required for OOM and the @type payload splits into pass / SIGSEGV / OOM regions` | This reconnects the residual tail-body family to header pressure: class and `@type` payload size still matter, but the threshold shape is now different from the original `prefix-4` blocker |
| `parser_skip_doc_comments` rooting retry | `python3 tools/probe_selfhost_stmt34_spawn_doc_presence.py --stage1 /tmp/draton_s1` plus `python3 tools/probe_selfhost_stmt34_spawn_doc_contexts.py --stage1 /tmp/draton_s1` | `no observable behavior change` | A local self-host-only retry that rooted extra values inside `parser_skip_doc_comments` left the residual spawn-doc and main blocker patterns unchanged, so the experiment was backed out instead of being kept as diff noise |
| Parser backtrace | `python3 tools/capture_selfhost_parser_bt.py --stage1 /tmp/draton_s1` | `parser_current -> parser_current_kind -> parser_skip_doc_comments -> parser_match_kind -> parse_or -> parse_nullish -> parse_expression -> parse_arg_list` | Current stable top-of-stack on `tests/programs/selfhost/parser_main_prefix4.dt`; deeper frames still continue through `parse_postfix` and `parse_return_stmt` |
| Linux hello fixture | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `build-hello -> 0` | String IR and print runtime blockers are cleared |

### Current baseline commands

Run these before and after each tranche.

- `[x]` `python3 tools/check_selfhost_strict_subset.py`
- `[x]` `cargo run -p drat -- build src/main.dt -o /tmp/draton_s1`
- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_main_prefixes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_main_subsets.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_return_call_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_minimal_return_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_header_dependencies.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_header_payloads.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_header_thresholds.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt1_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt1_operator_families.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt1_body_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_return_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_grouped_returns.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_grouped_contexts.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_arities.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_commas.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_branch_dependency.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_order_spacing.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_adjacency.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_layout_only_barriers.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_intervening_statements.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_empty_boundaries.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_block_separators.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_plain_block_only.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_spawn_block_fast_path.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_doc_comment_blocks.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_doc_comment_positions.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_doc_comment_only_blocks.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_spawn_doc_contexts.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_spawn_doc_presence.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_backtrace.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_functions.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_stmt_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_backtraces.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_dispatch.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_keyword_dispatch.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_dispatch_matrix.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_body_prefixes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_tail_prefix_interactions.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_context_contrast.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_spawn_doc_header_thresholds.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/capture_selfhost_parser_bt.py --stage1 /tmp/draton_s1`
- `[x]` `python3 -u tools/verify_stage2.py`

## Phase S0: Bootstrap Truth And Gates

Goal: remove ambiguity about what "self-host" means in this repository and make every bootstrap claim reproducible.

### S0.A Definitions

- `[x]` Split progress into:
  - syntax parity
  - semantic parity
  - bootstrap parity
  - runtime parity
  - backend parity
  - toolchain parity
- `[x]` Treat Rust as authoritative until S2 gate passes
- `[x]` Treat self-host under `src/` as the bootstrap target and parity mirror

### S0.B Verification harnesses

- `[x]` `tools/verify_stage2.py` prints signal-based failures
- `[x]` `tools/verify_stage2.py` runs a preflight stage1 `check src/main.dt`
- `[x]` `tools/repro_selfhost_blockers.py` exists for focused repros
- `[ ]` Add a stage3 verification path
- `[ ]` Add a one-shot readiness command that runs the agreed baseline suite
- `[ ]` Write baseline timings to a checked-in results file

### S0.C Documentation alignment

- `[ ]` Update `README` to stop implying more self-host readiness than currently proven
- `[ ]` Update `docs/selfhost-canonical-migration-status.md` to match current blocker list
- `[ ]` Update `docs/gc-scorecard.md` to reflect current bootstrap blockers and not older ones
- `[ ]` Link this checklist from the main self-host status docs

### S0 Exit Gate

- `[x]` `python3 tools/check_selfhost_strict_subset.py`
- `[x]` `python3 -u tools/verify_stage2.py` fails with a concrete blocker instead of a blank failure
- `[ ]` Stage3 verification path exists
- `[ ]` Public self-host status docs agree with the harness output

## Phase S1: Self-Host Compiler Bootstrap Stability

Goal: make the stage1 self-host binary capable of checking and rebuilding the compiler without crashing.

### S1.A Parser and frontend crash elimination

Objective: remove the `SIGSEGV` in the self-host frontend before stage2 bootstrap.

- `[x]` Confirm crash exists in both `check src/main.dt` and `ast-dump src/main.dt`
- `[x]` Narrow crash below the full compiler source
- `[x]` Confirm `header only` from `src/main.dt` parses successfully
- `[x]` Confirm `header + main()` from `src/main.dt` is sufficient to crash
- `[x]` Check in a parser regression fixture derived from the current repro
- `[x]` Check in a smaller parser repro fixture derived from the first crashing `main()` prefix
- `[x]` Identify the first crashing `main()` prefix with an automated probe
- `[x]` Confirm the crash is an interaction between all first 4 statements, not any probed strict subset alone
- `[x]` Confirm that inside `prefix-4`, replacing the crashing `return` with another call-bearing form still crashes
- `[x]` Confirm that minimal standalone `return foo(2)`-style files parse successfully
- `[x]` Confirm that all proper header subsets pass and only the full `imports + class + @type + main` fixture fails
- `[x]` Confirm that shrinking either the `class ParsedArgs` payload or the `@type` payload removes the crash
- `[x]` Measure the first failing class-field and top-level `@type` thresholds
- `[x]` Confirm that statement-1 only preserves the crash when a binary-expression condition appears inside an `if` with a non-empty body
- `[x]` Confirm that all probed binary-operator families in statement 1 preserve the crash
- `[x]` Confirm that, once statement 1 has the bad condition shape, any probed non-empty body preserves the crash
- `[x]` Confirm that statement-3/4 only preserve the crash under high-pressure condition+call-return pairs, not under simpler conditions or non-call returns
- `[x]` Confirm that under the original statement-3/4 conditions, only ident returns and ungrouped zero-arg calls pass while grouped or more complex call returns still crash
- `[x]` Confirm that under the original statement-3/4 conditions, grouping alone is already sufficient to crash ident, literal, and zero-arg-call returns
- `[x]` Confirm that under the original statement-3/4 conditions, grouped ident/literal/zero-arg-call expressions also crash in `let` and bare-expression positions, while the same grouped expressions pass once the condition is simplified
- `[x]` Confirm that grouped statement-3/4 bodies crash under `str_eq_main(...)`-family conditions but still pass under simpler call and binary conditions
- `[x]` Confirm that grouped statement-3/4 bodies fail for multi-argument call-like conditions but still pass for zero-arg and one-arg call conditions
- `[x]` Confirm that grouped statement-3/4 bodies fail for comma-bearing condition expressions more generally, while single-index conditions still pass
- `[x]` Confirm that one bad stmt3/stmt4 branch still crashes while the sibling stays original, but replacing both bad or deleting the sibling clears the crash
- `[x]` Confirm that mixed stmt3/stmt4 branch pairs still crash when reordered or separated by a neutral statement
- `[x]` Confirm that inserting one neutral statement is enough to make the `both-bad` stmt3/stmt4 pair crash
- `[x]` Confirm that blank lines and comments do not break the passing `both-bad` pair, so layout-only barriers are not the relevant variable
- `[x]` Confirm that every probed non-block intervening statement shape is enough to make the `both-bad` stmt3/stmt4 pair crash
- `[x]` Confirm that empty blocks are harmless between the `both-bad` pair, while semicolons and empty control-flow statements are still harmful
- `[x]` Confirm that only a plain empty block is harmless; non-empty blocks and annotated empty blocks are still harmful
- `[x]` Confirm that a bare `{}` with no semicolon is one harmless empty-block separator
- `[x]` Confirm that, among probed variants, `spawn {}` is the only wrapper that shares the harmless empty-block behavior
- `[x]` Confirm that line-comment-only harmless blocks still pass while only the mixed-context `spawn` doc-comment-only block remains crashing
- `[x]` Confirm that the same doc-comment tokens remain harmless before, after, or between the harmless block separators and no longer fail in standalone plain/spawn blocks
- `[x]` Confirm that minimal standalone plain/spawn doc-comment-only blocks now parse after `78234a8`
- `[x]` Confirm that `spawn { /// gap }` only changes outcomes inside the `both-bad` stmt3/stmt4 pair and is otherwise harmless
- `[x]` Confirm that, inside that residual `both-bad` case, any probed doc-comment presence inside `spawn { ... }` restores the crash
- `[-]` Try targeted postfix/lookahead rooting hardening and record whether the crash signature changes
- `[x]` Confirm that fully bypassing `parser_looks_like_type_args_before_class_literal` does not change the current crash pattern
- `[ ]` Make the minimal fixture fail under an automated self-host parser test
- `[ ]` Identify whether the root cause is:
  - parser synchronization bug
  - token lifetime / rooting bug
  - AST node lifetime / rooting bug
  - another frontend memory-safety issue
- `[x]` Capture a stable backtrace on the checked-in fixture
- `[!]` Current stable backtrace is:
  - `parser_current`
  - `parser_current_kind`
  - `parser_check`
  - `parser_looks_like_type_args_before_class_literal`
  - `parse_postfix`
  - `parse_arg_list`
  - `parse_return_stmt`
- `[ ]` Fix the crash in the smallest affected parser or frontend surface
- `[ ]` Rerun the reduced fixture until it exits `0`
- `[ ]` Rerun `ast-dump src/main.dt` until it exits `0`
- `[ ]` Rerun `check src/main.dt` until it exits `0`
- `[ ]` Rerun `type-dump src/main.dt` until it exits `0`

#### S1.A Verification commands

- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_main_prefixes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_main_subsets.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_return_call_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_minimal_return_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_header_dependencies.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_variants.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_return_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_grouped_returns.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_grouped_contexts.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_shapes.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_arities.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_condition_commas.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_branch_dependency.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_order_spacing.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_adjacency.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_layout_only_barriers.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_intervening_statements.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_empty_boundaries.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_block_separators.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_plain_block_only.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_spawn_block_fast_path.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_doc_comment_blocks.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_doc_comment_positions.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_doc_comment_only_blocks.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/probe_selfhost_stmt34_spawn_doc_contexts.py --stage1 /tmp/draton_s1`
- `[x]` `python3 tools/capture_selfhost_parser_bt.py --stage1 /tmp/draton_s1`
- `[x]` `/tmp/draton_s1 ast-dump tests/programs/selfhost/parser_main_prefix4.dt`
- `[ ]` `/tmp/draton_s1 ast-dump src/main.dt`
- `[ ]` `/tmp/draton_s1 check src/main.dt`
- `[ ]` `/tmp/draton_s1 type-dump src/main.dt`

#### S1.A Artifact targets

- `[x]` checked-in parser repro fixture
- `[x]` checked-in smaller parser repro fixture
- `[x]` automated subset interaction probe
- `[x]` automated return-call variant probe
- `[x]` automated minimal-shape contrast probe
- `[x]` automated header dependency probe
- `[x]` automated header payload probe
- `[x]` automated header threshold probe
- `[x]` automated statement-1 variant probe
- `[x]` automated statement-3/4 variant probe
- `[x]` automated statement-3/4 return-shape probe
- `[x]` automated statement-3/4 grouped-return probe
- `[x]` automated statement-3/4 grouped-context probe
- `[x]` automated statement-3/4 condition-shape probe
- `[x]` automated statement-3/4 condition-arity probe
- `[x]` automated statement-3/4 condition-comma probe
- `[x]` automated statement-3/4 branch-dependency probe
- `[x]` automated statement-3/4 order/spacing probe
- `[x]` automated statement-3/4 adjacency probe
- `[x]` automated statement-3/4 layout-only barrier probe
- `[x]` automated statement-3/4 intervening-statement probe
- `[x]` automated statement-3/4 empty-boundary probe
- `[x]` automated statement-3/4 block-separator probe
- `[x]` automated statement-3/4 plain-block-only probe
- `[x]` automated statement-3/4 spawn-block fast-path probe
- `[x]` automated statement-3/4 doc-comment-block probe
- `[x]` automated statement-3/4 doc-comment-position probe
- `[x]` automated standalone doc-comment-only block probe
- `[x]` automated statement-3/4 spawn-doc-context probe
- `[x]` automated statement-3/4 spawn-doc-presence probe
- `[x]` automated statement-1 operator-family probe
- `[x]` automated statement-1 body probe
- `[ ]` regression test path for that fixture
- `[ ]` notes in this file naming the exact root cause once confirmed

### S1.B Self-host textual LLVM correctness

Objective: make stage1-generated textual LLVM valid enough to compile and run basic programs on Linux.

- `[x]` Use LLVM textual float types `double` and `float`
- `[x]` Emit real line breaks in generated `.ll` files
- `[x]` Emit a native `main(argc, argv)` wrapper for top-level Draton `main`
- `[x]` Remove the Linux build path dependence on Windows-only libraries
- `[x]` Fix string literal global escaping so LLVM accepts emitted constants
- `[x]` Fix string literal length accounting so constant sizes match actual bytes
- `[x]` Lower `print` / `println` to runtime symbols in self-host direct-call dispatch
- `[x]` Emit self-host LLVM fallback definitions for `draton_print` / `draton_println`
- `[x]` Verify `examples/hello.dt` builds via stage1
- `[x]` Verify `examples/hello.dt` runs via stage1
- `[ ]` Verify at least one arithmetic fixture builds and runs via stage1
- `[ ]` Verify at least one branch/control-flow fixture builds and runs via stage1
- `[ ]` Verify at least one class/layer fixture builds via stage1

#### S1.B Verification commands

- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `DRATON_RUNTIME_LIB=... /tmp/draton_s1 build examples/hello.dt -o /tmp/selfhost_hello`
- `[x]` `/tmp/selfhost_hello`
- `[ ]` stage1 build and run commands for arithmetic and control-flow fixtures

#### S1.B Artifact targets

- `[x]` checked-in notes of the string-literal IR root cause
- `[ ]` at least three passing Linux stage1 executable fixtures

### S1.C Bootstrap ladder

Objective: move from "Rust can build stage1" to "Draton can rebuild itself repeatedly".

- `[x]` Rust stage0 builds stage1
- `[!]` Stage1 self-check on `src/main.dt` passes
- `[ ]` Stage1 builds stage2
- `[ ]` Stage2 self-check on `src/main.dt` passes
- `[ ]` Stage2 builds stage3
- `[ ]` Stage3 self-check on `src/main.dt` passes
- `[ ]` Stage2 and stage3 exhibit matching CLI behavior on the bootstrap corpus
- `[ ]` `tools/verify_stage2.py` passes end to end
- `[ ]` Stage3 verification command exists and passes end to end

#### S1.C Verification commands

- `[x]` `cargo run -p drat -- build src/main.dt -o /tmp/draton_s1`
- `[ ]` `/tmp/draton_s1 check src/main.dt`
- `[ ]` `/tmp/draton_s1 build src/main.dt -o /tmp/draton_s2`
- `[ ]` `/tmp/draton_s2 check src/main.dt`
- `[ ]` `/tmp/draton_s2 build src/main.dt -o /tmp/draton_s3`
- `[ ]` `/tmp/draton_s3 check src/main.dt`
- `[ ]` `python3 -u tools/verify_stage2.py`

### S1 Exit Gate

- `[ ]` Stage1 no longer crashes on `src/main.dt`
- `[ ]` Stage1 builds and runs `examples/hello.dt`
- `[ ]` Stage1 builds stage2
- `[ ]` Stage2 builds stage3
- `[ ]` Stage2 and stage3 agree on the bootstrap corpus

## Phase S2: Promote Draton Compiler-Core To Primary

Goal: move compiler-core authority from Rust to Draton only after bootstrap stability is real.

### S2.A Parity discipline

- `[ ]` Define the selected parity corpus for frontend behavior
- `[ ]` Compare Rust stage0 vs Draton stage1 on:
  - `ast-dump`
  - `type-dump`
  - `check`
  - executable fixtures
- `[ ]` Record known mismatches explicitly instead of leaving them implicit
- `[ ]` Add anti-drift checks so new semantic changes do not land only in Rust

### S2.B Interface cleanup

- `[ ]` Split compiler-core surfaces in Draton into explicit layers:
  - lex
  - parse
  - check
  - mono
  - emit
- `[ ]` Separate host-facing services from compiler-core logic
- `[ ]` Mark which surfaces are allowed to depend on runtime or host ABI

### S2.C Source-of-truth transition

- `[ ]` Document the exact acceptance conditions for promoting `src/` to primary compiler-core
- `[ ]` Switch status docs from "mirror" to "primary" only after S2 gate passes
- `[ ]` Re-scope Rust crates as bootstrap/parity references

### S2 Exit Gate

- `[ ]` Draton stage1 and Rust stage0 agree on the selected parity corpus
- `[ ]` Stage2 and stage3 remain stable across repeated bootstrap runs
- `[ ]` No new language semantic change lands only in Rust

## Phase S3: Runtime And Host Surface Extraction

Goal: stop depending on the Rust runtime crate for the normal bootstrap path.

### S3.A Linux x86_64 host ABI minimum

- `[ ]` file read
- `[ ]` file write
- `[ ]` process exec
- `[ ]` argv
- `[ ]` env
- `[ ]` stdout
- `[ ]` stderr
- `[ ]` wall-clock time
- `[ ]` monotonic time
- `[ ]` heap allocation primitive
- `[ ]` bootstrap host ABI document

### S3.B Bootstrap-minimal runtime

- `[ ]` startup / shutdown glue
- `[ ]` string primitives needed by compiler-core
- `[ ]` array primitives needed by compiler-core
- `[ ]` panic path
- `[ ]` allocation path sufficient for bootstrap
- `[ ]` documented bootstrap-mode GC or non-GC policy
- `[ ]` separate bootstrap-minimal runtime from full runtime ambitions

### S3.C Stdlib surface needed for bootstrap

- `[ ]` `io`
- `[ ]` `string`
- `[ ]` `os`
- `[ ]` `fs`
- `[ ]` `time`
- `[ ]` `collections`
- `[ ]` `json`
- `[ ]` `math`
- `[ ]` leave `net` deferred unless needed
- `[ ]` leave `crypto` deferred unless needed

### S3 Exit Gate

- `[ ]` Stage1 compiler-core and bootstrap toolchain run without the Rust runtime crate
- `[ ]` Required stdlib modules used by bootstrap no longer depend on Rust-backed FFI

## Phase S4: Direct Assembly Backend

Goal: replace the LLVM-text path with a native assembly path for the first supported host target.

### S4.A Backend architecture

- `[ ]` Freeze initial backend target to `linux-x86_64`
- `[ ]` Define a stable internal lowering boundary after typecheck / mono
- `[ ]` Define calling convention policy
- `[ ]` Define stack-frame policy
- `[ ]` Define data section and string/global layout policy
- `[ ]` Define external symbol ABI for runtime hooks

### S4.B Backend implementation

- `[ ]` integer arithmetic
- `[ ]` comparisons
- `[ ]` branches
- `[ ]` structured control flow
- `[ ]` function calls
- `[ ]` returns
- `[ ]` local stack slots
- `[ ]` string/object references needed for bootstrap
- `[ ]` entrypoint emission
- `[ ]` assembler invocation
- `[ ]` linker invocation

### S4.C Backend verification

- `[ ]` build and run constant/arithmetic fixtures
- `[ ]` build and run control-flow fixtures
- `[ ]` build and run string printing fixtures
- `[ ]` build and run compiler-facing subset
- `[ ]` build the self-host compiler with the assembly backend

### S4 Exit Gate

- `[ ]` Direct-asm backend builds and runs the bootstrap fixture set on Linux
- `[ ]` Direct-asm backend builds the self-host compiler itself

## Phase S5: Full Toolchain In Draton

Goal: move from self-host compiler-core to self-host day-to-day tooling.

### S5.A Core commands already present in Draton

- `[x]` `build`
- `[x]` `run`
- `[x]` `check`
- `[x]` `ast-dump`
- `[x]` `type-dump`

### S5.B Commands still to port

- `[ ]` `fmt`
- `[ ]` `lint`
- `[ ]` `task`
- `[ ]` `test`
- `[ ]` `doc`
- `[ ]` `repl`
- `[ ]` `lsp`
- `[ ]` package management commands
- `[ ]` publish / update commands

### S5.C Tooling quality gates

- `[ ]` formatter regression corpus
- `[ ]` lint corpus
- `[ ]` task runner smoke suite
- `[ ]` test command smoke suite
- `[ ]` doc generation smoke suite
- `[ ]` LSP smoke suite
- `[ ]` package workflow smoke suite

### S5 Exit Gate

- `[ ]` Draton-first toolchain covers the commands needed for normal compiler development
- `[ ]` Rust `drat` CLI can be retired or reduced to bootstrap-only compatibility

## Phase S6: Rust Retirement

Goal: reach the repository shape "Draton + assembly/bootstrap glue" without Rust in the normal path.

- `[ ]` Remove Rust as source of truth for compiler-core
- `[ ]` Remove Rust runtime crate from the normal build path
- `[ ]` Remove Rust-backed stdlib implementation from the normal build path
- `[ ]` Remove Rust CLI/tooling from the normal build path
- `[ ]` Keep only assembly/bootstrap glue that remains justified and documented
- `[ ]` Document the final bootstrap chain from released artifact to self-host rebuild

### S6 Exit Gate

- `[ ]` Repository can bootstrap, build, and run the official toolchain without Rust source code participating in the normal path
- `[ ]` Remaining non-Draton code is limited to explicit assembly/bootstrap glue

## Immediate Next Tasks

These are the tasks that should move next unless a newly discovered blocker supersedes them.

### Active tranche

- `[ ]` Fix self-host parser `SIGSEGV` on `src/main.dt`
- `[ ]` Check in a minimal parser regression fixture for the current crash
- `[x]` Switch the main parser blocker harness label to the smaller `prefix-4` naming
- `[ ]` Audit `parser_current`, `parser_current_kind`, `parser_skip_doc_comments`, and `parser_expect` on the checked-in fixture
- `[ ]` Audit `parser_looks_like_type_args_before_class_literal`, `parse_postfix`, and `parse_arg_list` on the checked-in `prefix-4` fixture
- `[ ]` Explain why `stmt1_if_argc+stmt2_let_cmd+stmt3_if_build+stmt4_if_run` crashes while all probed strict subsets pass
- `[ ]` Explain why call-bearing `return` variants fail only inside `prefix-4` while minimal standalone call-return files pass
- `[ ]` Explain why only the full `imports + class + @type + main` combination crashes while every proper header subset passes
- `[ ]` Explain why the full `class ParsedArgs` payload and full `@type` payload are both needed for the crash
- `[ ]` Explain why the crash threshold starts at 2 class fields and 16 top-level `@type` entries
- `[ ]` Explain why only statement-1 `if` conditions with binary expressions and non-empty bodies preserve the crash while simple statement-1 variants do not
- `[ ]` Explain why statement-3/4 keep the crash only under high-pressure condition+call-return pairs while simpler conditions or non-call returns clear it
- `[ ]` Explain why statement-3/4 crash on `return (cmd)` and `return (0)` even though `return cmd` and `return 0` pass under the same original condition
- `[ ]` Explain why statement-3/4 accept `return cli_argc()` but crash on `return (cli_argc())` under the same original condition
- `[ ]` Explain why statement-3/4 reject one-arg, wrapper, and nested call returns under the original condition while ident returns still pass
- `[ ]` Explain why grouped expressions also crash in `let` and bare-expression positions under `str_eq_main(cmd, ...)`, but the same grouped expressions pass when the condition is simplified
- `[ ]` Explain why `str_eq_main(...)`-family conditions are sufficient to trigger grouped-expression failures while other call conditions still pass
- `[ ]` Explain why multi-argument call-like conditions are sufficient to trigger grouped-body failures while zero-arg and one-arg call conditions still pass
- `[ ]` Explain why comma-bearing condition expressions are sufficient to trigger grouped-body failures while single-index and non-comma conditions still pass
- `[ ]` Explain why one bad stmt3/stmt4 branch plus one original sibling crashes, while an adjacent `both-bad` pair passes but a separated `both-bad` pair crashes
- `[ ]` Explain why the mixed stmt3/stmt4 crash survives branch reordering and an intervening neutral statement
- `[ ]` Explain why the `both-bad` stmt3/stmt4 pair passes across layout-only and block-only separators but crashes once certain intervening statement paths appear
- `[ ]` Explain why blank lines and comments do not disturb the passing `both-bad` pair while parsed statement separators do
- `[ ]` Explain why every probed non-block intervening statement shape is enough to make the `both-bad` stmt3/stmt4 pair crash
- `[ ]` Explain why empty blocks are harmless between the `both-bad` pair while semicolons and empty control-flow statements are not
- `[ ]` Explain why only a plain empty block is harmless while non-empty blocks and annotated empty blocks are not
- `[ ]` Explain why a bare `{}` and `spawn {}` are the only currently known harmless empty-block separators while `{};` and other wrapped empty blocks are not
- `[ ]` Explain why the mixed-context `spawn` doc-comment-only block still crashes after the general `parse_block` fix
- `[ ]` Explain why the same doc-comment tokens are now harmless in standalone plain/spawn blocks but still fail inside the mixed self-host `spawn` block fast path
- `[ ]` Explain why `spawn { /// gap }` only changes outcomes inside the `both-bad` stmt3/stmt4 pair and is otherwise harmless
- `[ ]` Explain why, inside that residual `both-bad` case, any doc-comment presence inside `spawn { ... }` restores the crash while line-comment-only and doc-comment-free spawn blocks do not
- `[ ]` Explain why the residual `both-bad + spawn { /// gap }` crash now dies in `parser_is_eof -> parser_parse` instead of following the main `parse_return_stmt -> parse_arg_list -> parse_postfix` stack
- `[ ]` Explain why the residual `both-bad + spawn { /// gap }` case stays at `SIGSEGV` across later `class` / `@type` items and later empty/comment-only tail functions, but flips to `SIGABRT` / OOM when a later top-level function has a non-empty body
- `[ ]` Explain why that later tail-function path only flips to `SIGABRT` / OOM for bare call/grouped expression statements, literal returns, and `spawn {}`, while `let`, empty control-flow, `spawn expr`, and call/grouped returns stay on `SIGSEGV`
- `[ ]` Explain why later-tail `SIGSEGV` variants still die at `parser_is_eof -> parse_block -> parse_fn_def`, while later-tail `SIGABRT` / OOM variants fall through to `parser_record_error -> parser_error_unexpected -> parse_primary -> parse_postfix -> parse_expr_stmt_or_assignment`
- `[ ]` Explain why later-tail OOM variants like `return 0` and `spawn {}` no longer reach `parse_return_stmt` / `parse_spawn_stmt`, but instead fall through the `parse_expr_stmt_or_assignment` sink
- `[ ]` Explain why `let x = ready()` and `let x = (0)` are now misrouted into `parse_if_stmt_tail` instead of `parse_let_stmt`, while `let x = 0` still dies even earlier before `parse_stmt`
- `[ ]` Explain why, in the residual tail-function dispatch matrix, literal `return` and empty-block `spawn` alone hit expr-dispatch drift, `let` only misroutes on call/grouped values, and ident/call/grouped `return/spawn` plus ident/literal `let` die pre-dispatch
- `[ ]` Explain why layout-only prefixes leave tail-body buckets unchanged, while real leading statements like `{}`, `spawn {}`, and `let warm = 0` can move the same target statement between `pre-dispatch`, `expr-dispatch`, and `parse_if_stmt_tail`
- `[ ]` Explain why prefix statements are not uniformly harmful on their own: `{}` and `let warm = 0` stay `pre-dispatch` alone, `spawn {}` is `expr-dispatch` alone, and the resulting bucket still depends on the exact prefix+target pair
- `[ ]` Explain why the same representative tail-body prefix pairs land in different first-parse sinks across `orig`, `both-bad`, and `residual` contexts, and what exact poisoning step introduced by `spawn { /// gap }` creates the extra state machine
- `[ ]` Explain why the representative residual tail-body pair `spawn {} + return ready()` needs the full class payload before OOM appears, while the `@type` payload now splits into pass / `SIGSEGV` / OOM regions instead of following the simpler original `prefix-4` thresholds
- `[ ]` Explain why the extra self-host spawn-path rooting hardening in `d770374` did not change the residual `spawn { /// gap }` mixed-context crash
- `[ ]` Explain why extra local rooting inside `parser_skip_doc_comments` also failed to change the residual `spawn { /// gap }` mixed-context crash
- `[x]` Explain why minimal standalone plain/spawn doc-comment-only blocks used to fail with `invalid expression at line 4, col 5`
  Fixed by `78234a8`: `parse_block` now re-checks `RBrace` / `Eof` immediately after `parser_skip_doc_comments`
- `[ ]` Explain why operator family does not matter once statement 1 is a binary-expression `if` with a non-empty body
- `[ ]` Explain why body emptiness is the decisive variable for statement 1 once the bad condition shape is present
- `[ ]` Decide whether the unsuccessful postfix/lookahead rooting hardening should be kept as harmless hardening or backed out to reduce diff noise
- `[ ]` Explain why the current `prefix-4` crash now lands in `parser_skip_doc_comments -> parser_match_kind -> parse_or -> parse_arg_list` while fully bypassing `parser_looks_like_type_args_before_class_literal` still does not change the overall crash pattern
- `[ ]` Confirm whether the crash happens while consuming the `{` that starts the `then` block in `parse_if_stmt_tail`
- `[ ]` Decide whether the crash is caused by token rooting/copying or by parser position drift
- `[ ]` Rerun `tools/verify_stage2.py` after parser/frontend crash is fixed
- `[ ]` Update this checklist immediately after the next tranche lands

### Ready after current blockers

- `[ ]` Add stage3 verification path
- `[ ]` Add a one-shot self-host readiness command
- `[ ]` Add arithmetic and control-flow Linux stage1 fixtures now that `hello.dt` passes
- `[ ]` Start a small parity corpus for Rust stage0 vs Draton stage1

## Definition Of "Good Update"

Each future checklist update should include:

- the exact command rerun
- the exact result observed
- the smallest remaining blocker if the item is still not done
- the commit that changed the state, once the change is committed
