import * as vscode from "vscode";
import {
    CloseAction,
    ErrorAction,
    LanguageClient,
    LanguageClientOptions,
    RevealOutputChannelOn,
    State,
} from "vscode-languageclient/node";

import { registerCommands } from "./commands";
import { loadConfig } from "./config";
import { createServerOptions } from "./serverPath";

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let traceChannel: vscode.OutputChannel | undefined;

export async function activate(
    context: vscode.ExtensionContext,
): Promise<void> {
    outputChannel = vscode.window.createOutputChannel("Rhai Analyzer");
    traceChannel = vscode.window.createOutputChannel("Rhai Analyzer Trace");
    context.subscriptions.push(outputChannel, traceChannel);
    outputChannel.appendLine("Activating Rhai Analyzer extension...");

    registerCommands(context, async () => {
        await restartClient(context);
    });

    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(async (event) => {
            if (event.affectsConfiguration("rhai")) {
                await restartClient(context);
            }
        }),
    );

    await startClient(context);
}

export async function deactivate(): Promise<void> {
    if (!client) {
        return;
    }

    await client.stop();
    client = undefined;
}

async function restartClient(
    context: vscode.ExtensionContext,
): Promise<void> {
    outputChannel?.appendLine("Restarting Rhai Analyzer language client...");
    if (client) {
        await client.stop();
        client = undefined;
    }

    await startClient(context);
}

async function startClient(
    context: vscode.ExtensionContext,
): Promise<void> {
    const config = loadConfig();

    try {
        const serverOptions = createServerOptions(context, config);
        outputChannel?.appendLine(
            `Starting Rhai Analyzer using transport=${config.transport}`,
        );
        const clientOptions: LanguageClientOptions = {
            documentSelector: [{ scheme: "file", language: "rhai" }],
            synchronize: {
                fileEvents: vscode.workspace.createFileSystemWatcher("**/*.rhai"),
            },
            initializationOptions: {
                inlayHints: config.inlayHints,
                formatting: config.formatting,
            },
            outputChannel,
            traceOutputChannel: traceChannel,
            revealOutputChannelOn: RevealOutputChannelOn.Error,
            errorHandler: {
                error(error, _message, count) {
                    outputChannel?.appendLine(
                        `Rhai Analyzer connection error (count=${count ?? 0}): ${error.message}`,
                    );
                    return {
                        action: ErrorAction.Continue,
                    };
                },
                closed() {
                    outputChannel?.appendLine(
                        "Rhai Analyzer connection closed.",
                    );
                    return {
                        action: CloseAction.DoNotRestart,
                    };
                },
            },
        };

        client = new LanguageClient(
            "rhai-analyzer",
            "Rhai Analyzer",
            serverOptions,
            clientOptions,
        );
        client.onDidChangeState((event) => {
            outputChannel?.appendLine(
                `Rhai Analyzer state changed: ${stateName(event.oldState)} -> ${stateName(event.newState)}`,
            );
        });
        client.setTrace(config.trace);
        await client.start();
        outputChannel?.appendLine("Rhai Analyzer language client started.");
    } catch (error) {
        client = undefined;
        const message =
            error instanceof Error ? error.message : String(error);
        outputChannel?.appendLine(`Failed to start Rhai Analyzer: ${message}`);
        void vscode.window.showErrorMessage(
            `Failed to start Rhai Analyzer: ${message}`,
        );
    }
}

function stateName(state: State): string {
    switch (state) {
        case State.Starting:
            return "starting";
        case State.Running:
            return "running";
        case State.Stopped:
            return "stopped";
        default:
            return "unknown";
    }
}
