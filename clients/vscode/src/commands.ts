import * as vscode from "vscode";

export function registerCommands(
    context: vscode.ExtensionContext,
    restartServer: () => Promise<void>,
): void {
    context.subscriptions.push(
        vscode.commands.registerCommand("rhai.restartServer", async () => {
            await restartServer();
        }),
    );
}
