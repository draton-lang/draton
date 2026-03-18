---
title: Functions, calls, and lambdas
sidebar_position: 17
---

# Functions, calls, and lambdas

This page covers function definitions, parameter syntax, calls, methods, and lambda expressions.

## Function definitions

Canonical function definition:

```draton
@type {
    add: (Int, Int) -> Int
}

fn add(a, b) {
    return a + b
}
```

## Public functions

Top-level functions can be public:

```draton
pub fn build() {
    return 0
}
```

Methods inside classes or layers can also be public:

```draton
layer Api {
    pub fn ready() {
        return true
    }
}
```

## Accepted compatibility parameter and return syntax

The parser still accepts inline parameter type hints and return arrows:

```draton
fn add(a: Int, b: Int) -> Int {
    return a + b
}
```

These forms are accepted for compatibility. They are not canonical style.

## Generic parameters

Classes support explicit type parameters:

```draton
class Stack[T] {
    let items
}
```

Function definitions also parse type parameter lists after the function name:

```draton
fn identity[T](value) {
    return value
}
```

The syntax surface exists even when many examples in the repo rely on inference-driven generic usage.

## Calls

Normal calls:

```draton
add(1, 2)
id(value)
```

Method calls:

```draton
user.name()
service.ready()
```

Field access and method calls chain in the usual left-to-right way:

```draton
self.reader().value
items[0].len()
```

## Index access

Index expressions:

```draton
items[0]
table[key]
```

## Lambdas

Lambda expressions use the `lambda ... => ...` form:

```draton
lambda x => x + 1
lambda x, y => x + y
```

Important current rule:

- lambda bodies are expressions, not blocks

Examples from the repo also show capture:

```draton
let y = 10
let addY = lambda x => x + y
```

## Special call rewrites

Two constructors are recognized specially when called as bare identifiers with one argument:

```draton
Ok(value)
Err(problem)
```

Those become result-constructor expressions in the AST rather than ordinary calls.
