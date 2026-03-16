# Draton Early Tooling Preview Quickstart

1. Extract the archive for your platform.
2. Add the extracted archive root to `PATH`.
3. Verify the CLI:

```sh
drat --version
```

4. Run the bundled hello-world file:

```sh
drat run examples/hello.dt
```

5. Try the bundled sample project:

```sh
cd examples/early-preview/hello-app
drat fmt --check src
drat lint src
drat task
drat task build
```

6. Start the language server when you want editor support:

```sh
drat lsp
```

The archive also includes the Draton runtime static library required for `drat build` and `drat run`.

Prebuilt preview archives do not require a separate LLVM install. On unusually minimal Linux systems, if `drat` fails to start because a common system runtime library is missing, see `INSTALL.md` in the archive or `docs/install.md` in the repository for the exact packages to add.
