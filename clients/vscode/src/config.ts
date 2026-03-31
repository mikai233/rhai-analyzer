import * as vscode from "vscode";
import { Trace } from "vscode-languageclient/node";

export type RhaiServerTransport = "stdio" | "tcp";

export interface RhaiInlayHintConfig {
    readonly variables: boolean;
    readonly parameters: boolean;
    readonly returnTypes: boolean;
}

export interface RhaiFormattingConfig {
    readonly maxLineLength: number;
    readonly trailingCommas: boolean;
    readonly finalNewline: boolean;
    readonly containerLayout: "auto" | "preferSingleLine" | "preferMultiLine";
    readonly importSortOrder: "preserve" | "modulePath";
}

export interface RhaiExtensionConfig {
    readonly serverPath: string | undefined;
    readonly transport: RhaiServerTransport;
    readonly tcpAddress: string;
    readonly logLevel: string;
    readonly trace: Trace;
    readonly inlayHints: RhaiInlayHintConfig;
    readonly formatting: RhaiFormattingConfig;
}

export function loadConfig(): RhaiExtensionConfig {
    const config = vscode.workspace.getConfiguration("rhai");
    const serverPath = config.get<string>("server.path")?.trim();
    const transport = config.get<RhaiServerTransport>("server.transport", "stdio");
    const tcpAddress = config.get<string>("server.tcpAddress", "127.0.0.1:9257");
    const logLevel = config.get<string>("server.logLevel", "warn");
    const traceSetting = config.get<string>("trace.server", "off");
    const inlayHints = {
        variables: config.get<boolean>("inlayHints.variables", true),
        parameters: config.get<boolean>("inlayHints.parameters", true),
        returnTypes: config.get<boolean>("inlayHints.returnTypes", true),
    };
    const formatting = {
        maxLineLength: config.get<number>("format.maxLineLength", 100),
        trailingCommas: config.get<boolean>("format.trailingCommas", true),
        finalNewline: config.get<boolean>("format.finalNewline", true),
        containerLayout: config.get<"auto" | "preferSingleLine" | "preferMultiLine">(
            "format.containerLayout",
            "auto",
        ),
        importSortOrder: config.get<"preserve" | "modulePath">(
            "format.importSortOrder",
            "preserve",
        ),
    };

    return {
        serverPath: serverPath && serverPath.length > 0 ? serverPath : undefined,
        transport,
        tcpAddress,
        logLevel,
        trace: traceFromConfig(traceSetting),
        inlayHints,
        formatting,
    };
}

function traceFromConfig(value: string): Trace {
    switch (value) {
        case "messages":
            return Trace.Messages;
        case "verbose":
            return Trace.Verbose;
        default:
            return Trace.Off;
    }
}
