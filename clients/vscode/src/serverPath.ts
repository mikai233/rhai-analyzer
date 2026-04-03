import * as path from "node:path";
import * as net from "node:net";
import * as fs from "node:fs";
import * as vscode from "vscode";
import { familySync, MUSL } from "detect-libc";
import {
    Executable,
    ServerOptions,
    StreamInfo,
} from "vscode-languageclient/node";

import { RhaiExtensionConfig } from "./config";

export function createServerOptions(
    context: vscode.ExtensionContext,
    config: RhaiExtensionConfig,
): ServerOptions {
    if (config.transport === "tcp") {
        return async (): Promise<StreamInfo> => {
            const endpoint = parseTcpAddress(config.tcpAddress);
            const socket = net.connect(endpoint.port, endpoint.host);

            await new Promise<void>((resolve, reject) => {
                socket.once("connect", () => resolve());
                socket.once("error", (error) => reject(error));
            });

            return {
                reader: socket,
                writer: socket,
            };
        };
    }

    const command = resolveServerCommand(context, config);
    ensureServerExecutable(command);

    const executable: Executable = {
        command,
        args: [
            "--transport",
            "stdio",
            "--log-level",
            config.logLevel,
        ],
    };

    return executable;
}

function resolveServerCommand(
    context: vscode.ExtensionContext,
    config: RhaiExtensionConfig,
): string {
    if (config.serverPath) {
        return config.serverPath;
    }

    const executable = executableName("rhai-lsp");
    const repoDevelopmentBuild = path.resolve(
        context.extensionPath,
        "..",
        "..",
        "target",
        "debug",
        executable,
    );
    if (fs.existsSync(repoDevelopmentBuild)) {
        return repoDevelopmentBuild;
    }

    const packagedCandidates = packagedServerCandidates(context.extensionPath);
    for (const packagedServer of packagedCandidates) {
        if (fs.existsSync(packagedServer)) {
            return packagedServer;
        }
    }

    throw new Error(
        [
            "Could not locate rhai-lsp.",
            `Looked for: ${repoDevelopmentBuild}`,
            ...packagedCandidates.map((candidate) => `Looked for: ${candidate}`),
            "Set `rhai.server.path` to the built rhai-lsp executable, or run `cargo build -p rhai-lsp` in the rhai-analyzer repository.",
        ].join(" "),
    );
}

function packagedServerCandidates(extensionPath: string): string[] {
    const candidates = preferredBundledTargets().map((target) =>
        path.join(extensionPath, "server", target, executableNameForTarget(target)),
    );

    // Preserve compatibility with older locally packaged layouts.
    candidates.push(path.join(extensionPath, "server", executableName("rhai-lsp")));
    return candidates;
}

function preferredBundledTargets(): string[] {
    const target = currentVsCodeTarget();
    if (!target) {
        return [];
    }

    const fallbacks = [target];
    if (target.startsWith("alpine-")) {
        fallbacks.push(target.replace("alpine-", "linux-"));
    }

    return fallbacks;
}

function currentVsCodeTarget(): string | undefined {
    switch (process.platform) {
        case "win32":
            switch (process.arch) {
                case "x64":
                    return "win32-x64";
                case "arm64":
                    return "win32-arm64";
                default:
                    return undefined;
            }

        case "darwin":
            switch (process.arch) {
                case "x64":
                    return "darwin-x64";
                case "arm64":
                    return "darwin-arm64";
                default:
                    return undefined;
            }

        case "linux":
            switch (process.arch) {
                case "x64":
                    return familySync() === MUSL ? "alpine-x64" : "linux-x64";
                case "arm64":
                    return familySync() === MUSL ? "alpine-arm64" : "linux-arm64";
                case "arm":
                    return "linux-armhf";
                default:
                    return undefined;
            }

        default:
            return undefined;
    }
}

function executableNameForTarget(target: string): string {
    return target.startsWith("win32-")
        ? executableName("rhai-lsp")
        : "rhai-lsp";
}

function executableName(base: string): string {
    return process.platform === "win32" ? `${base}.exe` : base;
}

function parseTcpAddress(address: string): { host: string; port: number } {
    const separator = address.lastIndexOf(":");
    if (separator <= 0 || separator === address.length - 1) {
        throw new Error(
            `Invalid Rhai TCP address "${address}". Expected host:port.`,
        );
    }

    const host = address.slice(0, separator).trim();
    const portText = address.slice(separator + 1).trim();
    const port = Number.parseInt(portText, 10);

    if (!host || Number.isNaN(port) || port <= 0 || port > 65535) {
        throw new Error(
            `Invalid Rhai TCP address "${address}". Expected host:port.`,
        );
    }

    return { host, port };
}

function ensureServerExecutable(serverPath: string): void {
    if (process.platform === "win32" || !fs.existsSync(serverPath)) {
        return;
    }

    const stats = fs.statSync(serverPath);
    const executableBits = 0o111;
    if ((stats.mode & executableBits) === executableBits) {
        return;
    }

    fs.chmodSync(serverPath, stats.mode | 0o755);
}
