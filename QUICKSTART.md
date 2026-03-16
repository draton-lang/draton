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

If `drat` fails to start with a missing LLVM library error, install the LLVM 14 runtime for your platform first. See `INSTALL.md` in the archive or `docs/install.md` in the repository.
