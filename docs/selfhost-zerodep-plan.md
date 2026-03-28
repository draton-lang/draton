---
title: Self-Host & Zero-Dependency Plan
sidebar_position: 36
---

# Self-Host & Zero-Dependency Plan

## Mục tiêu cuối

> Người dùng tải `drat` về → chạy ngay.
> Không cần LLVM, GCC, libc, hay bất kỳ thứ gì khác được cài sẵn trên máy.
> Codebase chỉ còn Draton + ASM. Rust bị loại bỏ hoàn toàn sau 60 releases.

---

## Cấu trúc file self-host compiler (mục tiêu)

> Quy tắc đặt tên: **folder = nhóm**, **file = một thứ duy nhất**.
> Không dùng prefix trong tên file — prefix chính là tên folder.
> Không file nào vượt quá ~300 dòng.

```
compiler/
│
├── lexer/
│   ├── token.dt          TokenKind enum + Token class + Span class
│   ├── lexer.dt          Lexer class — fn tokenize()
│   ├── result.dt         LexResult class — tokens + errors
│   └── errors.dt         LexError enum
│
├── ast/
│   ├── span.dt           Span { start, end, line, col }
│   ├── types.dt          TypeExpr enum — Named, Generic, Fn, Pointer, Infer
│   │
│   ├── expr/
│   │   ├── expr.dt       Expr enum — tất cả expression nodes
│   │   ├── ops.dt        BinOp enum + UnOp enum
│   │   ├── fstr.dt       FStrPart enum
│   │   └── matching.dt   MatchArm class + MatchArmBody enum
│   │
│   ├── stmt/
│   │   ├── stmt.dt       Stmt enum + Block class
│   │   ├── binding.dt    LetStmt + LetDestructureStmt + DestructureBinding
│   │   ├── assign.dt     AssignStmt + AssignOp enum
│   │   ├── control.dt    IfStmt + ElseBranch + ForStmt + WhileStmt
│   │   ├── spawning.dt   SpawnStmt + SpawnBody enum
│   │   └── misc.dt       ReturnStmt + IfCompileStmt + GcConfigStmt + GcConfigEntry
│   │
│   └── item/
│       ├── item.dt       Item enum + Program class
│       ├── func.dt       FnDef + Param
│       ├── klass.dt      ClassDef + ClassMember enum + FieldDef
│       ├── layers.dt     LayerDef
│       ├── iface.dt      InterfaceDef
│       ├── variants.dt   EnumDef
│       ├── errors.dt     ErrorDef
│       ├── constant.dt   ConstDef
│       ├── imports.dt    ImportDef + ImportItem
│       ├── extern.dt     ExternBlock
│       └── type_block.dt TypeBlock + TypeMember enum
│
├── parser/
│   ├── parser.dt         Parser class — state, peek/advance/expect/sync
│   ├── errors.dt         ParseError enum + recovery helpers
│   │
│   ├── parse/
│   │   ├── items.dt      parse_program() + tất cả top-level items
│   │   ├── stmts.dt      tất cả statement variants
│   │   ├── exprs.dt      expressions — precedence climbing
│   │   ├── types.dt      TypeExpr parsing
│   │   └── patterns.dt   match pattern parsing
│
├── typeck/
│   │
│   ├── — type system ——————————————————————————————————
│   ├── types/
│   │   ├── ty.dt         Type enum — tất cả runtime types
│   │   ├── scheme.dt     Scheme class — type scheme cho generics
│   │   ├── env.dt        TypeEnv class — scope stack
│   │   ├── subst.dt      Substitution class — immutable type var map
│   │   ├── unify.dt      fn unify() — Algorithm W unification
│   │   └── errors.dt     TypeError enum
│   │
│   ├── — typed ast ————————————————————————————————————
│   ├── typed/
│   │   ├── program.dt    TypedProgram + TypedItem enum
│   │   ├── items.dt      TypedFnDef + TypedParam + TypedClassDef
│   │   │                 + TypedFieldDef + TypedInterfaceDef
│   │   │                 + TypedEnumDef + TypedErrorDef + TypedConstDef
│   │   │                 + TypedImportDef + TypedImportItem
│   │   │                 + TypedExternBlock + TypedTypeBlock + TypedTypeMember
│   │   ├── exprs.dt      TypedExpr + TypedExprKind enum + TypedFStrPart
│   │   ├── stmts.dt      TypedStmt + TypedStmtKind + TypedBlock
│   │   │                 + TypedLetStmt + TypedAssignStmt + TypedReturnStmt
│   │   │                 + TypedIfStmt + TypedElseBranch + TypedForStmt
│   │   │                 + TypedWhileStmt + TypedSpawnStmt + TypedSpawnBody
│   │   │                 + TypedLetDestructureStmt + TypedDestructureBinding
│   │   └── ownership.dt  OwnershipState + UseEffect
│   │                     + ParamOwnershipSummary + FnOwnershipSummary
│   │
│   ├── — inference engine —————————————————————————————
│   ├── infer/
│   │   ├── checker.dt    TypeChecker class — tất cả 20 fields
│   │   ├── result.dt     TypeCheckResult + DeprecatedSyntaxMode
│   │   ├── hints.dt      fn extract_hints() — xử lý @type blocks
│   │   ├── items.dt      fn check_item() — tất cả Item variants
│   │   ├── stmts.dt      fn infer_stmt() — tất cả Stmt variants
│   │   ├── exprs.dt      fn infer_expr() — tất cả Expr variants
│   │   └── exhaust.dt    ExhaustivenessChecker — match coverage
│   │
│   ├── — ownership ————————————————————————————————————
│   └── ownership/
│       ├── env.dt        OwnershipEnv + BindingState + BorrowRecord + BorrowKind
│       ├── meta.dt       ClosureMeta + FunctionRecord
│       │                 + InternalFnSummary + FunctionIndex
│       ├── copy.dt       fn is_copy() — copy type predicate
│       ├── checker.dt    OwnershipChecker class — tất cả fields
│       ├── infer.dt      fn infer_program() + infer_fn() + infer_block()
│       ├── exprs.dt      fn check_expr() — ownership effect per expression
│       ├── stmts.dt      fn check_stmt() — ownership effect per statement
│       ├── free.dt       free() insertion — last-use span tracking
│       └── errors.dt     OwnershipError enum
│
├── codegen/
│   │
│   ├── — llvm bindings ————————————————————————————————
│   ├── llvm/
│   │   ├── handles.dt    LLVMContextRef, LLVMModuleRef, ... (@pointer aliases)
│   │   ├── context.dt    @extern — LLVMContextCreate/Dispose
│   │   ├── module.dt     @extern — LLVMModuleCreate/Print/Dispose
│   │   ├── builder.dt    @extern — LLVMBuilder* + tất cả build instructions
│   │   ├── types.dt      @extern — LLVMInt*, LLVMDouble*, LLVMStruct*, ...
│   │   ├── values.dt     @extern — LLVMConst*, LLVMGetUndef, ...
│   │   ├── target.dt     @extern — target machine init + emit to file
│   │   └── pass.dt       @extern — pass manager API
│   │
│   ├── — support ——————————————————————————————————————
│   ├── mode.dt           BuildMode enum — Debug, Release, Size, Fast
│   ├── errors.dt         CodeGenError enum
│   ├── mangle.dt         fn mangle_fn() — name mangling
│   ├── layout.dt         ClassLayout class — struct field index map
│   │
│   ├── — type mapping —————————————————————————————————
│   ├── typemap/
│   │   ├── map.dt        fn draton_type_to_llvm() — Type → LLVMTypeRef
│   │   ├── string.dt     string struct layout { i8*, i64 }
│   │   └── array.dt      array struct layout { T*, i64, i64 }
│   │
│   ├── — monomorphization —————————————————————————————
│   ├── mono/
│   │   ├── collector.dt  MonoCollector — generic instantiation tracker
│   │   └── resolve.dt    fn resolve_function_type_args()
│   │                     + GenericFnDef + GenericClassDef
│   │
│   ├── — vtable ———————————————————————————————————————
│   ├── vtable/
│   │   ├── registry.dt   InterfaceRegistry class
│   │   └── emit.dt       fn emit_vtable() — vtable global constant
│   │
│   ├── — codegen core —————————————————————————————————
│   ├── core/
│   │   ├── codegen.dt    CodeGen class — tất cả fields
│   │   ├── init.dt       fn new() — init LLVM context + module + builder
│   │   └── emit.dt       fn emit_program() — orchestrate toàn bộ
│   │
│   ├── — emit —————————————————————————————————————————
│   └── emit/
│       ├── items.dt      emit_fn_def, emit_class_def, emit_interface_def,
│       │                 emit_enum_def, emit_const_def, emit_extern_block
│       ├── exprs.dt      fn emit_expr() — tất cả TypedExprKind variants
│       ├── stmts.dt      fn emit_stmt() — tất cả TypedStmtKind variants
│       ├── closures.dt   closure capture struct + invoke function
│       └── builtins.dt   print, len, push, pop, assert, string ops, casts
│
├── driver/
│   ├── pipeline.dt       fn compile_file() — full pipeline orchestration
│   ├── diagnostics.dt    fn format_error() — human-readable error output
│   └── options.dt        CompileOptions class — target, mode, paths
│
└── drat/
    ├── main.dt           entry point
    ├── config.dt         drat.toml / project config parsing
    │
    └── cmd/
        ├── build.dt      drat build
        ├── run.dt        drat run
        ├── fmt.dt        drat fmt
        ├── lint.dt       drat lint
        ├── lsp.dt        drat lsp
        ├── doc.dt        drat doc
        ├── task.dt       drat task
        ├── test.dt       drat test
        ├── init.dt       drat init
        ├── add.dt        drat add
        └── remove.dt     drat remove
```

---

```
runtime/
│
├── syscall/
│   ├── linux/
│   │   ├── x86_64.asm    write, read, open, close, exit, mmap, munmap
│   │   └── aarch64.asm   same, aarch64 calling convention
│   └── macos/
│       ├── x86_64.asm    same, macOS syscall numbers (0x2000000 offset)
│       └── aarch64.asm   same, macOS/ARM64
│
├── alloc/
│   ├── allocator.dt      Allocator class — mmap-based heap
│   ├── free_list.dt      FreeList class — free block tracking
│   └── error.dt          AllocError enum — OOM, invalid free
│
├── io/
│   ├── stdio.dt          fn write_stdout(), write_stderr(), read_stdin()
│   └── file.dt           fn open(), read(), write(), close()
│
├── panic/
│   └── handler.dt        @panic_handler — fn panic_halt()
│
├── scheduler/
│   ├── coop.dt           cooperative scheduler — task queue
│   ├── coroutine.dt      context switch via @asm
│   ├── channel.dt        chan[T] send/recv
│   └── poll.dt           I/O readiness polling (hosted only)
│
└── platform/
    └── platform.dt       Platform interface — hosted vs bare-metal impl
```

---

```
vendor/
└── llvm/
    ├── linux-x86_64/
    │   ├── lib/          libLLVM*.a, libLLD*.a
    │   └── include/      LLVM C API headers
    ├── linux-aarch64/
    ├── macos-x86_64/
    ├── macos-aarch64/
    └── windows-x86_64/
```

---

## Nguyên tắc bất biến

- `drat` là single static binary — mang LLVM + LLD bên trong, không gọi CLI ngoài
- Runtime không dùng libc — syscall trực tiếp qua `@asm`
- Sau khi xoá Rust: codebase chỉ chứa `.dt` và `.asm`
- DraGen (backend Draton thuần) là mục tiêu dài hạn, không có deadline, không block bất kỳ phase nào
- LLVM luôn là fallback khi DraGen chưa ổn định

---

## Kiến trúc binary mục tiêu

```
drat  (single static binary, ~30–50MB)
├── Draton compiler         (.dt — self-host)
├── LLVM statically linked  (bundled, không cần install)
├── LLD statically linked   (linker bundled, không cần system ld)
└── draton-runtime          (.dt + .asm)
    ├── allocator            mmap syscall, không malloc/free
    ├── syscall stubs        write/read/exit qua @asm per-arch
    ├── panic handler
    └── coop-scheduler       bare metal support
```

---

## Cấu trúc repo mục tiêu (sau v0.1.103)

```
draton/
├── compiler/
│   ├── lexer/
│   ├── parser/
│   ├── typeck/
│   └── codegen/         gọi LLVM C API từ Draton
├── runtime/
│   ├── alloc.asm
│   ├── syscall/
│   │   ├── linux-x86_64.asm
│   │   ├── linux-aarch64.asm
│   │   ├── macos-x86_64.asm
│   │   └── macos-aarch64.asm
│   ├── panic.dt
│   ├── scheduler.dt
│   └── io.dt
├── stdlib/
├── drat/
├── vendor/
│   └── llvm/
│       ├── linux-x86_64/
│       ├── linux-aarch64/
│       ├── macos-x86_64/
│       ├── macos-aarch64/
│       └── windows-x86_64/
└── docs/
```

Không còn: `crates/`, `Cargo.toml`, `Cargo.lock`, bất kỳ file `.rs` nào.

---

## Phase 1 — LLVM Bundle + Self-Host Foundation

**Scope:** v0.1.43 → v0.1.58 (16 releases)
**Gate:** `drat build hello.dt` chạy được trên máy không có LLVM/GCC installed.

### 1.1 Vendor LLVM (v0.1.43–.46)

- [x] Chọn LLVM version để vendor (pin hiện tại: LLVM 18.1.8)
- [x] Khai báo bundle LLVM 18 cho linux-x86_64 trong `vendor/llvm/manifest.json`
- [x] Khai báo bundle LLVM 18 cho linux-aarch64 trong `vendor/llvm/manifest.json`
- [x] Khai báo bundle LLVM 18 cho macos-x86_64 trong `vendor/llvm/manifest.json`
- [x] Khai báo bundle LLVM 18 cho macos-aarch64 trong `vendor/llvm/manifest.json`
- [x] Khai báo bundle LLVM 18 cho windows-x86_64 trong `vendor/llvm/manifest.json`
- [x] Chuẩn hoá source of truth qua `vendor/llvm/manifest.json` + `scripts/vendor_llvm.py`
- [x] Viết build/release path chọn đúng LLVM bundle dựa trên host platform
- [x] Verify: `drat` build được với vendored LLVM 18, không cần system LLVM trên Linux host

### 1.2 Bundle LLD (v0.1.47–.49)

- [ ] Extract LLD static libs từ LLVM 18 prebuilt (LLD là một phần của LLVM project)
- [ ] Statically link LLD vào `drat` binary
- [ ] Thay thế mọi lời gọi system linker (`ld`, `link.exe`) bằng LLD in-process API
- [ ] Test: `drat build` không cần `ld` trên PATH
- [ ] Test: `drat build` không cần `link.exe` trên Windows

### 1.3 Self-Host Compiler — Setup + Lexer (v0.1.50–.51)

**Điều kiện trước khi bắt đầu:**
- [ ] Xác nhận canonical syntax đã lock — không có syntax thay đổi pending nào
- [x] Tạo thư mục `compiler/` tại root repo
- [x] Tạo `compiler/README.md` mô tả location và boundary
- [x] Cập nhật `docs/selfhost-canonical-migration-status.md`: ghi nhận location, date bắt đầu
- [x] Cập nhật `AGENTS.md`: thêm rule về self-host reintroduction boundary
- [x] Hidden `drat selfhost-stage0` build và chạy binary tối thiểu từ `compiler/main.dt` + `compiler/driver/pipeline.dt`

**Port `draton-lexer` → `compiler/lexer/lexer.dt`:**
> Trạng thái thực thi hiện tại: stage0 lexer đang chạy trong `compiler/driver/pipeline.dt`.
> `compiler/lexer/` vẫn là split-tree foundation đang được đồng bộ dần, chưa là parity path chính.

- [x] Định nghĩa `enum TokenKind` — toàn bộ 60+ variants từ Rust (keywords, operators, literals, `@`-tokens, `Eof`)
- [x] Định nghĩa `class Span { start, end, line, col }` và `class Token { kind, lexeme, span }`
- [x] Định nghĩa `class LexResult { tokens: Array[Token], errors: Array[LexError] }`
- [x] Định nghĩa `enum LexError` — `UnexpectedChar`, `UnterminatedString`, `UnterminatedBlockComment`, `InvalidNumericLiteral`
- [x] Implement `class Lexer` với fields: `source: String`, `position: Int`, `line: Int`, `col: Int`
- [x] Implement `fn tokenize()` — main loop với `peek_char`/`advance_char`/`advance_newline`
- [x] Implement whitespace skip (`space`, `tab`, `\n`, `\r\n`)
- [x] Implement comment skip: `//` line comment, `/* */` block comment, `///` doc comment → emit `DocComment` token
- [x] Implement string literal lexer: `"..."` và `f"..."` (FStrLit), escape sequences, unterminated error
- [x] Implement number lexer: integer, float, hex (`0x`), binary (`0b`), invalid literal detection
- [x] Implement identifier + keyword dispatch — toàn bộ 25 keywords (`let`, `mut`, `fn`, `return`, ..., `lambda`, `const`)
- [x] Implement `@`-token lexer: `@type`, `@unsafe`, `@pointer`, `@asm`, `@comptime`, `@if`, `@acyclic`, `@gc_config`, `@panic_handler`, `@oom_handler`, `@extern`
- [x] Implement tất cả operator tokens bao gồm multi-char: `==`, `!=`, `<=`, `>=`, `=>`, `->`, `??`, `..`, `++`, `--`, `+=`, `-=`, `*=`, `/=`, `%=`, `<<`, `>>`
- [ ] **Parity test:** chạy Rust lexer và self-host lexer trên toàn bộ `examples/` — token stream phải khớp 100%

**Port `draton-ast` → `compiler/ast/`:**
- [ ] `compiler/ast/types.dt` — `enum TypeExpr`: Named, Array, Map, Set, Tuple, Option, Result, Chan, Fn, Pointer, Infer
- [ ] `compiler/ast/expr.dt` — `enum Expr`: Lit, Var, BinOp, UnOp, Call, MethodCall, FieldAccess, Index, FStr, Match, Lambda, Spawn, Chan, Await, Comptime, Unsafe, AsmBlock
- [ ] `compiler/ast/expr.dt` — `enum BinOp`, `enum UnOp`, `enum FStrPart`, `class MatchArm`, `enum MatchArmBody`
- [ ] `compiler/ast/stmt.dt` — `enum Stmt`: Let, LetDestructure, Assign, Return, If, While, For, Spawn, GcConfig, IfCompile, ExprStmt
- [ ] `compiler/ast/stmt.dt` — `class Block`, `class LetStmt`, `class AssignStmt`, `enum AssignOp`, `class IfStmt`, `class ElseBranch`, `class ForStmt`, `class WhileStmt`, `class SpawnStmt`, `class ReturnStmt`, `class DestructureBinding`
- [ ] `compiler/ast/item.dt` — `enum Item`: Fn, Class, Layer, Interface, Enum, Error, Const, Import, TypeBlock, Extern, PanicHandler, OomHandler
- [ ] `compiler/ast/item.dt` — `class FnDef`, `class Param`, `class ClassDef`, `class ClassMember`, `class FieldDef`, `class LayerDef`, `class InterfaceDef`, `class EnumDef`, `class ErrorDef`, `class ConstDef`, `class ImportDef`, `class ImportItem`, `class TypeBlock`, `class TypeMember`, `class ExternBlock`, `class Program`
- [ ] Tất cả node phải giữ `Span` để error reporting
- [ ] **Parity test:** không cần, AST là data structure — verify qua parser output

**Port `draton-parser` → `compiler/parser/`:**
- [x] `compiler/parser/parser.dt` — `class Parser` với fields: `tokens: Array[Token]`, `pos: Int`, error collection
- [ ] Implement `fn peek()`, `fn advance()`, `fn expect(kind)`, `fn at(kind)`, `fn sync_to(recovery_set)` (error recovery)
- [ ] `compiler/parser/item.dt` — parse top-level items: fn, class, layer, interface, enum, error, const, import, `@type`, `@extern`, `@panic_handler`, `@oom_handler`, `@if` compile-time block
- [ ] `compiler/parser/stmt.dt` — parse statements: `let`/`let mut`, destructure, assign (+=, -=, ...), return, if/elif/else, while, for..in, spawn, `@gc_config`, `@if` block
- [ ] `compiler/parser/expr.dt` — parse expressions với precedence climbing: binary ops, unary ops, call, method call, field access, index, f-string interpolation, match, lambda, `@comptime`, `@unsafe`, `@asm`
- [ ] Parse `@type` blocks: file-level, class-level, layer-level, interface-level, function-level contracts
- [ ] Parse generics: type params `[T, U]` trên class/fn, type args trong call/instantiation
- [ ] Parse pattern matching: literal, variable, constructor, tuple, wildcard `_`, guard `if`
- [ ] Implement error recovery: skip đến sync token thay vì crash toàn bộ file
- [ ] **Parity test:** chạy self-host parser trên toàn bộ `examples/` và `crates/*/tests/` fixture — AST phải khớp với Rust parser output (serialize cả hai sang JSON và diff)

---

### 1.4 Self-Host Compiler — TypeChecker (v0.1.52–.55)

> TypeChecker là phần phức tạp nhất của self-host. Chia làm 3 lớp độc lập, port theo thứ tự từ đơn giản đến phức tạp. Ownership inference được để sang Phase 3.

**Lớp 1 — Type System + Substitution (v0.1.52)**

- [ ] `compiler/typeck/types.dt` — `enum Type`: Bool, Int, Int8..Int64, UInt8..UInt64, Float, Float32, Float64, Char, String, Unit, Never, Pointer, Array, Map, Set, Tuple, Option, Result, Chan, Fn, Named, Row, Var (type variable)
- [ ] `compiler/typeck/subst.dt` — `class Substitution` với immutable map `Int → Type`
  - [ ] `fn empty()` — tạo substitution rỗng
  - [ ] `fn bind(var, ty)` — bind type variable, check occurs (infinite type detection)
  - [ ] `fn apply(ty)` — substitute type variables recursively cho tất cả `Type` variants
  - [ ] `fn compose(other)` — compose hai substitution
- [ ] `compiler/typeck/unify.dt` — `fn unify(a, b, subst)` → `Result[Substitution, TypeError]`
  - [ ] Unify primitives, named types, generic instances
  - [ ] Unify function types (params + return)
  - [ ] Unify container types (Array, Map, Set, Chan, Option, Result, Tuple)
  - [ ] Unify Row types (structural typing cho class/interface)
  - [ ] Occurs check để prevent infinite types
- [ ] `compiler/typeck/env.dt` — `class Scheme { vars: Array[Int], ty: Type }` (type scheme cho generics)
- [ ] `compiler/typeck/env.dt` — `class TypeEnv` với scope stack
  - [ ] `fn extend(name, scheme)` — thêm binding vào scope hiện tại
  - [ ] `fn lookup(name)` — tìm binding, walk scope stack
  - [ ] `fn push_scope()` / `fn pop_scope()` — scope management
  - [ ] `fn instantiate(scheme)` — tạo fresh type variables cho generic instantiation
  - [ ] `fn generalize(ty, env)` — tạo Scheme từ type (cho let-polymorphism)
- [ ] `compiler/typeck/error.dt` — `enum TypeError`: UnboundVariable, TypeMismatch, InfiniteType, ArityMismatch, NotCallable, MissingField, NotIterable, ...

**Lớp 2 — Inference Engine (v0.1.53–.54)**

- [ ] `compiler/typeck/check.dt` — `class TypeChecker` với các fields:
  - `env: TypeEnv`
  - `subst: Substitution`
  - `fresh_counter: Int` (type variable counter)
  - `errors: Array[TypeError]`, `warnings: Array[TypeError]`
  - `class_fields: Map[String, Map[String, Type]]`
  - `class_methods: Map[String, Map[String, Scheme]]`
  - `class_type_params: Map[String, Array[Type]]`
  - `class_parents: Map[String, String]`
  - `interface_methods: Map[String, Map[String, Scheme]]`
  - `class_interfaces: Map[String, Array[String]]`
  - `declared_classes: Set[String]`
  - `enum_defs: Map[String, Array[String]]`
  - `function_hints: Map[String, FnDef]` (từ `@type` blocks)
  - `binding_hints: Map[String, TypeExpr]` (từ `@type` blocks)
  - `class_function_hints: Map[String, Map[String, FnDef]]`
  - `current_return: Array[Type]` (stack cho nested fn)
  - `current_class: Array[String]` (stack cho nested class)
  - `type_param_scopes: Array[Map[String, Type]]`
  - `deprecated_syntax_mode: DeprecatedSyntaxMode` (Warn | Deny)
- [ ] Implement `fn fresh_var()` — tạo type variable mới
- [ ] Implement first-pass: collect tất cả class/interface/enum/fn definitions trước khi infer
- [ ] Implement `@type` block processing: extract hints vào `function_hints`/`binding_hints`/etc trước inference
- [ ] Implement `fn infer_expr(expr, env, subst)` → `(Type, Subst)` cho tất cả `Expr` variants:
  - Literals: Int, Float, Bool, String, FStr, None
  - Variable lookup với instantiation
  - Binary/unary ops với operator overloading rules
  - Function call: infer callee, args, unify với fn type
  - Method call: resolve method từ `class_methods`, handle receiver type
  - Field access: resolve field type từ `class_fields`, Row type
  - Index: Array/Map index với type constraints
  - Lambda: infer param types, body, create Fn type
  - Match: infer subject, check all arm patterns, unify arm body types
  - Spawn expression: infer spawned fn type
  - f-string: infer interpolation parts
  - `@comptime` / `@unsafe` / `@asm` expressions
- [ ] Implement `fn infer_stmt(stmt, env, subst)` cho tất cả `Stmt` variants:
  - `let`/`let mut`: infer RHS, generalize nếu không có hint, extend env
  - `let` destructure: infer RHS tuple type, bind individual names
  - Assignment với `AssignOp` (+=, -=, *=, /=, %=)
  - `return`: unify với `current_return` type
  - `if`/`elif`/`else`: infer condition (Bool), unify branch types
  - `while`: infer condition (Bool), infer body
  - `for..in`: infer iterable type, extract element type, bind loop var
  - `spawn`: infer spawn body
  - `@gc_config`, `@if` compile-time: handle separately
- [ ] Implement `fn check_item(item)` cho tất cả `Item` variants:
  - `fn`: check params vs hints, infer body, check return type
  - `class`: check fields, methods, layer members vs `@type` hints
  - `interface`: check method signatures
  - `enum`: register variants
  - `error`: register error type
  - `const`: infer type
  - `import`: resolve module type (stub cho giai đoạn này)
  - `@type` block: extract into hint maps
  - `@extern`: register external symbols với their types

**Lớp 3 — Exhaustiveness + Validation (v0.1.55)**

- [ ] `compiler/typeck/exhaust.dt` — exhaustiveness checker cho `match`:
  - [ ] `fn classify_subject(ty)` — phân loại subject: Bool, Enum, Option, Result, Tuple, Wildcard
  - [ ] `fn extract_pattern(arm)` — extract pattern structure từ match arm
  - [ ] `class ExhaustivenessChecker` — check tất cả cases được cover, report missing patterns
  - [ ] Handle `_` wildcard, literal patterns, constructor patterns, guard conditions
- [ ] Implement interface implementation checking: verify class implements tất cả required methods với đúng type
- [ ] Implement `@type` contract enforcement: verify inferred types khớp với explicit contracts
- [ ] Implement deprecated syntax detection: inline type annotations, typed params, inline return types → Warn hoặc Deny
- [ ] `compiler/typeck/typed_ast.dt` — định nghĩa `TypedProgram`, `TypedItem`, `TypedFnDef`, `TypedExpr`, `TypedStmt`, `TypedBlock`, `OwnershipState`, `UseEffect`, `ParamOwnershipSummary`, `FnOwnershipSummary` (ownership fields để trống cho Phase 3)
- [ ] `compiler/typeck/mod.dt` — wire tất cả lại, export `fn type_check(program)` → `TypeCheckResult`
- [ ] **Parity test:** chạy self-host typeck trên toàn bộ `crates/draton-typeck/tests/` fixtures — errors/warnings phải khớp với Rust typeck output

---

### 1.5 Self-Host Compiler — CodeGen via LLVM C API (v0.1.56–.58)

> CodeGen port từ `draton-codegen`. Rust dùng `inkwell` (LLVM Rust bindings). Self-host dùng LLVM C API trực tiếp qua `@extern` + `@unsafe`.

**LLVM C API Bindings (v0.1.56)**

- [ ] `compiler/codegen/llvm_c.dt` — `@extern` bindings cho LLVM C API:
  - Context: `LLVMContextCreate`, `LLVMContextDispose`
  - Module: `LLVMModuleCreateWithNameInContext`, `LLVMDisposeModule`, `LLVMPrintModuleToFile`
  - Builder: `LLVMCreateBuilderInContext`, `LLVMDisposeBuilder`, `LLVMPositionBuilderAtEnd`
  - Types: `LLVMInt1Type`, `LLVMInt8Type`, `LLVMInt32Type`, `LLVMInt64Type`, `LLVMDoubleType`, `LLVMVoidType`, `LLVMPointerType`, `LLVMArrayType`, `LLVMStructType`, `LLVMFunctionType`
  - Values: `LLVMConstInt`, `LLVMConstReal`, `LLVMConstString`, `LLVMConstStruct`, `LLVMConstNull`, `LLVMGetUndef`
  - Functions: `LLVMAddFunction`, `LLVMGetNamedFunction`, `LLVMAppendBasicBlock`, `LLVMGetParam`
  - Instructions: `LLVMBuildAlloca`, `LLVMBuildLoad`, `LLVMBuildStore`, `LLVMBuildGEP2`, `LLVMBuildCall2`, `LLVMBuildRet`, `LLVMBuildRetVoid`, `LLVMBuildBr`, `LLVMBuildCondBr`
  - Arithmetic: `LLVMBuildAdd`, `LLVMBuildSub`, `LLVMBuildMul`, `LLVMBuildSDiv`, `LLVMBuildSRem`, `LLVMBuildFAdd`, `LLVMBuildFSub`, `LLVMBuildFMul`, `LLVMBuildFDiv`
  - Compare: `LLVMBuildICmp`, `LLVMBuildFCmp`
  - Memory: `LLVMBuildMalloc`, `LLVMBuildFree`, `LLVMBuildMemCpy`
  - Globals: `LLVMAddGlobal`, `LLVMSetInitializer`, `LLVMSetLinkage`
  - Target: `LLVMInitializeX86Target`, `LLVMInitializeAArch64Target`, `LLVMGetDefaultTargetTriple`, `LLVMCreateTargetMachine`, `LLVMTargetMachineEmitToFile`
  - Optimize: `LLVMCreatePassManager`, `LLVMAddPromoteMemoryToRegisterPass`, `LLVMRunPassManager`

**CodeGen Core (v0.1.57)**

- [ ] `compiler/codegen/mangle.dt` — `fn mangle_fn(name, class, type_args)` → mangled name (khớp với Rust mangle.rs)
- [ ] `compiler/codegen/mono.dt` — monomorphization collector:
  - [ ] `class MonoCollector` — track generic instantiations đã gặp
  - [ ] `fn collect_program(typed_program)` — walk AST, collect all generic instantiations
  - [ ] `fn resolve_function_type_args(fn_def, type_args)` — substitute type params
  - [ ] `class GenericFnDef`, `class GenericClassDef` — lưu generic definitions
- [ ] `compiler/codegen/vtable.dt` — interface vtable registry:
  - [ ] `class InterfaceRegistry` — track interfaces và implementations
  - [ ] `fn register_interface(name, methods)` — đăng ký interface
  - [ ] `fn register_impl(class_name, iface_name)` — đăng ký implementation
  - [ ] `fn get_vtable_layout(iface_name)` — lấy vtable method order
- [ ] `compiler/codegen/types.dt` — map Draton `Type` → LLVM type:
  - [ ] Primitives: Bool→i1, Int→i64, Int8→i8, ..., Float→double, Char→i32, Unit→void
  - [ ] String → `{ i8*, i64 }` struct (ptr + len)
  - [ ] Array[T] → `{ T*, i64, i64 }` struct (ptr + len + cap)
  - [ ] Class → LLVM struct type theo `ClassLayout`
  - [ ] Fn type → function pointer
  - [ ] Option[T] → `{ i1, T }` struct
  - [ ] Result[T, E] → `{ i1, union{T, E} }` struct
- [ ] `compiler/codegen/codegen.dt` — `class CodeGen` với fields:
  - `context`, `module`, `builder` (LLVM C API handles dưới dạng `@pointer`)
  - `mode: BuildMode` (Debug | Release | Size | Fast)
  - `string_type`, `closure_record_type` — cached LLVM struct types
  - `functions: Map[String, LLVMValueRef]`
  - `class_layouts: Map[String, ClassLayout]`
  - `variables: Array[Map[String, LLVMValueRef]]` (scope stack)
  - `current_function: Option[LLVMValueRef]`
  - `current_return_type: Option[Type]`
  - `current_class: Option[String]`
  - `free_points: Map[Int, Array[LLVMValueRef]]` (ownership-driven free insertion)
  - `ownership_free_spans: Map[String, Array[Int]]`
  - `mono: MonoCollector`
  - `generic_classes: Map[String, GenericClassDef]`
  - `generic_functions: Map[String, GenericFnDef]`
  - `iface_registry: InterfaceRegistry`
  - `vtable_globals: Map[(String, String), LLVMValueRef]`
  - `closure_counter: Int`
  - `string_counter: Int`

**CodeGen Emit (v0.1.57–.58)**

- [ ] `compiler/codegen/item.dt` — emit top-level items:
  - [ ] `fn emit_fn_def(fn_def)` — emit function prototype + body
  - [ ] `fn emit_class_def(class_def)` — emit struct type + method functions
  - [ ] `fn emit_interface_def(iface_def)` — emit vtable struct type
  - [ ] `fn emit_enum_def(enum_def)` — emit enum as tagged union
  - [ ] `fn emit_const_def(const_def)` — emit global constant
  - [ ] `fn emit_extern_block(extern_block)` — declare external symbols
  - [ ] `fn emit_vtable(class_name, iface_name)` — emit vtable global constant
  - [ ] `fn emit_program(typed_program)` — drive tất cả items, gọi mono expansion
- [ ] `compiler/codegen/expr.dt` — emit expressions:
  - [ ] Literals: int, float, bool, string (interned globals), None
  - [ ] Variable load với scope lookup
  - [ ] Binary/unary ops → LLVM arithmetic/compare instructions
  - [ ] Function call: resolve mangled name, emit args, `LLVMBuildCall2`
  - [ ] Method call: load vtable function pointer hoặc direct call
  - [ ] Field access: `LLVMBuildGEP2` với field index từ `ClassLayout`
  - [ ] Array index: bounds check (debug mode), `LLVMBuildGEP2`
  - [ ] f-string: concatenate parts, emit sprintf-style call to runtime
  - [ ] Match: emit subject, branch per arm, emit phi node for result
  - [ ] Lambda/closure: emit closure struct, capture vars, emit fn pointer
  - [ ] Spawn: emit task creation via runtime ABI
- [ ] `compiler/codegen/stmt.dt` — emit statements:
  - [ ] `let`/`let mut`: `LLVMBuildAlloca`, store initial value
  - [ ] Assignment: load LHS pointer, store new value
  - [ ] `return`: emit return value, insert ownership `free()` calls từ `free_points`
  - [ ] `if`/`elif`/`else`: emit condition, `LLVMBuildCondBr`, merge block
  - [ ] `while`: emit loop header, body, back-edge
  - [ ] `for..in`: emit iterator protocol (array index loop hoặc iterator ABI)
  - [ ] Ownership `free()` insertion: sau last-use của owned non-copy bindings
- [ ] `compiler/codegen/closure.dt` — closure code generation:
  - [ ] Emit closure capture struct (one LLVM struct per closure)
  - [ ] Emit closure invoke function
  - [ ] Handle escaping closures (heap-allocate capture struct)
- [ ] `compiler/codegen/builtins.dt` — built-in function emit:
  - [ ] `print`, `println`, `len`, `push`, `pop`, `keys`, `values`, `contains`, `assert`
  - [ ] String ops: `+`, `len`, `slice`, `chars`
  - [ ] Type conversions: `Int(x)`, `Float(x)`, `String(x)`
- [ ] **End-to-end test:** self-host compiler compile được Hello World → binary chạy đúng
- [ ] **End-to-end test:** self-host compiler compile được tất cả `examples/` → binary output khớp với Rust compiler output
- [ ] **Gate:** `drat build hello.dt` trên máy fresh Linux không có LLVM installed → binary chạy được

---

## Phase 2 — Kill libc trong Runtime

**Scope:** v0.1.59 → v0.1.72 (14 releases)
**Gate:** `ldd $(which drat)` → *not a dynamic executable*. Binary output của `drat build` cũng fully static.

### 2.1 Allocator không libc (v0.1.59–.62)

- [ ] Viết `mmap`-based allocator cho linux-x86_64 bằng `@asm`
- [ ] Viết `mmap`-based allocator cho linux-aarch64 bằng `@asm`
- [ ] Viết `mmap`/`vm_allocate`-based allocator cho macos bằng `@asm`
- [ ] Implement free list / slab allocator cơ bản (đủ cho runtime, không cần tối ưu ngay)
- [ ] Thay toàn bộ `malloc`/`free` trong `draton-runtime` bằng allocator mới
- [ ] Test: không còn symbol `malloc`/`free` trong runtime object

### 2.2 Syscall I/O không libc (v0.1.63–.65)

- [ ] Viết `write` syscall wrapper cho linux-x86_64 bằng `@asm`
- [ ] Viết `write` syscall wrapper cho linux-aarch64 bằng `@asm`
- [ ] Viết `write` syscall wrapper cho macos bằng `@asm`
- [ ] Viết `read` syscall wrapper cho tất cả supported targets
- [ ] Viết `open`/`close` syscall wrapper cho tất cả supported targets
- [ ] Viết `exit` syscall wrapper cho tất cả supported targets
- [ ] Thay toàn bộ libc I/O trong runtime bằng syscall wrappers
- [ ] Port sang Draton (`runtime/syscall/`, `runtime/io.dt`)

### 2.3 Panic + Scheduler không libc (v0.1.66–.68)

- [ ] Port panic handler: không dùng `abort()`/`raise()` từ libc — dùng `@asm` `ud2` hoặc kill syscall
- [ ] Port coop-scheduler: không dùng `pthread` — dùng `clone` syscall (Linux) hoặc `sigaltstack` cho context switch
- [ ] Test: panic handler hoạt động đúng trên tất cả targets
- [ ] Test: coop-scheduler pass test suite hiện tại

### 2.4 Link Runtime Fully Static (v0.1.69–.72)

- [ ] Compile `draton-runtime` với `-nostdlib`
- [ ] Compile `draton-runtime` với `-static`
- [ ] Xoá toàn bộ libc symbol references khỏi runtime
- [ ] `drat build` output: `ldd` → *not a dynamic executable*
- [ ] `drat` binary itself: `ldd` → *not a dynamic executable*
- [ ] Test toàn bộ platform targets: linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64

---

## Phase 3 — Bootstrap + Xoá Rust

**Scope:** v0.1.73 → v0.1.90 (18 releases)
**Gate:** Repo không còn file `.rs` hay `Cargo.toml` nào.

### 3.1 Ownership Inference Engine (v0.1.73–.76)

> Đây là phần phức tạp nhất của toàn bộ self-host. Ownership inference quyết định tính đúng đắn của memory management — sai ở đây là use-after-free hoặc double-free. Port phải **semantics-preserving hoàn toàn**, không được redesign.

**Data structures (v0.1.73)**

- [ ] `compiler/typeck/ownership.dt` — toàn bộ types nội bộ của ownership checker:
  - [ ] `enum BorrowKind { Shared, Exclusive }`
  - [ ] `class BindingState`:
    - `name: String`
    - `ty: Type`
    - `state: OwnershipState` (Owned | BorrowedShared | BorrowedExclusive | Moved | Escaped)
    - `state_span: Span`
    - `origin: Option[Int]` (origin ID cho borrow tracking)
    - `is_mut: Bool`
    - `is_param: Bool`
    - `is_closure: Bool`
  - [ ] `class BorrowRecord`:
    - `kind: BorrowKind`
    - `span: Span`
    - `persistent: Bool` (borrow tồn tại qua block boundary không)
  - [ ] `class OwnershipEnv`:
    - `bindings: Map[String, BindingState]`
    - `borrows: Map[String, Array[BorrowRecord]]`
    - `origin_parents: Map[Int, Int]` (origin parent tracking)
  - [ ] `class ClosureMeta`:
    - `captures: Set[String]`
    - `exclusive_captures: Set[String]`
    - `escaping: Bool`
    - `last_call: Option[Span]`
  - [ ] `class FunctionRecord`:
    - `params: Array[TypedParam]`
    - `body: Option[TypedBlock]`
    - `ret_type: Type`
    - `receiver_ty: Option[Type]`
  - [ ] `class InternalFnSummary`:
    - `summary: FnOwnershipSummary`
    - `receiver_effect: Option[UseEffect]`
  - [ ] `class FunctionIndex`:
    - `records: Map[String, FunctionRecord]`
    - `top_level: Set[String]`
    - `methods: Map[(String, String), String]` (class, method) → mangled name
    - `enum_names: Set[String]`
    - `class_fields: Map[String, Map[String, Type]]`
    - `acyclic_classes: Set[String]`

**Copy type predicate (v0.1.73)**

- [ ] Implement `fn is_copy(ty)` — khớp chính xác với Rust `is_copy`:
  - Copy: Bool, Int, Int8–Int64, UInt8–UInt64, Float, Float32, Float64, Char, Unit, Never, Pointer, Fn
  - Copy: Tuple nếu `len <= 2` và tất cả elements là copy
  - Copy: Option[T] nếu T là copy
  - Non-copy: String, Array, Map, Set, Chan, Named (class), Result, Row
  - Non-copy: Tuple nếu len > 2 hoặc có element non-copy

**Ownership inference core (v0.1.74–.75)**

- [ ] `class OwnershipChecker` với fields:
  - `env: OwnershipEnv`
  - `fn_index: FunctionIndex`
  - `fn_summaries: Map[String, InternalFnSummary]`
  - `errors: Array[OwnershipError]`
  - `origin_counter: Int`
  - `closure_metas: Map[String, ClosureMeta]`
- [ ] Implement `fn build_function_index(typed_program)` — index tất cả functions/methods/classes trước khi check
- [ ] Implement `fn infer_program(typed_program)` → `Array[OwnershipError]`:
  - Phase 1: build function index
  - Phase 2: infer summary cho từng function (2 passes để handle mutual recursion)
  - Phase 3: check ownership correctness toàn bộ program
- [ ] Implement `fn infer_fn(fn_def)` → `InternalFnSummary`:
  - Khởi tạo `OwnershipEnv` với params (Owned state)
  - Infer body block
  - Summarize: với mỗi param, effect là gì (Copy/BorrowShared/BorrowExclusive/Move)?
  - Determine `returns_owned`: return type có transfer ownership không?
- [ ] Implement `fn check_block(block, env)` — iterate statements, propagate env
- [ ] Implement `fn check_stmt(stmt, env)`:
  - `let`: evaluate RHS effect, bind result (Owned state)
  - `let mut`: same, mark `is_mut = true`
  - `let` destructure: check tuple/array destructuring, bind each part
  - Assignment: check RHS effect, verify LHS is mutable, check borrow conflicts
  - `return`: check return value effect, insert `free()` cho owned bindings còn lại trong scope
  - `if`/`elif`/`else`: check condition (BorrowShared), merge env từ branches (union of states)
  - `while`: check condition, check body, verify no owned values escape loop iteration
  - `for..in`: check iterable (BorrowShared cho array/map), bind loop var
  - `spawn`: check spawn body — captures phải được move hoặc là copy
- [ ] Implement `fn check_expr(expr, env)` → `(UseEffect, OwnershipEnv)`:
  - Variable: nếu copy type → Copy, nếu non-copy → BorrowShared (default), escalate khi needed
  - Call: resolve function summary, apply effects to arguments
  - Method call: apply receiver_effect, apply param effects
  - Field access: propagate receiver borrow state
  - Binary op: determine operand effects (arithmetic → Copy for numeric)
  - Move semantics: khi value được passed to Move-effect parameter → mark Moved, error nếu dùng sau
  - Borrow conflicts: exclusive borrow khi shared borrow active → `OwnershipError::ConflictingBorrow`
  - Closure: analyze captures, determine if escaping, build `ClosureMeta`
- [ ] Implement `free()` insertion logic:
  - Track last-use span của mỗi owned binding
  - Sau last-use: record vào `ownership_free_spans` (pass tới codegen)
  - Ở `return` và end-of-block: insert free cho tất cả owned bindings chưa moved/escaped
- [ ] Implement `@acyclic` class handling: class được annotate `@acyclic` không có reference cycles → simplify ownership (không cần cycle detection)

**Error reporting + edge cases (v0.1.76)**

- [ ] `enum OwnershipError`:
  - `UseAfterMove { name, move_span, use_span }`
  - `MoveWhileBorrowed { name, borrow_span, move_span }`
  - `ConflictingBorrow { name, existing_span, new_span }`
  - `MutationWhileBorrowed { name, borrow_span, mutation_span }`
  - `CannotMoveOutOfBorrow { name, span }`
  - `AmbiguousOwnership { name, span }` (cycle hoặc unclear path)
  - `EscapingExclusiveBorrow { name, span }`
- [ ] Handle merge conflicts khi joining branches: nếu một branch moves và branch kia không → `AmbiguousOwnership`
- [ ] Handle loop ownership: owned value không được move trong loop body (would double-free)
- [ ] Handle closure capture conflicts: exclusive capture khi value borrowed elsewhere
- [ ] Handle `@unsafe` blocks: ownership rules relaxed nhưng không tắt hoàn toàn
- [ ] Wire ownership checker vào `type_check()` pipeline: chạy sau HM inference, trước TypedProgram finalization
- [ ] Wire `ownership_free_spans` vào `TypedFnDef` để codegen biết chỗ insert `free()`
- [ ] **Parity test:** chạy self-host ownership checker trên toàn bộ `crates/draton-typeck/tests/ownership*.rs` fixtures — errors phải khớp chính xác (same error variant, same span)

### 3.2 Bootstrap — Stage 0 → 1 → 2 (v0.1.77–.80)

- [ ] Stage 0: Rust compiler compile self-host compiler → `drat-stage0` binary
- [ ] Stage 1: `drat-stage0` compile self-host compiler → `drat-stage1` binary
- [ ] Stage 2: `drat-stage1` compile self-host compiler → `drat-stage2` binary
- [ ] Verify: `drat-stage1` và `drat-stage2` output byte-for-byte identical (reproducible build)
- [ ] Gate: bootstrap pass, output identical

### 3.3 Port `drat` CLI Driver (v0.1.81–.84)

- [ ] Port `drat build` command sang Draton
- [ ] Port `drat run` command sang Draton
- [ ] Port `drat fmt` command sang Draton
- [ ] Port `drat lsp` command sang Draton
- [ ] Port `drat lint` command sang Draton
- [ ] Port `drat doc` command sang Draton
- [ ] Port `drat task` command sang Draton
- [ ] Test: toàn bộ CLI commands hoạt động qua self-host driver

### 3.4 Freeze + Xoá Rust (v0.1.85–.90)

- [ ] Đánh dấu `crates/` là readonly — tạo `CRATES_FROZEN.md` nói rõ lý do
- [ ] Không nhận feature PR mới vào Rust crates
- [ ] Chỉ nhận security/critical bug fix vào Rust crates
- [ ] Xoá `crates/` hoàn toàn
- [ ] Xoá `Cargo.toml` root
- [ ] Xoá `Cargo.lock`
- [ ] Verify: `find . -name "*.rs"` → không có kết quả
- [ ] Cập nhật `docs/selfhost-canonical-migration-status.md`
- [ ] Cập nhật `docs/compiler-architecture.md`
- [ ] Cập nhật `AGENTS.md`

---

## Phase 4 — Hardening + Release

**Scope:** v0.1.91 → v0.1.103 (12 releases)
**Gate:** `curl .../install.sh | sh` → `drat` hoạt động, không cần gì khác.

### 4.1 CI/CD Migration (v0.1.91–.95)

- [ ] Viết `drat build` pipeline thay thế toàn bộ `cargo build` trong CI
- [ ] Xoá `rustup` / Rust toolchain khỏi CI setup
- [ ] Xoá `cargo test` — thay bằng `drat test` hoặc native test runner
- [ ] Cross-compile CI: build cho tất cả 5 targets từ một host
- [ ] CI green trên tất cả platforms

### 4.2 Platform Validation (v0.1.96–.99)

- [ ] Full test suite trên linux-x86_64 (qua bundled LLVM)
- [ ] Full test suite trên linux-aarch64
- [ ] Full test suite trên macos-x86_64
- [ ] Full test suite trên macos-aarch64
- [ ] Full test suite trên windows-x86_64
- [ ] Fuzz test: lexer, parser, typeck với random input
- [ ] Regression suite: compile toàn bộ examples/ và stdlib/

### 4.3 Install Flow (v0.1.100–.101)

- [ ] `install.sh` tải static binary trực tiếp — không cần Rust toolchain để build release
- [ ] SHA256 checksum verification giữ nguyên
- [ ] Test: install từ scratch trên fresh Linux VM
- [ ] Test: install từ scratch trên fresh macOS VM
- [ ] Test: install từ scratch trên fresh Windows VM
- [ ] Cập nhật docs/install.md

### 4.4 Release v0.1.103 "Zero Dependency" (v0.1.102–.103)

- [ ] Archive Rust source vào branch `legacy/rust-compiler` trước khi xoá
- [ ] Viết migration guide cho contributors
- [ ] Viết release notes chi tiết
- [ ] Update README.md với install instructions mới
- [ ] Announcement

---

## DraGen — Backend Draton Thuần (không có deadline)

> **Chỉ bắt đầu khi:** v0.1.103 stable ít nhất 6 tháng, self-host compiler tự compile ổn định, có thời gian.

### Điều kiện bắt đầu

- [ ] v0.1.103 released và stable 6 tháng
- [ ] Self-host compiler bootstrap không có regression trong 6 tháng
- [ ] Draton IR spec được thiết kế và review xong
- [ ] Có người/thời gian commit cho DraGen

### Kiến trúc DraGen (khi làm)

```
Draton IR  →  DraGen  →  x86_64 machine code
                      →  aarch64 machine code
                      →  (sau này) WASM, RISC-V
```

- DraGen viết hoàn toàn bằng Draton
- Draton IR: SSA form, typed, explicit ownership — đơn giản hơn LLVM IR
- Register allocator: linear scan (đủ tốt cho giai đoạn đầu)
- ELF/Mach-O/PE emitter: viết trực tiếp, không cần `as` hay `ld`
- LLVM vẫn là fallback: `drat build --backend=llvm` / `--backend=drangen`
- Chỉ xoá LLVM khi DraGen pass toàn bộ test suite và stable

---

## Rủi ro và biện pháp

| Rủi ro | Biện pháp |
|---|---|
| Ownership inference self-bootstrap phức tạp | Cho phép `@unsafe` trong internal compiler code ở giai đoạn đầu |
| LLVM API thay đổi giữa versions | Pin LLVM 18, upgrade theo phase riêng |
| Platform regression khi bỏ libc | Mỗi phase có per-platform CI gate bắt buộc trước khi merge |
| Self-host compiler chậm hơn Rust | Chấp nhận — performance là việc sau DraGen |
| Windows syscall không có libc wrapper đơn giản | Windows giữ libc lâu hơn nếu cần, Linux/macOS ưu tiên trước |

---

## Tracking

| Phase | Version Range | Gate | Status |
|---|---|---|---|
| Phase 1: LLVM Bundle + Self-Host Foundation | v0.1.43–.58 | `drat build` chạy không cần LLVM installed | 🟨 Đang làm |
| Phase 2: Kill libc trong Runtime | v0.1.59–.72 | `ldd drat` → not dynamic | ⬜ Chưa bắt đầu |
| Phase 3: Bootstrap + Xoá Rust | v0.1.73–.90 | Không còn `.rs` trong repo | ⬜ Chưa bắt đầu |
| Phase 4: Hardening + Release | v0.1.91–.103 | Install từ scratch hoạt động | ⬜ Chưa bắt đầu |
| DraGen | Không có deadline | DraGen pass full test suite | ⬜ Chưa bắt đầu |
