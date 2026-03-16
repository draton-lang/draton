# VS Code local install

This folder contains the temporary VS Code extension for Draton syntax + LSP.

## Quick local install

From the repository root:

```sh
cd editors/vscode
npm install
rm -rf "$HOME/.vscode/extensions/draton-local.draton-0.1.0"
mkdir -p "$HOME/.vscode/extensions/draton-local.draton-0.1.0"
cp -r . "$HOME/.vscode/extensions/draton-local.draton-0.1.0"
```

Then run `Developer: Reload Window` in VS Code or reopen the editor.

The extension resolves the language server in this order:

1. `draton.server.path` from VS Code settings
2. `target/debug/drat`
3. `target/release/drat`
4. `target/debug/draton-lsp`
5. `target/release/draton-lsp`
6. `drat` or `draton-lsp` from `PATH`

Workspace settings in this repository already point `draton.server.path` at `target/debug/drat`, so a local debug build is enough for temporary use.
