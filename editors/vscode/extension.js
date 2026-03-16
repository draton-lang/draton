const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
    const outputChannel = vscode.window.createOutputChannel("Draton");
    context.subscriptions.push(outputChannel);

    const resolvedServerOptions = resolveServerOptions(outputChannel);
    if (!resolvedServerOptions) {
        outputChannel.appendLine("Draton LSP was not started because no server executable was found.");
        void vscode.window.showWarningMessage(
            "Draton LSP: khong tim thay drat/draton-lsp. Hay build repo hoac dat draton.server.path trong VS Code settings."
        );
        return;
    }

    const serverOptions = {
        ...resolvedServerOptions,
        transport: TransportKind.stdio,
    };
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "draton" }],
        outputChannel,
    };

    client = new LanguageClient("draton", "Draton LSP", serverOptions, clientOptions);
    context.subscriptions.push(client.start());
}

function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

module.exports = {
    activate,
    deactivate,
};

function resolveServerOptions(outputChannel) {
    const config = vscode.workspace.getConfiguration("draton");
    const configuredPath = config.get("server.path");
    const extraArgs = config.get("server.args", []);
    const workspaceFolders = vscode.workspace.workspaceFolders;
    const workspaceFolder =
        workspaceFolders && workspaceFolders.length > 0 ? workspaceFolders[0].uri.fsPath : null;
    const suffix = process.platform === "win32" ? ".exe" : "";

    const candidates = [];
    if (configuredPath) {
        candidates.push({
            label: "configured draton.server.path",
            command: configuredPath,
            args: extraArgs,
        });
    }
    if (workspaceFolder) {
        candidates.push(
            {
                label: "workspace target/debug/drat",
                command: path.join(workspaceFolder, "target", "debug", `drat${suffix}`),
                args: ["lsp", ...extraArgs],
            },
            {
                label: "workspace target/release/drat",
                command: path.join(workspaceFolder, "target", "release", `drat${suffix}`),
                args: ["lsp", ...extraArgs],
            },
            {
                label: "workspace target/debug/draton-lsp",
                command: path.join(workspaceFolder, "target", "debug", `draton-lsp${suffix}`),
                args: extraArgs,
            },
            {
                label: "workspace target/release/draton-lsp",
                command: path.join(workspaceFolder, "target", "release", `draton-lsp${suffix}`),
                args: extraArgs,
            }
        );
    }
    candidates.push(
        {
            label: "drat on PATH",
            command: "drat",
            args: ["lsp", ...extraArgs],
        },
        {
            label: "draton-lsp on PATH",
            command: "draton-lsp",
            args: extraArgs,
        }
    );

    for (const candidate of candidates) {
        if (isRunnable(candidate.command)) {
            outputChannel.appendLine(
                `Starting Draton LSP with ${candidate.label}: ${candidate.command} ${candidate.args.join(" ")}`
            );
            return {
                command: candidate.command,
                args: candidate.args,
            };
        }
    }

    outputChannel.appendLine(
        `Checked Draton server candidates:\n${candidates.map((candidate) => `- ${candidate.command}`).join("\n")}`
    );
    return null;
}

function isRunnable(command) {
    if (!command) {
        return false;
    }
    if (path.isAbsolute(command)) {
        return fs.existsSync(command);
    }
    return true;
}
