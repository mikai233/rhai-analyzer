import * as path from "node:path";
import * as net from "node:net";
import * as fs from "node:fs";
import * as vscode from "vscode";
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

    const executable: Executable = {
        command: resolveServerCommand(context, config),
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

    const packagedServer = path.join(
        context.extensionPath,
        "server",
        executable,
    );
    if (fs.existsSync(packagedServer)) {
        return packagedServer;
    }

    throw new Error(
        [
            "Could not locate rhai-lsp.",
            `Looked for: ${repoDevelopmentBuild}`,
            `Looked for: ${packagedServer}`,
            "Set `rhai.server.path` to the built rhai-lsp executable, or run `cargo build -p rhai-lsp` in the rhai-analyzer repository.",
        ].join(" "),
    );
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
