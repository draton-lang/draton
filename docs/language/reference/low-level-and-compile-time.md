---
title: Low-level and compile-time syntax
sidebar_position: 24
---

# Low-level and compile-time syntax

This page covers the syntax Draton exposes for low-level escape hatches and compile-time control.

## Unsafe block

```draton
@unsafe {
    let x = 1 + 2
}
```

## Pointer block

```draton
@pointer {
    let x = 1
}
```

This is the surface escape hatch for pointer-oriented code regions.

## Compile-time block

```draton
@comptime {
    let size = 4 * 1024
}
```

## Inline assembly block

```draton
@asm { mov eax, 1 }
```

The parser currently treats the inside of `@asm { ... }` as raw token text joined back together into one assembly string.

## Compile-time conditional

```draton
@if condition {
    print("enabled")
}
```

This is a statement form, not the same thing as ordinary runtime `if`.

## Deprecated `@gc_config` block

Deprecated and ignored.

```draton
@gc_config {
    threshold = 1024
    young_size = 4096
}
```

This block is accepted for compatibility but has no effect. Draton manages memory through Inferred Ownership at compile time.

## Extern declarations

Extern blocks are the top-level syntax for binding external functions:

```draton
@extern "C" {
    fn malloc(size: UInt64) -> @pointer
    fn free(ptr: @pointer)
}
```

## Special runtime handlers

The parser recognizes two special top-level handler items:

```draton
@panic_handler
fn on_panic(msg) { }

@oom_handler
fn on_oom() { }
```

Use these only when working at the runtime boundary.
