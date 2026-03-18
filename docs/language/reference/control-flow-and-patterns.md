---
title: Control flow and pattern matching
sidebar_position: 19
---

# Control flow and pattern matching

This page covers statements and control-flow forms.

## Return

Canonical return:

```draton
return value
return
```

Explicit `return` remains the canonical style even where the parser may accept expression-tail bodies in some compatibility paths.

## If / elif / else

Accepted forms:

```draton
if ready {
    return 1
}

if ready {
    return 1
} elif waiting {
    return 2
} else {
    return 3
}
```

## While loops

```draton
while x < 10 {
    x += 1
}
```

## For-in loops

```draton
for item in items {
    print(item)
}
```

The current parsed form is `for <name> in <expr> { ... }`.

## Nested blocks

Standard blocks are statements:

```draton
{
    let x = 1
    print(x)
}
```

## Match as control-oriented expression

While `match` is an expression, it often carries control flow:

```draton
fn render(value) {
    return match value {
        true => "yes",
        false => "no",
    }
}
```

## Spawn

Spawn is a statement form, not a function call:

```draton
spawn work()
spawn { let x = 1 }
```

Both forms are supported:

- spawn an expression
- spawn a block

See also [Concurrency and channels](./concurrency-and-channels.md).
