# Task Runner

`drat task` is the official structured task runner for early Draton projects.

It gives projects a small, reviewable alternative to ad-hoc shell snippets while staying outside the language itself.

## Task file

Tasks live in a repository-root `drat.tasks` file using TOML.

Example:

```toml
[tasks.build]
description = "Build the project"
run = "cargo build"

[tasks.test]
description = "Run parser and typechecker tests"
deps = ["build"]
run = [
  "cargo test -p draton-parser --test items",
  "cargo test -p draton-typeck --test errors",
]
```

Supported fields:

- `description`: optional short description
- `run`: one shell command or an array of shell commands
- `deps`: optional list of task dependencies
- `cwd`: optional working directory relative to the `drat.tasks` file
- `env`: optional environment overrides

## Usage

```sh
drat task
drat task build
drat task test
drat task lint
```

Running `drat task` with no name lists available tasks.

## Repository usage

This repository now ships an official `drat.tasks` with:

- `build`
- `test`
- `lint`
- `fmt`

That file is the reference for early Draton automation style.
